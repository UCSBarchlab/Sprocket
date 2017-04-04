extern crate x86;

use traps;
use self::x86::shared::io;
// Intel 8259A programmable interrupt controllers.

// I/O Addresses of the two programmable interrupt controllers
const IO_PIC1: u16 = 0x20; // Master (IRQs 0-7)
const IO_PIC2: u16 = 0xA0; // Slave (IRQs 8-15)

const IRQ_SLAVE: u8 = 2; // IRQ at which slave connects to master

// Current IRQ mask.
// Initial IRQ mask has interrupt 2 enabled (for slave 8259A).
static mut irqmask: u16 = 0xFFFF & !(1 << IRQ_SLAVE);
// TODO: this might be race-prone?  xv6 doesn't care, but it makes me nervous
// Might only be a problem if we support multiprocessing?  Or might use some
// kind of thread-local kernel storage

unsafe fn picsetmask(mask: u16) {
    irqmask = mask;
    io::outb(IO_PIC1 + 1, mask as u8);
    io::outb(IO_PIC2 + 1, (mask >> 8) as u8);
}

unsafe fn picenable(irq: i32) {
    picsetmask(irqmask & !(1 << irq));
}

// Initialize the 8259A interrupt controllers.
pub fn picinit() {
    unsafe {
        // mask all interrupts
        io::outb(IO_PIC1 + 1, 0xFF);
        io::outb(IO_PIC2 + 1, 0xFF);

        // Set up master (8259A-1)

        // ICW1:  0001g0hi
        //    g:  0 = edge triggering, 1 = level triggering
        //    h:  0 = cascaded PICs, 1 = master only
        //    i:  0 = no ICW4, 1 = ICW4 required
        io::outb(IO_PIC1, 0x11);

        // ICW2:  Vector offset
        io::outb(IO_PIC1 + 1, traps::T_IRQ0);

        // ICW3:  (master PIC) bit mask of IR lines connected to slaves
        //        (slave PIC) 3-bit # of slave's connection to master
        io::outb(IO_PIC1 + 1, 1 << IRQ_SLAVE);

        // ICW4:  000nbmap
        //    n:  1 = special fully nested mode
        //    b:  1 = buffered mode
        //    m:  0 = slave PIC, 1 = master PIC
        //      (ignored when b is 0, as the master/slave role
        //      can be hardwired).
        //    a:  1 = Automatic EOI mode
        //    p:  0 = MCS-80/85 mode, 1 = intel x86 mode
        io::outb(IO_PIC1 + 1, 0x3);

        // Set up slave (8259A-2)
        io::outb(IO_PIC2, 0x11); // ICW1
        io::outb(IO_PIC2 + 1, traps::T_IRQ0 + 8); // ICW2
        io::outb(IO_PIC2 + 1, IRQ_SLAVE); // ICW3
        // NB Automatic EOI mode doesn't tend to work on the slave.
        // Linux source code says it's "to be investigated".
        io::outb(IO_PIC2 + 1, 0x3); // ICW4

        // OCW3:  0ef01prs
        //   ef:  0x = NOP, 10 = clear specific mask, 11 = set specific mask
        //    p:  0 = no polling, 1 = polling mode
        //   rs:  0x = NOP, 10 = read IRR, 11 = read ISR
        io::outb(IO_PIC1, 0x68); // clear specific mask
        io::outb(IO_PIC1, 0x0a); // read IRR by default

        io::outb(IO_PIC2, 0x68); // OCW3
        io::outb(IO_PIC2, 0x0a); // OCW3

        if irqmask != 0xFFFF {
            picsetmask(irqmask);
        }
    }
}
