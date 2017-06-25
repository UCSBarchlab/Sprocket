use fs;
use alloc::rc::Rc;

pub trait UnixFileSystem {
    type File: FileHandle;
    fn open(&self, path: &[u8], name: &[u8]) -> Self::File;
}

pub struct SimpleFs<T: fs::Disk> {
    fs: Rc<fs::FileSystem<T>>,
}

impl<T: fs::Disk> SimpleFs<T> {
    pub fn new(disk: T) -> Self {
        SimpleFs { fs: Rc::new(fs::FileSystem::new(disk)) }
    }
}

impl<T: fs::Disk> UnixFileSystem for SimpleFs<T> {
    type File = SimpleFile<T>;

    fn open(&self, path: &[u8], name: &[u8]) -> Self::File {
        let inum = (*self.fs).namex(path, name).unwrap();
        let inode = (*self.fs).read_inode(fs::ROOT_DEV, inum).unwrap();
        SimpleFile {
            inum: inum,
            inode: inode,
            offset: 0,
            fs: self.fs.clone(),
        }
    }
}

pub trait FileHandle {
    fn read(&mut self, buffer: &mut [u8]);
    fn write(&mut self, buffer: &mut [u8]);
    fn seek_absolute(&mut self, offset: usize);
    fn offset(&self) -> usize;
    fn size(&self) -> usize;
}

pub struct SimpleFile<T: fs::Disk> {
    inum: u32,
    inode: fs::Inode,
    offset: usize,
    fs: Rc<fs::FileSystem<T>>,
}

impl<T> SimpleFile<T>
    where T: fs::Disk
{
    fn fs(&self) -> &fs::FileSystem<T> {
        &*self.fs
    }

    fn fs_mut(&mut self) -> &mut fs::FileSystem<T> {
        Rc::get_mut(&mut self.fs).unwrap()
    }
}

// TODO: implement better error handling semantics here
impl<T: fs::Disk> FileHandle for SimpleFile<T> {
    fn read(&mut self, buffer: &mut [u8]) {
        let offset = self.offset as u32;
        let inode = self.inode;
        self.fs().read(&inode, buffer, offset).unwrap();
        self.offset += buffer.len();
    }
    fn write(&mut self, _buffer: &mut [u8]) {
        unimplemented!();
        // TODO: implement write, taking care to make sure that the changed inode is written back
        // to the disk
    }
    fn seek_absolute(&mut self, offset: usize) {
        self.offset = offset;
    }
    fn offset(&self) -> usize {
        self.offset
    }

    fn size(&self) -> usize {
        self.inode.size as usize
    }
}

/*
impl FileHandle for SimpleFile {
    type fs = Rc<UnixFileSystem<File = SimpleFile>>;

    fn read(&mut self, buffer: &mut [u8]) {}
    fn write(&mut self, buffer: &mut [u8]) {}
    fn seek(&mut self, offset: usize) {
        self.offset = offset;
    }
    fn offset(&self) -> usize {
        self.offset
    }
}
*/
