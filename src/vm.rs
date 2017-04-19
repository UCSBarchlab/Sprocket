use kalloc;
use alloc::boxed::Box;
use core;
use core::ops::Sub;
use process;
use x86::shared::segmentation::SegmentDescriptor;
use x86::shared::segmentation as seg;
use x86::shared::dtables;
use x86::shared;
use mmu;


extern "C" {
    static data: u8;
}

// Virtual address at beginning of data segment
// see kernel.ld for more info

pub struct Kmap {
    virt: VirtAddr,
    p_start: PhysAddr,
    p_end: PhysAddr,
    perm: Entry,
}

lazy_static! {
    static ref KMAP: [Kmap; 4] = {
        #[allow(non_snake_case)]
        let DATA_BEGIN: VirtAddr = unsafe {VirtAddr(&data as *const _ as usize)};
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
pub struct PageDirEntry(pub Entry);

bitflags! {
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

#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone)]
#[repr(C)]
pub struct PageTableEntry(Entry);

#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Default)]
pub struct PhysAddr(pub usize);
#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Default)]
pub struct VirtAddr(pub usize);

pub const PDXSHIFT: usize = 22;
enum Segment {
    Null = 0,
    Kcode = 1,
    Kdata = 2,
    Kcpu = 3,
    Ucode = 4,
    Udata = 5,
    Tss = 6,
}


impl PhysAddr {
    pub fn new(addr: usize) -> PhysAddr {
        PhysAddr(addr)
    }

    pub fn to_virt(&self) -> VirtAddr {
        VirtAddr::new(self.0)
    }
}

impl Sub for PhysAddr {
    type Output = usize;

    fn sub(self, other: PhysAddr) -> usize {
        self.0 - other.0
    }
}

impl Sub for VirtAddr {
    type Output = usize;
    fn sub(self, other: VirtAddr) -> usize {
        self.0 - other.0
    }
}

impl VirtAddr {
    pub fn new(addr: usize) -> VirtAddr {
        VirtAddr(addr)
    }

    pub fn to_phys(&self) -> PhysAddr {
        PhysAddr::new(self.0)
    }
}

pub trait Address {
    fn new(usize) -> Self where Self: core::marker::Sized;

    fn addr(&self) -> usize;

    fn page_dir_index(&self) -> usize {
        (self.addr() >> PDXSHIFT) & 0x3FF
    }

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
    fn int_offset(&self, off: isize) -> Self
        where Self: core::marker::Sized
    {
        if off > 0 {
            Self::new(self.addr() + (4 * off as usize))
        } else {
            Self::new(self.addr() - (4 * off as usize))
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

pub fn seginit() {

    unsafe {
        /*
        if process::CPU.is_none() {
        }
        */
        if process::CPU.is_none() {
            process::CPU = Some(process::Cpu::new());
        }


        // TODO: make sure these are proper type of segment being created
        if let Some(ref mut cpu) = process::CPU {
            cpu.gdt[Segment::Null as usize] =
                SegmentDescriptor::new(0,
                                       0,
                                       seg::Type::Code(seg::CODE_READ),
                                       false,
                                       shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::Kcode as usize] =
                SegmentDescriptor::new(0,
                                       0xffffffff,
                                       seg::Type::Code(seg::CODE_READ),
                                       false,
                                       shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::Kdata as usize] =
                SegmentDescriptor::new(0,
                                       0xffffffff,
                                       seg::Type::Data(seg::DATA_WRITE),
                                       false,
                                       shared::PrivilegeLevel::Ring0);
            cpu.gdt[Segment::Ucode as usize] =
                SegmentDescriptor::new(0,
                                       0xffffffff,
                                       seg::Type::Code(seg::CODE_READ),
                                       false,
                                       shared::PrivilegeLevel::Ring3);
            cpu.gdt[Segment::Udata as usize] =
                SegmentDescriptor::new(0,
                                       0xffffffff,
                                       seg::Type::Data(seg::DATA_WRITE),
                                       false,
                                       shared::PrivilegeLevel::Ring3);

            cpu.gdt[Segment::Kcpu as usize] =
                SegmentDescriptor::new(&(cpu.cpu) as *const _ as u32,
                                       8,
                                       seg::Type::Data(seg::DATA_WRITE),
                                       false,
                                       shared::PrivilegeLevel::Ring0);

            {

                let d = dtables::DescriptorTablePointer::new_gdtp(&cpu.gdt[0..mmu::NSEGS]);
                dtables::lgdt(&d);
                seg::load_gs(seg::SegmentSelector::new(Segment::Kcpu as u16,
                                                       shared::PrivilegeLevel::Ring0));
                // TODO: figure out how to make a TSS
                /*
                asm!("mov $0, %gs:0" : : "r" (cpu) : : "volatile");
                asm!("mov $0, %gs:4" : : "r" (0)   : : "volatile");
                */


            }
        }
    }
}

pub fn kvmalloc() {
    unsafe {
        KPGDIR = setupkvm().unwrap();
    }
    switchkvm();
}

fn setupkvm() -> Result<VirtAddr, ()> {

    let pgdir = Box::into_raw(box [PageDirEntry(Entry::empty()); 1024]); // allocate new page table

    // We know this is okay, just for convenience
    let slice = unsafe { &mut (*pgdir)[0..1024] };
    assert!(kalloc::PHYSTOP.to_virt() <= kalloc::DEVSPACE);


    for k in KMAP.iter() {
        map_pages(slice, k.virt, k.p_end - k.p_start, k.p_start, k.perm)?;
    }

    return Ok(VirtAddr::new(pgdir as usize));
}

fn switchkvm() {
    unsafe {
        let addr = KPGDIR.to_phys().addr();
        asm!("mov $0, %cr3" : : "r" (addr) : : "volatile")
    }
}

fn map_pages(p: &mut [PageDirEntry],
             va: VirtAddr,
             size: usize,
             mut pa: PhysAddr,
             permissions: Entry)
             -> Result<(), ()> {
    let mut a = va.page_rounddown();
    let last = va.int_offset((size - 1) as isize).page_rounddown();

    loop {
        let pte = walkpgdir(p, a, true)?;

        if pte.0.contains(PRESENT) {
            panic!("remap failed");
        }
        let mut new_entry = permissions | PRESENT;
        new_entry.bits |= pa.addr();
        *pte = PageTableEntry(new_entry);
        if a == last {
            break;
        }

        a = a.int_offset(1024);
        pa = pa.int_offset(1024);
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
    let r = pgtab.0 as *mut PageTableEntry;
    unsafe { Ok(r.as_mut().unwrap()) }
}
