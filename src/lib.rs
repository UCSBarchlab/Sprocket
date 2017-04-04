#![no_std]
#![feature(lang_items)]
#![feature(const_fn)]

extern crate rlibc;
extern crate linked_list_allocator;

mod kalloc;
mod traps;
mod picirq;

#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn panic_fmt(_: core::fmt::Arguments, _: &str, _: u32) -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
