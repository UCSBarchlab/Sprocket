extern crate spin;
use self::spin::Mutex;
use rlibc::memset;

extern "C" {
    pub static mut end: u8;
}

const PGSIZE: usize = 4096;
const PHYSTOP: usize = 0xE000000;
const KERNBASE: usize = 0x80000000;

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

//#define V2P(a) (((uint) (a)) - KERNBASE)
#[allow(non_snake_case)]
#[allow(const_err)]
unsafe fn V2P(addr: *const u8) -> *const u8 {
    assert!(addr as usize >= KERNBASE);
    (addr as usize - KERNBASE) as *const u8
}

#[allow(const_err)]
#[allow(non_snake_case)]
unsafe fn V2P_mut(addr: *mut u8) -> *mut u8 {
    assert!(addr as usize >= KERNBASE);
    (addr as usize - KERNBASE) as *mut u8
}

//#define P2V(a) (((void *) (a)) + KERNBASE)
#[allow(non_snake_case)]
unsafe fn P2V(addr: *const u8) -> *const u8 {
    assert!((addr as usize) < KERNBASE);
    addr.offset(KERNBASE as isize)
}

#[allow(non_snake_case)]
pub unsafe fn P2V_mut(addr: *mut u8) -> *mut u8 {
    assert!((addr as usize) < KERNBASE);
    addr.offset(KERNBASE as isize) as *mut u8
}

// So this is exactly what XV6 does, although it scares the hell out of me
unsafe fn page_roundup(addr: *const u8) -> *const u8 {
    (addr.offset((PGSIZE - 1) as isize) as usize & !(PGSIZE - 1)) as *const u8
}
//
// So this is exactly what XV6 does, although it scares the hell out of me
unsafe fn page_roundup_mut(addr: *mut u8) -> *mut u8 {
    (addr.offset((PGSIZE - 1) as isize) as usize & !(PGSIZE - 1)) as *mut u8
}

pub unsafe fn kinit1(vstart: *mut u8, vend: *mut u8) {
    free_range(vstart, vend);
}

unsafe fn free_range(vstart: *mut u8, vend: *mut u8) {
    let mut p = page_roundup_mut(vstart);
    while p.offset(PGSIZE as isize) <= vend {
        kfree(p);
        p = p.offset(PGSIZE as isize);
    }
}


fn kfree(addr: *mut u8) {
    unsafe {
        if (addr as usize) % PGSIZE != 0 || (addr as *const _) < &end ||
           V2P_mut(addr) > PHYSTOP as *mut _ {
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


// todo: figure out if there's a better way to have a lock other than conditionally locking like
// xv6.  Maybe init it without a lock, then and then MOVE the linked list into the lock, where it
// can then be forceably locked/unlocked
// or maybe figure out why the lock can't be used in the firsst place?