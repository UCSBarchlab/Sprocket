extern crate simple_fs as fs;
extern crate slice_cast;
use std::fs::{File, OpenOptions};
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
    for arg in env::args().skip(2) {
        write_file(&mut fs, &arg);
        println!("Wrote {}", arg);
    }
}

fn write_file<T>(fs: &mut fs::FileSystem<T>, path: &String) -> Result<(), ()>
    where T: fs::Disk
{
    let mut f = File::open(path).expect("Could not open file");
    let mut inode = fs::Inode {
        type_: fs::InodeType::File,
        device: fs::ROOT_DEV,
        major: 0,
        minor: 0,
        size: 0,
        blocks: [0; fs::NDIRECT],
    };
    let inum = fs.alloc_inode(fs::ROOT_DEV, inode)?;
    assert!(inum != fs::ROOT_INUM);
    let mut buf = vec![];
    f.read_to_end(&mut buf);
    fs.write(&mut inode, &buf, 0)?;
    fs.update_inode(inum, &inode);

    let new_inode = fs.read_inode(fs::ROOT_DEV, inum)?;
    let mut buf2 = Vec::with_capacity(new_inode.size as usize);
    fs.read(&new_inode, buf2.as_mut_slice(), 0);
    assert_eq!(buf.len(), new_inode.size as usize);
    for (i, b) in buf.iter().enumerate() {
        let mut b2 = [0u8; 1];
        fs.read(&new_inode, &mut b2[0..1], i as u32)?;
        assert_eq!(*b, b2[0]);
    }
    println!("File writeback was successful!");

    let mut root = fs.read_inode(fs::ROOT_DEV, fs::ROOT_INUM)?;
    println!("{}", inum);
    let name = path.bytes().take(fs::DIRNAME_SIZE).collect::<Vec<_>>();
    fs.dir_add(&mut root, name.as_slice(), inum);

    Ok(())
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

    let tmp_sb = fs::SuperBlock {
        size: 0,
        nblocks: FS_SIZE,
        ninodes: fs::NUM_INODES,
        inode_start: 1,
        freelist_start: 0,
    };
    let mut buf = [0u8; fs::BLOCKSIZE];
    {
        let sb: &mut fs::SuperBlock =
            unsafe {
                &mut slice_cast::cast_mut(&mut buf[..std::mem::size_of::<fs::SuperBlock>()])[0]
            };

        *sb = tmp_sb;
    }
    // write the new superblock
    fs.disk.write(&buf, 0, fs::SUPERBLOCK_ADDR)?;
    let sb2: fs::SuperBlock =
        unsafe { *&mut slice_cast::cast_mut(&mut buf[..std::mem::size_of::<fs::SuperBlock>()])[0] };
    fs.disk.read(&mut buf, 0, fs::SUPERBLOCK_ADDR)?;
    assert!(sb2 == tmp_sb);

    // add every data block to the freelist
    for blockno in datablocks_start..FS_SIZE {
        fs.free_block(0, blockno)?;
    }

    // finally, create root dir
    let mut inode = fs::Inode {
        type_: fs::InodeType::Directory,
        device: fs::ROOT_DEV,
        major: 0,
        minor: 0,
        size: 0,
        blocks: [0; fs::NDIRECT],
    };

    let dirent_size = std::mem::size_of::<fs::DirEntry>();

    assert_eq!(fs.alloc_inode(0, inode)?, fs::ROOT_INUM);
    println!("Adding /");
    fs.dir_add(&mut inode, b".", fs::ROOT_INUM)?;
    assert_eq!(inode.size as usize, dirent_size);
    println!("Added '.'");
    fs.dir_add(&mut inode, b"..", fs::ROOT_INUM)?;
    assert_eq!(inode.size as usize, 2 * dirent_size);
    println!("Added '..'");
    fs.update_inode(fs::ROOT_INUM, &mut inode)?;

    let inum2 = fs.read_inode(fs::ROOT_DEV, fs::ROOT_INUM)?;
    assert!(inode.type_ == inum2.type_);
    assert!(inode.size == inum2.size);
    assert!(inode.blocks[0] == inum2.blocks[0]);

    assert_eq!(fs.dir_lookup(&inode, b"."), Ok((0, 0)));
    assert_eq!(fs.dir_lookup(&inode, b".."),
               Ok((0, std::mem::size_of::<fs::DirEntry>())));

    Ok(())
}

struct DiskFile {
    file: File,
}

impl DiskFile {
    fn new(path: std::string::String) -> DiskFile {
        DiskFile {
            file: OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path)
                .expect("Could not create file"),
        }
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
        assert!(buffer.len() <= 512, "length was {}", buffer.len());
        let _ = self.file.seek(SeekFrom::Start((sector as u64) * (Self::sector_size()) as u64));
        let written = self.file.write_all(buffer);
        if written.is_ok() {
            Ok(buffer.len())
        } else {
            loop {}
            Err(())
        }
    }

    fn sector_size() -> usize {
        512
    }
}
