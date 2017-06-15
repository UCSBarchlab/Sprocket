use pci;
use picirq;
use traps;
use x86::shared::io;
pub const REALTEK: u16 = 0x10ec;
pub const RTL_8139: u16 = 0x8139;
use alloc::boxed::Box;
use mem::{VirtAddr, Address};
use smoltcp::Error;
use smoltcp::phy::Device;

const CONFIG_REG1: u16 = 0x52;
const CMD_REG: u16 = 0x37;
const RB_START_REG: u16 = 0x30;
const RX_CONFIG_REG: u16 = 0x44;
const IMR_REG: u16 = 0x3C;
const ISR_REG: u16 = 0x3E;
const TSR0_OFF: u16 = 0x10;
const BASE_BUF_SIZE: usize = 8192;
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


            let virt_addr = VirtAddr::new(&mut (rtl.rx_buffer[0]) as *mut u8 as usize);
            println!("Buffer lives at {:#08x}", virt_addr.to_phys().addr());
            io::outl(rtl.iobase + RB_START_REG, virt_addr.to_phys().addr() as u32);

            // Enable interrupts for TX OK & RX OK
            io::outw(rtl.iobase + IMR_REG, IntStatus::all().bits);

            // Enable card in promiscuous mode, enable wrap bit,
            // tell it the size of the buffer
            let config = WRAP | ACCEPT_PHYS_MATCH | ACCEPT_BCAST | RX_BUF_8K;
            io::outl(rtl.iobase + RX_CONFIG_REG, config.bits);

            // Enable TX and RX
            io::outb(rtl.iobase + CMD_REG, (RX_ENABLE | TX_ENABLE).bits);

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
        unsafe { io::inw(self.iobase + CAPR) as usize }
    }

    pub fn interrupt(&mut self) {
        let isr = self.get_isr();
        //println!("{:?}", isr);
        self.clear_isr();
        while !self.rx_empty() && isr.contains(RX_OK) {

            /*
            println!("Packet header: {:?}", self.get_rx_hdr());
            println!("Length: {:#04x}", self.get_rx_len());
            unsafe {
                println!("CBA: {:#04x}", io::inw(self.iobase + CBA));
            }
            */

            let b = {
                self.read().unwrap().to_vec()
            };
            use smoltcp::wire::{EthernetFrame, PrettyPrinter};
            print!("{}", PrettyPrinter::<EthernetFrame<&[u8]>>::new("", &b));

            // Ensure that the new CAPR is dword aligned
            self.rx_offset = (self.rx_offset + self.get_rx_len() + 4 + 3) & !3;
            //println!("NEW CAPR: {:#04x}", self.rx_offset);

            // set CAPR slightly below actual offset because cryptic manual told us to
            let new_capr = self.rx_offset; // force copy to appease borrowck
            self.set_capr(new_capr - 0x10);

            if self.rx_offset >= BASE_BUF_SIZE {
                self.rx_offset %= BASE_BUF_SIZE;
            }
        }
    }

    pub fn rx_empty(&self) -> bool {
        let reg = unsafe { io::inb(self.iobase + CMD_REG) };
        CommandReg::from_bits_truncate(reg).contains(RX_BUF_EMPTY)
    }

    fn read(&mut self) -> Option<&[u8]> {
        if !self.rx_empty() {
            let len = self.get_rx_len() as usize;
            //println!("len={}", len);
            Some(&self.rx_buffer[self.rx_offset + 4..self.rx_offset + 4 + len])
        } else {
            None
        }
    }

    fn get_rx_hdr(&self) -> RxHeader {
        let off = self.rx_offset as usize;
        let b1 = self.rx_buffer[off];
        let b2 = self.rx_buffer[off + 1];
        let h = RxHeader::from_bits_truncate(((b2 as u16) << 8) | (b1 as u16));
        h
    }

    fn get_rx_len(&self) -> usize {
        let off = self.rx_offset as usize;
        let b1 = self.rx_buffer[off + 2];
        let b2 = self.rx_buffer[off + 3];
        let len = (((b2 as u16) << 8) | (b1 as u16)) as usize;
        len
    }

    fn get_isr(&self) -> IntStatus {
        let reg = unsafe { io::inw(self.iobase + ISR_REG) };
        IntStatus::from_bits(reg).unwrap()
    }
    fn clear_isr(&mut self) {
        unsafe {
            io::outw(self.iobase + ISR_REG, 0xffff);
        };
    }

    fn set_capr(&mut self, off: usize) {
        assert!(off < BUF_SIZE);
        unsafe { io::outw(self.iobase + CAPR, off as u16) };
    }
}

bitflags! {
    pub flags CommandReg: u8 {
        const RX_BUF_EMPTY = 1,
        // reserved
        const TX_ENABLE = 1 << 2,
        const RX_ENABLE = 1 << 3,
        const RESET     = 1 << 4,
    }
}

bitflags! {
    pub flags RxConfig: u32 {
        const ACCEPT_ALL = 1,
        const ACCEPT_PHYS_MATCH = 1 << 1,
        const ACCEPT_MULTICAST = 1 << 2,
        const ACCEPT_BCAST = 1 << 3,
        const ACCEPT_RUNT = 1 << 4,
        const ACCEPT_ERR = 1 << 5,
        const WRAP               = 1 << 7,
        // Max DMA burst config flags are not implemented here
        const RX_BUF_8K = 0b00 << 11,
        const RX_BUF_16K = 0b01 << 11,
        const RX_BUF_32K = 0b10 << 11,
        const RX_BUF_64K = 0b11 << 11,
        // RX FIFO Threshold flags are not implemented here
        const RER8               = 1 << 16,
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

bitflags! {
    pub flags RxHeader: u16 {
        const RX_OK_          = 1,
        const FRAME_ALIGN_ERR = 1 << 1,
        const CRC_ERR         = 1 << 2,
        const LONG_PKT        = 1 << 3,
        const RUNT_PKT        = 1 << 4,
        const INVAL_SYM_ERR   = 1 << 5,
        const BCAST_PKT       = 1 << 13,
        const PHYS_MATCH      = 1 << 14,
        const MULTICAST_PKT   = 1 << 15,
    }
}

impl Device for Rtl8139 {
    type RxBuffer = EthernetRxBuffer;
    type TxBuffer = EthernetTxBuffer;

    fn receive(&mut self) -> Result<Self::RxBuffer, Error> {
        unsafe {
            if let Some(ref mut n) = NIC {
                if let Some(ref b) = n.read() {
                    let rx = EthernetRxBuffer(b);
                    return Ok(rx);
                }
            }
        }
        Err(Error::Exhausted)
    }

    fn transmit(&mut self, _length: usize) -> Result<Self::TxBuffer, Error> {
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

pub struct EthernetRxBuffer(pub &'static [u8]);

impl AsRef<[u8]> for EthernetRxBuffer {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl Drop for EthernetRxBuffer {
    fn drop(&mut self) {
        unsafe {
            //NIC.as_mut().unwrap().hw_transmit(self.0);
            // update CAPR to point to next packet, no longer need this
            //println!("packet is done!");
            if let Some(ref mut n) = NIC {
                n.rx_offset = (n.rx_offset + self.0.len()) % BUF_SIZE;
                let new_capr = n.rx_offset;
                n.set_capr(new_capr);
            }
        }
    }
}
