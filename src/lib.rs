#![no_std]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(repr_simd)]
#![feature(allocator)]
#![feature(alloc)]
#![feature(box_syntax)]
#![allocator]
#![allow(dead_code)]

extern crate rlibc;
extern crate linked_list_allocator;
extern crate spin;
extern crate alloc;
extern crate x86;
#[macro_use]
extern crate bitflags;

pub mod kalloc;
mod flags;
mod vm;
mod traps;
mod process;
mod mmu;
mod file;
mod fs;

const KERNBASE: u32 = 0x80000000;

#[no_mangle]
pub extern "C" fn main() {
    let vga = 0xb8000 as *mut u16;
    unsafe {
        for (i, c) in b"Kernel successfully booted!".iter().enumerate() {
            core::ptr::write(vga.offset(i as isize + 80), 0x0F << 8 | *c as u16);
        }
        //        kalloc::kinit1(&mut kalloc::end as *mut u8,

        kalloc::kinit1(&mut kalloc::end,
                       kalloc::P2V_mut((4 * 1024 * 1024) as *mut u8));
    }


    loop {}
}


#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn panic_fmt() -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

const PTE_P: u32 = 0x001; // Present
const PTE_W: u32 = 0x002; // Writeable
const PTE_PS: u32 = 0x080; // Page Size

#[repr(C)]
pub struct EntryPgDir {
    align: [PageAligner4K; 0],
    array: [u32; 1024],
}

// NOTE!  This manually puts the entry in KERNBASE >> PDXSHIFT.  This is 512,
// but if you ever want to change those constants, CHANGE THIS TOO!
impl EntryPgDir {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const fn new() -> EntryPgDir {
        EntryPgDir {
            align: [],
            array: [PTE_P | PTE_W | PTE_PS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            PTE_P | PTE_W | PTE_PS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }
}

#[no_mangle]
pub static mut ENTRYPGDIR: EntryPgDir = EntryPgDir::new();

// This idiotic piece of code exists because Rust doesn't provide a way to ask
// that a variable be aligned on a certain boundary (the way that with GCC, you
// can use __align).  The workaround is to create a fictional SIMD type that must be aligned to 4K.  Then, you can put a zero-length array of type PageAligner4K at the start of an arbitrary struct, to force it to be aligned in a certain way.
// THIS IS INCREDIBLY FRAGILE AND MAY BREAK!!!
#[cfg_attr(rustfmt, rustfmt_skip)]
#[repr(simd)]
pub struct PageAligner4K(u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64, u64, u64, u64,
                       u64, u64, u64, u64, u64, u64, u64);
