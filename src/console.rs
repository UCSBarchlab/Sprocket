use spin;
use uart;

use core::fmt;

const BACKSPACE: u8 = 0x08;

// Ensure that console is only initialized once
//static CONSOLE: spin::Once<Console> = spin::Once::new();

lazy_static! {
    pub static ref CONSOLE: spin::Mutex<Console> = spin::Mutex::new(Console::new());
}


const INPUT_BUF: usize = 128;

struct Input {
    buf: [u8; INPUT_BUF],
    r: usize, // Read index
    w: usize, // Write index
    e: usize, // Edit index
}

pub struct Console {
    uart: Option<uart::Uart>,
}

impl Console {
    fn new() -> Console {
        Console { uart: uart::Uart::new().ok() }
    }

    fn write_byte(&mut self, b: u8) {
        if let Some(ref mut u) = self.uart {
            if b == BACKSPACE {
                u.write_byte(b'\r');
                u.write_byte(b' ');
                u.write_byte(b'\r');
            } else {
                u.write_byte(b);
            }
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for b in s.bytes() {
            self.write_byte(b);
        }
        Ok(())
    }
}

// Copied from Rust stdlib
macro_rules! println {
    () => (print!("\n"));
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

macro_rules! print {
    ($($arg:tt)*) => (
        { use ::core::fmt::Write;
          $crate::console::CONSOLE.lock().write_fmt(format_args!($($arg)*)).ok();
    });
}
