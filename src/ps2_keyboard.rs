use core::{
	mem::MaybeUninit,
	ops::{Index, IndexMut},
	sync::atomic::{AtomicBool, Ordering},
};

use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

use crate::svec::SVec;

/// Initializes the PS/2 keyboard driver.
///
/// # Safety
///
/// This should not be called if another call to this function has not yet returned.
///
/// The module `ps2` must be initialized before this function is called.
pub unsafe fn initialize() {
	crate::idt::register_irq(0x20 + 1, interrupt_handler);
}

static DRIVER: Mutex<Driver> = Mutex::new(Driver::new());

static HAS_KEYEVENT_IN_BUFFER: AtomicBool = AtomicBool::new(false);

struct Driver {
	state: DriverState,
	pressed_keys: [bool; 256],
	keyevent_buffer: SVec<KeyEvent, 256>,
}

impl Driver {
	const fn new() -> Self {
		Self {
			state: DriverState::WaitingForNewKeypress,
			pressed_keys: [false; 256],
			keyevent_buffer: SVec::new(),
		}
	}

	fn handle_byte(&mut self, byte: u8) {
		match &mut self.state {
			DriverState::WaitingForNewKeypress => {
				// All bytes lower than 0xE0 are single byte scancodes
				if byte >= 0xE0 {
					let mut svec = SVec::new();
					svec.push(byte);
					self.state = DriverState::InTheMiddleOfReceivingAKeypress(svec);
				} else {
					self.handle_scancode(&mut [byte]);
				}
			}
			DriverState::InTheMiddleOfReceivingAKeypress(svec) => {
				fn handle_scancode_helper<const N: usize>(s: &mut Driver, mut svec: SVec<u8, N>, byte: u8) {
					svec.push(byte);
					s.handle_scancode(svec.get_slice_mut());
					s.state = DriverState::WaitingForNewKeypress;
				}

				match (svec.get_slice(), byte) {
					(&[0xE0], 0x2A | 0xB7) => svec.push(byte),
					(&[0xE0, _], _) => svec.push(byte),
					(&[0xE0, _, _], _) => {
						let svec = svec.clone();
						handle_scancode_helper(self, svec, byte);
					}
					(slice @ &[0xE1, ..], _) => {
						let len = slice.len();
						if len >= 5 {
							let svec = svec.clone();
							handle_scancode_helper(self, svec, byte);
						} else {
							svec.push(byte);
						}
					}
					(&[0xE0], _) => {
						let svec = svec.clone();
						handle_scancode_helper(self, svec, byte);
					}
					_ => {
						svec.push(byte);
						panic!("Unrecognized byte sequence {:#X?}", svec.get_slice());
					}
				}
			}
		}
	}

	fn handle_scancode(&mut self, scancode: &mut [u8]) {
		let was_released = match scancode {
			[b] | [0xE0, b] => {
				let was_released = *b & 0x80 != 0;
				*b &= !0x80;
				was_released
			}
			[0xE0, 0x2A, 0xE0, 0x37] => false,
			[0xE0, 0xB7, 0xE0, 0xAA] => {
				scancode[1] = 0x2A;
				scancode[3] = 0x37;
				true
			}
			_ => false,
		};

		let keycode = match scancode {
			[0x01] => KeyCode::Escape,
			[0x02] => KeyCode::Digit1,
			[0x03] => KeyCode::Digit2,
			[0x04] => KeyCode::Digit3,
			[0x05] => KeyCode::Digit4,
			[0x06] => KeyCode::Digit5,
			[0x07] => KeyCode::Digit6,
			[0x08] => KeyCode::Digit7,
			[0x09] => KeyCode::Digit8,
			[0x0A] => KeyCode::Digit9,
			[0x0B] => KeyCode::Digit0,
			[0x0C] => KeyCode::Plus,
			[0x0D] => KeyCode::Accent,
			[0x0E] => KeyCode::Backspace,
			[0x0F] => KeyCode::Tab,
			[0x10] => KeyCode::Q,
			[0x11] => KeyCode::W,
			[0x12] => KeyCode::E,
			[0x13] => KeyCode::R,
			[0x14] => KeyCode::T,
			[0x15] => KeyCode::Y,
			[0x16] => KeyCode::U,
			[0x17] => KeyCode::I,
			[0x18] => KeyCode::O,
			[0x19] => KeyCode::P,
			[0x1A] => KeyCode::Å,
			[0x1B] => KeyCode::Umlaut,
			[0x1C] => KeyCode::Enter,
			[0x1D] => KeyCode::LeftControl,
			[0x1E] => KeyCode::A,
			[0x1F] => KeyCode::S,
			[0x20] => KeyCode::D,
			[0x21] => KeyCode::F,
			[0x22] => KeyCode::G,
			[0x23] => KeyCode::H,
			[0x24] => KeyCode::J,
			[0x25] => KeyCode::K,
			[0x26] => KeyCode::L,
			[0x27] => KeyCode::Ö,
			[0x28] => KeyCode::Ä,
			[0x29] => KeyCode::Paragraph,
			[0x2A] => KeyCode::LeftShift,
			[0x2B] => KeyCode::Apostrophe,
			[0x2C] => KeyCode::Z,
			[0x2D] => KeyCode::X,
			[0x2E] => KeyCode::C,
			[0x2F] => KeyCode::V,
			[0x30] => KeyCode::B,
			[0x31] => KeyCode::N,
			[0x32] => KeyCode::M,
			[0x33] => KeyCode::Comma,
			[0x34] => KeyCode::Period,
			[0x35] => KeyCode::Dash,
			[0x36] => KeyCode::RightShift,
			[0x37] => KeyCode::NumpadMultiply,
			[0x38] => KeyCode::LeftAlt,
			[0x39] => KeyCode::Space,
			[0x3A] => KeyCode::CapsLock,
			[0x3B] => KeyCode::F1,
			[0x3C] => KeyCode::F2,
			[0x3D] => KeyCode::F3,
			[0x3E] => KeyCode::F4,
			[0x3F] => KeyCode::F5,
			[0x40] => KeyCode::F6,
			[0x41] => KeyCode::F7,
			[0x42] => KeyCode::F8,
			[0x43] => KeyCode::F9,
			[0x44] => KeyCode::F10,
			[0x45] => KeyCode::NumLock,
			[0x46] => KeyCode::ScrollLock,
			[0x47] => KeyCode::Numpad7,
			[0x48] => KeyCode::Numpad8,
			[0x49] => KeyCode::Numpad9,
			[0x4A] => KeyCode::NumbadSubtract,
			[0x4B] => KeyCode::Numpad4,
			[0x4C] => KeyCode::Numpad5,
			[0x4D] => KeyCode::Numpad6,
			[0x4E] => KeyCode::NumbadAdd,
			[0x4F] => KeyCode::Numpad1,
			[0x50] => KeyCode::Numpad2,
			[0x51] => KeyCode::Numpad3,
			[0x52] => KeyCode::Numpad0,
			[0x53] => KeyCode::NumpadDecimal,
			[0x56] => KeyCode::LessThan,
			[0x57] => KeyCode::F11,
			[0x58] => KeyCode::F12,
			[0xE0, 0x10] => KeyCode::PreviousTrack,
			[0xE0, 0x19] => KeyCode::NextTrack,
			[0xE0, 0x1C] => KeyCode::NumpadEnter,
			[0xE0, 0x1D] => KeyCode::RightControl,
			[0xE0, 0x20] => KeyCode::Mute,
			[0xE0, 0x21] => KeyCode::Calculator,
			[0xE0, 0x22] => KeyCode::PlayPause,
			[0xE0, 0x24] => KeyCode::Unknown, //Stop
			[0xE0, 0x2E] => KeyCode::VolumeDown,
			[0xE0, 0x30] => KeyCode::VolumeUp,
			[0xE0, 0x32] => KeyCode::Unknown, // WWW home
			[0xE0, 0x35] => KeyCode::NumpadDivide,
			[0xE0, 0x38] => KeyCode::AltGr,
			[0xE0, 0x47] => KeyCode::Home,
			[0xE0, 0x48] => KeyCode::Up,
			[0xE0, 0x49] => KeyCode::PageUp,
			[0xE0, 0x4B] => KeyCode::Left,
			[0xE0, 0x4D] => KeyCode::Right,
			[0xE0, 0x4F] => KeyCode::End,
			[0xE0, 0x50] => KeyCode::Down,
			[0xE0, 0x51] => KeyCode::PageDown,
			[0xE0, 0x52] => KeyCode::Insert,
			[0xE0, 0x53] => KeyCode::Delete,
			[0xE0, 0x5B] => KeyCode::LeftMeta,  //left GUI
			[0xE0, 0x5C] => KeyCode::RightMeta, //right GUI
			[0xE0, 0x5D] => KeyCode::Menu,      //"apps"
			[0xE0, 0x5E] => KeyCode::Unknown,   //Power
			[0xE0, 0x5F] => KeyCode::Unknown,   //Sleep
			[0xE0, 0x63] => KeyCode::Unknown,   //Wake
			[0xE0, 0x65] => KeyCode::Unknown,   //WWW search
			[0xE0, 0x66] => KeyCode::Unknown,   //WWW favorites
			[0xE0, 0x67] => KeyCode::Unknown,   //WWW refesh (Maybe bind to F5?)
			[0xE0, 0x68] => KeyCode::Unknown,   //WWW stop
			[0xE0, 0x69] => KeyCode::Unknown,   //WWW forward
			[0xE0, 0x6A] => KeyCode::Unknown,   //WWW back
			[0xE0, 0x6B] => KeyCode::Unknown,   //My computer
			[0xE0, 0x6C] => KeyCode::Unknown,   //email
			[0xE0, 0x6D] => KeyCode::Unknown,   //media select
			[0xE0, 0x2A, 0xE0, 0x37] => KeyCode::PrintScreen,
			[0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5] => KeyCode::PauseBreak,
			_ => panic!("Unrecognized keycode"),
		};

		let held = self.is_pressed(keycode);

		if keycode != KeyCode::PauseBreak {
			self.pressed_keys[keycode as usize] = !was_released;
		}

		if !was_released {
			let shift = self.is_pressed(KeyCode::LeftShift) || self.is_pressed(KeyCode::RightShift);
			let ctrl = self.is_pressed(KeyCode::LeftControl) || self.is_pressed(KeyCode::RightControl);
			let alt = self.is_pressed(KeyCode::LeftAlt);
			let altgr = self.is_pressed(KeyCode::AltGr);
			let meta = self.is_pressed(KeyCode::LeftMeta) || self.is_pressed(KeyCode::RightMeta);

			let modifiers = Modifiers {
				shift,
				ctrl,
				alt,
				altgr,
				meta,
			};

			let char = self.translate_keycode(keycode, modifiers);

			let keystate = if held {
				KeyState::Held
			} else {
				KeyState::Pressed
			};

			let keyevent = KeyEvent {
				keycode,
				modifiers,
				char,
				state: keystate,
			};

			self.keyevent_buffer.push(keyevent);

			HAS_KEYEVENT_IN_BUFFER.store(true, Ordering::Release);
		}
	}

	fn translate_keycode(&self, keycode: KeyCode, modifiers: Modifiers) -> Option<char> {
		const NONE: Modifiers = Modifiers::NONE;
		const SHIFT: Modifiers = Modifiers::SHIFT;
		const ALTGR: Modifiers = Modifiers::ALTGR;

		Some(match (keycode, modifiers) {
			(KeyCode::Paragraph, NONE) => '§',
			(KeyCode::Digit1, NONE) => '1',
			(KeyCode::Digit2, NONE) => '2',
			(KeyCode::Digit3, NONE) => '3',
			(KeyCode::Digit4, NONE) => '4',
			(KeyCode::Digit5, NONE) => '5',
			(KeyCode::Digit6, NONE) => '6',
			(KeyCode::Digit7, NONE) => '7',
			(KeyCode::Digit8, NONE) => '8',
			(KeyCode::Digit9, NONE) => '9',
			(KeyCode::Digit0, NONE) => '0',
			(KeyCode::Plus, NONE) => '+',
			(KeyCode::Accent, NONE) => '´',
			(KeyCode::NumpadDivide, NONE) => '/',
			(KeyCode::NumpadMultiply, NONE) => '*',
			(KeyCode::NumbadSubtract, NONE) => '-',
			(KeyCode::Tab, NONE) => '\t',
			(KeyCode::Q, NONE) => 'q',
			(KeyCode::W, NONE) => 'w',
			(KeyCode::E, NONE) => 'e',
			(KeyCode::R, NONE) => 'r',
			(KeyCode::T, NONE) => 't',
			(KeyCode::Y, NONE) => 'y',
			(KeyCode::U, NONE) => 'u',
			(KeyCode::I, NONE) => 'i',
			(KeyCode::O, NONE) => 'o',
			(KeyCode::P, NONE) => 'p',
			(KeyCode::Å, NONE) => 'å',
			(KeyCode::Umlaut, NONE) => '¨',
			(KeyCode::Enter, NONE) => '\n',
			(KeyCode::Numpad7, NONE) => '7',
			(KeyCode::Numpad8, NONE) => '8',
			(KeyCode::Numpad9, NONE) => '9',
			(KeyCode::NumbadAdd, NONE) => '+',
			(KeyCode::A, NONE) => 'a',
			(KeyCode::S, NONE) => 's',
			(KeyCode::D, NONE) => 'd',
			(KeyCode::F, NONE) => 'f',
			(KeyCode::G, NONE) => 'g',
			(KeyCode::H, NONE) => 'h',
			(KeyCode::J, NONE) => 'j',
			(KeyCode::K, NONE) => 'k',
			(KeyCode::L, NONE) => 'l',
			(KeyCode::Ö, NONE) => 'ö',
			(KeyCode::Ä, NONE) => 'ä',
			(KeyCode::Apostrophe, NONE) => '\'',
			(KeyCode::Numpad4, NONE) => '4',
			(KeyCode::Numpad5, NONE) => '5',
			(KeyCode::Numpad6, NONE) => '6',
			(KeyCode::LessThan, NONE) => '<',
			(KeyCode::Z, NONE) => 'z',
			(KeyCode::X, NONE) => 'x',
			(KeyCode::C, NONE) => 'c',
			(KeyCode::V, NONE) => 'v',
			(KeyCode::B, NONE) => 'b',
			(KeyCode::N, NONE) => 'n',
			(KeyCode::M, NONE) => 'm',
			(KeyCode::Comma, NONE) => ',',
			(KeyCode::Period, NONE) => '.',
			(KeyCode::Dash, NONE) => '-',
			(KeyCode::Numpad1, NONE) => '1',
			(KeyCode::Numpad2, NONE) => '2',
			(KeyCode::Numpad3, NONE) => '3',
			(KeyCode::NumpadEnter, NONE) => '\n',
			(KeyCode::Space, NONE) => ' ',
			(KeyCode::Numpad0, NONE) => '0',
			(KeyCode::NumpadDecimal, NONE) => '.',

			(KeyCode::Paragraph, SHIFT) => '½',
			(KeyCode::Digit1, SHIFT) => '!',
			(KeyCode::Digit2, SHIFT) => '"',
			(KeyCode::Digit3, SHIFT) => '#',
			(KeyCode::Digit4, SHIFT) => '¤',
			(KeyCode::Digit5, SHIFT) => '%',
			(KeyCode::Digit6, SHIFT) => '&',
			(KeyCode::Digit7, SHIFT) => '/',
			(KeyCode::Digit8, SHIFT) => '(',
			(KeyCode::Digit9, SHIFT) => ')',
			(KeyCode::Digit0, SHIFT) => '=',
			(KeyCode::Plus, SHIFT) => '?',
			(KeyCode::Accent, SHIFT) => '`',
			(KeyCode::NumpadDivide, SHIFT) => '/',
			(KeyCode::NumpadMultiply, SHIFT) => '*',
			(KeyCode::NumbadSubtract, SHIFT) => '-',
			(KeyCode::Tab, SHIFT) => '\t',
			(KeyCode::Q, SHIFT) => 'Q',
			(KeyCode::W, SHIFT) => 'W',
			(KeyCode::E, SHIFT) => 'E',
			(KeyCode::R, SHIFT) => 'R',
			(KeyCode::T, SHIFT) => 'T',
			(KeyCode::Y, SHIFT) => 'Y',
			(KeyCode::U, SHIFT) => 'U',
			(KeyCode::I, SHIFT) => 'I',
			(KeyCode::O, SHIFT) => 'O',
			(KeyCode::P, SHIFT) => 'P',
			(KeyCode::Å, SHIFT) => 'Å',
			(KeyCode::Umlaut, SHIFT) => '^',
			(KeyCode::Enter, SHIFT) => '\n',
			(KeyCode::NumbadAdd, SHIFT) => '+',
			(KeyCode::A, SHIFT) => 'A',
			(KeyCode::S, SHIFT) => 'S',
			(KeyCode::D, SHIFT) => 'D',
			(KeyCode::F, SHIFT) => 'F',
			(KeyCode::G, SHIFT) => 'G',
			(KeyCode::H, SHIFT) => 'H',
			(KeyCode::J, SHIFT) => 'J',
			(KeyCode::K, SHIFT) => 'K',
			(KeyCode::L, SHIFT) => 'L',
			(KeyCode::Ö, SHIFT) => 'Ö',
			(KeyCode::Ä, SHIFT) => 'Ä',
			(KeyCode::Apostrophe, SHIFT) => '*',
			(KeyCode::LessThan, SHIFT) => '>',
			(KeyCode::Z, SHIFT) => 'Z',
			(KeyCode::X, SHIFT) => 'X',
			(KeyCode::C, SHIFT) => 'C',
			(KeyCode::V, SHIFT) => 'V',
			(KeyCode::B, SHIFT) => 'B',
			(KeyCode::N, SHIFT) => 'N',
			(KeyCode::M, SHIFT) => 'M',
			(KeyCode::Comma, SHIFT) => ';',
			(KeyCode::Period, SHIFT) => ':',
			(KeyCode::Dash, SHIFT) => '_',
			(KeyCode::NumpadEnter, SHIFT) => '\n',
			(KeyCode::Space, SHIFT) => ' ',

			(KeyCode::Digit2, ALTGR) => '@',
			(KeyCode::Digit3, ALTGR) => '£',
			(KeyCode::Digit4, ALTGR) => '$',
			(KeyCode::Digit5, ALTGR) => '€',
			(KeyCode::Digit7, ALTGR) => '{',
			(KeyCode::Digit8, ALTGR) => '[',
			(KeyCode::Digit9, ALTGR) => ']',
			(KeyCode::Digit0, ALTGR) => '}',
			(KeyCode::Plus, ALTGR) => '\\',
			(KeyCode::E, ALTGR) => '€',
			(KeyCode::Umlaut, ALTGR) => '~',
			(KeyCode::LessThan, ALTGR) => '|',
			(KeyCode::M, ALTGR) => 'µ',

			_ => return None,
		})
	}

	fn is_pressed(&self, keycode: KeyCode) -> bool {
		self.pressed_keys[keycode as usize]
	}
}

enum DriverState {
	WaitingForNewKeypress,
	InTheMiddleOfReceivingAKeypress(SVec<u8, 6>),
}

pub struct KeyEvent {
	pub keycode: KeyCode,
	pub modifiers: Modifiers,
	pub char: Option<char>,
	pub state: KeyState,
}

// TODO: Add explicit discriminant values
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum KeyCode {
	Unknown,

	Escape,
	F1,
	F2,
	F3,
	F4,
	F5,
	F6,
	F7,
	F8,
	F9,
	F10,
	F11,
	F12,
	PrintScreen,
	ScrollLock,
	PauseBreak,

	Paragraph,
	Digit1,
	Digit2,
	Digit3,
	Digit4,
	Digit5,
	Digit6,
	Digit7,
	Digit8,
	Digit9,
	Digit0,
	Plus,
	Accent,
	Backspace,
	Insert,
	Home,
	PageUp,
	NumLock,
	NumpadDivide,
	NumpadMultiply,
	NumbadSubtract,
	Tab,
	Q,
	W,
	E,
	R,
	T,
	Y,
	U,
	I,
	O,
	P,
	Å,
	Umlaut,
	Enter,
	Delete,
	End,
	PageDown,
	Numpad7,
	Numpad8,
	Numpad9,
	NumbadAdd,

	CapsLock,
	A,
	S,
	D,
	F,
	G,
	H,
	J,
	K,
	L,
	Ö,
	Ä,
	Apostrophe,
	Numpad4,
	Numpad5,
	Numpad6,

	LeftShift,
	LessThan,
	Z,
	X,
	C,
	V,
	B,
	N,
	M,
	Comma,
	Period,
	Dash,
	RightShift,
	Up,
	Numpad1,
	Numpad2,
	Numpad3,
	NumpadEnter,

	LeftControl,
	LeftMeta,
	LeftAlt,
	Space,
	AltGr,
	RightMeta,
	Menu,
	RightControl,
	Left,
	Down,
	Right,
	Numpad0,
	NumpadDecimal,

	VolumeUp,
	VolumeDown,
	PlayPause,
	Calculator,
	PreviousTrack,
	NextTrack,
	Mute,
}

impl KeyCode {
	fn write<W: core::fmt::Write>(&self, w: &mut W) {
		match self {
			Self::Å => write!(w, "AO"),
			Self::Ä => write!(w, "AE"),
			Self::Ö => write!(w, "OE"),
			_ => write!(w, "{:?}", self),
		}
		.unwrap()
	}
}

pub enum KeyState {
	Pressed,
	Held,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Modifiers {
	shift: bool,
	alt: bool,
	altgr: bool,
	ctrl: bool,
	meta: bool,
}

impl Modifiers {
	const ALT: Self = Self {
		shift: false,
		alt: true,
		altgr: false,
		ctrl: false,
		meta: false,
	};
	const ALTGR: Self = Self {
		shift: false,
		alt: false,
		altgr: true,
		ctrl: false,
		meta: false,
	};
	const CTRL: Self = Self {
		shift: false,
		alt: false,
		altgr: false,
		ctrl: true,
		meta: false,
	};
	const META: Self = Self {
		shift: false,
		alt: false,
		altgr: false,
		ctrl: false,
		meta: true,
	};
	const NONE: Self = Self {
		shift: false,
		alt: false,
		altgr: false,
		ctrl: false,
		meta: false,
	};
	const SHIFT: Self = Self {
		shift: true,
		alt: false,
		altgr: false,
		ctrl: false,
		meta: false,
	};
}

/// Be careful of deadlocks when calling this function from an interrupt handler
pub fn get_key_event() -> KeyEvent {
	while HAS_KEYEVENT_IN_BUFFER.load(Ordering::Acquire) == false {}

	let mut driver = DRIVER.lock();
	let ret = driver.keyevent_buffer.remove(0);
	if driver.keyevent_buffer.len() == 0 {
		HAS_KEYEVENT_IN_BUFFER.store(false, Ordering::Release);
	}
	ret
}

extern "x86-interrupt" fn interrupt_handler(_: InterruptStackFrame) {
	let byte = unsafe { crate::ps2::get_byte() };

	DRIVER
		.try_lock()
		.expect("PS/2 driver deadlock")
		.handle_byte(byte);

	unsafe { crate::pic::send_eoi(1) };
}
