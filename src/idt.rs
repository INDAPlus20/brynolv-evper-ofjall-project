use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};



pub fn initialize() {
    todo!()
}


pub fn register_irq(irq: u8, func: extern "x86-interrupt" fn(InterruptStackFrame)) {
    todo!()
}

