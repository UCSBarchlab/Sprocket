#![no_std]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(integer_atomics)]
#![feature(repr_simd)]

extern crate rlibc;
extern crate linked_list_allocator;
extern crate spin;

//mod kallocbak;
mod traps;
mod picirq;
//mod lapic;
mod flags;
//mod mp;

#[no_mangle]
pub extern "C" fn main() {
    let vga = 0xb8000 as *mut u16;
    unsafe {
        for (i, c) in b"Kernel successfully booted!".iter().enumerate() {
            core::ptr::write(vga.offset(i as isize + 80), 0x0F << 8 | *c as u16);
        }
    }
    loop {}
}


#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn panic_fmt(_: core::fmt::Arguments, _: &str, _: u32) -> ! {
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
const PTE_U: u32 = 0x004; // User
const PTE_PWT: u32 = 0x008; // Write-Through
const PTE_PCD: u32 = 0x010; // Cache-Disable
const PTE_A: u32 = 0x020; // Accessed
const PTE_D: u32 = 0x040; // Dirty
const PTE_PS: u32 = 0x080; // Page Size
const PTE_MBZ: u32 = 0x180; // Bits must be zero

const KERNBASE: usize = 0x80000000;
const PDXSHIFT: usize = 22;

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
            array: [(0) | PTE_P | PTE_W | PTE_PS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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
            (0) | PTE_P | PTE_W | PTE_PS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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
pub static mut entrypgdir: EntryPgDir = EntryPgDir::new();

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
