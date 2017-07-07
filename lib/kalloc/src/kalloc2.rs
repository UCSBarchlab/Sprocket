use mem_utils::{VirtAddr, Address, PGSIZE, PHYSTOP, end};
use core::ptr::Unique;
use spinlock::Mutex;
use core::slice;
use core::mem;
use core::cmp;

pub struct Allocator {
    start: Range,
    length: usize,
}

pub static ALLOC: Mutex<Allocator> = Mutex::new(Allocator {
    start: Range {
        next: None,
        size: 0,
    },
    length: 0,
});

#[repr(C)]
pub struct Range {
    next: Option<Unique<Range>>, // we have a ref to the next link in the chain
    size: usize, // the length not including this struct
}

impl Range {
    unsafe fn offset(&mut self, pages: isize) -> *mut u8 {
        let base = self.base_addr();
        base.offset(pages * (PGSIZE as isize))
    }

    unsafe fn base_addr(&mut self) -> *mut u8 {
        self as *mut Range as *mut u8
    }

    unsafe fn end_addr(&mut self) -> *mut u8 {
        // NB This may cause problems when virtual memory is more than ~2GiB due to downcasting
        // from usize to isize
        let size = (self.size as isize) + 1;
        self.offset(size)
    }

    fn unwrap_next(&mut self) -> &mut Range {
        unsafe { self.next.as_mut().unwrap().as_mut() }
    }

    // allocate some subset of the range, or possibly the entire range,
    // but leave the Range intact other than updating its slice
    fn allocate_from_range(&mut self, num_pages: usize) -> *mut u8 {
        assert!(self.size >= num_pages);

        trace!("Allocating from within range {:?} {}",
               &self as *const _,
               self.size);

        let s = self.size + 1;
        // get start address
        let allocation = unsafe { self.offset((s - num_pages) as isize) };
        self.size -= num_pages;
        allocation
    }

    // consume the entire range's pages (and also the Range struct itself)
    fn allocate_entire_range(mut range: Unique<Range>) -> *mut u8 {
        unsafe {
            trace!("Allocating entire range {:?} {}",
                   range.as_ref() as *const _,
                   range.as_ref().size);
        }
        unsafe { range.as_mut() as *mut Range as *mut u8 }
    }
}

impl Allocator {
    pub unsafe fn free_range(&mut self, vstart: VirtAddr, vend: VirtAddr) {
        trace!("Freeing range from {:#x} to {:#x}",
               vstart.addr(),
               vend.addr());
        self.verify();
        assert!(vstart < vend);
        assert!(vstart.is_page_aligned());
        assert!(vend.is_page_aligned());

        // Create the new range object
        let mut new_range = Unique::new(vstart.addr() as usize as *mut Range);
        *new_range.as_mut() = Range {
            next: None,
            size: (vstart.pageno()..vend.pageno()).len() - 1,
        };

        self.length += new_range.as_ref().size + 1;

        let mut prev: &mut Range = &mut self.start;

        loop {
            // we should insert if we're larger than the previous element and smaller than the next
            // element, or if the next element is None (because we've reached the end of the list)
            let should_insert = prev.next
                .map_or(true, |n| {
                    (new_range.as_ref() as *const _) < n.as_ref() as *const Range &&
                    (new_range.as_ref() as *const _) > *&prev as *const Range
                });

            if should_insert {
                new_range.as_mut().next = prev.next.take();
                prev.next = Some(new_range);

                // can we merge with the next entry?
                // check if there is a next entry, and if our last address is its first address
                if new_range.as_mut().next.is_some() {
                    if new_range.as_mut().end_addr() as usize ==
                       (new_range.as_mut().unwrap_next() as *mut Range as usize) {
                        new_range.as_mut().size += new_range.as_mut().unwrap_next().size + 1;
                        new_range.as_mut().next = new_range.as_mut().unwrap_next().next.take();
                    }
                }

                // if we can merge with the previous entry
                if prev.end_addr() as usize == (new_range.as_mut() as *mut Range as usize) {
                    prev.next = new_range.as_mut().next.take();
                    prev.size += new_range.as_ref().size + 1; // extend the previous range to include our space
                }
                return;

            } else {
                prev = Self::move_helper(prev).unwrap_next();
            }
        }
    }

    pub fn size_to_pages(size: usize) -> usize {
        (PGSIZE + size - 1) / PGSIZE
    }

    fn allocate(&mut self, size: usize) -> Result<*mut u8, &'static str> {
        self.verify();

        let mut prev: &mut Range = &mut self.start;
        let requested_pages = Self::size_to_pages(size);

        // this code inspired by Phillip Oppermann's Linked List Allocator
        // https://github.com/phil-opp/linked-list-allocator/blob/master/src/hole.rs
        // available under the terms of the MIT License
        loop {
            let next_size = prev.next.map(|ref mut n| unsafe { n.as_ref().size });
            match next_size {
                Some(s) if s >= requested_pages => {
                    let allocation = prev.unwrap_next().allocate_from_range(requested_pages);
                    self.length -= requested_pages;
                    return Ok(allocation);
                }

                Some(s) if s + 1 == requested_pages => {
                    // Update the linked list so that prev's next points to current's next
                    let next_next = prev.unwrap_next().next.take();
                    let next = mem::replace(&mut prev.next, next_next);
                    self.length -= requested_pages;
                    return Ok(Range::allocate_entire_range(next.unwrap()));
                }

                Some(_) => prev = Self::move_helper(prev).unwrap_next(),
                None => return Err("Could not find large enough contiguous area"),
            }
        }
    }

    fn move_helper<T>(x: T) -> T {
        x
    }

    // Verify that the linked list is well-formed.  Useful for debugging
    fn verify(&mut self) {
        let kernel_start: VirtAddr = VirtAddr(unsafe { &end } as *const _ as usize);

        let mut size = 0;
        let mut next = self.start.next;
        while let Some(mut n) = next {
            unsafe {
                let addr = VirtAddr::new(n.as_ref() as *const _ as usize);
                assert!(addr > kernel_start);
                assert!(addr.to_phys() < PHYSTOP);
                size += n.as_ref().size + 1;
                next = n.as_ref().next;
                if let Some(s) = n.as_ref().next {
                    // assert that addresses in the list must monotonically increase, and that
                    // there are no overlaps between ranges
                    assert!(s.as_ref() as *const _ > n.as_ref() as *const _);
                    assert!(s.as_ref() as *const _ > n.as_mut().end_addr() as *const _);
                }
            }
        }
        assert_eq!(size, self.length);
    }
}
// Allocator interface allowing us to use Box, etc.
// See https://doc.rust-lang.org/book/custom-allocators.html for more info

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
