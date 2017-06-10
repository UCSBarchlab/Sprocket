use x86::shared::io;

pub const INVALID_VENDOR: u16 = 0xffff;

const CONFIG_ADDRESS: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;

pub const REALTEK: u16 = 0x10ec;
pub const RTL_8139: u16 = 0x8139;
pub const VEND_ID_OFFSET: u8 = 0;
pub const DEV_ID_OFFSET: u8 = 2;
pub const CMD_REG_OFFSET: u8 = 4;
pub const HDR_TYPE_OFFSET: u8 = 0xe;

pub const BAR_TYPE_IO: u8 = 0x1;

unsafe fn config_read16(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    (config_read32(bus, slot, func, offset) >> ((offset & 2) * 8) & 0xffff) as u16
}

// Adapted from http://wiki.osdev.org/PCI
unsafe fn config_read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let lbus: u32 = bus as u32;
    let lslot: u32 = slot as u32;
    let lfunc: u32 = func as u32;

    let address: u32 = (lbus << 16) | (lslot << 11) | (lfunc << 8) | ((offset as u32) & 0xfc) |
                       (0x80000000);

    io::outl(CONFIG_ADDRESS, address);
    io::inl(CONFIG_DATA)
}

unsafe fn config_write32(bus: u8, slot: u8, func: u8, offset: u8, config: u32) {
    let lbus: u32 = bus as u32;
    let lslot: u32 = slot as u32;
    let lfunc: u32 = func as u32;

    let address: u32 = (lbus << 16) | (lslot << 11) | (lfunc << 8) | ((offset as u32) & 0xfc) |
                       (0x80000000);

    io::outl(CONFIG_ADDRESS, address);
    io::outl(CONFIG_DATA, config);
}

unsafe fn config_write16(bus: u8, slot: u8, func: u8, offset: u8, config: u16) {

    // if our 16-bit word is the low half of the register, fetch the higher half, or vice versa
    let other_off = if (offset & 0xfc) % 4 == 0 {
        offset + 2
    } else {
        offset - 2
    };

    let word = config_read16(bus, slot, func, other_off);

    let data: u32 = if (offset & 0xfc) % 4 == 0 {
        (word as u32) << 16 | (config as u32)
    } else {
        (config as u32) << 16 | (word as u32)
    };

    config_write32(bus, slot, func, offset, data);
}

pub fn enumerate() {
    for bus in 0..256u16 {
        for slot in 0..32 {
            let vendor = unsafe { config_read16(bus as u8, slot, 0, VEND_ID_OFFSET) };
            match vendor {
                0x10ec => {
                    let dev_id = unsafe { config_read16(bus as u8, slot, 0, DEV_ID_OFFSET) };
                    println!("    Found RTL-{:x} at {},{}", dev_id, bus, slot);
                }
                INVALID_VENDOR => {}
                v_id @ _ => {
                    println!("    Found unknown device at {},{} with vendor ID {:x}",
                             bus,
                             slot,
                             v_id)
                }
            }
        }
    }
}

pub fn find(vendor: u16, device: u16) -> Option<PciDevice> {
    for bus in 0..256u16 {
        for slot in 0..32 {
            let vend_id = unsafe { config_read16(bus as u8, slot, 0, VEND_ID_OFFSET) };
            let dev_id = unsafe { config_read16(bus as u8, slot, 0, DEV_ID_OFFSET) };
            if vendor == vend_id && dev_id == device {
                return Some(PciDevice::new(bus as u8, slot, 0));
            }
        }
    }
    None
}

bitflags! {
    pub flags Command: u16 {
        const IO_SPACE            = 1,
        const MEM_SPACE           = 1 << 1,
        const BUS_MASTER          = 1 << 2,
        const SPECIAL_CYCLES      = 1 << 3,
        const MEM_WR_AND_INVALID  = 1 << 4,
        const VGA_PALETTE_SNOOP   = 1 << 5,
        const PARITY_ERR_RESP     = 1 << 6,
        const RESERVED_1          = 1 << 7,
        const SERR                = 1 << 8,
        const FAST_BACK2BACK      = 1 << 9,
        const INT_DISABLE         = 1 << 10,
        const RESERVED_2          = 1 << 11,
        const RESERVED_3          = 1 << 12,
        const RESERVED_4          = 1 << 13,
        const RESERVED_5          = 1 << 14,
        const RESERVED_6          = 1 << 15,
    }
}

pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
}

impl PciDevice {
    pub fn new(bus: u8, slot: u8, func: u8) -> PciDevice {
        PciDevice {
            bus: bus,
            slot: slot,
            func: func,
        }
    }

    // unsafe because we're manipulating external PCI state that rustc can't even begin to comprehend
    pub unsafe fn set_command_flags(&mut self, flag: Command) {
        let mut config =
            Command::from_bits(config_read16(self.bus, self.slot, self.func, CMD_REG_OFFSET))
                .expect("Invalid PCI bits!");

        config.insert(flag);

        config_write16(self.bus, self.slot, self.func, CMD_REG_OFFSET, config.bits);
    }

    pub fn read16(&self, offset: u8) -> u16 {
        unsafe { config_read16(self.bus, self.slot, self.func, offset) }
    }

    pub fn read32(&self, offset: u8) -> u32 {
        unsafe { config_read32(self.bus, self.slot, self.func, offset) }
    }

    pub fn write32(&mut self, offset: u8, data: u32) {
        unsafe { config_write32(self.bus, self.slot, self.func, offset, data) };
    }

    pub fn header_type(&self) -> u16 {
        self.read16(HDR_TYPE_OFFSET)
    }

    pub fn read_bar(&self, bar: Bar) -> u32 {
        self.read32(bar as u8)
    }

    // return (line, pin)
    pub fn read_irq(&self) -> (u8, u8) {
        let word = self.read16(0x3c);
        ((word & 0xff) as u8, (word >> 8) as u8)
    }
}

#[repr(u8)]
pub enum Bar {
    Bar0 = 0x10,
    Bar1 = 0x14,
    Bar2 = 0x18,
    Bar3 = 0x1C,
    Bar4 = 0x20,
    Bar5 = 0x24,
}


/// Support for PCIe memory-mapped configuration
pub mod pcie {
    use vm::{KPGDIR, VirtAddr, PhysAddr, Address};
    use vm;
    use kalloc;
    #[repr(C, packed)]
    #[derive(Debug)]
    pub struct RSDPDescriptor {
        pub signature: [u8; 8],
        pub checksum: u8,
        pub oem_id: [u8; 6],
        pub revision: u8,
        pub rsdt_address: u32,
    }

    #[repr(C, packed)]
    pub struct RSDPDescriptor20 {
        pub first: RSDPDescriptor,
        pub length: u32,
        pub xsdt_address: u64,
        pub extended_checksum: u8,
        pub reserved: [u8; 3],
    }

    #[repr(C, packed)]
    pub struct ACPISDTHeader {
        pub signature: [u8; 4],
        pub length: u32,
        pub revision: u8,
        pub checksum: u8,
        pub oemid: [u8; 6],
        pub oem_table_id: [u8; 8],
        pub oem_revision: u32,
        pub creator_id: u32,
        pub creator_revision: u32,
    }

    #[repr(C, packed)]
    pub struct RSDT {
        pub header: ACPISDTHeader,
        other_sdts: *const u32, // pointers to the rest of the structure
    }

    #[repr(C, packed)]
    pub struct MCFG {
        header: ACPISDTHeader,
        reserved: [u8; 8],
        config_space: *mut ConfigSpace,
    }

    #[repr(C, packed)]
    pub struct ConfigSpace {
        base_addr: u64,
        group_no: u16,
        bus_start: u8,
        bus_end: u8,
        reserved: [u8; 4],
    }
    impl RSDT {
        pub unsafe fn find_sdt<T>(rsdt_address: u32, sig: &[u8; 4]) -> Option<*const T> {
            let rsdt = rsdt_address as *const RSDT;
            let len = (*rsdt).header.length as usize;
            let size = ::core::mem::size_of::<ACPISDTHeader>();
            let sdts = ::core::slice::from_raw_parts((*rsdt).other_sdts, len - size / 4);

            sdts.iter()
                .map(|x| (x & 0xffff_ffff) as *const ACPISDTHeader)
                .find(|&x| (*x).signature == *sig)
                .map(|x| x as *const T)
        }
    }


    pub enum RSDP {
        V1(*const RSDPDescriptor),
        V2(*const RSDPDescriptor20),
    }



    /// Discover the address of the RSDP Descriptor
    pub unsafe fn probe_rsdp() -> Option<RSDP> {
        let string = b"RSD PTR ";
        for addr in 0x000e0000..0x000fffff_usize {
            let mem = ::core::slice::from_raw_parts(addr as *const u8, 8);
            if mem == string {
                println!("Found ACPI Table at {}", addr);
                let rsdp: *const RSDPDescriptor = addr as *const RSDPDescriptor;
                if (*rsdp).revision == 0 {
                    return Some(RSDP::V1(rsdp));

                } else {
                    return Some(RSDP::V2(rsdp as *const RSDPDescriptor20));
                }
            }
        }
        None
    }
    pub fn haha() {

        println!("Enumerating PCI");
        unsafe {
            if let Some(p) = probe_rsdp() {
                if let RSDP::V1(rsdp) = p {
                    println!("Discovered RSDP (ACPI 1.0) header at {:x}", rsdp as usize);
                    let pgdir = ::core::slice::from_raw_parts_mut(KPGDIR.addr() as
                                                                  *mut vm::PageDirEntry,
                                                                  1024);
                    vm::map_pages(pgdir,
                                  VirtAddr::new((*rsdp).rsdt_address as usize),
                                  ::core::mem::size_of::<RSDT>(),
                                  PhysAddr::new((*rsdp).rsdt_address as usize),
                                  vm::PRESENT)
                        .unwrap();
                    let rsdt = ((*rsdp).rsdt_address) as *const RSDT;
                    println!("entries: {}", (*rsdt).header.length as usize);
                    vm::map_pages(pgdir,
                                  VirtAddr::new((*rsdp).rsdt_address as usize + kalloc::PGSIZE),
                                  (*rsdt).header.length as usize,
                                  PhysAddr::new((*rsdp).rsdt_address as usize + kalloc::PGSIZE),
                                  vm::PRESENT)
                        .unwrap();
                    println!("Searching for MCFG table");
                    RSDT::find_sdt::<MCFG>((*rsdp).rsdt_address, b"MCFG");
                }
            }
        }
    }


}
