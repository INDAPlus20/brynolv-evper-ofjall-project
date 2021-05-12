#![no_std]
#![no_main]
#![feature(const_fn_transmute)]
#![feature(panic_info_message)]
#![feature(abi_x86_interrupt)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]
#![feature(maybe_uninit_ref)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![feature(const_fn_trait_bound)]
#![feature(const_option)]
#![feature(option_result_unwrap_unchecked)]
#![feature(associated_type_defaults)]
#![feature(asm)]
#![feature(const_generics)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_evaluatable_checked)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

#[macro_use]
mod printer;
mod allocator;
mod gdt;
mod gui;
mod harddisk;
mod idt;
mod pic;
mod ps2;
mod ps2_keyboard;
mod svec;

use alloc::{boxed::Box, string::String};
use core::{
	panic::PanicInfo,
	sync::atomic::{AtomicBool, Ordering},
};

use bootloader::BootInfo;
use gui::widget::{message_box::MessageBox, Event, Widget};
use harddisk::fat32::FatError;
use ps2_keyboard::KeyState;

use crate::{
	gui::{display::Point, widget::container::Container},
	ps2_keyboard::{KeyCode, KeyEvent, Modifiers},
	svec::SVec,
};

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
	if INITIALIZED
		.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
		.is_ok()
	{
		// Safety: Some modules' initialization functions have orderings that must be guaranteed
		// to avoid undefined behaviour. These are respected in the below block.
		unsafe {
			gdt::initialize();
			idt::initialize();

			// The call to `ptr::read` is safe here, as a reference is always valid for reads,
			// and as `Framebuffer` has no custom destructor and is only comprised of
			// integers and structs of integers. (and an enum with #[repr(C)])
			printer::initialize(core::ptr::read(boot_info.framebuffer.as_ref().unwrap()));
			printer::clear();

			allocator::initialize(&*boot_info.memory_regions);

			gui::initialize(core::ptr::read(boot_info.framebuffer.as_ref().unwrap()));

			pic::initialize();
			// Enabling interrupts must happen AFTER both the GDT and the IDT have been initialized
			x86_64::instructions::interrupts::enable();
			ps2::initialize();
			ps2_keyboard::initialize();

			harddisk::initialize();
		}
	}
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
	static mut ORIGINAL_MESSAGE: Option<String> = None;
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
			println!("{}", orig);

			loop {}
		}
	}
	let mut msg = String::new();
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

	let char_count = msg.chars().count();
	let mut widget = MessageBox::new("Panic!".into(), msg);

	let res = unsafe { gui::display::resolution() };

	let max_line_count = (res.x - 32) / 8;
	let mut rows = 1;
	while char_count.saturating_sub((rows - 1) * max_line_count) > max_line_count {
		rows += 1;
	}

	let max_line_length = char_count.min(max_line_count);

	let inner = Point::new(max_line_length * 8 + 16, rows * 16 + 16 + 32);
	let mut container = Container::new(widget, inner);

	unsafe {
		gui::display::add_widget(Box::new(container));
	}

	unsafe {
		gui::display::check_redraw();
	}

	loop {}
}
