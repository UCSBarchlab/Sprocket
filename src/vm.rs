use kalloc;
use alloc::boxed::Box;


extern "C" {
    static mut data: u8;
}

pub struct PageDirEntry(u32);

#[derive(Eq, PartialOrd, Ord, PartialEq, Copy, Clone, Default)]
pub struct PhysAddr(pub usize);
#[derive(Eq, PartialOrd, Ord, PartialEq, Copy, Clone, Default)]
pub struct VirtAddr(pub usize);

impl PhysAddr {
    pub fn new(addr: usize) -> PhysAddr {
        PhysAddr(addr)
    }

    pub fn to_virt(&self) -> VirtAddr {
        VirtAddr::new(self.0)
    }

    pub fn page_roundup(&self) -> PhysAddr {
        let addr = (self.0 + kalloc::PGSIZE - 1) & !(kalloc::PGSIZE - 1);
        PhysAddr(addr)
    }

    pub fn page_rounddown(&self) -> PhysAddr {
        let addr = (self.0) & !(kalloc::PGSIZE - 1);
        PhysAddr(addr)
    }

    pub fn is_page_aligned(&self) -> bool {
        self.0 % kalloc::PGSIZE == 0
    }
}

impl VirtAddr {
    pub fn new(addr: usize) -> VirtAddr {
        VirtAddr(addr)
    }

    pub fn to_phys(&self) -> PhysAddr {
        PhysAddr::new(self.0)
    }

    pub fn page_roundup(&self) -> VirtAddr {
        let addr = (self.0 + kalloc::PGSIZE - 1) & !(kalloc::PGSIZE - 1);
        VirtAddr(addr)
    }

    pub fn page_rounddown(&self) -> VirtAddr {
        let addr = (self.0) & !(kalloc::PGSIZE - 1);
        VirtAddr(addr)
    }

    pub fn is_page_aligned(&self) -> bool {
        self.0 % kalloc::PGSIZE == 0
    }
}


pub static mut KPGDIR: u32 = 0;

fn seginit() {
    /*
    if let Some(cpu) = CPUS[lapic::cpunum() as usize] {
        cpu.gdt = 0;
    }
    */
}

fn kvmalloc() {
    unsafe {
        KPGDIR = setupkvm();
    }
    switchkvm();
}

fn setupkvm() -> u32 {

    let pgdir = Box::new(0); // allocate new page table

    assert!(kalloc::PHYSTOP.to_virt() > kalloc::DEVSPACE);

    0
}

fn switchkvm() {}

fn map_pages(p: PageDirEntry, v: VirtAddr, size: usize, permissions: u32) {}
