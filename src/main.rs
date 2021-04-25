#![no_std]
#![no_main]
#![feature(const_fn_transmute)]
#![feature(panic_info_message)]
#![feature(abi_x86_interrupt)]

extern crate rlibc;

mod printer;
mod idt;
mod pic;
mod ps2;
mod ps2_keyboard;
mod gdt;

use core::panic::PanicInfo;

use bootloader::BootInfo;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    // Safety: this is safe
    unsafe {
        gdt::initialize();

        printer::initialize(core::ptr::read(boot_info.framebuffer.as_ref().unwrap()));
        printer::clear();
    }
    println!("Hello, World!");

    loop {}
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    let loc = info.location().unwrap();
    match info.message() {
        Some(message) => {
            println!("{}: Panic at '{}'", loc, message);
        }
        None => {
            println!("{}: Panic", loc);
        }
    }
    loop {}
}
