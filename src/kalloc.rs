extern crate spin;
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
    tail: Option<*mut Run>,
}

pub struct Run {
    next: Option<*mut Run>,
}

static mut KMEM: Kmem = Kmem {
    freelist: None,
    tail: None,
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
}

unsafe fn free_range_(vstart: *mut u8, vend: *mut u8) {
    let mut p = page_roundup_mut(vstart);
    println!("start: {:#?}", p);
    while p.offset(PGSIZE as isize) <= vend {
        kfree(p);
        p = p.offset(PGSIZE as isize);
    }
    println!("end: {:#?}", p.offset(-(PGSIZE as isize)));
}

unsafe fn free_range(vstart: *mut u8, vend: *mut u8) {
    assert!(vstart < vend);
    let mut p = page_roundup_mut(vend).offset(-(PGSIZE as isize));
    println!("end: {:#?}", p);
    while p >= vstart {
        kfree(p);
        p = p.offset(-(PGSIZE as isize));
    }
    println!("start: {:#?}", p.offset((PGSIZE as isize)));
}

pub unsafe fn validate() {
    let mut p: Option<*mut Run> = KMEM.freelist;
    while let Some(page) = p {
        print!("{:#?}", page);
        if let Some(next) = (*page).next {
            println!("-> {:#?}", next);
            assert!(page < next);
            p = Some(next);
        } else {
            break;
        }
    }
    return;
}


fn kfree(addr: *mut u8) {
    let v = VirtAddr(addr as usize);
    let kernel_start: VirtAddr = VirtAddr(unsafe { &end } as *const _ as usize);
    unsafe {
        if !v.is_page_aligned() || v < kernel_start || v.to_phys() > PHYSTOP {
            panic!("kfree");
        }

        //memset(addr, 1, PGSIZE);

        // Get address of thing we're actually freeing
        let freed = addr as *mut Run;

        // if the freelist is already populated
        if let Some(mut ptr) = KMEM.freelist {
            if ptr > freed {
                println!("Prepending address {:#?} to {:#?}", freed, ptr);
                // else we are the head
                (*freed).next = KMEM.freelist.take();

                // Now update freelist to point to our new head
                KMEM.freelist = Some(freed);
                return;
            }

            if let Some(mut t) = KMEM.tail {
                if freed > t {
                    KMEM.tail = Some(freed);
                    (*t).next = Some(freed);
                    (*freed).next = None;
                    t = freed
                }
                return;
            }

            let mut i = 0;

            loop {
                if (*ptr).next.is_none() || freed < (*ptr).next.unwrap() {
                    break;
                } else {
                    ptr = (*ptr).next.unwrap();
                    i += 1;
                }
            }

            //        println!("Putting address {:#?} in middle at {}", ptr, i);

            // Now update freelist to point to our new head
            (*freed).next = (*ptr).next.take();
            (*ptr).next = Some(freed);
            if (*freed).next.is_none() {
                KMEM.tail = Some(freed);
            }
            // else we are the head
            //println!("New head: {:#?}", KMEM.freelist.unwrap());
        } else {
            println!("Prepending address");
            // else we are the head
            (*freed).next = KMEM.freelist.take();

            // Now update freelist to point to our new head
            KMEM.freelist = Some(freed);
            KMEM.tail = Some(freed)
        }
    }
}

pub fn kalloc() -> Result<*mut u8, &'static str> {
    unsafe {
        // Take the head element from the list out of the freelist
        let head = match KMEM.freelist.take() {
            Some(h) => h,
            None => return Err("Error: could not allocate physical page"),
        };

        // Now update freelist with Option<> pointing to the next element
        KMEM.freelist = (*head).next.take();
        if KMEM.freelist.is_none() {
            KMEM.tail = None;
        }


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
