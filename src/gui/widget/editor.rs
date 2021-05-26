use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt::Write;

use super::{
	super::display::Color,
	file_dialog::{OpenDialog, SaveDialog},
	Event, KeyEvent, Response, Widget,
};
use crate::{
	gui::display::{self, Point, Rect, Window},
	harddisk,
	ps2_keyboard::{KeyCode, Modifiers},
};

/// A widget that supports multi-line text editing.
pub struct Editor {
	/// The width in chars
	width: usize,
	/// The height in chars
	height: usize,
	char_buffer: Vec<char>,
	// line_breaks: SVec<usize, 4096>,
	/// How many lines the printer has scrolled down
	scroll: usize,
	logical_cursor: usize,
	graphical_cursor: Point,
	/// The index of the char which is at the top left of the screen.
	/// Can point to the index after the last character in the buffer.
	top_row_char_index: usize,
	dirty: bool,
	invalidated: Rect,
	// Holds paths for current open file
	current_file_dir_path: Vec<u8>,
	current_file_name: Vec<u8>,
}

impl Editor {
	/// Creates an instance which must be initialied before use.
	pub const fn new() -> Self {
		Self {
			width: 0,
			height: 0,
			char_buffer: Vec::new(),
			scroll: 0,
			logical_cursor: 0,
			graphical_cursor: Point::new(0, 0),
			top_row_char_index: 0,
			dirty: false,
			invalidated: Rect::new(0, 0, 0, 0),
			current_file_dir_path: Vec::new(),
			current_file_name: Vec::new(),
		}
	}

	/// Get's the index of the next newline, `index` included.
	fn get_next_newline(&self, index: usize) -> Option<usize> {
		for i in index..self.char_buffer.len() {
			if self.char_buffer[i] == '\n' {
				return Some(i);
			}
		}
		None
	}

	/// Get's the index of the previous newline, not including `index`.
	fn get_prev_newline(&self, index: usize) -> Option<usize> {
		for i in (0..index).rev() {
			if self.char_buffer[i] == '\n' {
				return Some(i);
			}
		}
		None
	}

	/// Inserts a char at the current logical cursor.
	///
	/// This updates both the logical and the graphical cursors, as well as
	/// invalidates the regions changed.
	fn insert_char(&mut self, char: char) {
		self.invalidate(Rect::new(
			0,
			(self.graphical_cursor.y - self.scroll) * 16,
			self.width * 8,
			(self.height + self.scroll - self.graphical_cursor.y) * 16,
		));
		self.char_buffer.insert(self.logical_cursor, char);
		if char == '\n' {
			self.graphical_cursor.x = 0;
			self.graphical_cursor.y += 1;
		} else {
			self.graphical_cursor.x += 1;
		}
		self.logical_cursor += 1;

		if self.graphical_cursor.x >= self.width {
			self.graphical_cursor.x = 0;
			self.graphical_cursor.y += 1;
		}
		if self.graphical_cursor.y >= self.height + self.scroll {
			self.scroll_down();
		}
	}

	/// Deletes the character at the current logical cursor, as well as invalidates the regions changed.
	fn delete_char(&mut self) {
		self.invalidate(Rect::new(
			0,
			(self.graphical_cursor.y - self.scroll) * 16,
			self.width * 8,
			(self.height + self.scroll - self.graphical_cursor.y) * 16,
		));
		self.char_buffer.remove(self.logical_cursor);
	}

	/// Scrolls the view down one row.
	/// Updates `self.top_row_char_index` to always keep it up to date.
	fn scroll_down(&mut self) {
		self.invalidate(self.used_area());

		self.scroll += 1;

		let next_nl_index = self
			.get_next_newline(self.top_row_char_index)
			.unwrap_or(self.char_buffer.len());
		if next_nl_index - self.top_row_char_index < self.width {
			self.top_row_char_index = next_nl_index + 1;
		} else {
			self.top_row_char_index += self.width;
		}
	}

	/// Scrolls the view up one row.
	/// Updates `self.top_row_char_index` to always keep it up to date.
	fn scroll_up(&mut self) {
		assert!(self.scroll > 0);
		self.invalidate(self.used_area());

		self.scroll -= 1;

		if self.char_buffer[self.top_row_char_index - 1] == '\n' {
			let nl_index = self.top_row_char_index - 1;
			let prev_nl_index = self.get_prev_newline(nl_index);

			let line_start_index = if let Some(prev_nl_index) = prev_nl_index {
				prev_nl_index + 1
			} else {
				0
			};
			let start_index = nl_index - (nl_index - line_start_index) % self.width;
			self.top_row_char_index = start_index;
		} else {
			self.top_row_char_index -= self.width;
		}
	}

	/// Moves the cursor one character to the left.
	/// Updates both the logical and the graphical cursors.
	/// Handles scrolling to keep the cursor in view.
	fn cursor_left(&mut self) {
		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
		assert!(self.logical_cursor > 0);
		if self.graphical_cursor.x == 0 {
			if self.graphical_cursor.y == 0 {
				unreachable!()
			} else {
				if self.char_buffer[self.logical_cursor - 1] == '\n' {
					let prev_nl = self.get_prev_newline(self.logical_cursor - 1);

					let first_on_line = if let Some(nl) = prev_nl { nl + 1 } else { 0 };

					let line_length = self.logical_cursor - first_on_line;
					let line_length = line_length % self.width - 1;

					self.graphical_cursor.x = line_length;
					self.graphical_cursor.y -= 1;
				} else {
					self.graphical_cursor.x = self.width - 1;
					self.graphical_cursor.y -= 1;
				}
			}
		} else {
			self.graphical_cursor.x -= 1;
		}

		if self.graphical_cursor.y < self.scroll {
			self.scroll_up()
		}

		self.logical_cursor -= 1;

		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
	}

	/// Moves the cursor one character to the right.
	/// Updates both the logical and the graphical cursors.
	/// Handles scrolling to keep the cursor in view.
	fn cursor_right(&mut self) {
		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
		assert!(self.logical_cursor < self.char_buffer.len());
		if self.graphical_cursor.x + 1 >= self.width {
			self.graphical_cursor.x = 0;
			self.graphical_cursor.y += 1;
		} else {
			if self.char_buffer[self.logical_cursor] == '\n' {
				self.graphical_cursor.x = 0;
				self.graphical_cursor.y += 1;
			} else {
				self.graphical_cursor.x += 1;
			}
		}

		if self.graphical_cursor.y > self.height + self.scroll {
			self.scroll_down();
		}

		self.logical_cursor += 1;
		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
	}

	fn cursor_up(&mut self) {
		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
		assert!(self.logical_cursor > 0);

		let target_x = self.graphical_cursor.x;
		let start_of_current_line = self.logical_cursor - target_x;
		if start_of_current_line == 0 {
			self.logical_cursor = 0;
			self.graphical_cursor.x = 0;
		} else {
			if self.char_buffer[start_of_current_line - 1] == '\n' {
				let start_of_logical_line =
					if let Some(prev_nl) = self.get_prev_newline(start_of_current_line - 1) {
						prev_nl + 1
					} else {
						0
					};

				let logical_line_length = start_of_current_line - start_of_logical_line;
				let graphical_line_length = logical_line_length % self.width;
				let target_x = target_x.min(graphical_line_length);
				if graphical_line_length <= target_x {
					self.logical_cursor = start_of_current_line - 1;
					self.graphical_cursor.x = graphical_line_length - 1;
					self.graphical_cursor.y -= 1;
				} else {
					self.logical_cursor = start_of_current_line - (graphical_line_length - target_x);
					self.graphical_cursor.y -= 1;
				}
			} else {
				self.logical_cursor -= self.width;
				self.graphical_cursor.y -= 1;
			}
		}

		while self.graphical_cursor.y < self.scroll {
			self.scroll_up();
		}

		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
	}

	fn cursor_down(&mut self) {
		assert!(self.logical_cursor < self.char_buffer.len());
		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));

		let start_of_current_graphical_line = self.logical_cursor - self.graphical_cursor.x;
		if let Some(next_nl) = self.get_next_newline(self.logical_cursor) {
			if next_nl - start_of_current_graphical_line >= self.width {
				let next_line_length = (next_nl - start_of_current_graphical_line) - self.width;
				if next_line_length <= self.graphical_cursor.x {
					self.logical_cursor = next_nl;
					self.graphical_cursor.x = next_line_length;
					self.graphical_cursor.y += 1;
				} else {
					self.logical_cursor += self.width;
					self.graphical_cursor.y += 1;
				}
			} else {
				let start_of_next_logical_line = next_nl + 1;
				let next_logical_line_length = self
					.get_next_newline(start_of_next_logical_line)
					.unwrap_or(self.char_buffer.len())
					- start_of_next_logical_line;
				if next_logical_line_length <= self.graphical_cursor.x {
					self.logical_cursor = start_of_next_logical_line + next_logical_line_length;
					self.graphical_cursor.x = next_logical_line_length;
					self.graphical_cursor.y += 1;
				} else {
					self.logical_cursor = start_of_next_logical_line + self.graphical_cursor.x;
					self.graphical_cursor.y += 1;
				}
			}
		} else {
			let end_of_logical_line = self.char_buffer.len();
			let start_of_graphical_line = self.logical_cursor - self.graphical_cursor.x;
			let line_length = end_of_logical_line - start_of_graphical_line;
			if line_length <= self.width + self.graphical_cursor.x {
				self.logical_cursor = self.char_buffer.len();
				self.graphical_cursor.x = line_length % self.width;
				if line_length >= self.width {
					self.graphical_cursor.y += 1;
				}
			} else {
				self.logical_cursor += self.width;
				self.graphical_cursor.y += 1;
			}
		}

		while self.graphical_cursor.y >= self.scroll + self.height {
			self.scroll_down();
		}

		self.invalidate(Rect::new(
			self.graphical_cursor.x * 8,
			(self.graphical_cursor.y - self.scroll) * 16,
			8,
			16,
		));
	}
}

impl Widget for Editor {
	fn set_size(&mut self, size: Point) {
		self.width = size.x / 8;
		self.height = size.y / 16 - 1;
		self.scroll = 0;

		self.char_buffer.clear();
		self.logical_cursor = 0;
		self.graphical_cursor = Point::new(0, 0);
		self.top_row_char_index = 0;
		self.dirty = true;
	}

	fn draw(&mut self, mut window: Window) {
		if !self.dirty {
			return;
		}

		let invalid = self.invalidated;
		let start_x = invalid.x / 8;
		let start_y = invalid.y / 16;
		// end_x and end_y are exclusive; they should not be written to
		let end_x = (invalid.x + invalid.width) / 8;
		let end_y = (invalid.y + invalid.height) / 16;

		// This function walks through the characters on screen,
		// and if they are in the invalidated area, prints it to the screen.
		// Any space that is invalidated but not covered in a character is drawn to with background color.

		// We start in the upper left corner of the screen.
		let start_char_index = self.top_row_char_index;
		// This keeps track of our position on the screen.
		let mut gpos = Point::new(0, 0);

		for i in start_char_index.. {
			// Out of characters, or out of screen
			if i >= self.char_buffer.len() || gpos.y >= self.height {
				break;
			}
			let c = self.char_buffer[i];
			if c == '\n' {
				// There might be invalidated space after a newline, in which case we draw a
				// rectangle covering the area.
				// If there isn't an invalidated area here, either the width or height will be 0.
				window.draw_rect(
					Rect::new(
						gpos.x * 8,
						gpos.y * 16,
						(end_x.saturating_sub(gpos.x)) * 8,
						16,
					),
					Color::new(0, 0, 0),
				);
				gpos.x = 0;
				gpos.y += 1;
			} else {
				// If we are in an invalidated area, print the character.
				// Else, don't.
				if gpos.x >= start_x && gpos.x < end_x && gpos.y >= start_y && gpos.y < end_y {
					window.draw_char(
						Point::new(gpos.x * 8, gpos.y * 16),
						1,
						c,
						Color::WHITE,
						Color::BLACK,
						None,
					);
				}
				gpos.x += 1;
				// Make sure to wrap when hitting the right edge
				if gpos.x >= self.width {
					gpos.x = 0;
					gpos.y += 1;
				}
			}
		}

		// There might be a lot of the screen which needs to be updated, so draw rectangles
		// to the rest of the invalidated area.
		if gpos.y < self.height {
			// This is one character row tall
			window.draw_rect(
				Rect::new(
					gpos.x * 8,
					gpos.y * 16,
					(end_x.saturating_sub(gpos.x)) * 8,
					16,
				),
				Color::BLACK,
			);
			// This covers the rest of the invalidated area
			window.draw_rect(
				Rect::new(
					start_x * 8,
					(gpos.y + 1) * 16,
					(end_x.saturating_sub(start_x)) * 8,
					(end_y.saturating_sub(gpos.y + 1)) * 16,
				),
				Color::BLACK,
			);
		}

		// If the graphical cursor is in the invalidated area, print it too.
		// As this will first get printed over and then reprinted, there might be some flickering.
		if self.graphical_cursor.x >= start_x
			&& self.graphical_cursor.x < end_x
			&& self.graphical_cursor.y >= start_y + self.scroll
			&& self.graphical_cursor.y < end_y + self.scroll
		{
			// The cursor is a 1px thin rectangle sitting just below the baseline, with 1px black padding.
			window.draw_rect(
				Rect::new(
					self.graphical_cursor.x * 8,
					(self.graphical_cursor.y - self.scroll) * 16 + 12,
					8,
					3,
				),
				Color::BLACK,
			);
			window.draw_rect(
				Rect::new(
					self.graphical_cursor.x * 8 + 1,
					(self.graphical_cursor.y - self.scroll) * 16 + 13,
					6,
					1,
				),
				Color::WHITE,
			);
		}

		let mut bottom_bar = String::new();
		write!(
			bottom_bar,
			"{:3} -- {:3} : {:3} -- {:3}",
			self.logical_cursor,
			self.graphical_cursor.x,
			self.graphical_cursor.y,
			self.char_buffer.len()
		)
		.unwrap();

		window.draw_rect(
			Rect::new(0, self.height * 16, self.width * 8, 16),
			Color::new(0x44, 0x44, 0x44),
		);
		for (i, c) in bottom_bar.chars().enumerate() {
			window.draw_char(
				Point::new(i * 8, self.height * 16),
				1,
				c,
				Color::WHITE,
				Color::new(0x44, 0x44, 0x44),
				None,
			);
		}

		// Reset invalidated area and dirty flag.
		self.invalidated = Rect::new(0, 0, 0, 0);
		self.dirty = false;
	}

	fn dirty(&self) -> bool {
		self.dirty
	}

	fn used_area(&self) -> Rect {
		Rect {
			x: 0,
			y: 0,
			width: self.width * 8,
			height: self.height * 16 + 16,
		}
	}

	fn invalidate(&mut self, area: Rect) {
		if self.width == 0 && self.height == 0 {
			self.invalidated = area;
		} else {
			self.invalidated = Rect::smallest_containing(self.invalidated, area);
		}
		if self.invalidated.width > 0 && self.invalidated.height > 0 {
			self.dirty = true;
		}
	}

	fn on_event(&mut self, event: Event) -> Response {
		match event {
			Event::KeyEvent(event) => match event {
				KeyEvent { char: Some(c), .. } => {
					self.insert_char(c);
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Left,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.logical_cursor > 0 {
						self.cursor_left();
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Right,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.logical_cursor < self.char_buffer.len() {
						self.cursor_right();
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Up,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.logical_cursor > 0 {
						self.cursor_up();
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Down,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.logical_cursor < self.char_buffer.len() {
						self.cursor_down();
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Delete,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.logical_cursor < self.char_buffer.len() {
						self.delete_char();
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Backspace,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.logical_cursor > 0 {
						self.cursor_left();
						self.delete_char();
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::P,
					modifiers: Modifiers::CTRL,
					..
				} => {
					panic!("Panic initiated by ctrl-P");
				}
				KeyEvent {
					keycode: KeyCode::S,
					modifiers: Modifiers::CTRL,
					..
				} => {
					let save_file = SaveDialog::new(
						self.current_file_name.clone(),
						self.current_file_dir_path.clone(),
						"editor:save_file".into(),
					);
					unsafe {
						display::add_widget(save_file);
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::O,
					modifiers: Modifiers::CTRL,
					..
				} => {
					let open_file = OpenDialog::new(Vec::new(), "editor:open_file".into());
					unsafe {
						display::add_widget(open_file);
					}
					Response::Nothing
				}
				_ => Response::Nothing,
			},
			Event::Custom("editor:open_file", path) => match path.downcast_ref::<Vec<u8>>() {
				Some(path) => {
					let file_info = unsafe { harddisk::fat32::get_file_info(path) };
					if file_info.is_err() {
						return Response::Nothing;
					}

					let size = file_info.unwrap().size;
					// TODO: check if current buffer has been saved, else, prompt

					let mut byte_buffer = Vec::with_capacity(size);
					byte_buffer.resize(size, 0);
					unsafe {
						harddisk::fat32::read_file(path, &mut byte_buffer).unwrap();
					}
					let str = core::str::from_utf8(&byte_buffer).unwrap();
					self.char_buffer.clear();
					for c in str.chars() {
						self.char_buffer.push(c);
					}

					self.logical_cursor = 0;
					self.graphical_cursor.x = 0;
					self.graphical_cursor.y = 0;
					self.scroll = 0;
					self.top_row_char_index = 0;

					// Separate path into dir and filename parts
					self.current_file_dir_path.clone_from(path);
					// Removes end of string until a directory separator is found
					// Not beautiful, but it works
					loop {
						let c = self.current_file_dir_path.pop();
						if c.is_none() || c.unwrap() == harddisk::fat32::SEPARATOR_CHAR {
							break;
						}
					}
					if self.current_file_dir_path.len() > 0 {
						self.current_file_name = path[self.current_file_dir_path.len()+1..].to_vec();
					} else {
						self.current_file_name.clone_from(path);
					}

					self.invalidate(self.used_area());

					Response::Nothing
				}
				None => panic!("Wrong type for event 'editor:open_file'"),
			},
			Event::Custom("editor:save_file", path) => match path.downcast_ref::<Vec<u8>>() {
				Some(path) => {
					// Convert char buffer to byte buffer (utf8)
					let mut byte_buffer = Vec::with_capacity(self.char_buffer.len() * 2);
					let mut buf = [0; 4];
					for c in &self.char_buffer {
						let s = c.encode_utf8(&mut buf);
						for b in s.bytes() {
							byte_buffer.push(b);
						}
					}

					// Write byte buffer to disk
					unsafe {
						harddisk::fat32::write_file(path, &byte_buffer).unwrap();
					}

					// Separate path into dir and filename parts
					self.current_file_dir_path.clone_from(path);
					// Removes end of string until a directory separator is found
					// Not beautiful, but it works
					loop {
						let c = self.current_file_dir_path.pop();
						if c.is_none() || c.unwrap() == harddisk::fat32::SEPARATOR_CHAR {
							break;
						}
					}
					if self.current_file_dir_path.len() > 0 {
						self.current_file_name = path[self.current_file_dir_path.len()+1..].to_vec();
					} else {
						self.current_file_name.clone_from(path);
					}

					// Invalidate used area, to clean up any debug print from writing file to disk
					self.invalidate(self.used_area());

					Response::Nothing
				}
				None => panic!("Wrong type for event 'editor:save_file'"),
			},
			_ => Response::NotHandled,
		}
	}
}
