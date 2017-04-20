use x86::shared::io;
use traps;
use picirq;


const COM1: u16 = 0x3f8;
static mut UART_PRESENT: bool = false;

pub struct Uart {}

impl Uart {
    pub unsafe fn new() -> Result<Uart, &'static str> {
        io::outb(COM1, 0);
        io::outb(COM1 + 3, 0x80); // Unlock divisor
        io::outb(COM1, (115200u32 / 9600u32) as u8);
        io::outb(COM1 + 1, 0);
        io::outb(COM1 + 3, 0x03); // Lock divisor, 8 data bits.
        io::outb(COM1 + 4, 0);
        io::outb(COM1 + 1, 0x01); // Enable receive interrupts.

        // If status is 0xFF, no serial port.
        if io::inb(COM1 + 5) == 0xFF {
            return Err("No console present");
        }

        UART_PRESENT = true;

        // Acknowledge pre-existing interrupt conditions;
        // enable interrupts.
        io::inb(COM1 + 2);
        io::inb(COM1);
        picirq::picenable(traps::IRQ_COM1 as i32);

        /*
        // Announce that we're here.
        for(p="xv6...\n"; *p; p++) {
            uartputc(*p);
        }
        */
        Ok(Uart {})
    }

    fn uartputc(&self, c: u8) {
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
}

// Copied from Rust stdlib
macro_rules! println {
    () => (print!("\n"));
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
