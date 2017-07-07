#![no_std]
#![feature(allocator)]
#![allocator]
#![feature(unique)]
#![feature(const_fn)]

extern crate mem_utils;
extern crate spinlock;
#[macro_use]
extern crate log;

use core::slice;
use core::cmp;

use mem_utils::{VirtAddr, Address, PGSIZE};

mod allocator;

use allocator::{Allocator, Range};
use spinlock::Mutex;

pub static ALLOC: Mutex<Allocator> = Mutex::new(Allocator {
    start: Range {
        next: None,
        size: 0,
    },
    length: 0,
});

#[no_mangle]
pub extern "C" fn __rust_allocate(size: usize, _align: usize) -> *mut u8 {
    ALLOC.lock().allocate(size).expect("Allocation failed")
}

#[no_mangle]
pub extern "C" fn __rust_allocate_zeroed(size: usize, _align: usize) -> *mut u8 {
    let new_mem = ALLOC.lock().allocate(size).expect("Allocation failed");
    let num_bytes = Allocator::size_to_pages(size) * PGSIZE;
    {
        let slice: &mut [u8] = unsafe { slice::from_raw_parts_mut(new_mem, num_bytes) };
        for b in slice.iter_mut() {
            *b = 0;
        }
    }
    new_mem
}

#[no_mangle]
pub extern "C" fn __rust_usable_size(size: usize, _align: usize) -> usize {
    Allocator::size_to_pages(size)
}

pub unsafe fn init(vstart: VirtAddr, vend: VirtAddr) {
    ALLOC.lock().free_range(vstart, vend);
}

#[no_mangle]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, size: usize, _align: usize) {
    let num_pages = Allocator::size_to_pages(size);
    unsafe {
        let start_addr = VirtAddr::new(ptr as usize);
        let end_addr = VirtAddr::new(ptr.offset((num_pages * PGSIZE) as isize) as usize);
        assert_eq!(end_addr.addr() - start_addr.addr(), num_pages * PGSIZE);
        assert_eq!(end_addr.pageno() - start_addr.pageno(), num_pages);
        trace!("Deallocating {:#08x} to {:#08x}",
               start_addr.addr(),
               end_addr.addr());
        ALLOC.lock().free_range(start_addr, end_addr);
    }
}

#[no_mangle]
pub extern "C" fn __rust_reallocate(ptr: *mut u8,
                                    size: usize,
                                    new_size: usize,
                                    _align: usize)
                                    -> *mut u8 {
    let num_old_pages = Allocator::size_to_pages(size);
    let num_new_pages = Allocator::size_to_pages(new_size);
    let new_mem = ALLOC.lock().allocate(new_size).expect("Allocation failed");

    let old_mem = unsafe { slice::from_raw_parts_mut(ptr, num_old_pages * PGSIZE) };
    let new = unsafe { slice::from_raw_parts_mut(new_mem, num_new_pages * PGSIZE) };

    let overlap = cmp::min(num_old_pages, num_new_pages) * PGSIZE;
    new[..overlap].copy_from_slice(&old_mem[..overlap]);

    unsafe {
        let start_addr = VirtAddr::new(ptr as usize);
        let end_addr = VirtAddr::new(ptr.offset((num_old_pages * PGSIZE) as isize) as usize);
        ALLOC.lock().free_range(start_addr, end_addr);
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
