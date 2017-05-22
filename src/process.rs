use mmu;
//use file;
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

use spin::{Mutex, MutexGuard};

pub static mut CPU: Option<Cpu> = None;
lazy_static! {
    static ref PTABLE: Mutex<LinkedList<Process>> = Mutex::new(LinkedList::<Process>::new());
}
static mut PID: u32 = 0;
const FL_IF: u32 = 0x200;

extern "C" {
    fn trapret();
    fn forkret();
    static _binary_initcode_start: u8;
    static _binary_initcode_size: u8;
    fn swtch(old: *mut *mut Context, new: *mut Context);
}

pub struct Cpu {
    // does this need to be a pointer?
    pub scheduler: *const Context, // swtch() here to enter scheduler
    pub ts: TaskStateSegment, // Used by x86 to find stack for interrupt
    pub gdt: [SegmentDescriptor; mmu::NSEGS], // x86 global descriptor table
    //pub started: bool, // Has the CPU started?
    //pub ncli: i32, // Depth of pushcli nesting.
    pub int_enabled: bool, // Were interrupts enabled before pushcli?

    // Cpu-local storage variables; see below
    pub process: Option<Process>, // The currently-running process.
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            scheduler: core::ptr::null(),
            ts: TaskStateSegment::new(),
            gdt: [SegmentDescriptor::NULL; mmu::NSEGS],
            int_enabled: true,
            process: None,
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
                  //cwd: file::Inode, // Current directory
                  //name: [char; 16], // Process name (debugging)
}

impl Process {
    fn new(new_pid: u32, parent_pid: u32, pagedir: Box<vm::PageDir>) -> Process {
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
        }
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
    let mut p = Process::new(1, 0, pgdir);
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

    // TODO: finish once file system is written


}


//pub fn sleep(
//
//    //pub/sub system? need some way to say that process is blocked on something, and then
//    //later wake it up based on that?  probably use pointers, although it's nasty
//            )
//
impl Cpu {
    pub fn scheduler(&mut self) -> ! {
        loop {
            let mut ptable = PTABLE.lock();
            unsafe { irq::enable() };

            // scan queue to find runnable process
            let mut run_idx = ptable.iter_mut().position(|p| p.state == ProcState::Runnable);

            if let Some(idx) = run_idx {
                // Remove the runnable process from the queue
                let mut list = ptable.split_off(idx);
                let mut runnable = list.pop_front().unwrap();
                let old_proc = self.process.take();
                ptable.append(&mut list);

                // If we had a process running, append it to ptable
                if let Some(pr) = old_proc {
                    ptable.push_back(pr);
                }

                // vm::switchuvm()

                // prepare to execute new process
                assert!(runnable.state == ProcState::Runnable);
                runnable.state = ProcState::Running;
                self.process = Some(runnable);

                // hideously unsafe because we're context switching with assembly call
                // probably not a lot we can do here though
                unsafe {
                    // actual context switching
                    swtch(self.scheduler as *mut _,
                          &mut self.process.as_mut().unwrap().context as *mut _);
                }

                vm::switchkvm();
            }
        }
    }

    fn reschedule(&mut self) {
        if let Some(ref mut p) = self.process {

            //  if(cpu->ncli != 1)
            //    panic("sched locks");
            assert!(p.state != ProcState::Running);
            if readeflags() & FL_IF != 0 {
                panic!("sched interruptible");
            }
            let int_enabled = self.int_enabled;

            p.state = ProcState::Sleeping;

            // unsafe because we're calling out to external code
            unsafe {
                swtch(&mut (&mut p.context as *mut _), self.scheduler as *mut _);
            }
            self.int_enabled = int_enabled;
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

pub fn sleep<'a, T>(lock: &'a Mutex<T>) -> MutexGuard<'a, T> {
    // lock must have been released to make it in here in the first place
    // do weird context switching stuff
    //schedule_process();
    unsafe {
        CPU.as_mut().unwrap().reschedule();
    }
    lock.lock() // possibly get rid of this
}

#[macro_export]
macro_rules! until {
    // in the case where we simply want to stop until the condition is met
    ($cond: expr, $lock: expr) => {
        until!($cond, $lock, {})
    };
    // else where we have arbitrary code to run after the condition is met
    ($cond: expr, $lock: expr, $code: expr) => {
        loop {
            {
                // TODO: consider attempting to grab lock, and if locked then sleep
                if let Some(_) = $lock.try_lock() {
                    if $cond { // test
                        $code;
                        break;
                    } // release lock and sleep?
                }
            }
            let _ = process::sleep($lock);
        }
    }
}
