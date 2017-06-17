#![no_std]
#![feature(step_by)]
#![allow(dead_code)]

extern crate slice_cast;
use core::num::Wrapping;

pub const NDIRECT: usize = 64;
pub const NINDIRECT: usize = 0;
// max file size, based on the number of blocks addressible (direct and indirect)
pub const MAXFILE: usize = NDIRECT + INDIRECT_PER_BLOCK * NINDIRECT;

pub const BLOCKSIZE: usize = 512;

pub const ROOT_INUM: u32 = 0;
pub const ROOT_DEV: u32 = 1;

pub const BLOCKADDR_SIZE: usize = 4; // block address size in bytes.  32-bit

pub const INDIRECT_PER_BLOCK: usize = BLOCKSIZE / BLOCKADDR_SIZE;

// Rust doesn't support compile-time sizeof, so we manually compute this :(
pub const INODE_SIZE: usize = 62;
pub const NUM_INODES: u32 = 256;

pub const DIRNAME_SIZE: usize = 254;

pub const SUPERBLOCK_ADDR: u32 = 0;
pub const UNUSED_BLOCKADDR: u32 = 0;
pub const UNUSED_INUM: u32 = ::core::u32::MAX;

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


pub trait Disk {
    fn read(&mut self, buffer: &mut [u8], device: u32, sector: u32) -> Result<(), DiskError>;
    fn write(&mut self, buffer: &[u8], device: u32, sector: u32) -> Result<usize, DiskError>;
    fn sector_size() -> usize;
}
pub struct FileSystem<T>
    where T: Disk
{
    pub disk: T,
}

impl<T> FileSystem<T>
    where T: Disk
{
    pub fn new(driver: T) -> FileSystem<T> {
        FileSystem { disk: driver }
    }

    pub fn alloc_inode(&mut self, device: u32, inode: Inode) -> Result<u32, FsError> {

        assert!(inode.type_ != InodeType::Unused);

        // Read superblock to get the ilist start
        let mut sb_buf = [0; 512];
        self.disk.read(&mut sb_buf, device, SUPERBLOCK_ADDR)?;
        let sb = buffer_to_sb(&mut sb_buf);

        let inodes_per_block: u32 = (BLOCKSIZE / INODE_SIZE!()) as u32;
        let mut block: [u8; BLOCKSIZE] = [0; BLOCKSIZE];

        for blockno in sb.inode_start..(NUM_INODES / inodes_per_block) + 1 {
            self.disk.read(&mut block, device, blockno as u32)?;
            // unsafe because we're playing a dangerous game with types and memory

            let inum;
            {
                let inodes: &mut [Inode] = unsafe {
                    slice_cast::cast_mut(&mut block[..INODES_PER_BLOCK!() * INODE_SIZE!()])
                };
                inum = inodes.iter().position(|y| y.type_ == InodeType::Unused);

                if let Some(i) = inum {
                    inodes[i] = inode;
                }
            }

            if let Some(i) = inum {
                self.disk.write(&block, device, blockno as u32)?;
                return Ok((blockno - sb.inode_start) * (INODES_PER_BLOCK!() as u32) + i as u32);
            }

        }

        Err(FsError::ExhaustedInodes)
    }

    pub fn read_inode(&mut self, device: u32, inum: u32) -> Result<Inode, FsError> {

        assert!(inum <= NUM_INODES);
        // read superblock to get list start
        let mut sb_buf = [0; 512];
        self.disk.read(&mut sb_buf, device, SUPERBLOCK_ADDR)?;
        let superblock = buffer_to_sb(&mut sb_buf);

        // read block containing the inode
        let mut buf = [0; 512];
        self.disk.read(&mut buf, device, IBLOCK!(inum, superblock) as u32)?;

        {
            let inodes: &mut [Inode] =
                unsafe { slice_cast::cast_mut(&mut buf[..INODES_PER_BLOCK!() * INODE_SIZE!()]) };
            let offset = (inum as usize) % INODES_PER_BLOCK!();
            Ok(inodes[offset])
        }
    }

    pub fn update_inode(&mut self, inum: u32, inode: &Inode) -> Result<(), FsError> {
        // read superblock to get list start
        let mut sb_buf = [0; 512];
        self.disk.read(&mut sb_buf, inode.device, SUPERBLOCK_ADDR)?;
        let superblock = buffer_to_sb(&mut sb_buf);

        // read block containing the inode
        let mut buf = [0; 512];
        self.disk.read(&mut buf, inode.device, IBLOCK!(inum, superblock) as u32)?;

        {
            let inodes: &mut [Inode] =
                unsafe { slice_cast::cast_mut(&mut buf[..INODES_PER_BLOCK!() * INODE_SIZE!()]) };
            let offset = (inum as usize) % INODES_PER_BLOCK!();
            inodes[offset] = *inode;
        }

        // write back ilist block with the updated inode
        self.disk.write(&buf, inode.device, IBLOCK!(inum, superblock) as u32)?;

        Ok(())
    }

    fn alloc_block(&mut self, device: u32) -> Result<u32, FsError> {
        let mut block: [u8; 512] = [0; 512];

        // read superblock to get freelist head
        self.disk.read(&mut block, device, SUPERBLOCK_ADDR)?;

        let head_addr = buffer_to_sb(&mut block).freelist_start;
        if head_addr == UNUSED_BLOCKADDR {
            return Err(FsError::ExhaustedBlocks); // no more blocks we can allocate!!!
        }

        self.disk.read(&mut block, device, head_addr)?; // read head of list

        let free_idx: Option<_> = {
            let freelist: &mut [u32] = unsafe { slice_cast::cast_mut(&mut block) };
            // free_idx is small by one, since we ignore first item of the list (points to next
            // block)
            freelist.iter().skip(1).rposition(|x| *x != UNUSED_BLOCKADDR)
        };

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

                Ok(new_blockno)
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

    fn free_inode(&mut self, _device: u32, _inumber: u32) -> Result<(), FsError> {
        unimplemented!();
    }

    pub fn free_block(&mut self, device: u32, blockno: u32) -> Result<(), FsError> {
        let mut sb_buf = [0u8; 512];
        let mut buffer = [0u8; 512];

        // read superblock to get freelist head
        self.disk.read(&mut sb_buf, device, SUPERBLOCK_ADDR)?;

        let head_addr = buffer_to_sb(&mut sb_buf).freelist_start;
        if head_addr == UNUSED_BLOCKADDR {
            // update the superblock
            buffer_to_sb(&mut sb_buf).freelist_start = blockno;
            self.disk.write(&sb_buf, device, SUPERBLOCK_ADDR)?;

            // and zero out our new head of the freelist
            {
                let freeblock: &mut [u32] = unsafe { slice_cast::cast_mut(&mut buffer) };
                for i in freeblock.iter_mut() {
                    *i = UNUSED_BLOCKADDR;
                }
            }

            // write back the new link and exit
            self.disk.write(&buffer, device, blockno)?;


            return Ok(());
        }

        // get the head block of the freelist
        self.disk.read(&mut buffer, device, head_addr)?;

        let pos = {
            let freeblock: &mut [u32] = unsafe { slice_cast::cast_mut(&mut buffer) };
            freeblock.iter_mut().skip(1).position(|&mut x| x == UNUSED_BLOCKADDR)
        };

        if let Some(p) = pos {
            // if there's a free space in this block, put ourselves in the free list and write back
            {
                let freeblock: &mut [u32] = unsafe { slice_cast::cast_mut(&mut buffer) };
                freeblock[p + 1] = blockno;
            }
            self.disk.write(&buffer, device, head_addr)?;
        } else {
            // else we *are* the new head of the free list

            // update the superblock to point to us
            buffer_to_sb(&mut sb_buf).freelist_start = blockno;
            self.disk.write(&sb_buf, device, SUPERBLOCK_ADDR)?;

            // zero out our block
            {
                let freeblock: &mut [u32] = unsafe { slice_cast::cast_mut(&mut buffer) };
                for i in freeblock.iter_mut().skip(1) {
                    *i = UNUSED_BLOCKADDR;
                }
                // set our block's next addr to point to the old head
                freeblock[0] = head_addr;
            }

            // write back the new link and exit
            self.disk.write(&buffer, device, blockno)?;
        }

        Ok(())
    }

    pub fn dir_add(&mut self, dir: &mut Inode, name: &[u8], target: u32) -> Result<(), FsError> {
        // Don't add if it's already present
        if self.dir_lookup(dir, name).is_ok() {
            return Err(FsError::EntryExists);
        }

        let dirent_size = ::core::mem::size_of::<DirEntry>();
        let new_dirent = DirEntry {
            inumber: target,
            name: {
                let mut n = [0; DIRNAME_SIZE];
                n[..name.len()].copy_from_slice(name);
                n
            },
        };

        // search the dir for a free slot in the existing dir file
        for offset in (0..dir.size).step_by(dirent_size as u32) {
            let mut tmp_buf = [0; BLOCKSIZE];
            assert_eq!(self.read(dir, &mut tmp_buf[..dirent_size], offset)?,
                       dirent_size);

            let found_empty = {
                let dirent: &DirEntry = unsafe { &slice_cast::cast(&tmp_buf[..dirent_size])[0] };
                dirent.inumber == UNUSED_INUM
            };

            if found_empty {
                {
                    let dirent: &mut DirEntry =
                        unsafe { &mut slice_cast::cast_mut(&mut tmp_buf[..dirent_size])[0] };
                    *dirent = new_dirent;
                }
                self.write(dir, &tmp_buf[..dirent_size], offset)?;
                return Ok(());
            }
        }

        // We didn't find a free slot, so append a new entry
        // Copy the new directory entry into the buffer
        let mut tmp_buf = [0; BLOCKSIZE];
        {
            let buf: &mut DirEntry =
                unsafe { &mut slice_cast::cast_mut(&mut tmp_buf[..dirent_size])[0] };
            *buf = new_dirent;
        }

        // write the new entry
        let offset = dir.size;
        self.write(dir, &tmp_buf[..dirent_size], offset)?;

        Ok(())
    }

    pub fn dir_lookup(&mut self, dir: &Inode, name: &[u8]) -> Result<(u32, usize), FsError> {
        assert!(dir.type_ == InodeType::Directory, "{:?}", dir.type_);
        let dirent_size = ::core::mem::size_of::<DirEntry>();
        for offset in (0..dir.size).step_by(dirent_size as u32) {
            let mut buf = [0; BLOCKSIZE];
            self.read(dir, &mut buf[..dirent_size], offset)?;
            // read this as a directory entry
            let entry: &DirEntry = unsafe { &slice_cast::cast(&buf[..dirent_size])[0] };
            if entry.inumber == UNUSED_INUM {
                continue;
            }

            // Take the directory entry's name up to the first NUL (or all DIRNAME_SIZE bytes_)
            // Also, take desired name up to the first NUL. Pad iterator it if it's shorter than
            // dirname, so that equality will fail
            // And check if it's equal to the kyy

            // get entry name up to the first zero char, or all DIRNAME_SIZE bytes
            let entry_len = entry.name.iter().position(|&x| x == 0).unwrap_or(DIRNAME_SIZE);

            // must be that the query name is the same len as the entry's name
            // and their contents must be equal
            if entry_len == name.len() &&
               entry.name[..entry_len].iter().zip(name).all(|(x, y)| *x == *y) {
                return Ok((entry.inumber, offset as usize));
            }
        }

        // not found
        Err(FsError::NotFound)
    }

    /// Maps sequential block of file into a disk block address, or allocates one if the block
    /// isn't mapped
    fn bmap_or_alloc(&mut self, inode: &mut Inode, blockno: u32) -> Result<u32, FsError> {
        let addr = inode.blocks[blockno as usize];
        if addr == UNUSED_BLOCKADDR {
            inode.blocks[blockno as usize] = self.alloc_block(inode.device)?;
        }
        Ok(inode.blocks[blockno as usize])
    }

    /// Maps sequential block of file into a disk block address if mapped
    fn bmap(&mut self, inode: &Inode, blockno: u32) -> Result<u32, FsError> {
        let addr = inode.blocks[blockno as usize];
        if addr == UNUSED_BLOCKADDR {
            Err(FsError::BlockNotMapped(blockno))
        } else {
            Ok(addr)
        }
    }

    pub fn read(&mut self,
                inode: &Inode,
                dst_buf: &mut [u8],
                mut offset: u32)
                -> Result<usize, FsError> {
        match inode.type_ {
            InodeType::File | InodeType::Directory => {
                let mut len = dst_buf.len() as u32;
                // Don't allow reading past end of file, or reading large amount that would cause
                // an overflow
                if offset > inode.size || (Wrapping(offset) + Wrapping(len)).0 < offset {
                    return Err(FsError::ReadTooLarge);
                }

                // only read up to the end of the file
                if offset + len > inode.size {
                    len = inode.size - offset;
                }

                let short_buf = &mut dst_buf[..len as usize];

                // for 0th block, copy from (offset % BLOCKSIZE, BLOCKSIZE)
                // for intermediate blocks, we can copy BLOCKSIZE at a time
                // for last block, copy from block from (0, dst_buf.len() - cursor).

                // for the first block, we copy only from after the offset
                let blockaddr = self.bmap(inode, offset / (BLOCKSIZE as u32))?;
                let mut tmp_buf = [0; BLOCKSIZE];
                self.disk.read(&mut tmp_buf, inode.device, blockaddr)?;
                for (buf, tmp) in short_buf.iter_mut()
                    .zip(tmp_buf[(offset as usize) % BLOCKSIZE..].iter()) {
                    *buf = *tmp;
                }
                offset += tmp_buf[(offset as usize) % BLOCKSIZE..].len() as u32;
                if offset >= short_buf.len() as u32 {
                    return Ok(short_buf.len());
                }

                // no more reading to do if the offset is greater than the len

                // now, copy a block at a time, truncating the last block as necessary
                // this is fucked for so many reasons, fix later
                for mut chunk in short_buf[BLOCKSIZE + (offset as usize) % BLOCKSIZE..]
                    .chunks_mut(BLOCKSIZE) {
                    let blockaddr = self.bmap(inode, offset / (BLOCKSIZE as u32))?;
                    self.disk.read(&mut chunk, inode.device, blockaddr)?;
                    offset += chunk.len() as u32;
                }
                Ok(short_buf.len())
            }
            _ => Err(FsError::TypeMismatch),
        }
    }

    pub fn write(&mut self,
                 inode: &mut Inode,
                 src_buf: &[u8],
                 mut offset: u32)
                 -> Result<usize, FsError> {
        match inode.type_ {
            InodeType::File | InodeType::Directory => {
                let len = src_buf.len() as u32;

                // Don't allow writing large amount that would cause an overflow
                if (Wrapping(offset) + Wrapping(len)).0 < offset || offset > inode.size {
                    return Err(FsError::WriteTooLarge);
                }

                // if we're trying to write a file that's too large, abort
                if len + offset > (MAXFILE * BLOCKSIZE) as u32 {
                    return Err(FsError::WriteTooLarge);
                }

                // for the first block, take care to correctly overlap offset with the rest of the
                // block
                let blockaddr = self.bmap_or_alloc(inode, offset / (BLOCKSIZE as u32))?;

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
                    // note: offset doesn't mean don't pad the end, just means we don't need to do
                    // weird prepend logic
                    offset += self.disk
                        .write(&src_buf[..::core::cmp::min(BLOCKSIZE, src_buf.len())],
                               inode.device,
                               blockaddr)? as u32;
                }

                if offset >= src_buf.len() as u32 {
                    if offset > inode.size {
                        inode.size += src_buf.len() as u32;
                    }
                    return Ok(src_buf.len());
                }

                // should figure out how to offset correctly
                // now, copy a block at a time, truncating the last block as necessary
                for chunk in src_buf[(offset as usize)..].chunks(BLOCKSIZE) {
                    let blockaddr = self.bmap_or_alloc(inode, offset / (BLOCKSIZE as u32))?;
                    offset += self.disk.write(chunk, inode.device, blockaddr)? as u32;
                }

                // update file size if we extended past the end of the file
                if offset > inode.size {
                    inode.size = offset;
                }

                Ok(len as usize)
            }
            _ => Err(FsError::TypeMismatch),
        }
    }

    // return inum of the target file
    pub fn namex(&mut self, path: &[u8], name: &[u8]) -> Result<u32, FsError> {
        let inode: Inode = if path == b"/" {
            self.read_inode(ROOT_DEV, ROOT_INUM)?
        } else {
            return Err(FsError::NotFound); // if/when we decide to support subdirectories, inode is current working directory
        };

        let (inum, _) = self.dir_lookup(&inode, name)?;
        Ok(inum)
    }
}

// prevent weird aliasing violations by forcing borrow and lifetime with a function
// versus doing this in the same scope
fn buffer_to_sb(buffer: &mut [u8; 512]) -> &mut SuperBlock {
    unsafe { &mut *(buffer.as_mut_ptr() as *mut SuperBlock) }
}

#[repr(C)]
#[derive(PartialEq, Copy, Clone)]
pub struct SuperBlock {
    pub size: u32,
    pub nblocks: u32,
    pub ninodes: u32,
    pub inode_start: u32,
    pub freelist_start: u32,
}

#[repr(u16)]
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum InodeType {
    Unused,
    File,
    Directory,
}

#[derive(Debug, PartialEq)]
pub enum FsError {
    ExhaustedBlocks,
    ExhaustedInodes,
    NotDir,
    EOF,
    EntryExists,
    NotFound,
    ReadTooLarge,
    WriteTooLarge,
    BlockNotMapped(u32),
    TypeMismatch, // don't invoke the file system on a device file!
    DiskFault(DiskError),
}

impl core::convert::From<DiskError> for FsError {
    fn from(e: DiskError) -> Self {
        FsError::DiskFault(e)
    }
}

#[derive(Debug, PartialEq)]
pub enum DiskError {
    IoError,
}

#[repr(C)]
#[derive(Copy)]
pub struct Inode {
    pub type_: InodeType,
    pub device: u32,
    pub major: u16,
    pub minor: u16,
    pub size: u32,
    pub blocks: [u32; NDIRECT],
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
pub struct DirEntry {
    inumber: u32,
    name: [u8; DIRNAME_SIZE],
}
