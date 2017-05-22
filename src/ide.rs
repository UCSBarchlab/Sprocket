use collections::linked_list::LinkedList;
use buffercache::Buffer;
use buffercache;
use spin::Mutex;
use process;

struct IdeDriver {
    disk: Mutex<Disk>,
}

extern "C" {
    static buf: Buffer;
}

impl IdeDriver {
    fn iderw(&mut self, buffer_idx: usize) {
        {
            let mut d = self.disk.lock();
            d.list.push_back(buffer_idx); // this might break things by making a distinct copy
            if *d.list.front().unwrap() == buffer_idx {
                d.idestart(buffer_idx);
            }
        }

        until!(buf.flags & (buffercache::VALID | buffercache::DIRTY) == buffercache::VALID,
               &self.disk);
    }
}

struct Disk {
    list: LinkedList<usize>,
}

impl Disk {
    fn idestart(&mut self, buffer: usize) {
        unimplemented!()
    }
}

//while !condition:
//locked lock -> unlocked -> context switch -> wake up -> locked  -> do_things  -> release
