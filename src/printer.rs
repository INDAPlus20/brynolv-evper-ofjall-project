use bootloader::boot_info::FrameBuffer;

use crate::svec::SVec;

/// A glyph or character is 8*16 pixels
type Glyph = [[u8; 8]; 16];

/// Monospace pixelfont made by @Elekrisk
const DEFAULT_FONT: [Glyph; 128] = unsafe { core::mem::transmute(*include_bytes!("vgafont.bin")) };

/// Zeroed glyph
const EMPTY_GLYPH: Glyph = [[0; 8]; 16];

/// Cursor
const CURSOR_GLYPH: Glyph = [
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0; 8],
	[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00],
	[0; 8],
	[0; 8],
	[0; 8],
];

static mut PRINTER: Printer = unsafe { Printer::uninitialized() };

pub struct Printer {
	framebuffer: FrameBuffer,
	cursor: (usize, usize),
	font: &'static [Glyph; 128],
	initialized: bool,
	line_lengths: SVec<usize, 128>,
}

impl Printer {
	const unsafe fn uninitialized() -> Self {
		Self {
			framebuffer: core::mem::transmute([0; 16]),
			cursor: (0, 0),
			font: &DEFAULT_FONT,
			initialized: false,
			line_lengths: SVec::new(),
		}
	}

	/// Clears the screen by setting every byte to zero.
	unsafe fn clear(&mut self) {
		let (res_x, res_y, stride, bytes_per_pixel, buffer) = self.get_buffer_info();
		for y in 0..res_y {
			for x in 0..res_x {
				let i = (y * stride + x) * bytes_per_pixel;
				for b in 0..bytes_per_pixel {
					buffer[i + b] = 0;
				}
			}
		}
		self.line_lengths = SVec::new();
		self.cursor = (0, 0);
	}

	/// Replaces glyph at position with provided glyph
	unsafe fn replace_glyph_at_position(&mut self, glyph: Glyph, position: (usize, usize)) {
		let (_, _, _, bytes_per_pixel, buffer) = self.get_buffer_info();
		for y in 0..16 {
			for x in 0..8 {
				let color = glyph[y][x];
				for b in 0..bytes_per_pixel {
					buffer[buffer_offset_to_glyph_position(x, y, position) + b] = color;
				}
			}
		}
	}

	/// Gets the glyph at position
	unsafe fn get_glyph_at_position(&mut self, position: (usize, usize)) -> Glyph {
		let mut glyph: Glyph = EMPTY_GLYPH;
		let (_, _, _, _, buffer) = self.get_buffer_info();
		for y in 0..16 {
			for x in 0..8 {
				// Since it's all gray-scale, no need to check the individual bytes.
				// TODO: Actually check individual bytes if we start doing non gray-scale.
				glyph[y][x] = buffer[buffer_offset_to_glyph_position(x, y, position)];
			}
		}
		return glyph;
	}

	/// Prints a single ASCII character at the current cursor position.
	fn print_char(&mut self, mut char: char) {
		if char as u32 > 0x7F {
			char = 0x7F as char;
		}
		let glyph = self.font[char as usize];
		let (mut cursor_x, mut cursor_y) = self.cursor;
		match char {
			'\n' => {
				unsafe {
					self.replace_glyph_at_position(EMPTY_GLYPH, (cursor_x, cursor_y));
				}
				self.line_lengths.push(cursor_x);
				cursor_y += 1;
				cursor_x = 0;
			}
			'\x08' => {
				unsafe {
					self.replace_glyph_at_position(EMPTY_GLYPH, (cursor_x, cursor_y));
				}
				if cursor_x > 0 {
					cursor_x -= 1;
					unsafe {
						self.replace_glyph_at_position(EMPTY_GLYPH, (cursor_x, cursor_y));
					}
				} else {
					if cursor_y > 0 {
						cursor_y -= 1;
						cursor_x = self.line_lengths.remove(cursor_y);
						let chars_per_line = self.framebuffer.info().horizontal_resolution / 8;
						if cursor_x >= chars_per_line {
							cursor_x -= 1;
							unsafe {
								self.replace_glyph_at_position(EMPTY_GLYPH, (cursor_x, cursor_y));
							}
						}
					}
				}
			}
			other if other < ' ' => {}
			_ => {
				unsafe { self.replace_glyph_at_position(glyph, (cursor_x, cursor_y)) }
				let chars_per_line = self.framebuffer.info().horizontal_resolution / 8;
				cursor_x += 1;
				if cursor_x >= chars_per_line {
					self.line_lengths.push(cursor_x);
					cursor_y += 1;
					cursor_x = 0;
				}
			}
		}
		let line_count = self.framebuffer.info().vertical_resolution / 16;
		if cursor_y >= line_count {
			self.scroll_down();
			cursor_y -= 1;
		}

		unsafe {
			self.replace_glyph_at_position(CURSOR_GLYPH, (cursor_x, cursor_y));
		}
		self.cursor = (cursor_x, cursor_y);
	}

	/// Scrolls down the screen one text row.
	///
	/// TODO: remember offscreen lines for later retrival.
	fn scroll_down(&mut self) {
		let (res_x, res_y, stride, bytes_per_pixel, buffer) = self.get_buffer_info();
		for y in 16..res_y {
			for x in 0..res_x {
				for b in 0..bytes_per_pixel {
					let value = buffer[(y * stride + x) * bytes_per_pixel + b];
					buffer[((y - 16) * stride + x) * bytes_per_pixel + b] = value;
				}
			}
		}
		for y in res_y - 16..res_y {
			for x in 0..res_x {
				for b in 0..bytes_per_pixel {
					buffer[(y * stride + x) * bytes_per_pixel + b] = 0;
				}
			}
		}
		self.line_lengths.remove(0);
	}

	/// Returns (x, y, stride, bytes_per_pixel, buffer)
	fn get_buffer_info(&mut self) -> (usize, usize, usize, usize, &mut [u8]) {
		(
			self.framebuffer.info().horizontal_resolution,
			self.framebuffer.info().vertical_resolution,
			self.framebuffer.info().stride,
			self.framebuffer.info().bytes_per_pixel,
			self.framebuffer.buffer_mut(),
		)
	}
}

impl core::fmt::Write for Printer {
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
		unsafe {
			print_str(s);
		}
		Ok(())
	}
}

/// The offset (index) of the buffer to get to the glyph at position.
///
/// Example:
/// ```
/// let position = (pos_x,pos_y);
/// for y in 0..16 {
///    for x in 0..8 {
///        let color = /*...*/;
///        for b in 0..bytes_per_pixel {
///            buffer[buffer_offset_to_glyph_position(x, y, position)+b] = color;
///        }
///    }
/// }
/// ```
unsafe fn buffer_offset_to_glyph_position(x: usize, y: usize, position: (usize, usize)) -> usize {
	let (_, _, stride, bytes_per_pixel, _) = PRINTER.get_buffer_info();
	let (pos_x, pos_y) = position;
	((y + pos_y * 16) * stride + (x + pos_x * 8)) * bytes_per_pixel
}

/// Initializes the printer.
///
/// # Safety
///
/// `framebuffer` must be a valid framebuffer.
/// This should not be called if another call to this function has not yet returned.
///
/// Any panics before this is initialized will trigger a processor reset.
/// To avoid this, this function should be called as early as possible.
pub unsafe fn initialize(framebuffer: FrameBuffer) {
	if PRINTER.initialized {
		panic!("PRINTER already initialized!");
	}
	PRINTER.framebuffer = framebuffer;
	PRINTER.initialized = true;
}

/// Clears the screen by setting every byte in the buffer to 0 and resets the cursor.
pub unsafe fn clear() {
	if !PRINTER.initialized {
		panic!("PRINTER not initialized!");
	}
	PRINTER.clear();
}

/// Use `print!()` macro or `print_str` instead.
/*pub unsafe fn print_char(char: char) {
		PRINTER.print_char(char);
}*/

/// Prints the input string (assuming ASCII)
pub unsafe fn print_str(string: &str) {
	if !PRINTER.initialized {
		panic!("PRINTER not initialized!");
	}
	for char in string.chars() {
		PRINTER.print_char(char);
	}
}

/// Scrolls entire screen down one text row.
///
/// **WARNING** rows going offscreen are gone from memory.
pub unsafe fn scroll_down() {
	if !PRINTER.initialized {
		panic!("PRINTER not initialized!");
	}
	PRINTER.scroll_down();
	PRINTER.cursor.1 -= 1;
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::printer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
	use core::fmt::Write;
	unsafe {
		PRINTER.write_fmt(args).unwrap();
	}
}
