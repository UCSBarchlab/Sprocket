#![no_std]

#![feature(lang_items)]
#![feature(asm)]

mod elf;
use elf::elf::{Header, ProgramHeader, ELF_MAGIC};

const SECTSIZE: usize = 512;

pub mod io {
    // copied from https://github.com/gz/rust-x86
    // available under terms of the MIT license
    #[inline(always)]
    pub unsafe fn insl(port: u16, buf: &mut [u32]) {
        asm!("rep insl %dx, (%edi)"
             :: "{ecx}"(buf.len()), "{dx}"(port), "{edi}"(buf.as_ptr())
             : "ecx", "edi" : "volatile");
    }

    #[inline(always)]
    pub unsafe fn outb(port: u16, val: u8) {
        asm!("outb %al, %dx" :: "{dx}"(port), "{al}"(val));
    }

    #[inline(always)]
    pub unsafe fn inb(port: u16) -> u8 {
        let ret: u8;
        asm!("inb %dx, %al" : "={ax}"(ret) : "{dx}"(port) :: "volatile");
        ret
    }
}

#[no_mangle]
pub extern "C" fn bootmain() {
    hello();
    let buffer = unsafe { core::slice::from_raw_parts_mut(0x10000 as *mut u8, 4096) };
    readseg(buffer.as_mut_ptr(), 4096, 0);
    let elf_header = unsafe { (buffer.as_ptr() as *mut Header).as_ref().unwrap() };

    if elf_header.magic != ELF_MAGIC {
        return;
    }

    let mut prog_header = unsafe {
        (elf_header as *const _ as *const u8).offset(elf_header.phoff as isize) as
        *const ProgramHeader
    };

    let end_prog_header = unsafe { prog_header.offset(elf_header.phnum as isize) };


    while prog_header < end_prog_header {
        unsafe {
            let pa = (*prog_header).paddr as *mut u8;
            readseg(pa,
                    (*prog_header).filesz as usize,
                    (*prog_header).off as usize);

            if (*prog_header).memsz > (*prog_header).filesz {

                let addr = pa.offset((*prog_header).filesz as isize) as *mut u8;

                memset(addr,
                       0,
                       ((*prog_header).memsz - (*prog_header).filesz) as usize);
            }

            prog_header = prog_header.offset(1);
        }
    }

    let entry = &elf_header.entry as *const _ as *const fn() -> !;

    // Load the operating system
    unsafe {
        (*entry)();
    }
}


#[inline(always)]
fn hello() {
    let vga = 0xb8000 as *mut u16;
    unsafe {
        core::ptr::write(vga, 0x0F << 8 | b'c' as u16);
        core::ptr::write(vga.offset(1), 0x0F << 8 | b'o' as u16);
        core::ptr::write(vga.offset(2), 0x0F << 8 | b'f' as u16);
        core::ptr::write(vga.offset(3), 0x0F << 8 | b'f' as u16);
        core::ptr::write(vga.offset(4), 0x0F << 8 | b'l' as u16);
        core::ptr::write(vga.offset(5), 0x0F << 8 | b'o' as u16);
        core::ptr::write(vga.offset(6), 0x0F << 8 | b's' as u16);
    }
}

#[allow(dead_code)]
#[inline(always)]
// prints item in little-endian decimal order
// useful for debugging
fn print(item: u32, line: u8) {
    let vga = 0xb8000 as *mut u16;
    unsafe {
        for i in (0..8).rev() {
            let mut acc = 1_u32;
            for _ in 0..i {
                acc *= 10;
            }
            if acc > 0 {
                let num: u8 = ((item / acc) % 10) as u8;
                let c: u16 = num as u16 | (0x0F << 8);
                core::ptr::write(vga.offset(i + (line * 80_u8) as isize), c as u16 + 48);
            }
        }
    }
}


unsafe fn waitdisk() {
    // Wait for disk ready.
    while (io::inb(0x1F7) & 0xC0) != 0x40 {}
}

unsafe fn readsect(dst: &mut [u32], offset: usize) {
    waitdisk();
    io::outb(0x1F2, 1); // count = 1
    io::outb(0x1F3, offset as u8);
    io::outb(0x1F4, (offset >> 8) as u8);
    io::outb(0x1F5, (offset >> 16) as u8);
    io::outb(0x1F6, ((offset >> 24) | 0xE0) as u8);
    io::outb(0x1F7, 0x20); // cmd 0x20 - read sectors

    // Read data.
    waitdisk();

    io::insl(0x1F0, dst);
}

// Read 'count' bytes at 'offset' from kernel into physical address 'pa'.
// Might copy more than asked.
fn readseg(mut pa: *mut u8, count: usize, mut offset: usize) {
    let epa: *const u8 = unsafe { pa.offset(count as isize) };

    // Round down to sector boundary.
    pa = unsafe { pa.offset(-((offset % SECTSIZE) as isize)) };

    // Translate from bytes to sectors; kernel starts at sector 1.
    offset = (offset / SECTSIZE) + 1;

    // If this is too slow, we could read lots of sectors at a time.
    // We'd write more to memory than asked, but it doesn't matter --
    // we load in increasing order.


    while (pa as *mut u8 as usize) < (epa as usize) {
        let slice: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(pa as *mut u32, 1024) };
        unsafe {
            readsect(slice, offset);
        }
        pa = (pa as usize + SECTSIZE) as *mut u8;
        offset += 1;
    }
}

#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn panic_fmt(_: core::fmt::Arguments, _: &str, _: u32) -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

// Copied from https://github.com/alexcrichton/rlibc
// Available under the terms of the MIT license
#[cfg_attr(all(feature = "weak", not(windows), not(target_os = "macos")), linkage = "weak")]
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *s.offset(i as isize) = c as u8;
        i += 1;
    }
    return s;
}
