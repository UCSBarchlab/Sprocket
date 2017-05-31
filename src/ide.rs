use process;
use x86::shared::io;
use fs;

pub struct Disk {
    busy: bool,
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

impl Disk {
    fn init() -> Disk {
        Disk { busy: false }
    }

    // we pass a buffer that's larger than 512:
    // truncate after 512?  can't really do anything else
    // shorter: read the last block, overwrite the first N bytes, writeback

    pub fn write(&mut self, buffer: &[u8], device: u32, sector: u32) -> Result<(), ()> {
        until!(!self.busy, process::Channel::UseDisk); // sleep until it's no longer busy
        self.busy = true; // "lock" it
        // should probably create some kind of sleep lock instead, and populate it with this
        // possibly playing with fire due to aliasing rules, modifying mutable state that we "own"
        let _ = self.wait();
        unsafe {
            Self::ide_cmd(device, sector);
            io::outb(0x1f7, IDE_CMD_WRITE);
            io::outsb(0x1f0, buffer); // write buffer
        }

        Ok(())
    }

    // we pass a buffer that's larger than 512:
    // just read the entire block into the 1st 512 bytes, can't do anything else
    // shorter: read the block and only read the first N bytes.  Doesn't really make sense to do

    // TODO: figure out a better way to indicate success/error?
    // i.e. Result<&mut [u8; SECTOR_SIZE], ()>
    pub fn read(&mut self, buffer: &mut [u8], device: u32, sector: u32) -> Result<(), ()> {
        until!(!self.busy, process::Channel::UseDisk); // sleep until it's no longer busy
        self.busy = true; // "lock" it
        // should probably create some kind of sleep lock instead, and populate it with this
        // possibly playing with fire due to aliasing rules, modifying mutable state that we "own"
        let _ = self.wait();
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
        unsafe {
            process::SCHEDULER.as_mut().unwrap().sleep(process::Channel::ReadDisk);
        }

        self.wait()?;
        // if the buffer is large enough for an entire block
        if buffer.len() >= 512 {
            // unsafe because of port I/O
            unsafe { io::insb(0x1f0, &mut buffer[0..SECTOR_SIZE]) };
        } else {
            // else read the entire block and truncate to the dest buffer length
            let mut tmp_buf = [0; 512];
            // unsafe because of port I/O
            unsafe { io::insb(0x1f0, &mut tmp_buf) };
            for (buf, tmp) in buffer.iter_mut().zip(tmp_buf.iter()) {
                *buf = *tmp;
            }
        }

        Ok(())
    }

    // boilerplate for making an ide read/write request
    unsafe fn ide_cmd(device: u32, sector: u32) {
        io::outb(0x3f6, 0); // generate interrupt
        io::outb(0x1f2, (fs::BLOCKSIZE / SECTOR_SIZE) as u8);
        io::outb(0x1f3, sector as u8 & 0xff);
        io::outb(0x1f4, (sector >> 8) as u8 & 0xff);
        io::outb(0x1f5, (sector >> 16) as u8 & 0xff);
        io::outb(0x1f6,
                 0xe0 | ((device & 0x1) as u8) << 4 | ((sector >> 24) as u8 & 0x0f));
    }

    // poll the IDE device until it's ready
    fn wait(&mut self) -> Result<(), ()> {
        let mut r: u8;
        while {
            r = unsafe { io::inb(0x1f7) };
            (r & (IDE_BSY | IDE_DRDY)) != IDE_DRDY
        } {}

        if r & (IDE_DF | IDE_ERR) == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}
