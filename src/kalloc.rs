extern crate spin;
use self::spin::Mutex;
use rlibc::memset;
use vm::{PhysAddr, VirtAddr, Address};

extern "C" {
    pub static mut end: u8;
}

pub const PGSIZE: usize = 4096;


// Memory layout

pub const KERNBASE: VirtAddr = VirtAddr(0x80000000);
pub const KERNLINK: VirtAddr = VirtAddr(KERNBASE.0 + EXTMEM.0); // Address where kernel is linked


pub const EXTMEM: PhysAddr = PhysAddr(0x100000); // Start of extended memory
pub const PHYSTOP: PhysAddr = PhysAddr(0xE000000); // Top physical memory
pub const DEVSPACE: VirtAddr = VirtAddr(0xFE000000); // Other devices are at high addresses

// Key addresses for address space layout (see kmap in vm.c for layout)

pub struct Kmem {
    freelist: Option<*mut Run>,
    use_lock: bool,
    lock: Mutex<()>,
}

pub struct Run {
    next: Option<*mut Run>,
}

static mut KMEM: Kmem = Kmem {
    freelist: None,
    use_lock: false,
    lock: Mutex::new(()),
};


// TODO: perhaps make a PhysAddr and a VirtAddr to ensure that
// nobody ever tries to convert a PhysAddr to a PhysAddr, etc.



// So this is exactly what XV6 does, although it scares the hell out of me
pub unsafe fn page_roundup(addr: *const u8) -> *const u8 {
    (addr.offset((PGSIZE - 1) as isize) as usize & !(PGSIZE - 1)) as *const u8
}
//
// So this is exactly what XV6 does, although it scares the hell out of me
pub unsafe fn page_roundup_mut(addr: *mut u8) -> *mut u8 {
    (addr.offset((PGSIZE - 1) as isize) as usize & !(PGSIZE - 1)) as *mut u8
}

pub unsafe fn kinit1(vstart: *mut u8, vend: *mut u8) {
    free_range(vstart, vend);
}

pub unsafe fn kinit2(vstart: *mut u8, vend: *mut u8) {
    free_range(vstart, vend);
    KMEM.use_lock = true;
}

unsafe fn free_range(vstart: *mut u8, vend: *mut u8) {
    let mut p = page_roundup_mut(vstart);
    while p.offset(PGSIZE as isize) <= vend {
        kfree(p);
        p = p.offset(PGSIZE as isize);
    }
}


fn kfree(addr: *mut u8) {
    let v = VirtAddr(addr as usize);
    let kernel_start: VirtAddr = VirtAddr(unsafe { &end } as *const _ as usize);
    unsafe {
        if !v.is_page_aligned() || v < kernel_start || v.to_phys() > PHYSTOP {
            panic!("kfree");
        }

        memset(addr, 1, PGSIZE);

        // Acquire lock if needed
        let _ = if KMEM.use_lock {
            Some(KMEM.lock.lock())
        } else {
            None
        };

        // Get address of thing we're actually freeing
        let freed = addr as *mut Run;

        // set freed element's next pointer to the head of the free list
        (*freed).next = KMEM.freelist.take();

        // Now update freelist to point to our new head
        KMEM.freelist = Some(freed);
    }
}

pub fn kalloc() -> Result<*mut u8, &'static str> {
    unsafe {
        // Obtain the lock if needed
        let _ = if KMEM.use_lock {
            Some(KMEM.lock.lock())
        } else {
            None
        };

        // Take the head element from the list out of the freelist
        let head = match KMEM.freelist.take() {
            Some(h) => h,
            None => return Err("Error: could not allocate physical page"),
        };

        // Now update freelist with Option<> pointing to the next element
        KMEM.freelist = (*head).next.take();


        // Return the struct as a ptr to the address
        Ok(head as *mut u8)
    }
}


// Customer allocator logic, allowing us to use Box, etc.
// See https://doc.rust-lang.org/book/custom-allocators.html for more info

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    // Keep allocator logic simple for now, by forbidding allocation larger than 1 page
    assert!(size <= PGSIZE);
    kalloc().expect("Allocation failed")
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_usable_size(size: usize, align: usize) -> usize {
    PGSIZE
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {
    kfree(ptr);
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_reallocate(ptr: *mut u8,
                                    size: usize,
                                    new_size: usize,
                                    align: usize)
                                    -> *mut u8 {
    panic!("Reallocation not supported!")
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_reallocate_inplace(ptr: *mut u8,
                                            size: usize,
                                            new_size: usize,
                                            align: usize)
                                            -> usize {
    PGSIZE
}



// todo: figure out if there's a better way to have a lock other than conditionally locking like
// xv6.  Maybe init it without a lock, then and then MOVE the linked list into the lock, where it
// can then be forceably locked/unlocked
// or maybe figure out why the lock can't be used in the firsst place?
