#![no_std]
#![feature(allocator)]
#![allocator]
#![feature(unique)]
#![feature(const_fn)]

extern crate mem_utils;
extern crate spinlock;

use mem_utils::{VirtAddr, Address, PGSIZE, PHYSTOP};

pub mod kalloc2;

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

pub unsafe fn init(vstart: VirtAddr, vend: VirtAddr) {
    free_range(vstart, vend);
}

unsafe fn free_range(vstart: VirtAddr, vend: VirtAddr) {
    assert!(vstart < vend);
    for page in vstart.pageno()..vend.pageno() {
        kfree(VirtAddr::from_pageno(page));
    }
}

pub unsafe fn validate() {
    let mut p: Option<*mut Run> = KMEM.freelist;
    let mut count = 0;

    while let Some(page) = p {
        count += 1;
        if let Some(next) = (*page).next {
            assert!(page < next);
            p = Some(next);
        } else {
            assert_eq!(page, KMEM.tail.unwrap());
            assert_eq!(count, KMEM.len);
            return;
        }
    }
    if count > 0 {
        panic!();
    }
}


fn kfree(addr: VirtAddr) {
    let kernel_start: VirtAddr = VirtAddr(unsafe { &mem_utils::end } as *const _ as usize);
    if !addr.is_page_aligned() || addr < kernel_start || addr.to_phys() > PHYSTOP {
        panic!("kfree");
    }

    unsafe {
        KMEM.len += 1;
        let freed = addr.addr() as *mut Run;

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
pub extern "C" fn __rust_allocate(size: usize, _align: usize) -> *mut u8 {
    kalloc(size).expect("Allocation failed")
}

#[no_mangle]
pub extern "C" fn __rust_allocate_zeroed(size: usize, _align: usize) -> *mut u8 {
    let new_mem = kalloc(size).expect("Allocation failed");
    let num_bytes = ((size + PGSIZE - 1) / PGSIZE) * PGSIZE;
    {
        let slice: &mut [u8] = unsafe { ::core::slice::from_raw_parts_mut(new_mem, num_bytes) };
        for b in slice.iter_mut() {
            *b = 0;
        }
    }
    new_mem
}

#[no_mangle]
pub extern "C" fn __rust_usable_size(size: usize, _align: usize) -> usize {
    (size + PGSIZE - 1) / PGSIZE
}

#[no_mangle]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, size: usize, _align: usize) {
    let num_pages = (PGSIZE + size - 1) / PGSIZE;
    for off in 0..num_pages {
        unsafe {
            kfree(VirtAddr::new(ptr.offset((off * PGSIZE) as isize) as usize));
        }
    }
}

#[no_mangle]
pub extern "C" fn __rust_reallocate(ptr: *mut u8,
                                    size: usize,
                                    new_size: usize,
                                    _align: usize)
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
            kfree(VirtAddr::new(ptr.offset((off * PGSIZE) as isize) as usize));
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
