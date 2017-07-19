#![no_std]
#![feature(alloc)]
#![feature(allocator_api)]
#![feature(unique)]
#![feature(const_fn)]

extern crate alloc;
extern crate mem_utils;
extern crate spinlock;
#[macro_use]
extern crate log;
mod allocator;

use mem_utils::{VirtAddr, Address, PGSIZE};
use spinlock::Mutex;
use allocator::{Allocator, Range}; // our system allocator
use alloc::allocator::{Alloc, Layout, AllocErr}; // Rust allocator trait

pub struct RangeAlloc(Mutex<Allocator>);

impl RangeAlloc {
    pub unsafe fn init(&self, vstart: VirtAddr, vend: VirtAddr) {
        self.0.lock().free_range(vstart, vend);
    }
}

pub const RANGE_ALLOC_INIT: RangeAlloc = RangeAlloc(Mutex::new(Allocator {
    start: Range {
        next: None,
        size: 0,
    },
    length: 0,
}));

unsafe impl<'a> Alloc for &'a RangeAlloc {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        assert!(layout.align() <= PGSIZE);
        let mut kalloc = self.0.lock();
        let size = layout.size();
        kalloc.allocate(size)
            .map_err(|_| AllocErr::Exhausted { request: layout })
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let mut kalloc = self.0.lock();

        let size = layout.size();
        let num_pages = Allocator::size_to_pages(size);

        let start_addr = VirtAddr::new(ptr as usize);
        let end_addr = VirtAddr::new(ptr.offset((num_pages * PGSIZE) as isize) as usize);

        assert_eq!(end_addr.addr() - start_addr.addr(), num_pages * PGSIZE);
        assert_eq!(end_addr.pageno() - start_addr.pageno(), num_pages);

        kalloc.free_range(start_addr, end_addr);
    }

    fn usable_size(&self, layout: &Layout) -> (usize, usize) {
        let size = layout.size();
        (size, Allocator::size_to_pages(size) * PGSIZE)
    }
}
