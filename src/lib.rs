#![no_std]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(repr_simd)]
#![feature(allocator)]
#![feature(alloc)]
#![feature(box_syntax)]
#![feature(collections)]
#![allocator]
#![feature(drop_types_in_const)]

#![allow(dead_code)]
#![cfg_attr(feature = "cargo-clippy", allow(empty_loop))]


extern crate rlibc;
extern crate spin;
extern crate alloc;
extern crate collections;
extern crate x86;
extern crate slice_cast;
extern crate simple_fs as fs;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
#[macro_use]
mod console;
#[macro_use]
mod process;



pub mod kalloc;
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
mod pci;
mod rtl8139;

use vm::{PhysAddr, Address};
pub use traps::trap;
use x86::shared::irq;

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
    unsafe { kalloc::validate() };


    println!("Initializing kernel paging");
    vm::kvmalloc();
    println!("Initializing kernel segments");
    vm::seginit();
    unsafe { kalloc::validate() };
    println!("Configuring PIC");
    unsafe { kalloc::validate() };
    picirq::picinit();
    println!("Setting up interrupt descriptor table");
    unsafe { kalloc::validate() };
    traps::trap_vector_init();
    timer::timerinit();
    println!("Loading new interrupt descriptor table");
    unsafe { kalloc::validate() };
    traps::idtinit();




    println!("Finishing allocator initialization");
    unsafe {
        kalloc::kinit2(PhysAddr(4 * 1024 * 1024).to_virt().addr() as *mut u8,
                       kalloc::PHYSTOP.to_virt().addr() as *mut u8);
    }

    unsafe { kalloc::validate() };

    let mut x = collections::vec::Vec::new();
    for _ in 0..5000 {
        x.push(0);
    }
    x.clear();
    x.shrink_to_fit();

    println!("Reading root fs");

    let mut fs = fs::FileSystem { disk: ide::Ide::init() };

    let inum = fs.namex(b"/", b"README").unwrap();
    let inode = fs.read_inode(fs::ROOT_DEV, inum);
    match inode {
        Ok(i) => {
            println!("OK! Found 'README' at {}", inum);
            println!("Size: {}", i.size);
            println!("======================================================================");

            let mut buf = [0; fs::BLOCKSIZE];
            let mut off = 0;
            while let Ok(n) = fs.read(&i, &mut buf, off) {
                let s = ::core::str::from_utf8(&buf[..n]);
                match s {
                    Ok(s) => print!("{}", s),
                    Err(e) => {
                        println!("error, up to {}", e.valid_up_to());
                        println!("at offset{}. Char is '{:x}'", off, buf[e.valid_up_to()]);
                    }
                }
                off += fs::BLOCKSIZE as u32;
            }
            println!("======================================================================");
        }
        Err(_) => println!("Something broke :("),
    }
    println!("Enumerating PCI");
    pci::enumerate();
    unsafe { rtl8139::Rtl8139::init() };

    println!("Launching scheduler...");
    unsafe {
        process::SCHEDULER = Some(process::Scheduler::new());
        process::SCHEDULER.as_mut().unwrap().scheduler();
    }
}


#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn panic_fmt(_fmt: ::core::fmt::Arguments, file: &'static str, line: u32) -> ! {
    println!("Panic! An unrecoverable error occurred at {}:{}",
             file,
             line);
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
