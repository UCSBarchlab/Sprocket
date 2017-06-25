#![no_std]

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(repr_simd)]
#![feature(alloc)]
#![feature(box_syntax)]
#![feature(drop_types_in_const)]

#![allow(dead_code)]
#![cfg_attr(feature = "cargo-clippy", allow(empty_loop))]


extern crate rlibc;
#[macro_use]
extern crate alloc;
extern crate x86;
extern crate slice_cast;
extern crate smoltcp;
#[macro_use]
extern crate log;

extern crate pci;
extern crate simple_fs as fs;
extern crate mem_utils as mem;
extern crate kalloc;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
#[macro_use]
mod console;
#[macro_use]
mod process;



mod flags;
mod vm;
mod traps;
mod mmu;
mod file;
mod picirq;
mod uart;
mod timer;
//mod sleeplock;
mod ide;
mod rtl8139;
mod logger;
mod service;

use mem::{PhysAddr, Address};
pub use traps::trap;
use x86::shared::irq;

use service::Service;

#[no_mangle]
pub extern "C" fn main() {
    unsafe {
        console::CONSOLE2 = Some(console::Console::new());
    }
    println!("COFFLOS OK!");
    println!("Initializing allocator");
    unsafe {
        kalloc::kinit1(&mut kalloc::end,
                       PhysAddr(4 * 1024 * 1024).to_virt().addr() as *mut u8);
    }
    logger::init().unwrap();


    println!("Initializing kernel paging");
    vm::kvmalloc();
    println!("Initializing kernel segments");
    vm::seginit();
    println!("Configuring PIC");
    picirq::picinit();
    println!("Setting up interrupt descriptor table");
    traps::trap_vector_init();
    timer::timerinit();
    println!("Loading new interrupt descriptor table");
    traps::idtinit();

    println!("Finishing allocator initialization");
    unsafe {
        kalloc::kinit2(PhysAddr(4 * 1024 * 1024).to_virt().addr() as *mut u8,
                       mem::PHYSTOP.to_virt().addr() as *mut u8);
    }

    //unsafe { kalloc::validate() };
    println!("Enumerating PCI");
    pci::enumerate();
    unsafe {
        rtl8139::NIC = rtl8139::Rtl8139::init();
    }

    info!("COFFLOS initialization complete, jumping to user code");
    service::UserService::start();
    panic!("User application ended");
}


#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn panic_fmt(fmt: ::core::fmt::Arguments, file: &'static str, line: u32) -> ! {
    println!("Panic! An unrecoverable error occurred at {}:{}",
             file,
             line);
    println!("{}", fmt);
    unsafe {
        irq::disable();
        x86::shared::halt();
    }
    loop {}
}

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn eh_personality() {}

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
