use pci;
use x86::shared::io;
pub const REALTEK: u16 = 0x10ec;
pub const RTL_8139: u16 = 0x8139;

const CONFIG_REG1: u16 = 0x52;
const CMD_REG: u16 = 0x37;
const RB_START_REG: u16 = 0x30;

pub struct Rtl8139 {
    pci: pci::PciDevice,
    iobase: u16,
}

impl Rtl8139 {
    pub unsafe fn init() -> Option<Rtl8139> {
        if let Some(mut dev) = pci::find(REALTEK, RTL_8139) {
            println!("{:016b}", dev.read16(pci::CMD_REG_OFFSET));
            dev.set_command_flags(pci::BUS_MASTER);
            println!("{:016b}", dev.read16(pci::CMD_REG_OFFSET));
            let iobase = dev.read32(0x10);
            assert_eq!((iobase & 0x1) as u8, pci::BAR_TYPE_IO);
            let mut rtl = Rtl8139 {
                pci: dev,
                iobase: (iobase & !0x3) as u16,
            };
            rtl.enable();
            rtl.reset();

            Some(rtl)
        } else {
            None
        }
    }

    fn enable(&mut self) {
        unsafe { io::outb(self.iobase + CONFIG_REG1, 0x0) };
    }

    fn reset(&mut self) {
        unsafe {
            io::outb(self.iobase + CMD_REG, 0x10);
            while {
                io::inb(self.iobase + CMD_REG) & 0x10 != 0
            } {}
        }
    }
}
