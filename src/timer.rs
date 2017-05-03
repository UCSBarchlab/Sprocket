use x86::shared::io;
use traps;
use picirq;

const IO_TIMER1: u16 = 0x040; // 8253 Timer #1

// Frequency of all three count-down timers;
// (TIMER_FREQ/freq) is the appropriate count
// to generate a frequency of freq Hz.

const TIMER_FREQ: u32 = 1193182;

macro_rules! timer_div {
    ($x:expr) => { ((TIMER_FREQ+($x)/2)/($x))}
}

const TIMER_MODE: u16 = (IO_TIMER1 + 3); // timer mode port
const TIMER_SEL0: u8 = 0x00; // select counter 0
const TIMER_RATEGEN: u8 = 0x04; // mode 2, rate generator
const TIMER_16BIT: u8 = 0x30; // r/w counter 16 bits, LSB first

pub fn timerinit() {
    unsafe {
        io::outb(TIMER_MODE, TIMER_SEL0 | TIMER_RATEGEN | TIMER_16BIT);
        io::outb(IO_TIMER1, (timer_div!(100) % 256) as u8);
        io::outb(IO_TIMER1, (timer_div!(100) / 256) as u8);
        picirq::picenable(traps::IRQ_TIMER as i32);
    }
}
