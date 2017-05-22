use kalloc;
use alloc::boxed::Box;
use core;
use core::ops::Sub;
use process;
use x86::shared::segmentation::SegmentDescriptor;
use x86::shared::segmentation as seg;
use x86::shared::dtables;
use x86::shared::descriptor;
use x86::shared;
use mmu;
use process::Process;


extern "C" {
    /// The virtual address at the beginning of the data segment
    static data: u8;
}


/// Struct to help define kernel mappings in each process's page table
struct Kmap {
    virt: VirtAddr,
    p_start: PhysAddr,
    p_end: PhysAddr,
    perm: Entry,
}


lazy_static! {
    /// Table to define kernel mappings in each process page table
    static ref KMAP: [Kmap; 4] = {
        #[allow(non_snake_case)]
        let DATA_BEGIN: VirtAddr = VirtAddr(unsafe { &data } as *const _ as usize);
        [
            Kmap {
                virt: kalloc::KERNBASE,
                p_start: PhysAddr(0),
                p_end: kalloc::EXTMEM,
                perm: WRITABLE,
            },
            Kmap {
                virt: kalloc::KERNLINK,
                p_start: kalloc::KERNLINK.to_phys(),
                p_end: DATA_BEGIN.to_phys(),
                perm: Entry::empty(),
            },
            Kmap {
                virt: DATA_BEGIN,
                p_start: DATA_BEGIN.to_phys(),
                p_end: kalloc::PHYSTOP,
                perm: WRITABLE,
            },
            Kmap {
                virt: kalloc::DEVSPACE,
                p_start: PhysAddr(kalloc::DEVSPACE.addr()),
                p_end: PhysAddr(0),
                perm: WRITABLE,
            },
        ]
    };
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


#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Default)]
/// A convenience class to safely work with and manipulate physical addresses
pub struct PhysAddr(pub usize);
#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Default)]
/// A convenience class to safely work with and manipulate virtual (paged) addresses
pub struct VirtAddr(pub usize);

pub const PDXSHIFT: usize = 22;
pub const PTXSHIFT: usize = 12;

/// GDT segment descriptor indices
pub enum Segment {
    Null = 0,
    KCode = 1,
    KData = 2,
    UCode = 3,
    UData = 4,
    TSS = 5,
}


impl PhysAddr {
    pub fn new(addr: usize) -> PhysAddr {
        PhysAddr(addr)
    }

    pub fn to_virt(&self) -> VirtAddr {
        VirtAddr::new(self.0 + kalloc::KERNBASE.addr())
    }
}

impl Sub for PhysAddr {
    type Output = usize;

    fn sub(self, other: PhysAddr) -> usize {
        self.0.wrapping_sub(other.0)
    }
}

impl Sub for VirtAddr {
    type Output = usize;
    fn sub(self, other: VirtAddr) -> usize {
        self.0.wrapping_sub(other.0)
    }
}

impl VirtAddr {
    pub fn new(addr: usize) -> VirtAddr {
        VirtAddr(addr)
    }

    pub fn to_phys(&self) -> PhysAddr {
        PhysAddr::new(self.0 - kalloc::KERNBASE.addr())
    }

    fn page_dir_index(&self) -> usize {
        (self.addr() >> PDXSHIFT) & 0x3FF
    }

    fn page_table_index(&self) -> usize {
        (self.addr() >> PTXSHIFT) & 0x3FF
    }
}

/// A utility trait that implements common methods for `PhysAddr` and `VirtAddr`
pub trait Address {
    fn new(usize) -> Self where Self: core::marker::Sized;

    fn addr(&self) -> usize;

    fn is_page_aligned(&self) -> bool {
        self.addr() % kalloc::PGSIZE == 0
    }

    fn page_roundup(&self) -> Self
        where Self: core::marker::Sized
    {
        let addr = (self.addr() + kalloc::PGSIZE - 1) & !(kalloc::PGSIZE - 1);
        Self::new(addr)
    }

    fn page_rounddown(&self) -> Self
        where Self: core::marker::Sized
    {
        let addr = self.addr() & !(kalloc::PGSIZE - 1);
        Self::new(addr)
    }

    // Simulate pointer arithmetic of adding/subtracting 4-byte int to address
    fn offset<T>(&self, off: isize) -> Self
        where Self: core::marker::Sized
    {
        let size = core::mem::size_of::<T>();
        if off > 0 {
            Self::new(self.addr() + (size * off as usize))
        } else {
            Self::new(self.addr() - (size * off as usize))
        }
    }
}

impl Address for VirtAddr {
    fn new(addr: usize) -> VirtAddr {
        VirtAddr(addr)
    }

    fn addr(&self) -> usize {
        self.0
    }
}

impl Address for PhysAddr {
    fn new(addr: usize) -> PhysAddr {
        PhysAddr(addr)
    }

    fn addr(&self) -> usize {
        self.0
    }
}


pub static mut KPGDIR: VirtAddr = VirtAddr(0);

// Initialize a SegmentDescriptor in a manner compatible with xv6
fn seg2(base: usize,
        limit: usize,
        ty: descriptor::Flags,
        ring: shared::PrivilegeLevel)
        -> SegmentDescriptor {
    SegmentDescriptor {
        limit1: ((limit >> 12) & 0xffff) as u16, // should be >> 12
        base1: (base & 0xffff) as u16,
        base2: ((base >> 16) & 0xff) as u8,
        base3: ((base >> 24) & 0xff) as u8,
        access: descriptor::Flags::from_priv(ring) | descriptor::FLAGS_PRESENT |
                descriptor::FLAGS_TYPE_SEG | ty,
        limit2_flags: seg::FLAGS_DB | seg::FLAGS_G | seg::Flags::from_limit2((limit >> 28) as u8), 
                      // should be limit >> 32
    }

}

fn seg(base: usize,
       limit: usize,
       ty: descriptor::Flags,
       ring: shared::PrivilegeLevel)
       -> SegmentDescriptor {
    SegmentDescriptor {
        limit1: (limit & 0xffff) as u16,
        base1: base as u16,
        base2: ((base & 0xff0000) >> 16) as u8,
        base3: ((base & 0xff000000) >> 24) as u8,
        access: descriptor::Flags::from_priv(ring) | descriptor::FLAGS_PRESENT |
                descriptor::FLAGS_TYPE_SEG | ty,
        limit2_flags: seg::FLAGS_DB | seg::FLAGS_G |
                      seg::Flags::from_limit2(((0xffffffff & 0xF0000) >> 16) as u8),
    }

}

fn seg16(base: usize,
         limit: usize,
         ty: descriptor::Flags,
         ring: shared::PrivilegeLevel)
         -> SegmentDescriptor {

    SegmentDescriptor {
        limit1: (limit >> 12 & 0xffff) as u16,
        base1: (base & 0xffff) as u16,
        base2: ((base >> 16) & 0xff) as u8,
        base3: (base >> 24) as u8,
        access: descriptor::Flags::from_priv(ring) | descriptor::FLAGS_PRESENT | ty,
        limit2_flags: seg::FLAGS_DB | seg::Flags::from_limit2((limit >> 16) as u8),
    }
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
            cpu.gdt[Segment::KCode as usize] = seg(0x00000000,
                                                   0xffffffff,
                                                   descriptor::FLAGS_TYPE_CODE |
                                                   descriptor::FLAGS_TYPE_SEG_C_READ,
                                                   shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::KData as usize] = seg(0x00000000,
                                                   0xffffffff,
                                                   descriptor::FLAGS_TYPE_DATA |
                                                   descriptor::FLAGS_TYPE_SEG_D_RW,
                                                   shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::UCode as usize] = seg(0x00000000,
                                                   0xffffffff,
                                                   descriptor::FLAGS_TYPE_CODE |
                                                   descriptor::FLAGS_TYPE_SEG_C_READ,
                                                   shared::PrivilegeLevel::Ring3);
            cpu.gdt[Segment::UData as usize] = seg(0x00000000,
                                                   0xffffffff,
                                                   descriptor::FLAGS_TYPE_DATA |
                                                   descriptor::FLAGS_TYPE_SEG_D_RW,
                                                   shared::PrivilegeLevel::Ring3);


            let d = dtables::DescriptorTablePointer::new_gdtp(&cpu.gdt[0..mmu::NSEGS]);
            dtables::lgdt(&d);
        }
    }
}

/// Allocate a page table for the kernel (for use by the scheduler, etc).
pub fn kvmalloc() {
    unsafe {
        KPGDIR = VirtAddr::new(Box::into_raw(setupkvm().unwrap()) as usize);
    }
    switchkvm();
}

/// Initialize kernel portion of page table
pub fn setupkvm() -> Result<Box<PageDir>, ()> {

    // Turn box into raw ptr because we need it to outlive this function
    let mut pgdir = box [PageDirEntry(Entry::empty()); 1024]; // allocate new page table

    // We know this is okay, just for convenience
    assert!(kalloc::PHYSTOP.to_virt() <= kalloc::DEVSPACE);

    {
        for k in KMAP.iter() {
            map_pages(&mut pgdir[..],
                      k.virt,
                      k.p_end - k.p_start,
                      k.p_start,
                      k.perm)?;
        }
    }

    Ok(pgdir)
}

/// Switch HW page table register (control reg 3) to the kernel page table.  This is used when no process
/// is running.
pub fn switchkvm() {
    // unsafe because we're accessing global state, and we're manipulating register CR3
    unsafe {
        lcr3(KPGDIR.to_phys());
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
    assert!(init.len() >= kalloc::PGSIZE,
            "inituvm: size larger than a page");

    let mut mem = box [0 as u8; 4096];

    mem[..init.len()].copy_from_slice(init);
    map_pages(pgdir,
              VirtAddr::new(0),
              kalloc::PGSIZE,
              VirtAddr::new(Box::into_raw(mem) as usize).to_phys(),
              USER | WRITABLE)
        .expect("inituvm: Map pages failed");

}

/// Given page directory entries, Create PTEs for virtual addresses starting at va.
fn map_pages(p: &mut [PageDirEntry],
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

        a = a.offset::<u8>(kalloc::PGSIZE as isize);
        pa = pa.offset::<u8>(kalloc::PGSIZE as isize);
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
