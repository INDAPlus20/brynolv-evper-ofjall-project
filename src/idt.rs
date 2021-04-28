use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

/// Initializes the Interrupt Descriptor Table and assigns interrupt handlers for the default interrupts coming the CPU.
/// This needs to be called before anything else in the module
pub fn initialize() {
    let idt = unsafe { &mut IDT };

    // Assign interrupt handlers for the default interrupts coming from the CPU
    idt.divide_error.set_handler_fn(default_handler);
    idt.debug.set_handler_fn(default_handler);
    idt.non_maskable_interrupt.set_handler_fn(default_handler);
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.overflow.set_handler_fn(default_handler);
    idt.bound_range_exceeded.set_handler_fn(default_handler);
    idt.invalid_opcode.set_handler_fn(default_handler);
    idt.device_not_available.set_handler_fn(default_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt.invalid_tss
        .set_handler_fn(default_handler_with_error_code);
    idt.segment_not_present
        .set_handler_fn(default_handler_with_error_code);
    idt.stack_segment_fault
        .set_handler_fn(default_handler_with_error_code);
    idt.general_protection_fault
        .set_handler_fn(default_handler_with_error_code);
    idt.page_fault.set_handler_fn(page_fault_handler);
    idt.x87_floating_point.set_handler_fn(default_handler);
    idt.alignment_check
        .set_handler_fn(default_handler_with_error_code);
    idt.machine_check.set_handler_fn(machine_check_handler);
    idt.simd_floating_point.set_handler_fn(default_handler);
    idt.virtualization.set_handler_fn(default_handler);
    idt.security_exception
        .set_handler_fn(default_handler_with_error_code);

    // Sets so the CPU uses this IDT
    idt.load();
}

/// Registers an interrupt handler
/// *Note*: IRQ's start at index 32 (0x20)
pub fn register_irq(irq: u8, func: extern "x86-interrupt" fn(InterruptStackFrame)) {
    if irq < 0x20 {
        panic!("Custom IRQ's needs to use index 32 or above (0x20)");
    }
    let idt = unsafe { &mut IDT };
    idt[irq as usize].set_handler_fn(func);
}

// Interrupt handlers
extern "x86-interrupt" fn default_handler(f: InterruptStackFrame) {
    println!("{:#?}\n", f);
}

extern "x86-interrupt" fn default_handler_with_error_code(f: InterruptStackFrame, code: u64) {
    println!("{} {:#?}\n", code, f);
}

extern "x86-interrupt" fn breakpoint_handler(f: InterruptStackFrame) {
    println!("Breakpoint\n{:#?}", f);
}

extern "x86-interrupt" fn double_fault_handler(f: InterruptStackFrame, code: u64) -> ! {
    println!("{} {:#?}\n", code, f);
    loop {}
}

extern "x86-interrupt" fn page_fault_handler(f: InterruptStackFrame, code: PageFaultErrorCode) {
    println!("Page Fault\n{:#?}\n{:#?}\n", code, f);
}

extern "x86-interrupt" fn machine_check_handler(f: InterruptStackFrame) -> ! {
    println!("{:#?}\n", f);
    loop {}
}
