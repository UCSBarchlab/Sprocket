use mmu;
pub use x86::shared::segmentation::SegmentDescriptor;
pub use x86::bits32::task::TaskStateSegment;
pub use x86::shared::descriptor;
pub use x86::shared::irq;
use traps;
use spinlock::Mutex;


lazy_static! {
    pub static ref CPU: Mutex<Cpu> = Mutex::new(Cpu::new());
}

pub struct Cpu {
    // does this need to be a pointer?
    pub ts: TaskStateSegment, // Used by x86 to find stack for interrupt
    pub gdt: [SegmentDescriptor; mmu::NSEGS], // x86 global descriptor table
    //pub started: bool, // Has the CPU started?
    //pub ncli: i32, // Depth of pushcli nesting.
    pub int_enabled: bool, // Were interrupts enabled before pushcli?
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            ts: TaskStateSegment::new(),
            gdt: [SegmentDescriptor::NULL; mmu::NSEGS],
            int_enabled: true,
        }
    }
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

impl Cpu {}

/// suspend execution until an event occurs
pub fn sleep() {
    use x86;
    unsafe { x86::shared::halt() }; // halt until interrupts come in
}
