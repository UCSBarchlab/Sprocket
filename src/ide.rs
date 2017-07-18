use x86::shared::io;
use fs;
use slice_cast;
use core::cell::Cell;

pub struct Ide {
    busy: Cell<bool>,
}

pub const SECTOR_SIZE: usize = 512;
pub const IDE_BSY: u8 = 0x80;
pub const IDE_DRDY: u8 = 0x40;
pub const IDE_DF: u8 = 0x20;
pub const IDE_ERR: u8 = 0x01;
pub const IDE_CMD_READ: u8 = 0x20;
pub const IDE_CMD_WRITE: u8 = 0x30;
pub const IDE_CMD_RDMUL: usize = 0xc4;
pub const IDE_CMD_WRMUL: usize = 0xc5;

impl fs::Disk for Ide {
    fn write(&mut self, buf: &[u8], dev: u32, sector: u32) -> Result<usize, fs::DiskError> {
        Ide::write(self, buf, dev, sector)
    }
    fn read(&self, mut buf: &mut [u8], dev: u32, sector: u32) -> Result<(), fs::DiskError> {
        Ide::read(self, &mut buf, dev, sector)
    }

    fn sector_size() -> usize {
        SECTOR_SIZE
    }
}

impl Ide {
    pub fn init() -> Ide {
        Ide { busy: Cell::new(false) }
    }

    // we pass a buffer that's larger than 512:
    // truncate after 512?  can't really do anything else
    // shorter: read the last block, overwrite the first N bytes, writeback

    pub fn write(&mut self,
                 buffer: &[u8],
                 device: u32,
                 sector: u32)
                 -> Result<usize, fs::DiskError> {
        //until!(!self.busy, process::Channel::UseDisk); // sleep until it's no longer busy
        self.busy.set(true); // "lock" it
        // should probably create some kind of sleep lock instead, and populate it with this
        // possibly playing with fire due to aliasing rules, modifying mutable state that we "own"
        let _ = self.wait();
        unsafe {
            Self::ide_cmd(device, sector);
            io::outb(0x1f7, IDE_CMD_WRITE);
        }

        // write an entire sector
        if buffer.len() >= SECTOR_SIZE {
            unsafe {
                let as_u32: &[u32] = slice_cast::cast(&buffer[0..SECTOR_SIZE]);
                io::outsl(0x1f0, as_u32);
            }
        } else {
            // or write the first N bytes of the sector and keep the latter half of the sector
            // untouched
            let mut tmp_buf = [0; SECTOR_SIZE];
            self.read(&mut tmp_buf, device, sector)?;
            for (tmp, src) in tmp_buf.iter_mut().zip(buffer.iter()) {
                *tmp = *src;
            }
            assert_eq!(tmp_buf.len(), SECTOR_SIZE);
            let as_u32: &[u32] = unsafe { slice_cast::cast(&tmp_buf[0..SECTOR_SIZE]) };
            unsafe {
                io::outsl(0x1f0, as_u32);
            }
        }
        self.wait()?;

        // notify caller how much we wrote (should just be the buffer size if <= SECTOR_SIZE)
        let n = ::core::cmp::min(SECTOR_SIZE, buffer.len());

        Ok(n)
    }

    // we pass a buffer that's larger than 512
    // just read the entire block into the 1st 512 bytes, can't do anything else
    // shorter: read the block and only read the first N bytes.  Doesn't really make sense to do

    // TODO: figure out a better way to indicate success/error?
    // i.e. Result<&mut [u8; SECTOR_SIZE], ()>
    pub fn read(&self, buffer: &mut [u8], device: u32, sector: u32) -> Result<(), fs::DiskError> {
        //until!(!self.busy, process::Channel::UseDisk); // sleep until it's no longer busy
        self.busy.set(true); // "lock" it
        // should probably create some kind of sleep lock instead, and populate it with this
        // possibly playing with fire due to aliasing rules, modifying mutable state that we "own"
        self.wait()?;
        // unsafe because port I/O
        unsafe {
            Self::ide_cmd(device, sector);
            io::outb(0x1f7, IDE_CMD_READ);
        }
        // could attempt to context switch here, switch back after interrupt
        // or we could have a seperate function that the interrupt handler goes into
        // and that reads the data
        // advantage of ctx switch: cleaner code.  IDE controller may not like waiting though,

        // unsafe because of global state
        //unsafe {
        //process::SCHEDULER.as_mut().unwrap().sleep(process::Channel::ReadDisk);
        //}

        self.wait()?;
        // if the buffer is large enough for an entire block
        if buffer.len() >= SECTOR_SIZE {
            // unsafe because of port I/O
            let mut as_u32: &mut [u32] =
                unsafe { slice_cast::cast_mut(&mut buffer[0..SECTOR_SIZE]) };
            unsafe { io::insl(0x1f0, &mut as_u32) };
        } else {
            // else read the entire block and truncate to the dest buffer length
            let mut tmp_buf = [0; SECTOR_SIZE];
            // unsafe because of port I/O
            {
                let mut as_u32: &mut [u32] =
                    unsafe { slice_cast::cast_mut(&mut tmp_buf[0..SECTOR_SIZE]) };
                unsafe { io::insl(0x1f0, &mut as_u32) };
            }
            for (buf, tmp) in buffer.iter_mut().zip(tmp_buf.iter()) {
                *buf = *tmp;
            }
        }

        Ok(())
    }

    // boilerplate for making an ide read/write request
    unsafe fn ide_cmd(device: u32, sector: u32) {
        //io::outb(0x3f6, 0); // generate interrupt

        // This only works if the sector size == blocksize == 512, since a different command must
        // be issued for multiple-sector read

        #[cfg_attr(feature = "cargo-clippy", allow(eq_op))]
        io::outb(0x1f2, (fs::BLOCKSIZE / SECTOR_SIZE) as u8);
        assert_eq!(fs::BLOCKSIZE, SECTOR_SIZE);

        io::outb(0x1f3, sector as u8 & 0xff);
        io::outb(0x1f4, (sector >> 8) as u8 & 0xff);
        io::outb(0x1f5, (sector >> 16) as u8 & 0xff);
        io::outb(0x1f6,
                 0xe0 | ((device & 0x1) as u8) << 4 | ((sector >> 24) as u8 & 0x0f));
    }

    // poll the IDE device until it's ready
    fn wait(&self) -> Result<(), fs::DiskError> {
        let mut r: u8;
        while {
            r = unsafe { io::inb(0x1f7) };
            (r & (IDE_BSY | IDE_DRDY)) != IDE_DRDY
        } {}

        if r & (IDE_DF | IDE_ERR) == 0 {
            Ok(())
        } else {
            Err(fs::DiskError::IoError)
        }
    }
}
