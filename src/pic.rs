use core::intrinsics::write_bytes;

use port::Port;
use x86_64::{instructions::port, structures::port::PortWrite};

// From OS-dev wiki
const PIC1: u8 = 0x20;
const PIC2: u8 = 0xA0;
const PIC1_COMMAND: u8 = PIC1;
const PIC1_DATA: u8 = PIC1 + 1;
const PIC2_COMMAND: u8 = PIC2;
const PIC2_DATA: u8 = PIC2 + 1;

/// End of interupt
const PIC_EOI: u8 = 0x20;


/// Sends end of interrupt
pub unsafe fn send_eoi(irq: u8) {
    if irq >= 8 {
        PortWrite::write_to_port(PIC2_COMMAND as u16, PIC_EOI);
    }
    PortWrite::write_to_port(PIC1_COMMAND as u16, PIC_EOI);
}


/// Initializes the PICs and tells them to ignore all interupts.
pub unsafe fn initialize() {
    const MASTER_OFFSET: u8 = 0x20;
    const SLAVE_OFFSET: u8 = 0x28;

    const ICW1_ICW4: u8 = 0x01;
    //const ICW1_SINGLE: u8 = 0x02;
    //const ICW1_INTERVAL4: u8 = 0x04;
    //const ICW1_LEVEL: u8 = 0x08;
    const ICW1_INIT: u8 = 0x10;

    const ICW4_8086: u8 = 0x01;
    //const ICW4_AUTO: u8 = 0x02;
    //const ICW4_BUF_SLAVE: u8 = 0x08;
    //const ICW4_BUF_MASTER: u8 = 0x0C;
    //const ICW4_SFNM: u8 = 0x10;

    let mut pic1_data: Port<u8> = Port::new(PIC1_DATA as u16);
    //let a1 = pic1_data.read();
    let mut pic2_data: Port<u8> = Port::new(PIC2_DATA as u16);
    //let a2 = pic2_data.read();

    PortWrite::write_to_port(PIC1_COMMAND as u16, ICW1_INIT | ICW1_ICW4);
    PortWrite::write_to_port(PIC2_COMMAND as u16, ICW1_INIT | ICW1_ICW4);

    pic1_data.write(MASTER_OFFSET);
    pic2_data.write(SLAVE_OFFSET);

    pic1_data.write(4);
    pic2_data.write(2);

    pic1_data.write(ICW4_8086);
    pic2_data.write(ICW4_8086);

    pic1_data.write(0xFF); //a1);
    pic2_data.write(0xFF); //a2);
}


/// Enable interrupt on `irq`
pub unsafe fn enable_interrupt(irq: u8) {
    let mut irq = irq; // Wait, this is legal??
    let p_val;

    if irq < 8 {
        p_val = PIC1_DATA as u16;
    } else {
        p_val = PIC2_DATA as u16;
        irq -= 8;
    }
    let mut port: Port<u8> = Port::new(p_val);
    let val = port.read() & !(1 << irq);
    port.write(val);
}

/// Disable interrupt on `irq`
pub unsafe fn disable_interrupt(irq: u8) {
    let mut irq = irq;
    let p_val;

    if irq < 8 {
        p_val = PIC1_DATA as u16;
    } else {
        p_val = PIC2_DATA as u16;
        irq -= 8;
    }
    let mut port: Port<u8> = Port::new(p_val);
    let val = port.read() | (1 << irq);
    port.write(val);
}
