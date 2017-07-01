#![allow(dead_code)]
use mem_utils::{VirtAddr, Address, PGSIZE, PHYSTOP, end};
use core::ptr::Unique;
use spinlock::Mutex;
use core::slice;
extern crate slice_cast;

const PTR_SIZE: usize = 4;

pub struct Allocator {
    freelist: Option<&'static mut FreePageRange>,
    length: usize,
}

pub static ALLOC: Mutex<Allocator> = Mutex::new(Allocator {
    freelist: None,
    length: 0,
});

/*
#[repr(C)]
pub struct FreePageRange {
    range: &'static [FreePage], // we have our own runtime-sized collection of pages, which doesn't include us but it should
    _padding: [u8; PGSIZE - 4 * PTR_SIZE],
    next_range: Option<&'static FreePageRange>, // we have a ref to the next link in the chain
}
*/

//[ NEXT(addr, size, __PADDING__) | FREE FREE FREE ... FREE ]
//[ NEXT(addr, size, __PADDING__) | &[FREE FREE FREE ... FREE] ]

#[repr(C)]
pub struct FreePageRange {
    next_range: Option<&'static mut FreePageRange>, // we have a ref to the next link in the chain
    _padding: [u8; PGSIZE - 2 * PTR_SIZE],
    pages: &'static mut [FreePage],
}

#[repr(C)]
pub struct FreePage([u8; 4096]);

unsafe fn free_range(vstart: VirtAddr, vend: VirtAddr) {
    assert!(vstart < vend);
    assert!(vstart.is_page_aligned());
    assert!(vend.is_page_aligned());

    let new_range = vstart.addr() as usize as *mut FreePageRange;
    //*new_range.range =

    //let new_range = slice::from_raw_parts_mut(vstart.addr() as usize as *mut FreePageRange);
}

impl Allocator {
    /*
    fn free(&mut self, addr: VirtAddr) {
        let kernel_start: VirtAddr = VirtAddr(unsafe { &end } as *const _ as usize);
        assert!(addr.is_page_aligned());
        assert!(addr >= kernel_start);
        assert!(addr.to_phys() <= PHYSTOP);

        let freed_page: &'static mut _ =
            unsafe { (addr.addr() as *mut FreePage).as_mut().unwrap() };

        let mut start = self.freelist;

        match start {
            Some(r) => {}
            None => self.freelist = None,

        }

        if let Some(range) = start {

        } else {

        }

        unsafe { self.length += 1 };
    }
    */

    fn size_to_pages(size: usize) -> usize {
        (PGSIZE + size - 1) / PGSIZE
    }

    fn allocate(&'static mut self, size: usize) -> Result<&'static mut [u8], &'static str> {
        assert_eq!(::core::mem::size_of::<FreePage>(), PGSIZE);
        assert_eq!(::core::mem::size_of::<FreePageRange>(), PGSIZE);

        let requested_pages = Self::size_to_pages(size);

        // If we have any memory we can possibly allocate
        if self.freelist.is_some() {
            let len = self.freelist.as_mut().unwrap().pages.len();

            // If the requested memory is the exact size of the range, plus the FreePageRange that
            // points to it
            if len + 1 == requested_pages {
                // update the freelist head to point to the next element (or None)
                let next = self.freelist.as_mut().unwrap().next_range.take();
                let old_head = ::core::mem::replace(&mut self.freelist, next);

                let slice = Self::allocate_entire_range(old_head.unwrap());
                return Ok(slice);
            } else if len >= requested_pages {
                // allocate some subset of the range, or possibly the entire range,
                let slice = Self::allocate_from_range(size, self.freelist.as_mut().unwrap());
                return Ok(slice);
            }
        } else {
            return Err("Unable to find a contiguous range");
        }

        /*
        let mut last = &mut self.freelist;
        let mut next = &mut self.freelist.next_range;
        while let Some(n) = next {}
        */


        Err("Unable to find a contiguous range")
    }

    fn allocate_from_range(size: usize, range: &'static mut FreePageRange) -> &'static mut [u8] {
        let requested_pages = Self::size_to_pages(size);
        let len = range.pages.len();
        assert!(len >= requested_pages);
        // allocate some subset of the range, or possibly the entire range,
        // but leave the FreePageRange intact other than updating its slice

        // Take the list of free pages from the FreePageRange, divide it as needed, and
        // replace the remainder back into the FreePageRange.  Return the allocated portion
        let pages = ::core::mem::replace(&mut range.pages, &mut []);
        let (remainder, allocation) = pages.split_at_mut(len - requested_pages);
        range.pages = remainder;

        let slice: &'static mut [u8] = unsafe { slice_cast::cast_mut(allocation) };
        slice
    }

    fn allocate_entire_range(range: &'static mut FreePageRange) -> &'static mut [u8] {
        let len = range.pages.len();
        let requested_pages = len + 1;

        // Re-cast the link and it's adjacently allocated page range into a new allocation
        // Invariant: it MUST be the case that the FreePageRange is contiguously
        // allocated preceding the free pages themselves
        unsafe {
            assert_eq!((range as *const FreePageRange as *const FreePage).offset(1),
                       &range.pages[0] as *const FreePage);
        }

        let slice: &'static mut [u8] = unsafe {
            let allocation = range as *mut FreePageRange;
            let sl: &'static mut [FreePageRange] = slice::from_raw_parts_mut(allocation,
                                                                             requested_pages);
            slice_cast::cast_mut(sl)
        };
        return slice;
    }
}
