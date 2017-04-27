use mmu;
//use file;
pub use x86::shared::segmentation::SegmentDescriptor;
pub use x86::bits32::task::TaskStateSegment;
pub use x86::shared::descriptor;
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
struct TrapFrame {
    // registers as pushed by pusha
    edi: u32,
    esi: u32,
    ebp: u32,
    oesp: u32, // useless & ignored
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,

    // rest of trap frame
    gs: u16,
    padding1: u16,
    fs: u16,
    padding2: u16,
    es: u16,
    padding3: u16,
    ds: u16,
    padding4: u16,
    trapno: u32,

    // below here defined by x86 hardware
    err: u32,
    eip: u32,
    cs: u16,
    padding5: u16,
    eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    esp: u32,
    ss: u16,
    padding6: u16,
}
