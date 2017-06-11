use pci;
use picirq;
use traps;
use x86::shared::io;
pub const REALTEK: u16 = 0x10ec;
pub const RTL_8139: u16 = 0x8139;
use alloc::boxed::Box;
use vm::{VirtAddr, PhysAddr, Address};
use smoltcp::Error;
use smoltcp::phy::Device;

const CONFIG_REG1: u16 = 0x52;
const CMD_REG: u16 = 0x37;
const RB_START_REG: u16 = 0x30;
const RX_CONFIG_REG: u16 = 0x44;
const IMR_REG: u16 = 0x3C;
const ISR_REG: u16 = 0x3E;
const TSR0_OFF: u16 = 0x10;
const BUF_SIZE: usize = 8192 + 1500 + 16;
const CAPR: u16 = 0x38;
const CBA: u16 = 0x3A;

const NUM_TX_BUFFERS: u8 = 4;

pub struct Rtl8139 {
    pci: pci::PciDevice,
    iobase: u16,
    rx_buffer: Box<[u8; BUF_SIZE]>,
    tx_buffer: Box<[[u8; 1600]; NUM_TX_BUFFERS as usize]>,
    tx_offset: u8, // which TX buffer we're using
    rx_offset: usize, // where in the RX ring buffer we are.  SW counterpart to CAPR
}

// NB Be aware that the RTL-8139 REALLY likes its buffers to be contiguous physical memory
// This isn't a problem with our page allocator, since it guarantees that virtual addresses are
// contiguously allocated and are page aligned.  That may not be the case for other allocators
// though.  If you feel like messing with the kernel allocator, proceed with caution!

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
                rx_buffer: box [0; BUF_SIZE],
                tx_buffer: box [[0; 1600]; NUM_TX_BUFFERS as usize],
                tx_offset: 0,
                rx_offset: 0,
            };

            // Power on the card
            io::outb(rtl.iobase + CONFIG_REG1, 0x0);

            // Perform software reset
            io::outb(rtl.iobase + CMD_REG, 0x10);
            while {
                (io::inb(rtl.iobase + CMD_REG) & 0x10) != 0
            } {}

            for (i, n) in rtl.rx_buffer.iter_mut().enumerate() {
                *n = 0xAB

            }

            io::outw(rtl.iobase + CAPR, 0);

            let virt_addr = VirtAddr::new(&mut (rtl.rx_buffer[0]) as *mut u8 as usize);
            println!("Buffer lives at {:#08x}", virt_addr.to_phys().addr());
            io::outl(rtl.iobase + RB_START_REG, virt_addr.to_phys().addr() as u32);
            println!("CAPR {:#08x}", rtl.get_capr());

            // Enable interrupts for TX OK & RX OK
            io::outw(rtl.iobase + IMR_REG, (TX_OK | RX_OK).bits);

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

    fn get_mac_address(&mut self) -> [u8; 6] {
        let mut mac = [0; 6];
        for (off, byte) in mac.iter_mut().enumerate() {
            *byte = unsafe { io::inb(self.iobase + (off as u16)) };
        }
        mac
    }

    fn next_tx_offset(tx_off: u8) -> u8 {
        (tx_off + 1) % NUM_TX_BUFFERS
    }

    fn hw_transmit(&mut self, buf: &[u8]) {
        unimplemented!();
    }

    pub fn get_capr(&self) -> usize {
        let addr = unsafe { io::inw(self.iobase + CAPR) };
        if addr > BUF_SIZE as u16 {
            (addr as usize) - BUF_SIZE
        } else {
            addr as usize
        }
    }

    pub fn interrupt(&mut self) {
        println!("{:#04x}", self.get_capr());
        println!("{:?}", self.get_isr());
        println!("Packet header: {:#04x}", self.get_rx_hdr());
        println!("Length: {:#04x}", self.get_rx_len());
        unsafe {
            println!("CBA: {:#04x}", io::inw(self.iobase + CBA));
        }
        self.clear_isr();
    }

    fn read(&mut self) -> &[u8] {
        let len = self.get_rx_len() as usize;
        &self.rx_buffer[self.rx_offset..len]
    }

    fn get_rx_hdr(&self) -> u16 {
        let off = self.rx_offset as usize;
        let b1 = self.rx_buffer[off];
        let b2 = self.rx_buffer[off + 1];
        ((b2 as u16) << 8) | (b1 as u16)
    }

    fn get_rx_len(&self) -> usize {
        let off = self.rx_offset as usize;
        let b1 = self.rx_buffer[off + 2];
        let b2 = self.rx_buffer[off + 3];
        (((b2 as u16) << 8) | (b1 as u16)) as usize
    }

    fn get_isr(&self) -> IntStatus {
        let reg = unsafe { io::inw(self.iobase + ISR_REG) };
        IntStatus::from_bits(reg).unwrap()
    }
    fn clear_isr(&mut self) {
        unsafe {
            let reg = io::inw(self.iobase + ISR_REG);
            io::outw(self.iobase + ISR_REG, reg);
        };
    }
}

bitflags! {
    pub flags CommandReg: u8 {
        const BUF_EMPTY = 1,
        const TX_ENABLE = 1 << 2,
        const RX_ENABLE = 1 << 3,
        const RESET     = 1 << 4,
    }
}

bitflags! {
    pub flags IntStatus: u16 {
        const RX_OK          = 1,
        const RX_ERR         = 1 << 1,
        const TX_OK          = 1 << 2,
        const TX_ERR         = 1 << 3,
        const RX_OVW         = 1 << 4,
        const PUN_LINKCHG    = 1 << 5,
        const FIFO_OVW       = 1 << 6,
        const LEN_CHG        = 1 << 13,
        const TIMEOUT        = 1 << 14,
        const SYS_ERR        = 1 << 15,
    }
}

impl Device for Rtl8139 {
    type RxBuffer = EthernetRxBuffer;
    type TxBuffer = EthernetTxBuffer;

    fn receive(&mut self) -> Result<Self::RxBuffer, Error> {
        unimplemented!();
    }

    fn transmit(&mut self, length: usize) -> Result<Self::TxBuffer, Error> {
        unimplemented!();
    }

    fn mtu(&self) -> usize {
        1536
    }
}

pub struct EthernetTxBuffer(&'static mut [u8]);

impl AsRef<[u8]> for EthernetTxBuffer {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl AsMut<[u8]> for EthernetTxBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0
    }
}

impl Drop for EthernetTxBuffer {
    fn drop(&mut self) {
        unsafe {
            NIC.as_mut().unwrap().hw_transmit(self.0);
        }
    }
}

pub struct EthernetRxBuffer(&'static mut [u8]);

impl AsRef<[u8]> for EthernetRxBuffer {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl AsMut<[u8]> for EthernetRxBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0
    }
}

impl Drop for EthernetRxBuffer {
    fn drop(&mut self) {
        unsafe {
            //NIC.as_mut().unwrap().hw_transmit(self.0);
            // update CAPR to point to next packet, no longer need this
        }
    }
}
