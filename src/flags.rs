pub const CF: u32 = 0x00000001; // Carry Flag
pub const PF: u32 = 0x00000004; // Parity Flag
pub const AF: u32 = 0x00000010; // Auxiliary carry Flag
pub const ZF: u32 = 0x00000040; // Zero Flag
pub const SF: u32 = 0x00000080; // Sign Flag
pub const TF: u32 = 0x00000100; // Trap Flag
pub const IF: u32 = 0x00000200; // Interrupt Enable
pub const DF: u32 = 0x00000400; // Direction Flag
pub const OF: u32 = 0x00000800; // Overflow Flag
pub const IOPL_MASK: u32 = 0x00003000; // I/O Privilege Level bitmask
pub const IOPL_0: u32 = 0x00000000; //   IOPL == 0
pub const IOPL_1: u32 = 0x00001000; //   IOPL == 1
pub const IOPL_2: u32 = 0x00002000; //   IOPL == 2
pub const IOPL_3: u32 = 0x00003000; //   IOPL == 3
pub const NT: u32 = 0x00004000; // Nested Task
pub const RF: u32 = 0x00010000; // Resume Flag
pub const VM: u32 = 0x00020000; // Virtual 8086 mode
pub const AC: u32 = 0x00040000; // Alignment Check
pub const VIF: u32 = 0x00080000; // Virtual Interrupt Flag
pub const VIP: u32 = 0x00100000; // Virtual Interrupt Pending
pub const ID: u32 = 0x00200000; // ID flag
