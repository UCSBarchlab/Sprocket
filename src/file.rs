use fs;

pub const NOFILE: usize = 16;

pub enum FileType {
    None,
    Pipe,
    Inode,
}
pub struct File {
    filetype: FileType,
    refcount: i32, // reference count
    readable: bool,
    writable: bool,
    pipe: *const Pipe,
    ip: *const Inode,
    off: i32,
}

pub struct Inode {
    dev: u32, // Device number
    inum: u32, // Inode number
    refcount: i32, // Reference count
    lock: SleepLock,
    flags: i32, // I_VALID

    inodetype: i16, // copy of disk inode
    major: i16,
    minor: i16,
    nlink: i16,
    size: u32,
    addrs: [u32; fs::NDIRECT + 1],
}

// TODO: implement this
struct Pipe {}

struct SleepLock {}
