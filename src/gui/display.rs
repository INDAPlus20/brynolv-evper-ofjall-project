use alloc::{boxed::Box, vec::Vec};
use core::{
	fmt::Write,
	mem::{zeroed, MaybeUninit},
	slice,
};

use bootloader::boot_info::FrameBuffer;
use x86_64::structures::paging::frame;

use super::widget::{Event, Widget};

macro_rules! zeroed {
	($t:ty) => {
		core::mem::transmute([0u8; core::mem::size_of::<$t>()])
	};
}

const DEFAULT_FONT: Font = Font::from(unsafe {
	core::mem::transmute::<_, [[[u8; 8]; 16]; 128]>(*include_bytes!("../vgafont.bin"))
});

pub struct Font {
	glyphs: [Glyph; 128],
}

impl const From<[[[u8; 8]; 16]; 128]> for Font {
	fn from(array: [[[u8; 8]; 16]; 128]) -> Self {
		let mut glyphs = [Glyph::EMPTY; 128];
		let mut i = 0;
		loop {
			glyphs[i] = Glyph::from(array[i]);
			i += 1;
			if i >= 128 {
				break;
			}
		}
		Self { glyphs }
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Glyph([[u8; 8]; 16]);

impl Glyph {
	const EMPTY: Self = Self([[0; 8]; 16]);
}

impl const From<[[u8; 8]; 16]> for Glyph {
	fn from(array: [[u8; 8]; 16]) -> Self {
		Self(array)
	}
}

pub struct Window<'a> {
	buffer: &'a mut [u32],
	buffer_width: usize,
	buffer_height: usize,
	rect: Rect,
}

impl<'a> Window<'a> {
	pub fn subwindow<'b, 'c>(&'c mut self, rect: Rect) -> Window<'b>
	where
		'a: 'b,
		'c: 'b, {
		assert!(rect.x + rect.width <= self.rect.width);
		assert!(rect.y + rect.height <= self.rect.height);
		Self {
			rect: Rect {
				x: self.rect.x + rect.x,
				y: self.rect.y + rect.y,
				width: rect.width,
				height: rect.height,
			},
			buffer_width: self.buffer_width,
			buffer_height: self.buffer_height,
			buffer: unsafe {
				let ptr = self.buffer as *mut [u32];
				&mut *ptr
			},
		}
	}

	pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
		assert!(x < self.rect.width);
		assert!(y < self.rect.height);
		let x = self.rect.x + x;
		let y = self.rect.y + y;
		self.buffer[y * self.buffer_width + x] = color.to_bgr();
	}

	pub fn get_pixel(&mut self, x: usize, y: usize) -> u32 {
		assert!(x < self.rect.width);
		assert!(y < self.rect.height);
		let x = self.rect.x + x;
		let y = self.rect.y + y;
		self.buffer[y * self.buffer_width + x]
	}

	pub fn draw_rect(&mut self, rect: Rect, color: Color) {
		if rect.is_empty() {
			return;
		}
		assert!(rect.x + rect.width <= self.rect.width);
		assert!(rect.y + rect.height <= self.rect.height);

		for y in rect.y..rect.y + rect.height {
			for x in rect.x..rect.x + rect.width {
				self.set_pixel(x, y, color);
			}
		}
	}

	pub fn draw_char(
		&mut self,
		pos: Point,
		scale: usize,
		mut char: char,
		foreground: Color,
		background: Color,
		font: Option<&Font>,
	) {
		assert!(pos.x + 8 * scale <= self.rect.width);
		assert!(pos.y + 16 * scale <= self.rect.height);

		if char > 0x7F as char {
			char = 0x7F as char;
		}

		let font = font.unwrap_or(&DEFAULT_FONT);
		let glyph = font.glyphs[char as usize];

		for y in 0..16 * scale {
			for x in 0..8 * scale {
				let cx = x / scale;
				let cy = y / scale;
				// let weight = glyph.0[cy][cx] as f64 / 255.0;
				// let bg = background * (1.0 - weight);
				// let fg = foreground * weight;
				// let color = fg + bg;
				let color = if glyph.0[cy][cx] > 0xFF / 2 {
					foreground
				} else {
					background
				};

				self.set_pixel(x + pos.x, y + pos.y, color);
			}
		}
	}

    pub fn draw_string(&mut self, rect: Rect, scale: usize, wrap: bool, align: Align, string: &str, foreground: Color, background: Color, font: Option<&Font>) {
        assert!(rect.x + rect.width <= self.rect.width);
        assert!(rect.y + rect.height <= self.rect.height);

        match (align, wrap) {
            (Align::Left, true) => {
                let mut y = rect.y;
                let mut chars = string.chars().peekable();
                while chars.peek().is_some() {
                    if y + 16 * scale > rect.y + rect.height {
                        break;
                    }
                    let mut x = rect.x;
                    while let Some(c) = chars.next() {
                        if x + 8 * scale > rect.x + rect.width {
                            break;
                        }
                        self.draw_char(Point::new(x, y), scale, c, foreground, background, font);
                        x += 8 * scale;
                    }
                    y += 16 * scale;
                }
            }
            (Align::Left, false) => {
                let mut x = rect.x;
                for c in string.chars() {
                    if x + 8 * scale > rect.x + rect.width {
                        break;
                    }
                    self.draw_char(Point::new(x, rect.y), scale, c, foreground, background, font);
                    x += 8 * scale;
                }
            }
            (Align::Center, true) => todo!(),
            (Align::Center, false) => {
                let char_count = string.chars().count();
                let mut x = rect.x + rect.width / 2;
                let mut skip = 0;
                while x < rect.x {
                    x += 8;
                    skip += 1;
                }

                for c in string.chars() {
                    if x + 8 * scale > rect.x + rect.width {
                        break;
                    }
                    self.draw_char(Point::new(x, rect.y), scale, c, foreground, background, font);
                    x += 8 * scale;
                }
            }
            (Align::Right, true) => todo!(),
            (Align::Right, false) => todo!()
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Color {
	pub red: u8,
	pub green: u8,
	pub blue: u8,
}

impl Color {
	pub const BLACK: Self = Self::new(0, 0, 0);
	pub const WHITE: Self = Self::new(0xFF, 0xFF, 0xFF);

	pub const fn new(red: u8, green: u8, blue: u8) -> Self {
		Self { red, green, blue }
	}

	pub const fn grayscale(data: u8) -> Self {
		Self::new(data, data, data)
	}

	pub const fn to_bgr(&self) -> u32 {
		(self.red as u32) << 16 | (self.green as u32) << 8 | (self.blue as u32) << 0
	}
}

impl core::ops::Mul<f64> for Color {
	type Output = Color;

	fn mul(self, rhs: f64) -> Self::Output {
		Self {
			red: (self.red as f64 * rhs) as u8,
			green: (self.green as f64 * rhs) as u8,
			blue: (self.blue as f64 * rhs) as u8,
		}
	}
}

impl core::ops::Add for Color {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self {
			red: self.red + rhs.red,
			green: self.green + rhs.green,
			blue: self.blue + rhs.blue,
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Point {
	pub x: usize,
	pub y: usize,
}

impl Point {
	pub const fn new(x: usize, y: usize) -> Self {
		Self { x, y }
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rect {
	pub x: usize,
	pub y: usize,
	pub width: usize,
	pub height: usize,
}

impl Rect {
	pub const EMPTY: Self = Self::new(0, 0, 0, 0);

	pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}

	pub const fn is_empty(&self) -> bool {
		self.width == 0 || self.height == 0
	}

	pub const fn contains(&self, point: Point) -> bool {
		point.x >= self.x
			&& point.x < self.x + self.width
			&& point.y >= self.y
			&& point.y < self.y + self.height
	}

	pub const fn smallest_containing(a: Rect, b: Rect) -> Rect {
		if a.is_empty() {
			return b;
		}
		if b.is_empty() {
			return a;
		}
		const fn min(a: usize, b: usize) -> usize {
			if a < b { a } else { b }
		}
		const fn max(a: usize, b: usize) -> usize {
			if a > b { a } else { b }
		}

		let left = min(a.x, b.x);
		let top = min(a.y, b.y);
		let right = max(a.x + a.width, b.x + b.width);
		let bottom = max(a.y + a.height, b.y + b.height);
		Rect::new(left, top, right - left, bottom - top)
	}

	pub const fn intersection(a: Rect, b: Rect) -> Rect {
		const fn min(a: usize, b: usize) -> usize {
			if a < b { a } else { b }
		}
		const fn max(a: usize, b: usize) -> usize {
			if a > b { a } else { b }
		}

		let left = max(a.x, b.x);
		let right = min(a.x + a.width, b.x + b.width);
		let top = max(a.y, b.y);
		let bottom = min(a.y + a.height, b.y + b.height);

		let width = right.saturating_sub(left);
		let height = bottom.saturating_sub(top);
		if height == 0 || width == 0 {
			Rect::new(0, 0, 0, 0)
		} else {
			Rect::new(left, top, width, height)
		}
	}
}

static mut DISPLAY: Display = unsafe { Display::uinitialized() };

/// The engine of the GUI system.
struct Display {
	framebuffer: FrameBuffer,
	widgets: Vec<Box<dyn Widget>>,
}

impl Display {
	/// Create an uninitialized instance of Display.
	///
	/// # Safety
	///
	/// `initialize` MUST be called on the returned value before any other method.
	/// Failure to do so may invoke undefined behaviour.
	pub const unsafe fn uinitialized() -> Self {
		Self {
			framebuffer: zeroed!(FrameBuffer),
			widgets: Vec::new(),
		}
	}

	/// Adds a widget to the end of the widget list.
	///
	/// # Panics
	///
	/// Panics if the widget list is full.
	pub fn add_widget(&mut self, mut widget: Box<dyn Widget>) {
		let info = self.framebuffer.info();
		let res = Point::new(info.horizontal_resolution, info.vertical_resolution);
		widget.set_size(res);
		let area = widget.used_area();
		widget.invalidate(area);
		self.widgets.push(widget);
		self.check_redraw();
	}

	/// Sends an event to the widgets.
	///
	/// The event is passed through the widgets from the top down, and will continue to
	/// be passed through until a widget responds with a response other than `Response::NotHandled`.
	pub fn send_event(&mut self, event: Event) {
		for (i, widget) in self.widgets.iter_mut().enumerate().rev() {
			match widget.on_event(event.clone()) {
				super::widget::Response::NotHandled => {}
				super::widget::Response::Nothing => break,
				super::widget::Response::RemoveMe => {
					let area = widget.used_area();
					self.widgets.remove(i);
					// As we removed a widget, the widget below might need to redraw (if there is one).
					for widget_index in 0..i {
						self.widgets[widget_index].invalidate(area);
					}
					break;
				}
			}
		}

		self.check_redraw();
	}

	/// Redraws if any widget is marked dirty.
	pub fn check_redraw(&mut self) {
		if self.widgets.iter().any(|w| w.dirty()) {
			self.draw();
		}
	}

	/// Draw the widgets to the screen.
	fn draw(&mut self) {
		for i in 0..self.widgets.len() {
			let window = (&mut self.framebuffer).into();
			self.widgets[i].draw(window);
		}
	}

	/// Invalidates all widgets and starts drawing them.
	pub fn force_redraw(&mut self) {
		for widget in &mut self.widgets {
			let area = widget.used_area();
			widget.invalidate(area);
		}
		self.clear();
		self.draw()
	}

	/// Clear the screen;
	fn clear(&mut self) {
		let mut window: Window = (&mut self.framebuffer).into();
		let rect = window.rect;
		window.draw_rect(rect, Color::new(0, 0, 0));
	}
}

impl<'a> From<&'a mut FrameBuffer> for Window<'a> {
	fn from(framebuffer: &'a mut FrameBuffer) -> Self {
		Self {
			rect: Rect {
				x: 0,
				y: 0,
				width: framebuffer.info().horizontal_resolution,
				height: framebuffer.info().vertical_resolution,
			},
			buffer_width: framebuffer.info().horizontal_resolution,
			buffer_height: framebuffer.info().vertical_resolution,
			buffer: {
				let ptr = framebuffer.buffer_mut().as_mut_ptr() as _;
				let len = framebuffer.buffer_mut().len() / 4;
				unsafe { slice::from_raw_parts_mut(ptr, len) }
			},
		}
	}
}

pub(super) unsafe fn initialize(framebuffer: FrameBuffer) {
	let cw = framebuffer.info().horizontal_resolution / 8;
	let ch = framebuffer.info().vertical_resolution / 16;
	DISPLAY.framebuffer = framebuffer;
	DISPLAY.widgets.clear();
}

pub unsafe fn add_widget(widget: Box<dyn Widget>) {
	DISPLAY.add_widget(widget)
}

pub unsafe fn send_event(event: Event) {
	DISPLAY.send_event(event)
}

pub unsafe fn force_redraw() {
	DISPLAY.force_redraw()
}

pub unsafe fn check_redraw() {
	DISPLAY.check_redraw();
}

pub unsafe fn resolution() -> Point {
	Point::new(
		DISPLAY.framebuffer.info().horizontal_resolution,
		DISPLAY.framebuffer.info().vertical_resolution,
	)
}

// #[macro_export]
// macro_rules! print {
//     ($($arg:tt)*) => ($crate::gui::display::_print(format_args!($($arg)*)));
// }

// #[macro_export]
// macro_rules! println {
//     () => ($crate::print!("\n"));
//     ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
// }

// #[doc(hidden)]
// pub fn _print(args: core::fmt::Arguments) {
//     unsafe {
//         // PRINTER_WIDGET.write_fmt(args).unwrap();
//     }
// }
