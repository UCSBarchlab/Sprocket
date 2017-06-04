extern crate simple_fs as fs;
extern crate slice_cast;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::env;

// size in blocks
const FS_SIZE: u32 = 1000;

fn main() {
    let path = env::args().nth(1).expect("You must pass a path!");

    let mut fs = fs::FileSystem::new(DiskFile::new(path));
    match mkfs(&mut fs) {
        Ok(_) => println!("System successfully formatted!"),
        Err(_) => panic!("An error occurred while formatting"),
    }
    // for each specified file, copy it into the new file system
    for arg in env::args().skip(2) {}
}

fn mkfs<T>(fs: &mut fs::FileSystem<T>) -> Result<(), ()>
    where T: fs::Disk
{
    // Write an unused inode for each of the inodes
    for i in 0..fs::NUM_INODES {
        fs.update_inode(i, &fs::UNUSED_INODE)?;
    }

    // create the freelist
    let inode_size = std::mem::size_of::<fs::Inode>();
    let datablocks_start = 1 + (fs::NUM_INODES as u32) / ((fs::BLOCKSIZE / inode_size) as u32);

    let mut buf = [0u8; fs::BLOCKSIZE];
    {

        let sb: &mut fs::SuperBlock = unsafe { &mut slice_cast::cast_mut(&mut buf)[0] };
        *sb = fs::SuperBlock {
            size: 0,
            nblocks: FS_SIZE,
            ninodes: fs::NUM_INODES,
            inode_start: 1,
            freelist_start: 0,
        };
    }
    // write the new superblock
    fs.disk.write(&buf, 0, fs::SUPERBLOCK_ADDR)?;

    // add every data block to the freelist
    for blockno in datablocks_start..FS_SIZE {
        fs.free_block(0, blockno)?;
    }

    Ok(())
}

struct DiskFile {
    file: File,
}

impl DiskFile {
    fn new(path: std::string::String) -> DiskFile {
        DiskFile { file: File::create(path).expect("Could not create file") }
    }
}

impl fs::Disk for DiskFile {
    fn read(&mut self, mut buffer: &mut [u8], _: u32, sector: u32) -> Result<(), ()> {
        // seek to the sector
        assert!(buffer.len() <= 512);
        let _ = self.file.seek(SeekFrom::Start((sector as u64) * (Self::sector_size()) as u64));
        let _ = self.file.read_exact(&mut buffer);
        Ok(())
    }

    fn write(&mut self, buffer: &[u8], _: u32, sector: u32) -> Result<usize, ()> {
        assert!(buffer.len() <= 512);
        let _ = self.file.seek(SeekFrom::Start((sector as u64) * (Self::sector_size()) as u64));
        let written = self.file.write_all(buffer);
        if written.is_ok() {
            Ok(buffer.len())
        } else {
            Err(())
        }
    }

    fn sector_size() -> usize {
        512
    }
}
