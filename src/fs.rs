use ide;
use slice_cast;
use core::num::Wrapping;

pub const NDIRECT: usize = 64;
pub const NINDIRECT: usize = 0;
// max file size, based on the number of blocks addressible (direct and indirect)
pub const MAXFILE: usize = NDIRECT + INDIRECT_PER_BLOCK * NINDIRECT;

pub const BLOCKSIZE: usize = 512;

pub const ROOT_INUM: u32 = 0;

pub const BLOCKADDR_SIZE: usize = 4; // block address size in bytes.  32-bit

pub const INDIRECT_PER_BLOCK: usize = BLOCKSIZE / BLOCKADDR_SIZE;

// Rust doesn't support compile-time sizeof, so we manually compute this :(
pub const INODE_SIZE: usize = 62;
pub const NUM_INODES: u32 = 10000;

pub const DIRNAME_SIZE: usize = 254;

pub const SUPERBLOCK_ADDR: u32 = 0;
pub const UNUSED_BLOCKADDR: u32 = 0;

macro_rules! INODE_SIZE {
    {} => {::core::mem::size_of::<Inode>()}
}

macro_rules! INODES_PER_BLOCK {
    {} => {BLOCKSIZE / INODE_SIZE!()}
}

// given an inumber, which block does the inode live in?
macro_rules! IBLOCK {
    {$i: expr, $sb: expr} => { $sb.inode_start + $i /((BLOCKSIZE / INODE_SIZE!()) as u32) }
}


pub struct FileSystem {
    disk: ide::Disk,
}

impl FileSystem {
    fn alloc_inode(&mut self, device: u32, inode: Inode) -> Result<u32, ()> {

        assert!(inode.type_ != InodeType::Unused);

        // Read superblock to get the ilist start
        let mut sb_buf = [0; 512];
        self.disk.read(&mut sb_buf, device, SUPERBLOCK_ADDR)?;
        let sb = buffer_to_sb(&mut sb_buf);

        let inodes_per_block: u32 = (BLOCKSIZE / INODE_SIZE!()) as u32;
        let mut block: [u8; ide::SECTOR_SIZE] = [0; ide::SECTOR_SIZE];

        for blockno in sb.inode_start..(NUM_INODES / inodes_per_block) + 1 {
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
                self.disk.write(&block, device, blockno as u32)?;
                return Ok(i as u32);
            }

        }

        Err(())
    }

    fn read_inode(&mut self, device: u32, inum: u32) -> Result<Inode, ()> {

        assert!(inum <= NUM_INODES);
        // read superblock to get list start
        let mut sb_buf = [0; 512];
        self.disk.read(&mut sb_buf, device, SUPERBLOCK_ADDR)?;
        let superblock = buffer_to_sb(&mut sb_buf);

        // read block containing the inode
        let mut buf = [0; 512];
        self.disk.read(&mut buf, device, IBLOCK!(inum, superblock) as u32)?;

        {
            let inodes: &mut [Inode] = unsafe { slice_cast::cast_mut(&mut buf) };
            let offset = (inum as usize) % INODES_PER_BLOCK!();
            Ok(inodes[offset])
        }
    }

    fn update_inode(&mut self, inum: u32, inode: &Inode) -> Result<(), ()> {
        // read superblock to get list start
        let mut sb_buf = [0; 512];
        self.disk.read(&mut sb_buf, inode.device, SUPERBLOCK_ADDR)?;
        let superblock = buffer_to_sb(&mut sb_buf);

        // read block containing the inode
        let mut buf = [0; 512];
        self.disk.read(&mut buf, inode.device, IBLOCK!(inum, superblock) as u32)?;

        {
            let inodes: &mut [Inode] = unsafe { slice_cast::cast_mut(&mut buf) };
            let offset = (inum as usize) % INODES_PER_BLOCK!();
            inodes[offset] = *inode;
        }

        // write back ilist block with the updated inode
        self.disk.write(&buf, inode.device, IBLOCK!(inum, superblock) as u32)?;

        Ok(())
    }

    fn alloc_block(&mut self, device: u32) -> Result<u32, ()> {
        let mut block: [u8; 512] = [0; 512];

        // read superblock to get freelist head
        self.disk.read(&mut block, device, SUPERBLOCK_ADDR)?;

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

    fn free_inode(&mut self, _device: u32, _inumber: u32) -> Result<(), ()> {
        unimplemented!();
    }

    fn free_block(&mut self, _device: u32, _blockno: u32) -> Result<(), ()> {
        unimplemented!();
    }

    fn dir_add(&mut self, _inode: &mut Inode, _name: &[u8], _target_inum: u32) -> Result<(), ()> {
        unimplemented!();
    }

    fn dir_lookup(&mut self,
                  _inode: &mut Inode,
                  _name: &[u8],
                  _target_inum: u32)
                  -> Result<(), ()> {
        assert!(_inode.type_ == InodeType::Directory);
        unimplemented!();
    }

    fn bmap(&mut self, inode: &mut Inode, blockno: u32) -> Result<u32, ()> {
        let addr = inode.blocks[blockno as usize];
        if addr == UNUSED_BLOCKADDR {
            inode.blocks[blockno as usize] = self.alloc_block(inode.device)?;
        }
        Ok(addr)
    }

    fn read(&mut self, inode: &mut Inode, dst_buf: &mut [u8], offset: u32) -> Result<usize, ()> {
        match inode.type_ {
            InodeType::File | InodeType::Directory => {
                let mut len = dst_buf.len() as u32;
                // Don't allow reading past end of file, or reading large amount that would cause
                // an overflow
                if offset > inode.size || (Wrapping(offset) + Wrapping(len)).0 < offset {
                    return Err(());
                }

                // only read up to the end of the file
                if offset + len > inode.size {
                    len = inode.size - offset;
                }

                // for 0th block, copy from (offset % BLOCKSIZE, BLOCKSIZE)
                // for intermediate blocks, we can copy BLOCKSIZE at a time
                // for last block, copy from block from (0, dst_buf.len() - cursor).

                // for the first block, we copy only from after the offset
                let blockaddr = self.bmap(inode, offset / (BLOCKSIZE as u32))?;
                let mut tmp_buf = [0; BLOCKSIZE];
                self.disk.read(&mut tmp_buf, inode.device, blockaddr)?;
                for (buf, tmp) in dst_buf.iter_mut()
                    .zip(tmp_buf[(offset as usize) % BLOCKSIZE..].iter()) {
                    *buf = *tmp;
                }

                // now, copy a block at a time, truncating the last block as necessary
                for mut chunk in dst_buf[(offset as usize) % BLOCKSIZE..len as usize]
                    .chunks_mut(BLOCKSIZE) {
                    let blockaddr = self.bmap(inode, offset / (BLOCKSIZE as u32))?;
                    self.disk.read(&mut chunk, inode.device, blockaddr)?;
                }
                Ok(len as usize)
            }
            _ => Err(()),
        }
    }

    fn write(&mut self, inode: &mut Inode, src_buf: &[u8], mut offset: u32) -> Result<usize, ()> {
        match inode.type_ {
            InodeType::File | InodeType::Directory => {
                let len = src_buf.len() as u32;

                // Don't allow writing large amount that would cause an overflow
                if (Wrapping(offset) + Wrapping(len)).0 < offset || offset > inode.size {
                    return Err(());
                }

                // if we're trying to write a file that's too large, abort
                if len + offset > (MAXFILE * BLOCKSIZE) as u32 {
                    return Err(());
                }

                // for the first block, take care to correctly overlap offset with the rest of the
                // block
                let blockaddr = self.bmap(inode, offset / (BLOCKSIZE as u32))?;

                if offset as usize % BLOCKSIZE != 0 {
                    let mut tmp_buf = [0; BLOCKSIZE];
                    self.disk.read(&mut tmp_buf, inode.device, blockaddr)?;

                    for (tmp, src) in tmp_buf[(offset as usize) % BLOCKSIZE..BLOCKSIZE]
                        .iter_mut()
                        .zip(src_buf.iter()) {
                        *tmp = *src;
                    }

                    offset += self.disk.write(&tmp_buf, inode.device, blockaddr)? as u32;
                } else {
                    // else business as usual, write the first block
                    offset += self.disk.write(&src_buf[0..BLOCKSIZE], inode.device, blockaddr)? as
                              u32;
                }

                // should figure out how to offset correctly
                // now, copy a block at a time, truncating the last block as necessary
                for chunk in src_buf[BLOCKSIZE - (offset as usize) % BLOCKSIZE..]
                    .chunks(BLOCKSIZE) {
                    let blockaddr = self.bmap(inode, offset / (BLOCKSIZE as u32))?;
                    offset += self.disk.write(chunk, inode.device, blockaddr)? as u32;
                }

                // update file size if we extended past the end of the file
                if offset > inode.size {
                    inode.size = offset;
                }

                Ok(len as usize)
            }
            _ => Err(()),
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
    device: u32,
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
    device: 0,
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
