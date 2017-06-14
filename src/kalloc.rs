use mem::{PhysAddr, VirtAddr, Address, PGSIZE, PHYSTOP};

extern "C" {
    pub static mut end: u8;
}

pub struct Kmem {
    freelist: Option<*mut Run>,
    tail: Option<*mut Run>,
    len: usize,
}

pub struct Run {
    next: Option<*mut Run>,
}

pub static mut KMEM: Kmem = Kmem {
    freelist: None,
    tail: None,
    len: 0,
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
    //println!("start: {:#?}", p);
    while p.offset(PGSIZE as isize) <= vend {
        kfree(p);
        p = p.offset(PGSIZE as isize);
    }
    //println!("end: {:#?}", p.offset(-(PGSIZE as isize)));
}

unsafe fn free_range(vstart: *mut u8, vend: *mut u8) {
    assert!(vstart < vend);
    let mut p = page_roundup_mut(vend).offset(-(PGSIZE as isize));
    //println!("end: {:#?}", p);
    while p >= vstart {
        kfree(p);
        p = p.offset(-(PGSIZE as isize));
    }
    //println!("start: {:#?}", p.offset((PGSIZE as isize)));
}

pub unsafe fn validate() {
    let mut p: Option<*mut Run> = KMEM.freelist;
    let mut count = 0;

    while let Some(page) = p {
        count += 1;
        //print!("{:#?}", page);
        if let Some(next) = (*page).next {
            //println!("-> {:#?}", next);
            assert!(page < next);
            p = Some(next);
        } else {
            assert_eq!(page, KMEM.tail.unwrap());
            if count != KMEM.len {
                println!("{} != {}", count, KMEM.len);
            }
            assert_eq!(count, KMEM.len);
            return;
        }
    }
    panic!();
}


fn kfree(addr: *mut u8) {
    let v = VirtAddr(addr as usize);
    let kernel_start: VirtAddr = VirtAddr(unsafe { &end } as *const _ as usize);
    if !v.is_page_aligned() || v < kernel_start || v.to_phys() > PHYSTOP {
        panic!("kfree");
    }

    unsafe {
        KMEM.len += 1;
        let freed = addr as *mut Run;

        // Freelist contains at least one element
        if let Some(ref mut h) = KMEM.freelist {
            // should we append?
            if let Some(ref mut t) = KMEM.tail {
                if *t < freed {
                    (**t).next = Some(freed);
                    *t = freed;
                    (*freed).next = None;
                    return;
                }
            }

            // should we prepend?
            if freed < *h {
                (*freed).next = Some(*h);
                *h = freed;
                return;
            }


            // find our place
            let mut current = *h;
            while freed > current {
                if let Some(next) = (*current).next {
                    if freed < next {
                        (*freed).next = Some(next);
                        (*current).next = Some(freed);
                        return;
                    } else {
                        current = next;
                    }
                } else {
                    panic!();
                }
            }
        } else {
            KMEM.freelist = Some(freed);
            KMEM.tail = Some(freed);
            (*freed).next = None;
        }
    }
}

pub fn kalloc(size: usize) -> Result<*mut u8, &'static str> {
    unsafe {
        let num_pages = (size + PGSIZE - 1) / PGSIZE;



        // Take the head element from the list out of the freelist
        let head = match KMEM.freelist {
            Some(h) => h,
            None => return Err("Error: could not allocate physical page"),
        };

        let mut start = head;
        let mut prev_end = head;
        let mut scan = head;
        let mut count = 1;
        while let Some(n) = (*scan).next {
            if count == num_pages {
                break;
            }
            if (scan as *mut u8).offset(PGSIZE as isize) == n as *mut u8 {
                count += 1;
                scan = n;
            } else {
                count = 1;
                prev_end = scan;
                scan = n;
                start = scan;
            }
        }
        if count == num_pages {
            if start == head {
                KMEM.freelist = (*scan).next;
            } else {
                (*prev_end).next = (*scan).next;
            }
            KMEM.len -= num_pages;

            return Ok(start as *mut u8);
        }

        Err("Couldn't find a large enough contiguous region")
    }
}


// Customer allocator logic, allowing us to use Box, etc.
// See https://doc.rust-lang.org/book/custom-allocators.html for more info

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    // Keep allocator logic simple for now, by forbidding allocation larger than 1 page
    kalloc(size).expect("Allocation failed")
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_usable_size(size: usize, align: usize) -> usize {
    (size / PGSIZE + 1) * PGSIZE
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {
    let num_pages = (PGSIZE + size - 1) / PGSIZE;
    for off in 0..num_pages {
        unsafe {
            kfree(ptr.offset((off * PGSIZE) as isize));
        }
    }
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_reallocate(ptr: *mut u8,
                                    size: usize,
                                    new_size: usize,
                                    align: usize)
                                    -> *mut u8 {
    let num_old_pages = (PGSIZE + size - 1) / PGSIZE;
    let num_new_pages = (PGSIZE + new_size - 1) / PGSIZE;
    let new_mem = kalloc(num_new_pages * PGSIZE).expect("Allocation failed");
    let old_mem = unsafe { ::core::slice::from_raw_parts_mut(ptr, num_old_pages * PGSIZE) };

    let new = unsafe { ::core::slice::from_raw_parts_mut(new_mem, num_new_pages * PGSIZE) };
    let overlap = ::core::cmp::min(num_old_pages, num_new_pages) * PGSIZE;
    new[..overlap].copy_from_slice(&old_mem[..overlap]);

    for off in 0..num_old_pages {
        unsafe {
            kfree(ptr.offset((off * PGSIZE) as isize));
        }
    }

    new.as_mut_ptr()
}

#[no_mangle]
#[allow(unused_variables)]
pub extern "C" fn __rust_reallocate_inplace(ptr: *mut u8,
                                            size: usize,
                                            new_size: usize,
                                            align: usize)
                                            -> usize {
    size
}
