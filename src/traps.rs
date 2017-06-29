use x86::bits32::irq::IdtEntry;
use x86::shared::paging::VAddr;
use x86::shared::dtables::{lidt, DescriptorTablePointer};
use x86::shared::PrivilegeLevel;
use x86::shared::control_regs;
use vm::Segment;
use process;
use timer;
use rtl8139;
use core::sync::atomic;

// x86 trap and interrupt constants.

// Processor-defined:
pub const T_IRQ0: u8 = 32; // IRQ 0 corresponds to int T_IRQ
pub const TIMER_IRQ: u8 = 0; // IRQ 0 corresponds to int T_IRQ
pub const COM1_IRQ: u8 = 4; // IRQ 0 corresponds to int T_IRQ
pub const NIC_IRQ: u8 = 0xb; // IRQ 0 corresponds to int T_IRQ

pub const FLAG_INT_ENABLED: u32 = 0x200;

pub static mut INT_ENABLED: atomic::AtomicBool = atomic::AtomicBool::new(false);

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum Interrupt {
    DivError = 0, // divide error
    DebugException = 1, // debug exception
    NonMaskableInt = 2, // non-maskable interrupt
    Breakpoint = 3, // breakpoint
    Overflow = 4, // overflow
    BoundsCheck = 5, // bounds check
    IllegalOp = 6, // illegal opcode
    DeviceNotAvailable = 7, // device not available
    DoubleFault = 8, // double fault
    Coproc = 9, // reserved (not used since 486)
    InvalidTss = 10, // invalid task switch segment
    SegmentNotPresent = 11, // segment not present
    StackException = 12, // stack exception
    GenProtectFault = 13, // general protection fault
    PageFault = 14, // page fault
    Reserved = 15, // reserved
    FloatingPointErr = 16, // floating point error
    AlignmentCheck = 17, // aligment check
    MachineCheck = 18, // machine check
    SimdErr = 19, // SIMD floating point error

    Syscall = 64, // system call

    TimerInt = T_IRQ0 + TIMER_IRQ,
    KeyboardInt = T_IRQ0 + 1,
    Com1Int = T_IRQ0 + COM1_IRQ,
    NetworkInt = T_IRQ0 + NIC_IRQ,
    IdeInt = T_IRQ0 + 14,
    ErrorInt = T_IRQ0 + 19,
    SpuriousInt = T_IRQ0 + 31,
}

impl Default for Interrupt {
    fn default() -> Interrupt {
        Interrupt::SpuriousInt
    }
}

impl ::core::fmt::Display for Interrupt {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

extern "C" {
    static vectors: [u32; 256];
}

use spinlock::Mutex;
pub static IDT: Mutex<[IdtEntry; 256]> = Mutex::new([IdtEntry::MISSING; 256]);

pub fn trap_vector_init() {
    for (interrupt, vec) in IDT.lock().iter_mut().zip(unsafe { vectors.iter() }) {
        *interrupt = IdtEntry::new(VAddr::from_usize(*vec as usize),
                                   (Segment::KCode as u16) << 3,
                                   PrivilegeLevel::Ring0,
                                   true);
    }

    IDT.lock()[Interrupt::Syscall as usize] =
        IdtEntry::new(VAddr::from_usize(unsafe { vectors[Interrupt::Syscall as usize] } as usize),
                      (Segment::KCode as u16) << 3,
                      PrivilegeLevel::Ring3,
                      true);
}

pub fn idtinit() {
    unsafe {
        // unsafe because we're calling asm and accessing global mutable state
        lidt(&DescriptorTablePointer::new_idtp(&IDT.lock()[..]))
    }
}

#[no_mangle]
pub extern "C" fn trap(tf: &process::TrapFrame) {

    match tf.trapno {
        Interrupt::Com1Int => {
            // print keyboard input for debugging
            use console;
            let ch = {
                console::CONSOLE.lock().read_byte()
            };
            if let Some(c) = ch {
                print!("{}", c as char);
            }
        }
        Interrupt::NetworkInt => {
            debug!("Network interrupt");
            if let Some(ref mut n) = rtl8139::NIC.lock().as_mut() {
                n.interrupt();
            }
        }
        Interrupt::TimerInt => unsafe {
            timer::TICKS += 1;
        },
        Interrupt::PageFault => {

            panic!("Page fault occured at {:#08x}",
                   unsafe { control_regs::cr2() });
        }
        t => debug!("Recieved {} ({:#x})", t, t as u8),
    }
}
