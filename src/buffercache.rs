use fs;

struct BufferCache {}

#[derive(Copy)]
pub struct Buffer {
    pub flags: Flags,
    device: u32,
    blockno: u32,
    refcount: u32,
    data: [u8; fs::BLOCKSIZE],
}

impl Clone for Buffer {
    fn clone(&self) -> Buffer {
        *self
    }
}

bitflags! {
    pub flags Flags: u32 {
        const VALID = 1 << 1,
        const DIRTY = 1 << 2,
    }
}
