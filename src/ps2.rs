use x86_64::{instructions::port::Port, structures::idt::InterruptStackFrame};

struct Driver {
    data_port: Port<u8>,
    status_command_port: Port<u8>
}

impl Driver {
    const fn new() -> Self {
        Self {
            data_port: Port::new(0x60),
            status_command_port: Port::new(0x64)
        }
    }

    /// Follows the initialization sequence from [the osdev wiki](https://wiki.osdev.org/%228042%22_PS/2_Controller#Initialising_the_PS.2F2_Controller),
    /// skipping some steps.
    unsafe fn initialize(&mut self) {
        // Disable devices to prevent incoming data from messing up the initialization sequence
        self.status_command_port.write(0xAD);
        self.status_command_port.write(0xA7);

        // // Flush data buffer
        if self.status_command_port.read() & 1 != 0 {
            self.data_port.read();
        }

        // // Disable all IRQs
        self.send_command(0x20);
        while self.status_command_port.read() & 1 == 0 {}
        let config = self.data_port.read();

        let mut config = self.get_config();
        config &= !0b00000011; // Clears bits 0 and 1, which are first/second port IRQs enable
        config |= 1 << 6; // Translation from scancode set 2 to set 1
        self.set_config(config);

        // Issue self test. 0x55 is success.
        self.status_command_port.write(0xAA);
        if self.read_data() != 0x55 {
            panic!("PS/2 controller failed self test");
        }

        // Test first port. 0x00 is success.
        self.status_command_port.write(0xAB);
        let response = self.read_data();
        if response != 0x00 {
            panic!("PS/2 controller port failed test: response was {:#02X}", response);
        }

        // Set IRQ1 handler
        crate::idt::register_irq(0x20 + 1, default_handler);

        // Enable IRQ1 in the PIC
        crate::pic::enable_interrupt(1);
        // Enable first port interrupt (IRQ1)
        let mut config = self.get_config();
        config |= 0b1; // Sets bit 0, which is first port IRQ enable
        self.set_config(config);

        // Enable first port
        self.send_command(0xAE);
    }

    unsafe fn get_config(&mut self) -> u8 {
        self.status_command_port.write(0x20);
        self.read_data()
    }

    unsafe fn set_config(&mut self, config: u8) {
        self.status_command_port.write(0x60);
        self.write_data(config);
    }

    unsafe fn read_data(&mut self) -> u8 {
        // While bit 0 (output buffer full) is not set, wait
        while self.status_command_port.read() & 1 == 0 {}
        self.data_port.read()
    }

    unsafe fn write_data(&mut self, data: u8) {
        // While bit 1 (input buffer full) is set, wait
        while self.status_command_port.read() & 0b10 != 0 {}
        self.data_port.write(data);
    }

    unsafe fn send_command(&mut self, command: u8) {
        self.status_command_port.write(command);
    }
}

static mut DRIVER: Driver = Driver { data_port: Port::new(0x60), status_command_port: Port::new(0x64) };

/// Initializes the PS/2 controller.
///
/// # Safety
///
/// This should not be called if another call to this function has not yet returned.
///
/// The modules `printer` and `pic` must be initialized before this function is called.
pub unsafe fn initialize() {
    DRIVER.initialize();
}

pub unsafe fn send_byte(byte: u8) {
    DRIVER.write_data(byte);
}

pub unsafe fn get_byte() -> u8 {
    DRIVER.read_data()
}

extern "x86-interrupt" fn default_handler(stack_frame: InterruptStackFrame) {
    println!("Default handler");

    unsafe { DRIVER.read_data(); }

    unsafe { crate::pic::send_eoi(1) };
}
