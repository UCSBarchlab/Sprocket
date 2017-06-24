#![no_std]

extern crate x86;
#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate log;

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
                    info!("    Found RTL-{:x} at {},{}", dev_id, bus, slot);
                }
                INVALID_VENDOR => {}
                v_id @ _ => {
                    info!("    Found unknown device at {},{} with vendor ID {:x}",
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
    pub struct Command: u16 {
        const IO_SPACE            = 1;
        const MEM_SPACE           = 1 << 1;
        const BUS_MASTER          = 1 << 2;
        const SPECIAL_CYCLES      = 1 << 3;
        const MEM_WR_AND_INVALID  = 1 << 4;
        const VGA_PALETTE_SNOOP   = 1 << 5;
        const PARITY_ERR_RESP     = 1 << 6;
        const RESERVED_1          = 1 << 7;
        const SERR                = 1 << 8;
        const FAST_BACK2BACK      = 1 << 9;
        const INT_DISABLE         = 1 << 10;
        const RESERVED_2          = 1 << 11;
        const RESERVED_3          = 1 << 12;
        const RESERVED_4          = 1 << 13;
        const RESERVED_5          = 1 << 14;
        const RESERVED_6          = 1 << 15;
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
