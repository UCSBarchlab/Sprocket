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
        unimplemented!();
    }

    fn allocate(&'static mut self, size: usize) -> Result<&'static mut [u8], &'static str> {
        //let start = &mut self.freelist;
        let mut start = ::core::mem::replace(&mut self.freelist, None);
        let num_pages = Self::size_to_pages(size);
        let mut last: Option<&'static mut FreePageRange> = None;

        if let Some(ref mut range) = start {

            // allocate the entire range, including the link
            // update the previous link to point to the next link
            if (*range).pages.len() - 1 == num_pages {
                // Get the next element in the list
                let next = range.next_range.take();

                if let Some(l) = last {
                    // set the previous element to point to the next link
                    l.next_range = next;
                } else {
                    // There was no preceding element, so we update the head to point past us
                    self.freelist = next;
                    // invariant: it MUST be the case that the FreePageRange is contiguously
                    // allocated preceding the free pages themselves
                }
                let slice: &'static mut [u8] = unsafe {
                    let sl = slice::from_raw_parts_mut::<FreePage>(*range as *mut _ as
                                                                   *mut FreePage,
                                                                   num_pages);
                    slice_cast::cast_mut(sl)
                };
                return Ok(slice);

            } else if range.pages.len() >= num_pages {
                // allocate some subset of the range, or possibly the entire range,
                // but leave the link intact other than updating its slice
                //range.pages = &mut [];

                self.freelist = start;
                //::core::mem::swap(&mut self.freelist.as_mut().unwrap(), range);


                let pages = ::core::mem::replace(&mut range.pages, &mut []);
                let len = pages.len();
                let (new_range, allocation) = pages.split_at_mut(len - num_pages);
                range.pages = new_range;
                let slice: &'static mut [u8] = unsafe { slice_cast::cast_mut(allocation) };

                let _ = ::core::mem::replace(&mut self.freelist, Some(*range));

                return Ok(slice);
            } else {
                last = Some(range);
            }
        }

        unimplemented!();
    }
}
