use alloc::boxed::Box;
use core;
use process;
use x86::shared::segmentation::SegmentDescriptor;
use x86::shared::segmentation as seg;
use x86::shared::dtables;
use x86::shared;
use spinlock::Mutex;
use mmu;
use mem::{PhysAddr, VirtAddr, Address, PGSIZE, KERNBASE, KERNLINK, PHYSTOP, DEVSPACE, EXTMEM};

extern "C" {
    /// The virtual address at the beginning of the data segment
    static data: u8;
}

pub static KPGDIR: Mutex<VirtAddr> = Mutex::new(VirtAddr(0));

lazy_static! {
    /// Table to define kernel mappings in each process page table
    static ref KMAP: [Kmap; 4] = {
        #[allow(non_snake_case)]
        let DATA_BEGIN: VirtAddr = VirtAddr(unsafe { &data } as *const _ as usize);
        [
            Kmap {
                virt: KERNBASE,
                p_start: PhysAddr(0),
                p_end: EXTMEM,
                perm: WRITABLE,
            },
            Kmap {
                virt: KERNLINK,
                p_start: KERNLINK.to_phys(),
                p_end: DATA_BEGIN.to_phys(),
                perm: Entry::empty(),
            },
            Kmap {
                virt: DATA_BEGIN,
                p_start: DATA_BEGIN.to_phys(),
                p_end: PHYSTOP,
                perm: WRITABLE,
            },
            Kmap {
                virt: DEVSPACE,
                p_start: PhysAddr(DEVSPACE.addr()),
                p_end: PhysAddr(0),
                perm: WRITABLE,
            },
        ]
    };
}

/// Struct to help define kernel mappings in each process's page table
struct Kmap {
    virt: VirtAddr,
    p_start: PhysAddr,
    p_end: PhysAddr,
    perm: Entry,
}


#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone)]
#[repr(C)]
/// A Page Directory Entry
pub struct PageDirEntry(pub Entry);

#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone)]
#[repr(C)]
/// A Page Table Entry
pub struct PageTableEntry(Entry);

/// A Page Table Entry
pub type PageDir = [PageDirEntry; 1024];

bitflags! {
    /// Flags to control permissions and other aspects of PageTableEntry and PageDirEntry
    pub flags Entry: usize {
        const PRESENT  = 1,
        const WRITABLE = 1 << 1,
        const USER     = 1 << 2,
    }
}


impl Entry {
    fn address(&self) -> PhysAddr {
        PhysAddr::new((self.bits & !0xFFF) as usize)
    }
}


/// GDT segment descriptor indices
pub enum Segment {
    Null = 0,
    KCode = 1,
    KData = 2,
    UCode = 3,
    UData = 4,
    TSS = 5,
}


pub fn seginit() {

    // Unsafe primarily because of global state manipulation.
    // TODO: see if we can navigate around this by passing it as an arg,
    // and/or refactoring this as a struct with methods.
    unsafe {
        if process::CPU.is_none() {
            process::CPU = Some(process::Cpu::new());
        }

        if let Some(ref mut cpu) = process::CPU {
            cpu.gdt[Segment::Null as usize] = SegmentDescriptor::NULL;
            cpu.gdt[Segment::KCode as usize] =
                SegmentDescriptor::new(0x00000000,
                                       0xffffffff,
                                       seg::Type::Code(seg::CODE_READ),
                                       false,
                                       shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::KData as usize] =
                SegmentDescriptor::new(0x00000000,
                                       0xffffffff,
                                       seg::Type::Data(seg::DATA_WRITE),
                                       false,
                                       shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::UCode as usize] =
                SegmentDescriptor::new(0x00000000,
                                       0xffffffff,
                                       seg::Type::Code(seg::CODE_READ),
                                       false,
                                       shared::PrivilegeLevel::Ring3);
            cpu.gdt[Segment::UData as usize] =
                SegmentDescriptor::new(0x00000000,
                                       0xffffffff,
                                       seg::Type::Data(seg::DATA_WRITE),
                                       false,
                                       shared::PrivilegeLevel::Ring3);

            /*
            info!("Flags {:?}", cpu.gdt[1].limit2_flags);
            info!("Flags 0x{:02x}", cpu.gdt[1].limit2_flags.bits());
            info!("Flags 0x{:02x}", cpu.gdt[1].access.bits());
            assert!(cpu.gdt[1].limit2_flags.contains(seg::FLAGS_G));
            */


            let d = dtables::DescriptorTablePointer::new_gdtp(&cpu.gdt[0..mmu::NSEGS]);
            dtables::lgdt(&d);
        }
    }
}

/// Allocate a page table for the kernel (for use by the scheduler, etc).
pub fn kvmalloc() {
    *KPGDIR.lock() = VirtAddr::new(Box::into_raw(setupkvm().unwrap()) as usize);
    switchkvm();
}

/// Initialize kernel portion of page table
pub fn setupkvm() -> Result<Box<PageDir>, ()> {

    // Turn box into raw ptr because we need it to outlive this function
    let mut pgdir = box [PageDirEntry(Entry::empty()); 1024]; // allocate new page table

    // We know this is okay, just for convenience
    assert!(PHYSTOP.to_virt() <= DEVSPACE);

    {
        for k in KMAP.iter() {
            map_pages(&mut pgdir[..],
                      k.virt,
                      k.p_end - k.p_start,
                      k.p_start,
                      k.perm)?;
        }

        // needed to do memory-mapped PCI configuration
        //map_pages(&mut pgdir[..],
        //          VirtAddr::new(0xe0000), // virtual base
        //          0xfffff - 0xe0000, // size
        //          PhysAddr::new(0xe0000), // phys base
        //          PRESENT)?; // read only
    }

    Ok(pgdir)
}

/// Switch HW page table register (control reg 3) to the kernel page table.  This is used when no process
/// is running.
pub fn switchkvm() {
    // unsafe because we're accessing global state, and we're manipulating register CR3
    unsafe {
        lcr3(KPGDIR.lock().to_phys());
    }
}

unsafe fn lcr3(addr: PhysAddr) {
    asm!("mov $0, %cr3" : : "r" (addr.addr()) : : "volatile");
}

/*
fn switchuvm(process: &mut Process) {
    //pushcli();
    unsafe {

        if let Some(ref mut cpu) = process::CPU {

            cpu.gdt[Segment::TSS as usize] = seg16(&cpu.ts as *const _ as usize,
                                                   core::mem::size_of::<SegmentDescriptor>() - 1,
                                                   descriptor::FLAGS_TYPE_SYS_NATIVE_TSS_AVAILABLE,
                                                   shared::PrivilegeLevel::Ring0);
        }
    }

    /*
  cpu.gdt[SEG_TSS] = SEG16(STS_T32A, &cpu->ts, sizeof(cpu->ts)-1, 0);
  cpu.gdt[SEG_TSS].s = 0;
  cpu.ts.ss0 = SEG_KDATA << 3;
  cpu.ts.esp0 = (uint)p->kstack + KSTACKSIZE;
  // setting IOPL=0 in eflags *and* iomb beyond the tss segment limit
  // forbids I/O instructions (e.g., inb and outb) from user space
  cpu->ts.iomb = (ushort) 0xFFFF;
  ltr(SEG_TSS << 3);
  lcr3(V2P(p->pgdir));  // switch to process's address space
  //popcli();
  */
}
*/


pub fn inituvm(pgdir: &mut PageDir, init: &[u8]) {
    assert!(init.len() >= PGSIZE, "inituvm: size larger than a page");

    let mut mem = box [0 as u8; 4096];

    mem[..init.len()].copy_from_slice(init);
    map_pages(pgdir,
              VirtAddr::new(0),
              PGSIZE,
              VirtAddr::new(Box::into_raw(mem) as usize).to_phys(),
              USER | WRITABLE)
        .expect("inituvm: Map pages failed");

}

/// Given page directory entries, Create PTEs for virtual addresses starting at va.
pub fn map_pages(p: &mut [PageDirEntry],
                 va: VirtAddr,
                 size: usize,
                 mut pa: PhysAddr,
                 permissions: Entry)
                 -> Result<(), ()> {
    let mut a = va.page_rounddown();
    let last = va.offset::<u8>((size - 1) as isize).page_rounddown();

    loop {
        let pte = walkpgdir(p, a, true)?;

        if pte.0 & PRESENT == PRESENT {
            panic!("remap failed");
        }
        let mut new_entry = permissions | PRESENT;
        new_entry.bits |= pa.addr();
        *pte = PageTableEntry(new_entry);
        if a == last {
            break;
        }

        a = a.offset::<u8>(PGSIZE as isize);
        pa = pa.offset::<u8>(PGSIZE as isize);
    }
    Ok(())
}

// Find the physical address of the PTE that corresponds to the virtual address
fn walkpgdir(p: &mut [PageDirEntry],
             va: VirtAddr,
             allocate: bool)
             -> Result<&mut PageTableEntry, ()> {
    let pde = &mut p[va.page_dir_index()];
    let pgtab;
    if pde.0.contains(PRESENT) {
        pgtab = pde.0.address().to_virt();
    } else {
        if !allocate {
            return Err(());
        }
        let alloc = box [PageTableEntry(Entry::empty()); 1024];
        // allocate new page table and consume Box to prevent deallocation
        pgtab = VirtAddr::new(Box::into_raw(alloc) as usize);
        let mut new_entry = PageDirEntry(Entry::empty());
        new_entry.0.bits |= pgtab.to_phys().0;
        *pde = PageDirEntry(new_entry.0 | PRESENT | USER | WRITABLE);
    }

    // Unsafe because we have a raw pointer, but we're absolutely sure it's valid
    let index = va.page_table_index();
    unsafe {
        let tab = core::slice::from_raw_parts_mut(pgtab.addr() as *mut PageTableEntry, 1024);
        Ok(&mut tab[index])
    }
}
