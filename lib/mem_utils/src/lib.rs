#![no_std]

use core::ops::Sub;

// Memory layout
pub const KERNBASE: VirtAddr = VirtAddr(0x80000000);
pub const KERNLINK: VirtAddr = VirtAddr(KERNBASE.0 + EXTMEM.0); // Address where kernel is linked
pub const EXTMEM: PhysAddr = PhysAddr(0x100000); // Start of extended memory
pub const PHYSTOP: PhysAddr = PhysAddr(0xE000000); // Top physical memory
pub const DEVSPACE: VirtAddr = VirtAddr(0xFE000000); // Other devices are at high addresses
pub const PGSIZE: usize = 4096;

pub const PDXSHIFT: usize = 22;
pub const PTXSHIFT: usize = 12;

/// A utility trait that implements common methods for `PhysAddr` and `VirtAddr`
pub trait Address {
    fn new(usize) -> Self where Self: core::marker::Sized;

    fn addr(&self) -> usize;

    fn is_page_aligned(&self) -> bool {
        self.addr() % PGSIZE == 0
    }

    fn page_roundup(&self) -> Self
        where Self: core::marker::Sized
    {
        let addr = (self.addr() + PGSIZE - 1) & !(PGSIZE - 1);
        Self::new(addr)
    }

    fn page_rounddown(&self) -> Self
        where Self: core::marker::Sized
    {
        let addr = self.addr() & !(PGSIZE - 1);
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

/// A convenience class to safely work with and manipulate physical addresses
#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Default)]
pub struct PhysAddr(pub usize);

/// A convenience class to safely work with and manipulate virtual (paged) addresses
#[derive(PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Default)]
pub struct VirtAddr(pub usize);


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

impl PhysAddr {
    pub fn new(addr: usize) -> PhysAddr {
        PhysAddr(addr)
    }

    pub fn to_virt(&self) -> VirtAddr {
        VirtAddr::new(self.0 + KERNBASE.addr())
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
        PhysAddr::new(self.0 - KERNBASE.addr())
    }

    pub fn page_dir_index(&self) -> usize {
        (self.addr() >> PDXSHIFT) & 0x3FF
    }

    pub fn page_table_index(&self) -> usize {
        (self.addr() >> PTXSHIFT) & 0x3FF
    }
}
