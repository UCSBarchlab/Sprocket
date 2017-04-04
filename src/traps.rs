// x86 trap and interrupt constants.

// Processor-defined:
pub const T_DIVIDE: u8 = 0; // divide error
pub const T_DEBUG: u8 = 1; // debug exception
pub const T_NMI: u8 = 2; // non-maskable interrupt
pub const T_BRKPT: u8 = 3; // breakpoint
pub const T_OFLOW: u8 = 4; // overflow
pub const T_BOUND: u8 = 5; // bounds check
pub const T_ILLOP: u8 = 6; // illegal opcode
pub const T_DEVICE: u8 = 7; // device not available
pub const T_DBLFLT: u8 = 8; // double fault
// pub const T_COPROC: u8 = 9;      // reserved (not used since 486)
pub const T_TSS: u8 = 10; // invalid task switch segment
pub const T_SEGNP: u8 = 11; // segment not present
pub const T_STACK: u8 = 12; // stack exception
pub const T_GPFLT: u8 = 13; // general protection fault
pub const T_PGFLT: u8 = 14; // page fault
// pub const T_RES: u8 = 15;      // reserved
pub const T_FPERR: u8 = 16; // floating point error
pub const T_ALIGN: u8 = 17; // aligment check
pub const T_MCHK: u8 = 18; // machine check
pub const T_SIMDERR: u8 = 19; // SIMD floating point error

// These are arbitrarily chosen, but with care not to overlap
// processor defined exceptions or interrupt vectors.
pub const T_SYSCALL: u8 = 64; // system call
pub const T_DEFAULT: u8 = 500; // catchall

pub const T_IRQ0: u8 = 32; // IRQ 0 corresponds to int T_IRQ

pub const IRQ_TIMER: u8 = 0;
pub const IRQ_KBD: u8 = 1;
pub const IRQ_COM1: u8 = 4;
pub const IRQ_IDE: u8 = 14;
pub const IRQ_ERROR: u8 = 19;
pub const IRQ_SPURIOUS: u8 = 31;
