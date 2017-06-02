use mmu;
use file;
pub use x86::shared::segmentation::SegmentDescriptor;
pub use x86::bits32::task::TaskStateSegment;
pub use x86::shared::descriptor;
pub use x86::shared::irq;
use x86::shared::PrivilegeLevel;
use core;
use alloc::boxed::Box;
use vm;
use kalloc;
use traps;
use collections::linked_list::LinkedList;
use fs;

pub static mut CPU: Option<Cpu> = None;
pub static mut SCHEDULER: Option<Scheduler> = None;
//lazy_static! {
//    static ref PTABLE: Mutex<LinkedList<Process>> = Mutex::new(LinkedList::<Process>::new());
//}
const FL_IF: u32 = 0x200;
const PROCNAME_LEN: usize = 16;

extern "C" {
    fn trapret(); // implement this later
    static _binary_initcode_start: u8;
    static _binary_initcode_size: u8;
    fn swtch(old: *mut *mut Context, new: *mut Context);
}

pub struct Cpu {
    // does this need to be a pointer?
    pub ts: TaskStateSegment, // Used by x86 to find stack for interrupt
    pub gdt: [SegmentDescriptor; mmu::NSEGS], // x86 global descriptor table
    //pub started: bool, // Has the CPU started?
    //pub ncli: i32, // Depth of pushcli nesting.
    pub int_enabled: bool, // Were interrupts enabled before pushcli?
    pub scheduler: Scheduler,
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            ts: TaskStateSegment::new(),
            gdt: [SegmentDescriptor::NULL; mmu::NSEGS],
            int_enabled: true,
            scheduler: Scheduler::new(),
        }
    }
}

#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct Context {
    edi: u32,
    esi: u32,
    ebx: u32,
    ebp: u32,
    eip: u32,
}

pub struct Process {
    size: usize, // Size of process memory (bytes)
    pgdir: Box<vm::PageDir>, // Page table
    kstack: Box<InitKstack>, // Bottom of kernel stack for this process
    state: ProcState, // Process state
    pid: u32, // Process ID
    parent: u32, // Parent process
    trapframe: TrapFrame, // Trap frame for current syscall
    context: Context, // swtch() here to run process
    //chan: Option<*const u8>, // If non-zero, sleeping on chan. TODO figure out type
    killed: bool, // If non-zero, have been killed
    // ofile: *const [file::File; file::NOFILE], // Open files
    cwd: u32, // Current directory inum
    name: [u8; PROCNAME_LEN], // Process name (debugging)
    channel: Option<Channel>,
}

impl Process {
    fn new(new_pid: u32, parent_pid: u32, pagedir: Box<vm::PageDir>, procname: &[u8]) -> Process {
        let mut stack: Box<InitKstack> = Box::new(Default::default());
        stack.trapret = trapret as u32;
        stack.context = Default::default();
        stack.context.eip = forkret as u32;
        Process {
            size: kalloc::PGSIZE,
            pgdir: pagedir,
            trapframe: stack.trapframe,
            context: stack.context,
            kstack: stack,
            pid: new_pid,
            parent: parent_pid,
            killed: false,
            state: ProcState::Embryo,
            channel: None,
            cwd: fs::ROOT_INUM,
            name: {
                let mut n = [0; PROCNAME_LEN];
                n.copy_from_slice(procname);
                n
            },
        }
    }
}

pub static mut FIRST_PROCESS: bool = true;

pub extern "C" fn forkret() {
    unsafe {
        if FIRST_PROCESS {
            // do any kind of first-process init here.  may not need this
            FIRST_PROCESS = false;
        }
        irq::enable();
    }
}

#[derive(PartialEq)]
pub enum ProcState {
    Unused,
    Embryo,
    Sleeping,
    Runnable,
    Running,
    Zombie,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Channel {
    UseDisk, // someone else is making a disk request, wait until it's available
    ReadDisk, // we initiated disk read operation earlier, wake us when the data is ready
    FileSystem, // Waiting to do file system operation.
    Other(usize),
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct InitKstack {
    // Padding forces struct to be size of a page allocation
    // Sadly Rust doesn't support parameterizing types over integers yet, and Copy is only
    // implemented for arrays up to size 32, so padding is done in this weird way
    padding1: [[u8; 32]; 32],
    padding2: [[u8; 32]; 32],
    padding3: [[u8; 32]; 32],
    padding4: [[u8; 32]; 28],
    padding5: [u8; 28],
    context: Context,
    trapret: u32,
    trapframe: TrapFrame,
}


#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct TrapFrame {
    // registers as pushed by pusha
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub oesp: u32, // useless & ignored
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,

    // rest of trap frame
    pub gs: u16,
    pub padding1: u16,
    pub fs: u16,
    pub padding2: u16,
    pub es: u16,
    pub padding3: u16,
    pub ds: u16,
    pub padding4: u16,
    pub trapno: traps::Interrupt,

    // below here defined by x86 hardware
    pub err: u32,
    pub eip: u32,
    pub cs: u16,
    pub padding5: u16,
    pub eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    pub esp: u32,
    pub ss: u16,
    pub padding6: u16,
}

pub fn userinit() {
    let pgdir = vm::setupkvm().expect("userinit: out of memory");
    let mut p = Process::new(1, 0, pgdir, b"initcode");
    let slice = unsafe {
        // unsafe because of memory shenanigans
        core::slice::from_raw_parts(&_binary_initcode_start,
                                    &_binary_initcode_size as *const _ as usize)
    };
    vm::inituvm(&mut p.pgdir, slice);
    p.size = kalloc::PGSIZE;
    p.trapframe.cs = (vm::Segment::UCode as u16) << 3 | PrivilegeLevel::Ring3 as u16;
    p.trapframe.ds = (vm::Segment::UData as u16) << 3 | PrivilegeLevel::Ring3 as u16;
    p.trapframe.es = p.trapframe.ds;
    p.trapframe.ss = p.trapframe.ds;
    p.trapframe.eflags = traps::FLAG_INT_ENABLED;
    p.trapframe.esp = kalloc::PGSIZE as u32;
    p.trapframe.eip = 0;
    p.kstack.trapframe = p.trapframe;

    p.state = ProcState::Runnable;
}


//pub fn sleep(
//
//    //pub/sub system? need some way to say that process is blocked on something, and then
//    //later wake it up based on that?  probably use pointers, although it's nasty
//            )
//
impl Cpu {}

pub struct Scheduler {
    ptable: LinkedList<Process>,
    current: Option<Process>,
    scheduler_context: *const Context, // swtch() here to enter scheduler
    next_pid: u32,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            ptable: LinkedList::<Process>::new(),
            current: None,
            scheduler_context: core::ptr::null(),
            next_pid: 1,
        }
    }


    pub fn scheduler(&mut self) -> ! {
        loop {
            unsafe { irq::disable() };

            // scan queue to find runnable process (if any exist)
            let run_idx = self.ptable.iter_mut().position(|p| p.state == ProcState::Runnable);
            if let Some(idx) = run_idx {
                // Remove the runnable process from the queue
                let mut list = self.ptable.split_off(idx);
                let runnable = list.pop_front().unwrap();
                self.ptable.append(&mut list);

                self.switch_to(runnable);
                // we've returned from that context, so reset to our page tables
                vm::switchkvm();

            }
            unsafe { irq::enable() };
        }
    }

    fn switch_to(&mut self, mut runnable: Process) {
        assert!(self.current.is_none());
        assert!(runnable.state == ProcState::Runnable);
        runnable.state = ProcState::Running;
        self.current = Some(runnable);

        // hideously unsafe because we're context switching with assembly call
        // probably not a lot we can do here though

        // vm::switchuvm()
        unsafe {
            // actual context switching
            swtch(self.scheduler_context as *mut _,
                  &mut self.current.as_mut().unwrap().context as *mut _);
        }
        // put process back on the run queue
        self.ptable.push_back(self.current.take().expect("Current process not found!"));
    }

    // sched
    pub fn reschedule(&mut self, cpu: &mut Cpu) {
        if let Some(ref mut p) = self.current {

            //  if(cpu->ncli != 1)
            //    panic("sched locks");
            assert!(p.state != ProcState::Running);
            if readeflags() & FL_IF != 0 {
                panic!("sched interruptible");
            }
            let int_enabled = cpu.int_enabled;

            p.state = ProcState::Sleeping;

            // unsafe because we're calling out to external code
            unsafe {
                swtch(&mut (&mut p.context as *mut _),
                      self.scheduler_context as *mut _);
            }
            cpu.int_enabled = int_enabled;
        } else {
            panic!("Called reschedule() with no process running!");
        }
    }

    /// Allows a thread to suspend itself in order to allow other execution to continue
    pub fn sleep(&mut self, channel: Channel) {
        {
            let p = self.current.as_mut().expect("Expected to call sleep from a thread!");
            p.channel = Some(channel);
            p.state = ProcState::Sleeping;
        }

        self.reschedule(unsafe { CPU.as_mut().unwrap() });

        self.current.as_mut().expect("Expected to call sleep from a thread!").channel = None;
    }

    pub fn yield_thread(&mut self) {
        {
            let p = self.current.as_mut().expect("Expected to call yield from a thread!");
            p.state = ProcState::Runnable;
        }
        self.reschedule(unsafe { CPU.as_mut().unwrap() });
    }

    /// Marks all threads that are blocked on channel as runnable
    pub fn wake(&mut self, channel: Channel) {
        for p in self.ptable
            .iter_mut()
            .filter(|p| p.state == ProcState::Sleeping && p.channel == Some(channel)) {
            p.state = ProcState::Runnable;
        }
    }
}

fn readeflags() -> u32 {
    let eflags: u32;
    unsafe {
        asm!("pushfl; popl $0" : "=r" (eflags) : : : "volatile");
    }
    return eflags;
}

#[macro_export]
macro_rules! until {
    // in the case where we simply want to stop until the condition is met
    ($cond: expr, $reason: expr) => {
        until!($cond, $reason, {})
    };
    // else where we have arbitrary code to run after the condition is met
    ($cond: expr, $reason: expr, $code: expr) => {
        loop {
            {
                // TODO: consider attempting to grab lock, and if locked then sleep
                    if $cond { // test
                        $code;
                        break;
                    } // release lock and sleep?
            }
            unsafe { process::SCHEDULER.as_mut().unwrap().sleep($reason)};
        }
    }
}
