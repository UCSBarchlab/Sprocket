use pci;
use picirq;
use traps;
use x86::shared::io;
pub const REALTEK: u16 = 0x10ec;
pub const RTL_8139: u16 = 0x8139;
use alloc::boxed::Box;
use vm::{VirtAddr, Address};
use smoltcp::Error;
use smoltcp::phy::Device;

const CONFIG_REG1: u16 = 0x52;
const CMD_REG: u16 = 0x37;
const RB_START_REG: u16 = 0x30;
const RX_CONFIG_REG: u16 = 0x44;
const IMR_REG: u16 = 0x3C;
const BUF_SIZE: usize = 8192 + 1500 + 16;

pub struct Rtl8139 {
    pci: pci::PciDevice,
    pub iobase: u16,
    pub buffer: Box<[u8; BUF_SIZE]>,
}

pub static mut NIC: Option<Rtl8139> = None;

impl Rtl8139 {
    pub unsafe fn init() -> Option<Rtl8139> {
        if let Some(mut dev) = pci::find(REALTEK, RTL_8139) {

            // Enable PCI bus mastering
            dev.set_command_flags(pci::BUS_MASTER);

            let bar0 = dev.read_bar(pci::Bar::Bar0);
            assert_eq!((bar0 & 0x1) as u8, pci::BAR_TYPE_IO);
            let iobase = (bar0 & !(0x3)) as u16;
            let mut rtl = Rtl8139 {
                pci: dev,
                iobase: iobase,
                buffer: box [0; BUF_SIZE],
            };

            // Power on the card
            io::outb(rtl.iobase + CONFIG_REG1, 0x0);

            // Perform software reset
            io::outb(rtl.iobase + CMD_REG, 0x10);
            while {
                (io::inb(rtl.iobase + CMD_REG) & 0x10) != 0
            } {}

            let virt_addr = VirtAddr::new(&mut (rtl.buffer[0]) as *mut u8 as usize);
            io::outl(rtl.iobase + RB_START_REG, virt_addr.to_phys().addr() as u32);

            // Enable interrupts for TX OK & RX OK
            io::outw(rtl.iobase + IMR_REG, 0x0005);

            // Enable card in promiscuous mode, enable wrap bit,
            // tell it the size of the buffer
            io::outl(rtl.iobase + RX_CONFIG_REG, 0xf | (1 << 7));

            // Enable TX and RX
            io::outb(rtl.iobase + CMD_REG, 0x0c);

            // Unmask NIC interrupts in the PIC
            let (line, _) = rtl.pci.read_irq();
            assert_eq!(traps::NIC_IRQ, line);
            picirq::picenable(traps::NIC_IRQ as i32);

            Some(rtl)
        } else {
            None
        }
    }
}
