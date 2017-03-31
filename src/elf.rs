// Format of an ELF executable file

pub mod elf {
    pub const ELF_MAGIC: u32 = 0x464C457F; // "\x7FELF" in little endian

    // File header
    #[repr(C)]
    pub struct Header {
        pub magic: u32, // must equal ELF_MAGIC
        pub elf: [u8; 12],
        pub _type: u16,
        pub machine: u16,
        pub version: u32,
        pub entry: u32,
        pub phoff: u32,
        pub shoff: u32,
        pub flags: u32,
        pub ehsize: u16,
        pub phentsize: u16,
        pub phnum: u16,
        pub shentsize: u16,
        pub shnum: u16,
        pub shstrndx: u16,
    }

    // Program section header
    #[repr(C)]
    pub struct ProgramHeader {
        pub _type: u32,
        pub off: u32,
        pub vaddr: u32,
        pub paddr: u32,
        pub filesz: u32,
        pub memsz: u32,
        pub flags: u32,
        pub align: u32,
    }

    // Values for Proghdr type
    pub const ELF_PROG_LOAD: u8 = 1;

    // Flag bits for Proghdr flags
    pub const ELF_PROG_FLAG_EXEC: u8 = 1;
    pub const ELF_PROG_FLAG_WRITE: u8 = 2;
    pub const ELF_PROG_FLAG_READ: u8 = 4;
}
