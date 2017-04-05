#![no_std]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(integer_atomics)]

extern crate rlibc;
extern crate linked_list_allocator;
extern crate spin;

//mod kallocbak;
mod traps;
mod picirq;
//mod lapic;
mod flags;
//mod mp;

//#[no_mangle]
//pub static entrypgdir: [u32; 1] = [0];
//pub static entrypgdiraddr: &[u32; 1] = &entrypgdir;

extern "C" {
    pub static entrypgdir: *const u32;
}

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
