// This file contains definitions for the
// x86 memory management unit (MMU).

// Eflags register
#define FL_CF           0x00000001      // Carry Flag
#define FL_PF           0x00000004      // Parity Flag
#define FL_AF           0x00000010      // Auxiliary carry Flag
#define FL_ZF           0x00000040      // Zero Flag
#define FL_SF           0x00000080      // Sign Flag
#define FL_TF           0x00000100      // Trap Flag
#define FL_IF           0x00000200      // Interrupt Enable
#define FL_DF           0x00000400      // Direction Flag
#define FL_OF           0x00000800      // Overflow Flag
#define FL_IOPL_MASK    0x00003000      // I/O Privilege Level bitmask
#define FL_IOPL_0       0x00000000      //   IOPL == 0
#define FL_IOPL_1       0x00001000      //   IOPL == 1
#define FL_IOPL_2       0x00002000      //   IOPL == 2
#define FL_IOPL_3       0x00003000      //   IOPL == 3
#define FL_NT           0x00004000      // Nested Task
#define FL_RF           0x00010000      // Resume Flag
#define FL_VM           0x00020000      // Virtual 8086 mode
#define FL_AC           0x00040000      // Alignment Check
#define FL_VIF          0x00080000      // Virtual Interrupt Flag
#define FL_VIP          0x00100000      // Virtual Interrupt Pending
#define FL_ID           0x00200000      // ID flag

// Control Register flags
#define CR0_PE          0x00000001      // Protection Enable
#define CR0_MP          0x00000002      // Monitor coProcessor
#define CR0_EM          0x00000004      // Emulation
#define CR0_TS          0x00000008      // Task Switched
#define CR0_ET          0x00000010      // Extension Type
#define CR0_NE          0x00000020      // Numeric Errror
#define CR0_WP          0x00010000      // Write Protect
#define CR0_AM          0x00040000      // Alignment Mask
#define CR0_NW          0x20000000      // Not Writethrough
#define CR0_CD          0x40000000      // Cache Disable
#define CR0_PG          0x80000000      // Paging

#define CR4_PSE         0x00000010      // Page size extension

// various segment selectors.
#define SEG_KCODE 1  // kernel code
#define SEG_KDATA 2  // kernel data+stack
#define SEG_KCPU  3  // kernel per-cpu data
#define SEG_UCODE 4  // user code
#define SEG_UDATA 5  // user data+stack
#define SEG_TSS   6  // this process's task state

// cpu->gdt[NSEGS] holds the above segments.
#define NSEGS     7

//PAGEBREAK!
#ifndef __ASSEMBLER__
// Segment Descriptor
struct segdesc {
  unsigned int lim_15_0 : 16;  // Low bits of segment limit
  unsigned int base_15_0 : 16; // Low bits of segment base address
  unsigned int base_23_16 : 8; // Middle bits of segment base address
  unsigned int type : 4;       // Segment type (see STS_ constants)
  unsigned int s : 1;          // 0 = system, 1 = application
  unsigned int dpl : 2;        // Descriptor Privilege Level
  unsigned int p : 1;          // Present
  unsigned int lim_19_16 : 4;  // High bits of segment limit
  unsigned int avl : 1;        // Unused (available for software use)
  unsigned int rsv1 : 1;       // Reserved
  unsigned int db : 1;         // 0 = 16-bit segment, 1 = 32-bit segment
  unsigned int g : 1;          // Granularity: limit scaled by 4K when set
  unsigned int base_31_24 : 8; // High bits of segment base address
};

// Normal segment
#define SEG(type, base, lim, dpl) (struct segdesc)    \
{ ((lim) >> 12) & 0xffff, (unsigned int)(base) & 0xffff,      \
  ((unsigned int)(base) >> 16) & 0xff, type, 1, dpl, 1,       \
  (unsigned int)(lim) >> 28, 0, 0, 1, 1, (unsigned int)(base) >> 24 }
#define SEG16(type, base, lim, dpl) (struct segdesc)  \
{ (lim) & 0xffff, (unsigned int)(base) & 0xffff,              \
  ((unsigned int)(base) >> 16) & 0xff, type, 1, dpl, 1,       \
  (unsigned int)(lim) >> 16, 0, 0, 1, 0, (unsigned int)(base) >> 24 }
#endif

#define DPL_USER    0x3     // User DPL

// Application segment type bits
#define STA_X       0x8     // Executable segment
#define STA_E       0x4     // Expand down (non-executable segments)
#define STA_C       0x4     // Conforming code segment (executable only)
#define STA_W       0x2     // Writeable (non-executable segments)
#define STA_R       0x2     // Readable (executable segments)
#define STA_A       0x1     // Accessed

// System segment type bits
#define STS_T16A    0x1     // Available 16-bit TSS
#define STS_LDT     0x2     // Local Descriptor Table
#define STS_T16B    0x3     // Busy 16-bit TSS
#define STS_CG16    0x4     // 16-bit Call Gate
#define STS_TG      0x5     // Task Gate / Coum Transmitions
#define STS_IG16    0x6     // 16-bit Interrupt Gate
#define STS_TG16    0x7     // 16-bit Trap Gate
#define STS_T32A    0x9     // Available 32-bit TSS
#define STS_T32B    0xB     // Busy 32-bit TSS
#define STS_CG32    0xC     // 32-bit Call Gate
#define STS_IG32    0xE     // 32-bit Interrupt Gate
#define STS_TG32    0xF     // 32-bit Trap Gate

// A virtual address 'la' has a three-part structure as follows:
//
// +--------10------+-------10-------+---------12----------+
// | Page Directory |   Page Table   | Offset within Page  |
// |      Index     |      Index     |                     |
// +----------------+----------------+---------------------+
//  \--- PDX(va) --/ \--- PTX(va) --/

// page directory index
#define PDX(va)         (((unsigned int)(va) >> PDXSHIFT) & 0x3FF)

// page table index
#define PTX(va)         (((unsigned int)(va) >> PTXSHIFT) & 0x3FF)

// construct virtual address from indexes and offset
#define PGADDR(d, t, o) ((unsigned int)((d) << PDXSHIFT | (t) << PTXSHIFT | (o)))

// Page directory and page table constants.
#define NPDENTRIES      1024    // # directory entries per page directory
#define NPTENTRIES      1024    // # PTEs per page table
#define PGSIZE          4096    // bytes mapped by a page

#define PGSHIFT         12      // log2(PGSIZE)
#define PTXSHIFT        12      // offset of PTX in a linear address
#define PDXSHIFT        22      // offset of PDX in a linear address

#define PGROUNDUP(sz)  (((sz)+PGSIZE-1) & ~(PGSIZE-1))
#define PGROUNDDOWN(a) (((a)) & ~(PGSIZE-1))

// Page table/directory entry flags.
#define PTE_P           0x001   // Present
#define PTE_W           0x002   // Writeable
#define PTE_U           0x004   // User
#define PTE_PWT         0x008   // Write-Through
#define PTE_PCD         0x010   // Cache-Disable
#define PTE_A           0x020   // Accessed
#define PTE_D           0x040   // Dirty
#define PTE_PS          0x080   // Page Size
#define PTE_MBZ         0x180   // Bits must be zero

// Address in page table or page directory entry
#define PTE_ADDR(pte)   ((unsigned int)(pte) & ~0xFFF)
#define PTE_FLAGS(pte)  ((unsigned int)(pte) &  0xFFF)

#ifndef __ASSEMBLER__
typedef unsigned int pte_t;

// Task state segment format
struct taskstate {
  unsigned int link;         // Old ts selector
  unsigned int esp0;         // Stack pointers and segment selectors
  unsigned short ss0;        //   after an increase in privilege level
  unsigned short padding1;
  unsigned int *esp1;
  unsigned short ss1;
  unsigned short padding2;
  unsigned int *esp2;
  unsigned short ss2;
  unsigned short padding3;
  void *cr3;         // Page directory base
  unsigned int *eip;         // Saved state from last task switch
  unsigned int eflags;
  unsigned int eax;          // More saved state (registers)
  unsigned int ecx;
  unsigned int edx;
  unsigned int ebx;
  unsigned int *esp;
  unsigned int *ebp;
  unsigned int esi;
  unsigned int edi;
  unsigned short es;         // Even more saved state (segment selectors)
  unsigned short padding4;
  unsigned short cs;
  unsigned short padding5;
  unsigned short ss;
  unsigned short padding6;
  unsigned short ds;
  unsigned short padding7;
  unsigned short fs;
  unsigned short padding8;
  unsigned short gs;
  unsigned short padding9;
  unsigned short ldt;
  unsigned short padding10;
  unsigned short t;          // Trap on task switch
  unsigned short iomb;       // I/O map base address
};

// PAGEBREAK: 12
// Gate descriptors for interrupts and traps
struct gatedesc {
  unsigned int off_15_0 : 16;   // low 16 bits of offset in segment
  unsigned int cs : 16;         // code segment selector
  unsigned int args : 5;        // # args, 0 for interrupt/trap gates
  unsigned int rsv1 : 3;        // reserved(should be zero I guess)
  unsigned int type : 4;        // type(STS_{TG,IG32,TG32})
  unsigned int s : 1;           // must be 0 (system)
  unsigned int dpl : 2;         // descriptor(meaning new) privilege level
  unsigned int p : 1;           // Present
  unsigned int off_31_16 : 16;  // high bits of offset in segment
};

// Set up a normal interrupt/trap gate descriptor.
// - istrap: 1 for a trap (= exception) gate, 0 for an interrupt gate.
//   interrupt gate clears FL_IF, trap gate leaves FL_IF alone
// - sel: Code segment selector for interrupt/trap handler
// - off: Offset in code segment for interrupt/trap handler
// - dpl: Descriptor Privilege Level -
//        the privilege level required for software to invoke
//        this interrupt/trap gate explicitly using an int instruction.
#define SETGATE(gate, istrap, sel, off, d)                \
{                                                         \
  (gate).off_15_0 = (unsigned int)(off) & 0xffff;                \
  (gate).cs = (sel);                                      \
  (gate).args = 0;                                        \
  (gate).rsv1 = 0;                                        \
  (gate).type = (istrap) ? STS_TG32 : STS_IG32;           \
  (gate).s = 0;                                           \
  (gate).dpl = (d);                                       \
  (gate).p = 1;                                           \
  (gate).off_31_16 = (unsigned int)(off) >> 16;                  \
}

#endif
