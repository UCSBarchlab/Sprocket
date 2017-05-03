use mmu;
//use file;
pub use x86::shared::segmentation::SegmentDescriptor;
pub use x86::bits32::task::TaskStateSegment;
pub use x86::shared::descriptor;
pub use x86::shared::irq;
use core;
use alloc::boxed::Box;
use vm;
use kalloc;

pub static mut CPU: Option<Cpu> = None;
static mut PID: u32 = 0;

extern "C" {
    fn trapret();
    fn forkret();
}

pub struct Cpu {
    // does this need to be a pointer?
    pub scheduler: *const Context, // swtch() here to enter scheduler
    pub ts: TaskStateSegment, // Used by x86 to find stack for interrupt
    pub gdt: [SegmentDescriptor; mmu::NSEGS], // x86 global descriptor table
    //pub started: bool, // Has the CPU started?
    //pub ncli: i32, // Depth of pushcli nesting.
    pub intena: bool, // Were interrupts enabled before pushcli?

    // Cpu-local storage variables; see below
    pub cpu: *const Cpu,
    pub process: *const Process, // The currently-running process.
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            scheduler: core::ptr::null(),
            ts: TaskStateSegment::new(),
            gdt: [SegmentDescriptor::NULL; mmu::NSEGS],
            intena: true,
            cpu: core::ptr::null(),
            process: core::ptr::null(),
        }
    }
}

#[derive(Copy, Clone, Default)]
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
            trapframe: stack.tf,
            context: stack.context,
            kstack: stack,
            pid: new_pid,
            parent: parent_pid,
            killed: false,
            state: ProcState::Embryo,
        }
    }
}

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
    tf: TrapFrame,
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
    pub trapno: u32,

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

pub fn scheduler() -> ! {
    unsafe { irq::enable() };
    loop {}
}
