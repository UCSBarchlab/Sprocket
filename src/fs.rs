use ide;
use slice_cast;

pub const NDIRECT: usize = 64;
pub const BLOCKSIZE: usize = 512;

pub const ROOT_INODE: u32 = 0;

pub const BLOCKADDR_SIZE: usize = 4; // block address size in bytes.  32-bit

pub const INDIRECT_PER_BLOCK: usize = BLOCKSIZE / BLOCKADDR_SIZE;

// Rust doesn't support compile-time sizeof, so we manually compute this :(
pub const INODE_SIZE: usize = 62;
pub const NUM_INODES: usize = 10000;

pub const DIRNAME_SIZE: usize = 254;

pub const SUPERBLOCK_ADDR: u32 = 0;
pub const UNUSED_BLOCKADDR: u32 = 0;

macro_rules! INODE_SIZE {
    {} => {::core::mem::size_of::<Inode>()}
}

pub struct FileSystem {
    disk: ide::Disk,
}

impl FileSystem {
    fn alloc_inode(&mut self, device: u32, inode: Inode) -> Result<u32, ()> {

        assert!(inode.type_ != InodeType::Unused);

        let inodes_per_block = BLOCKSIZE / INODE_SIZE!();
        let mut block: [u8; 512] = [0; 512];

        for blockno in 1..(NUM_INODES / inodes_per_block) + 1 {
            self.disk.read(&mut block, device, blockno as u32)?;
            // unsafe because we're playing a dangerous game with types and memory

            let inum;
            {
                let inodes: &mut [Inode] = unsafe { slice_cast::cast_mut(&mut block) };
                inum = inodes.iter().position(|y| y.type_ == InodeType::Unused);

                if let Some(i) = inum {
                    inodes[i] = inode;
                }
            }

            if let Some(i) = inum {
                self.disk.write(&block, device, blockno as u32)?; // TODO: weird pointer aliasing stuff, maybe reconsider
                return Ok(i as u32);
            }

        }

        Err(())
    }

    // TODO: figure out if there's a way to cast slices around that isn't hideously unsafe
    // since this doesn't do any kind of borrowck/lifetime analysis and I can do all sorts of dumb
    // aliasing things
    fn alloc_block(&mut self, device: u32) -> Result<u32, ()> {
        let mut block: [u8; 512] = [0; 512];

        self.disk.read(&mut block, device, SUPERBLOCK_ADDR)?; // read superblock to get freelist head


        // unsafe because of pointer and type shenanigans
        let head_addr: u32 = unsafe { &mut *(block.as_mut_ptr() as *mut SuperBlock) }
            .freelist_start;
        if head_addr == UNUSED_BLOCKADDR {
            return Err(()); // no more blocks we can allocate!!!
        }

        self.disk.read(&mut block, device, head_addr)?; // read head of list
        //let blocks: &mut [u32] = unsafe { &mut *(block.as_mut_ptr() as *mut u32) };

        let free_idx;
        {
            let freelist: &mut [u32] = unsafe { slice_cast::cast_mut(&mut block) };

            // free_idx is small by one, since we ignore first item of the list (which holds the next
            // part of the freelist)
            free_idx = freelist.iter().skip(1).rposition(|x| *x != UNUSED_BLOCKADDR);
        }

        match free_idx {
            // The head of the freelist has a free block in it
            Some(index) => {
                // Take our new free block addr and mark it as used
                let new_blockno;
                {
                    let freelist: &mut [u32] = unsafe { slice_cast::cast_mut(&mut block) };
                    new_blockno = freelist[index + 1];
                    freelist[index + 1] = UNUSED_BLOCKADDR;
                };

                // Write out the freelist element with the removed address
                self.disk.write(&block, device, head_addr)?;
                self.disk.write(&[0; 512], device, new_blockno)?;

                Ok(index as u32)
            }

            // The head of the freelist *is* the new block we're allocating
            None => {
                let freelist: &mut [u32] = unsafe { slice_cast::cast_mut(&mut block) };
                let new_head = freelist[0];

                let mut sb_buf = [0; 512];
                self.disk.read(&mut sb_buf, device, SUPERBLOCK_ADDR)?; // read superblock to get freelist head
                {
                    // update superblock to point to the new head of the freelist
                    let sb: &mut SuperBlock = buffer_to_sb(&mut sb_buf);
                    sb.freelist_start = new_head;
                }

                // zero out the allocated block and write back the updated superblock
                self.disk.write(&[0; 512], device, head_addr)?;
                self.disk.write(&sb_buf, device, SUPERBLOCK_ADDR)?;

                Ok(head_addr)
            }
        }
    }
}

// prevent weird aliasing violations by forcing borrow and lifetime with a function
// versus doing this in the same scope
fn buffer_to_sb(buffer: &mut [u8; 512]) -> &mut SuperBlock {
    unsafe { &mut *(buffer.as_mut_ptr() as *mut SuperBlock) }
}

#[repr(C)]
pub struct SuperBlock {
    size: u32,
    nblocks: u32,
    ninodes: u32,
    inode_start: u32,
    freelist_start: u32,
}

#[repr(u16)]
#[derive(PartialEq, Clone, Copy)]
enum InodeType {
    Unused,
    File,
    Directory,
}

#[repr(C)]
#[derive(Copy)]
pub struct Inode {
    type_: InodeType,
    major: u16,
    minor: u16,
    size: u32,
    blocks: [u32; NDIRECT],
}

// Rust doesn't yet support integer type parameterization, so we manually implement the clone
// trait for Inode, since blocks[64] doesn't already have a clone implementation.
impl Clone for Inode {
    fn clone(&self) -> Inode {
        *self
    }
}

pub const UNUSED_INODE: Inode = Inode {
    type_: InodeType::Unused,
    major: 0,
    minor: 0,
    size: 0,
    blocks: [0; NDIRECT],
};

#[repr(C)]
pub struct Directory {
    inumber: u16,
    name: [u8; DIRNAME_SIZE],
}

pub fn mkfs() {}

// marshalling/demarshalling
// we will have user data that is just [u8; 512]
// we will have blocks of inodes; [Inode; x | x * sizeof(Inode) <= 512 ]
// we will possibly have blocks of other block numbers; [u32; 128]
