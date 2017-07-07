use x86::shared::io;
use traps;
use picirq;
use core::sync::atomic::{AtomicBool, Ordering};

const COM1: u16 = 0x3f8;

static UART_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub struct Uart;

impl Uart {
    pub fn new() -> Result<Uart, ()> {
        // Consume token and init the UART (if present)
        if !UART_INITIALIZED.load(Ordering::SeqCst) {
            UART_INITIALIZED.store(true, Ordering::SeqCst);
            unsafe { Self::init() }
        } else {
            Err(())
        }
    }

    unsafe fn init() -> Result<Uart, ()> {
        // unsafe because port I/O is hideously unsafe and a misconfigured PIC is bad
        // we may be able to leverage better abstractions though
        // see: http://www.randomhacks.net/2015/11/16/bare-metal-rust-configure-your-pic-interrupts/
        io::outb(COM1, 0);
        io::outb(COM1 + 3, 0x80); // Unlock divisor
        io::outb(COM1, (115200u32 / 9600u32) as u8);
        io::outb(COM1 + 1, 0);
        io::outb(COM1 + 3, 0x03); // Lock divisor, 8 data bits.
        io::outb(COM1 + 4, 0);
        io::outb(COM1 + 1, 0x01); // Enable receive interrupts.

        // If status is 0xFF, no serial port.
        if io::inb(COM1 + 5) == 0xFF {
            return Err(());
        }

        // Acknowledge pre-existing interrupt conditions;
        // enable interrupts.
        io::inb(COM1 + 2);
        io::inb(COM1);
        picirq::PIC.lock().enable_irq(traps::COM1_IRQ as u32);

        /*
        // Announce that we're here.
        for(p="xv6...\n"; *p; p++) {
            uartputc(*p);
        }
        */
        Ok(Uart {})
    }

    pub fn write_byte(&mut self, c: u8) {
        unsafe {
            for _ in 0..128 {
                if io::inb(COM1 + 5) & 0x20 != 0 {
                    break;
                }
            }
            io::outb(COM1, c);
        }
    }

    fn microdelay(_: i32) {}

    pub fn read_byte(&mut self) -> Option<u8> {
        unsafe {
            if (io::inb(COM1 + 5) & 0x01) == 0 {
                None
            } else {
                Some(io::inb(COM1))
            }
        }

    }
}
