use mmu;
use file;
use x86::shared::segmentation::SegmentDescriptor;
use x86::bits32::task::TaskStateSegment;

struct Cpu {
    apicid: u8, // Local APIC ID
    scheduler: *const Context, // swtch() here to enter scheduler
    ts: TaskStateSegment, // Used by x86 to find stack for interrupt
    gdt: [SegmentDescriptor; mmu::NSEGS], // x86 global descriptor table
    started: bool, // Has the CPU started?
    ncli: i32, // Depth of pushcli nesting.
    intena: bool, // Were interrupts enabled before pushcli?

    // Cpu-local storage variables; see below
    cpu: *const Cpu,
    process: *const Process, // The currently-running process.
}

struct Context {
    edi: u32,
    esi: u32,
    ebx: u32,
    ebp: u32,
    eip: u32,
}

struct Process {
    size: usize, // Size of process memory (bytes)
    pgdir: *const u32, // Page table
    kstack: *const u8, // Bottom of kernel stack for this process
    state: ProcState, // Process state
    pid: u32, // Process ID
    parent: *const Process, // Parent process
    trapframe: *const TrapFrame, // Trap frame for current syscall
    context: *const Context, // swtch() here to run process
    chan: *const u8, // If non-zero, sleeping on chan. TODO figure out type
    killed: bool, // If non-zero, have been killed
    ofile: *const [file::File; file::NOFILE], // Open files
    cwd: *const file::Inode, // Current directory
    name: [char; 16], // Process name (debugging)
}

enum ProcState {
    Unused,
    Embryo,
    Sleeping,
    Runnable,
    Running,
    Zombie,
}


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
