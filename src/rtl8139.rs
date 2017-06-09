use pci;
use picirq;
use traps;
use x86::shared::io;
pub const REALTEK: u16 = 0x10ec;
pub const RTL_8139: u16 = 0x8139;
use alloc::boxed::Box;
use vm::{VirtAddr, PhysAddr, Address};

const CONFIG_REG1: u16 = 0x52;
const CMD_REG: u16 = 0x37;
const RB_START_REG: u16 = 0x30;
const RX_CONFIG_REG: u16 = 0x44;
const IMR_REG: u16 = 0x3C;
const BUF_SIZE: usize = 8192 + 1500 + 16;

pub struct Rtl8139 {
    pci: pci::PciDevice,
    iobase: u16,
    pub buffer: Box<[u8; BUF_SIZE]>,
}

pub static mut NIC: Option<Rtl8139> = None;

impl Rtl8139 {
    pub unsafe fn init() -> Option<Rtl8139> {
        if let Some(mut dev) = pci::find(REALTEK, RTL_8139) {
            println!("{:016b}", dev.read16(pci::CMD_REG_OFFSET));
            // Enable bus mastering
            dev.set_command_flags(pci::BUS_MASTER);
            println!("{:016b}", dev.read16(pci::CMD_REG_OFFSET));
            let iobase = dev.read32(0x10);

            assert_eq!((iobase & 0x1) as u8, pci::BAR_TYPE_IO);
            let mut rtl = Rtl8139 {
                pci: dev,
                iobase: (iobase & !0x3) as u16,
                buffer: box [0; BUF_SIZE],
            };
            rtl.enable();
            rtl.reset();
            io::outl(rtl.iobase + CMD_REG, 0x0c); // Enable TX/RX
            rtl.program_rx_buf();


            rtl.configure_interrupts();


            // Enable wrapping, accept all packets (promiscuous mode)
            io::outl(rtl.iobase + RX_CONFIG_REG, 0xf | (1 << 7));



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
            println!("Waiting for card");
            while {
                io::inb(self.iobase + CMD_REG) & 0x10 != 0
            } {
                print!(".")
            }
        }
    }

    fn program_rx_buf(&mut self) {
        unsafe {
            let virt_addr = VirtAddr::new(&mut (self.buffer[0]) as *mut u8 as usize);
            io::outl(self.iobase + RB_START_REG,
                     virt_addr.to_phys().addr() as u32);
            println!("Buffer stored at: {:#x}", virt_addr.to_phys().addr());
        }
    }

    fn configure_interrupts(&mut self) {
        // enable Transmit OK and Recieve OK interrupts
        unsafe {
            // io::outw(self.iobase + IMR_REG, 0x0005);
            io::outw(self.iobase + IMR_REG, 0b1110000001111111);
            let intr: u8 = 0x00ff & self.pci.read16(0x3c) as u8;
            picirq::picenable(intr as i32);
            picirq::picenable(2 as i32);
            let pin: u8 = (0xff00 & self.pci.read16(0x3c)) as u8;

            println!("Interrupts enabled on {:#x}, pin {}", intr, pin);
        }
    }
}
