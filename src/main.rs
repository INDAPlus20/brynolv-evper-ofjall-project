#![no_std]
#![no_main]
#![feature(const_fn_transmute)]
#![feature(panic_info_message)]
#![feature(abi_x86_interrupt)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]
#![feature(maybe_uninit_ref)]
#![feature(asm)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

#[macro_use]
mod printer;
mod allocator;
mod gdt;
mod harddisk;
mod idt;
mod pic;
mod ps2;
mod ps2_keyboard;
mod svec;

use alloc::format;
use core::{
	panic::PanicInfo,
	sync::atomic::{AtomicBool, Ordering},
};

use bootloader::BootInfo;
use harddisk::fat32::FatError;

use crate::{ps2_keyboard::KeyCode, svec::SVec};

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
	// No function call may precede this one, or else undefined behaviour may be invoked.
	initialize(boot_info);

	println!("Hello, World!");

	let mut path_buffer: SVec<u8, 128> = SVec::new();

	loop {
		let event = ps2_keyboard::get_key_event();
		if event.keycode == KeyCode::Enter {
			use harddisk::fat32::SplitLast;
			println!();
			match path_buffer.get_slice().split_last_2(&b' ') {
				(b"read", path) => match unsafe { harddisk::fat32::list_entries(path) } {
					Ok(e) => {
						for e in e {
							println!(
								"{:12}  {:3}  {}",
								e.name.to_str(),
								if e.is_directory { "DIR" } else { "   " },
								e.size
							);
						}
					}
					Err(FatError::IsntDirectory) => {
						let mut buffer = [0; 1024 * 2];
						match unsafe { harddisk::fat32::read_file(path, &mut buffer) } {
							Ok(v) => {
								println!("{}", core::str::from_utf8(&buffer[0..v]).unwrap());
							}
							Err(e) => println!("Error: {:#?}", e),
						}
					}
					Err(e) => {
						println!("Error: {:#?}", e)
					}
				},
				(b"create", path) => match unsafe { harddisk::fat32::create_empty_file(path) } {
					Ok(info) => println!("{:#?}", info),
					Err(e) => println!("Error: {:#?}", e),
				},
				(b"write", path) => {
					let data_to_write = include_bytes!("../file_to_write.txt");
					match unsafe { harddisk::fat32::write_file(path, data_to_write) } {
						Ok(_) => {}
						Err(e) => println!("Error: {:#?}", e),
					}
				}
				(b"test", _) => {
					for i in 0..32 {
						println!("Creating file {}", i);
						match unsafe {
							harddisk::fat32::write_file(
								format!("EFI>{}", i).as_bytes(),
								format!("File number {}\n", i).as_bytes(),
							)
						} {
							Ok(_) => {}
							Err(e) => println!("Error: {:#?}", e),
						}
					}
				}
				(other, _) => println!(
					"Unrecognized command '{}'",
					core::str::from_utf8(other).unwrap()
				),
			}
			while path_buffer.len() > 0 {
				path_buffer.pop();
			}
		} else if let Some(char) = event.char {
			let mut b = [0; 4];
			let s = char.encode_utf8(&mut b);
			for b in s.bytes() {
				path_buffer.push(b);
			}
			print!("{}", s);
		} else if event.keycode == KeyCode::Backspace {
			if path_buffer.len() > 0 {
				path_buffer.pop();
			}
			print!("\x08");
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
