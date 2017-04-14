use core::ptr;
use traps;
use flags;
use core::sync::atomic::{AtomicBool, Ordering};
use mp;

const ID: isize = 0x0020 / 4; // ID
const VER: isize = 0x0030 / 4; // Version
const TPR: isize = 0x0080 / 4; // Task Priority
const EOI: isize = 0x00B0 / 4; // EOI
const SVR: isize = 0x00F0 / 4; // Spurious Interrupt Vector
const ENABLE: u32 = 0x00000100; // Unit Enable
const ESR: isize = 0x0280 / 4; // Error Status
const ICRLO: isize = 0x0300 / 4; // Interrupt Command
const INIT: u32 = 0x00000500; // INIT/RESET
const STARTUP: u32 = 0x00000600; // Startup IPI
const DELIVS: u32 = 0x00001000; // Delivery status
const ASSERT: u32 = 0x00004000; // Assert interrupt vs deassert
const DEASSERT: u32 = 0x00000000;
const LEVEL: u32 = 0x00008000; // Level triggered
const BCAST: u32 = 0x00080000; // Send to all APICs, including self.
const BUSY: u32 = 0x00001000;
const FIXED: u32 = 0x00000000;
const ICRHI: isize = 0x0310 / 4; // Interrupt Command [63:32]
const TIMER: isize = 0x0320 / 4; // Local Vector Table 0 TIMER
const X1: u32 = 0x0000000B; // divide counts by 1
const PERIODIC: u32 = 0x00020000; // Periodic
const PCINT: isize = 0x0340 / 4; // Performance Counter LVT
const LINT0: isize = 0x0350 / 4; // Local Vector Table 1 LINT0
const LINT1: isize = 0x0360 / 4; // Local Vector Table 2 LINT1
const ERROR: isize = 0x0370 / 4; // Local Vector Table 3 ERROR
const MASKED: u32 = 0x00010000; // Interrupt masked
const TICR: isize = 0x0380 / 4; // Timer Initial Count
const TCCR: isize = 0x0390 / 4; // Timer Current Count
const TDCR: isize = 0x03E0 / 4; // Timer Divide Configuration

// make this an Option<&mut [T]>?
pub const LAPIC: Option<*mut u32> = None;

static IS_FIRST_CPU: AtomicBool = AtomicBool::new(true);

fn cprintf(_: &'static str) {}

unsafe fn lapicw(index: isize, value: u32) {
    if let Some(lapic) = LAPIC {
        ptr::write_volatile(lapic.offset(index), value);
        ptr::write_volatile(lapic.offset(ID), value); // wait for write to finish by forcing a read
    }
}

#[inline(always)]
fn readeflags() -> u32 {
    let eflags: u32;
    unsafe {
        asm!("pushfl; popl $0" : "=r" (eflags) : : : "volatile");
    }
    return eflags;
}

fn lapic_init() {
    if LAPIC.is_none() {
        return;
    }

    unsafe {
        if let Some(lapic) = LAPIC {

            // Enable local APIC; set spurious interrupt vector.
            lapicw(SVR, ENABLE | (traps::T_IRQ0 + traps::IRQ_SPURIOUS) as u32);

            // The timer repeatedly counts down at bus frequency
            // from lapic[TICR] and then issues an interrupt.
            // If xv6 cared more about precise timekeeping,
            // TICR would be calibrated using an external time source.
            lapicw(TDCR, X1);
            lapicw(TIMER, PERIODIC | (traps::T_IRQ0 + traps::IRQ_TIMER) as u32);
            lapicw(TICR, 10000000);

            // Disable logical interrupt lines.
            lapicw(LINT0, MASKED);
            lapicw(LINT1, MASKED);

            // Disable performance counter overflow interrupts
            // on machines that provide that interrupt entry.
            if ((ptr::read_volatile(lapic.offset(VER)) >> 16) & 0xFF) >= 4 {
                lapicw(PCINT, MASKED);
            }

            // Map error interrupt to IRQ_ERROR.
            lapicw(ERROR, (traps::T_IRQ0 + traps::IRQ_ERROR) as u32);

            // Clear error status register (requires back-to-back writes).
            lapicw(ESR, 0);
            lapicw(ESR, 0);

            // Ack any outstanding interrupts.
            lapicw(EOI, 0);

            // Send an Init Level De-Assert to synchronise arbitration ID's.
            lapicw(ICRHI, 0);
            lapicw(ICRLO, BCAST | INIT | LEVEL);
            while ptr::read_volatile(lapic.offset(ICRLO)) & DELIVS != 0 {}

            // Enable interrupts on the APIC (but not on the processor).
            lapicw(TPR, 0);
        }
    }
}

pub fn cpunum() -> i32 {
    unsafe {

        if readeflags() & flags::IF != 0 {
            // This is probably overkill
            if IS_FIRST_CPU.compare_and_swap(true, false, Ordering::SeqCst) {
                cprintf("cpu called from {} with interrupts enabled\n");
            }
        }

        if let Some(lapic) = LAPIC {

            let apicid = ptr::read_volatile(lapic.offset(ID)) >> 24;

            for i in 0..mp::CPUS.len() {
                if let Some(c) = mp::CPUS[i] {
                    if c == apicid {
                        return i as i32;
                    }
                }
            }
        }
        panic!("Unknown ACPID!");
    }
}
