#![no_std]
#![no_main]
#![feature(const_fn_transmute)]
#![feature(panic_info_message)]
#![feature(abi_x86_interrupt)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]
#![feature(maybe_uninit_ref)]
#![feature(non_ascii_idents)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![feature(const_fn_trait_bound)]
#![feature(const_option)]
#![feature(option_result_unwrap_unchecked)]
#![feature(associated_type_defaults)]
#![feature(asm)]
#![feature(const_generics)]

extern crate rlibc;

#[macro_use]
mod printer;
mod idt;
mod pic;
mod ps2;
mod ps2_keyboard;
mod gdt;
mod svec;
mod gui;
mod harddisk;

use core::{panic::PanicInfo, sync::atomic::{AtomicBool, Ordering}};

use bootloader::BootInfo;
use gui::widget::{Event, Widget, message_box::MessageBox};
use ps2_keyboard::KeyState;

use crate::{gui::{display::Point, widget::{container::Container, initializer::Initializer}}, ps2_keyboard::{KeyCode, KeyEvent, Modifiers}, svec::SVec};

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    // No function call may precede this one, or else undefined behaviour may be invoked.
    initialize(boot_info);

    loop {
        let event = ps2_keyboard::get_key_event();
        unsafe {
            gui::display::send_event(Event::KeyEvent(event));
        }
    }
}

/// Initializes all modules.
///
/// Must be the first function called in `_start`.
fn initialize(boot_info: &BootInfo) {
    static INITIALIZED: AtomicBool = AtomicBool::new(false);

    // Atomics can be quite confusing.
    // This call to `compare_exchange` compares the value of `INITIALIZED`, and if it is `false`, we write `true` to it.
    // This happens atomically, so there is no risk of data races.
    // When the comparison succeeds, `compare_exchange` returns an Ok(_), which means that `INITIALIZED` was `false` and now is `true`,
    // ergo we haven't yet initialized, and should do so.
    // If it returns an `Err(_)`, the comparison failed, so `INITIALIZED` is `true`, and we should not try to initialize again.
    if INITIALIZED.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_ok() {

        // Safety: All modules' initialization functions have orderings that must be guaranteed
        // to avoid undefined behaviour. These are currently undocumented,
        // but the orderings are respected in the below block.
        unsafe {
            gdt::initialize();
            idt::initialize();
    
            // The call to `ptr::read` is safe here, as a reference is always valid for reads,
            // and as `Framebuffer` has no custom destructor and is only comprised of
            // integers and structs of integers. (and an enum with #[repr(C)])
            printer::initialize(core::ptr::read(boot_info.framebuffer.as_ref().unwrap()));
            // printer::force_redraw();
            printer::clear();

            gui::initialize(core::ptr::read(boot_info.framebuffer.as_ref().unwrap()));
            
            pic::initialize();
            x86_64::instructions::interrupts::enable();
            ps2::initialize();
            ps2_keyboard::initialize();
        }
    }
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    static mut ORIGINAL_MESSAGE: Option<SVec<u8, 1024>> = None;
    unsafe {
        if let Some(orig) = ORIGINAL_MESSAGE.take() {
            println!("Panic while trying to pretty_print other panic");
            
            let loc = info.location().unwrap();
            match info.message() {
                Some(message) => {
                    println!("{}: Panic at '{}'", loc, message);
                }
                None => {
                    println!("{}: Panic", loc);
                }
            }
            println!("Original panic:");
            println!("{}", core::str::from_utf8(orig.get_slice()).unwrap());

            loop {}
        }
    }
    let mut msg = SVec::<u8, 1024>::new();
    let loc = info.location().unwrap();
    use core::fmt::Write;
    match info.message() {
        Some(message) => {
            write!(msg, "{}: Panic at '{}'", loc, message).unwrap();
        }
        None => {
            write!(msg, "{}: Panic", loc).unwrap();
        }
    }
    unsafe {
        ORIGINAL_MESSAGE = Some(msg.clone());
    }
    
    let mut widget = MessageBox::uninitialized();
    widget.title = "Panic!";
    widget.text = unsafe { &*(core::str::from_utf8(msg.get_slice()).unwrap() as *const str) };

    let res = unsafe { gui::display::resolution() };

    let char_count = core::str::from_utf8(msg.get_slice()).unwrap().chars().count();
    let max_line_count = (res.x - 32) / 8;
    let mut rows = 1;
    while char_count.saturating_sub((rows - 1) * max_line_count) > max_line_count {
        rows += 1;
    }

    let max_line_length = char_count.min(max_line_count);

    let inner = Point::new(max_line_length * 8 + 16, rows * 16 + 16 + 32);
    let mut container = Container::uninitialized(widget);
    let mut initializer = Initializer::uninitialized(container, (inner, ()));
    initializer.initialize(res, ());

    unsafe {
        let ptr = &mut initializer as *mut Initializer<_>;
        gui::display::add_initialized_widget(ptr.as_mut().unwrap());
    }

    unsafe {
        gui::display::check_redraw();
    }

    loop {}
}
