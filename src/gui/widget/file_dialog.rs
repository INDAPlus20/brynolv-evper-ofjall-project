use alloc::{string::String, vec::Vec};

use super::{
	message_box::{ButtonTypes, MessageBox},
	Event, Response, Widget,
};
use crate::{
	gui::display::{self, Align, Color, Point, Rect, Window},
	harddisk::{
		self,
		fat32::{FileInfo, SEPARATOR_CHAR},
	},
	ps2_keyboard::{KeyCode, KeyEvent, Modifiers},
};

pub struct OpenDialog {
	size: Point,
	dirty: bool,
	invalidated: Rect,
	current_path: Vec<u8>,
	current_entries: Vec<FileInfo>,
	receiver: String,
	selected: usize,
}

impl OpenDialog {
	const MARGIN: usize = 8;

	pub fn new(dir_path: Vec<u8>, receiver: String) -> Self {
		Self {
			size: Point::new(0, 0),
			dirty: false,
			invalidated: Rect::EMPTY,
			current_entries: unsafe { harddisk::fat32::list_entries(&dir_path) }
				.unwrap()
				.into(),
			current_path: dir_path,
			receiver,
			selected: 0,
		}
	}
}

impl Widget for OpenDialog {
	fn set_size(&mut self, size: Point) {
		self.size = size;
	}

	fn draw(&mut self, mut window: Window) {
		if !self.dirty || self.invalidated.is_empty() {
			return;
		}

		let used_area = self.used_area();
		let title_bar_area = Rect::new(used_area.x, used_area.y, used_area.width, 32);
		let main_area = Rect::new(
			used_area.x,
			used_area.y + title_bar_area.height,
			used_area.width,
			used_area.height - title_bar_area.height,
		);

		let title_bar_color = Color::grayscale(0x44);
		let main_color = Color::grayscale(0x22);
		let text_color = Color::WHITE;
		let dir_color = Color::new(0xFF, 0xFF, 0);
		let selected_color = Color::grayscale(0x55);

		window.draw_rect(
			Rect::intersection(title_bar_area, self.invalidated),
			title_bar_color,
		);
		window.draw_rect(Rect::intersection(main_area, self.invalidated), main_color);

		window.draw_string(
			Rect::new(
				title_bar_area.x,
				title_bar_area.y + 8,
				title_bar_area.width,
				16,
			),
			1,
			false,
			Align::Center,
			"Open File",
			text_color,
			title_bar_color,
			None,
		);

		let mut y = main_area.y;
		for (i, entry) in self.current_entries.iter().enumerate() {
			let fg = if entry.is_directory {
				dir_color
			} else {
				text_color
			};
			let bg = if self.selected == i {
				selected_color
			} else {
				main_color
			};
			window.draw_string(
				Rect::new(
					main_area.x + Self::MARGIN,
					y,
					main_area.width - Self::MARGIN * 2,
					16,
				),
				1,
				false,
				Align::Left,
				entry.name.to_str(),
				fg,
				bg,
				None,
			);
			y += 16;
		}

		self.dirty = false;
		self.invalidated = Rect::EMPTY;
	}

	fn used_area(&self) -> Rect {
		Rect::new(
			Self::MARGIN,
			Self::MARGIN,
			self.size.x - Self::MARGIN * 2,
			self.size.y - Self::MARGIN * 2,
		)
	}

	fn invalidate(&mut self, area: Rect) {
		if self.invalidated.is_empty() {
			self.invalidated = area;
		} else {
			self.invalidated = Rect::smallest_containing(self.invalidated, area);
		}
		self.dirty = true;
	}

	fn on_event(&mut self, event: Event) -> Response {
		match event {
			Event::KeyEvent(event) => match event {
				KeyEvent {
					keycode: KeyCode::Enter,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.current_entries.len() == 0 {
						return Response::Nothing;
					}

					let name: Vec<_> = self.current_entries[self.selected].name.clone().into();

					if self.current_path.len() > 0 {
						self.current_path.push(b'>');
					}
					self.current_path.extend_from_slice(&name);

					let file_result = unsafe { harddisk::fat32::get_file_info(&self.current_path) };
					if file_result.is_err() {
						return Response::Nothing;
					}

					if file_result.unwrap().is_directory {
						self.current_entries = unsafe {
							harddisk::fat32::list_entries(&self.current_path)
								.unwrap()
								.into()
						};
						self.invalidate(self.used_area());
						Response::Nothing
					} else {
						unsafe { display::send_event(Event::Custom(&self.receiver, &self.current_path)) };
						Response::RemoveMe
					}
				}
				KeyEvent {
					keycode: KeyCode::Down,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.selected + 1 < self.current_entries.len() {
						self.invalidate(Rect::new(
							Self::MARGIN * 2,
							Self::MARGIN + 32 + self.selected * 16,
							self.used_area().width - Self::MARGIN,
							32,
						));
						self.selected += 1;
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Up,
					modifiers: Modifiers::NONE,
					..
				} => {
					if self.selected > 0 {
						self.selected -= 1;
						self.invalidate(Rect::new(
							Self::MARGIN * 2,
							Self::MARGIN + 32 + self.selected * 16,
							self.used_area().width - Self::MARGIN,
							32,
						));
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Escape,
					modifiers: Modifiers::NONE,
					..
				} => Response::RemoveMe,
				_ => Response::Nothing,
			},
			Event::Custom(..) => Response::NotHandled,
		}
	}

	fn dirty(&self) -> bool {
		self.dirty
	}
}

pub struct SaveDialog {
	size: Point,
	dirty: bool,
	invalidated: Rect,
	current_path: Vec<u8>,
	current_directory_path: Vec<u8>,
	current_directory_entries: Vec<FileInfo>,
	selected_entry: usize,
	receiver: String,
	full_path: Vec<u8>,   // Holds full path for lifetime requirement
	filename_area: Rect,  // For easy invalidating
	directory_area: Rect, //
}

impl SaveDialog {
	const DIR_ENTRY_HEIGHT: usize = 32;
	const MARGIN: usize = 8;
	const TEXT_HEIGHT: usize = 16;

	pub fn new(file_path: Vec<u8>, directory_path: Vec<u8>, receiver: String) -> Self {
		Self {
			size: Point::new(0, 0),
			dirty: false,
			invalidated: Rect::EMPTY,
			current_path: file_path,
			current_directory_entries: unsafe { harddisk::fat32::list_entries(&directory_path) }
				.unwrap()
				.into(),
			current_directory_path: directory_path,
			selected_entry: 0,
			full_path: Vec::new(),
			receiver,
			filename_area: Rect::EMPTY,
			directory_area: Rect::EMPTY,
		}
	}
}

impl Widget for SaveDialog {
	fn set_size(&mut self, size: Point) {
		self.size = size;
	}

	fn draw(&mut self, mut window: Window) {
		if !self.dirty || self.invalidated.is_empty() {
			return;
		}

		let used_area = self.used_area();
		let title_bar_area = Rect::new(used_area.x, used_area.y, used_area.width, 32);
		self.filename_area = Rect::new(
			used_area.x,
			used_area.y + title_bar_area.height,
			used_area.width,
			32,
		);
		self.directory_area = Rect::new(
			used_area.x,
			self.filename_area.y + self.filename_area.height,
			used_area.width,
			used_area.height - title_bar_area.height - self.filename_area.height,
		);

		let title_bar_color = Color::grayscale(0x44);
		let filename_bg_color = Color::grayscale(0x11);
		let main_color = Color::grayscale(0x22);
		let text_color = Color::WHITE;
		let dir_color = Color::new(0xFF, 0xFF, 0);
		let selected_color = Color::grayscale(0x55);

		// title background
		window.draw_rect(
			Rect::intersection(title_bar_area, self.invalidated),
			title_bar_color,
		);
		// title text
		window.draw_string(
			Rect::new(
				title_bar_area.x,
				title_bar_area.y + Self::MARGIN,
				title_bar_area.width,
				Self::TEXT_HEIGHT,
			),
			1,
			false,
			Align::Center,
			"Save File",
			text_color,
			title_bar_color,
			None,
		);

		// filename background
		window.draw_rect(
			Rect::intersection(self.filename_area, self.invalidated),
			filename_bg_color,
		);

		let s = core::str::from_utf8(&self.current_path);
		if s.is_err() {
			panic!("Error: invalid uft8 character in current path");
		}
		// filename text
		window.draw_string(
			Rect::new(
				self.filename_area.x,
				self.filename_area.y + Self::MARGIN,
				self.filename_area.width,
				Self::TEXT_HEIGHT,
			),
			1,
			false,
			Align::Center,
			s.unwrap(),
			text_color,
			main_color,
			None,
		);

		// directory selector background
		window.draw_rect(
			Rect::intersection(self.directory_area, self.invalidated),
			main_color,
		);

		// directory selector entries
		let mut y = self.directory_area.y + Self::MARGIN;
		for (i, entry) in self.current_directory_entries.iter().enumerate() {
			let fg = if entry.is_directory {
				dir_color
			} else {
				text_color
			};
			let bg = if self.selected_entry == i {
				selected_color
			} else {
				main_color
			};
			window.draw_string(
				Rect::new(
					self.directory_area.x + Self::MARGIN,
					y,
					self.directory_area.width - Self::MARGIN * 2,
					Self::TEXT_HEIGHT,
				),
				1,
				false,
				Align::Left,
				entry.name.to_str(),
				fg,
				bg,
				None,
			);
			y += Self::TEXT_HEIGHT;
		}

		self.dirty = false;
		self.invalidated = Rect::EMPTY;
	}

	fn used_area(&self) -> Rect {
		Rect::new(
			Self::MARGIN,
			Self::MARGIN,
			self.size.x - Self::MARGIN * 2,
			self.size.y - Self::MARGIN * 2,
		)
	}

	fn invalidate(&mut self, area: Rect) {
		if self.invalidated.is_empty() {
			self.invalidated = area;
		} else {
			self.invalidated = Rect::smallest_containing(self.invalidated, area);
		}
		self.dirty = true;
	}

	fn on_event(&mut self, event: Event) -> Response {
		match event {
			Event::KeyEvent(event) => match event {
				KeyEvent {
					keycode: KeyCode::Down,
					modifiers: Modifiers::NONE,
					..
				} => {
					// Goes down one entry in the directory list
					if self.selected_entry + 1 < self.current_directory_entries.len() {
						self.invalidate(Rect::new(
							self.directory_area.x,
							self.directory_area.y
								+ Self::DIR_ENTRY_HEIGHT
								+ self.selected_entry * Self::TEXT_HEIGHT,
							self.directory_area.width,
							Self::DIR_ENTRY_HEIGHT,
						));
						self.selected_entry += 1;
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Up,
					modifiers: Modifiers::NONE,
					..
				} => {
					// Goes up entry in the directory list
					if self.selected_entry > 0 {
						self.selected_entry -= 1;
						self.invalidate(Rect::new(
							self.directory_area.x,
							self.directory_area.y
								+ Self::DIR_ENTRY_HEIGHT
								+ self.selected_entry * Self::TEXT_HEIGHT,
							self.directory_area.width,
							Self::DIR_ENTRY_HEIGHT,
						));
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Right,
					modifiers: Modifiers::NONE,
					..
				} => {
					// Enters the entry in the directory list, if it's a directory
					if self.current_directory_entries.len() == 0 {
						return Response::Nothing;
					}

					let name: Vec<_> = self.current_directory_entries[self.selected_entry]
						.name
						.clone()
						.into();

					let mut new_directory_path = self.current_directory_path.clone();
					if new_directory_path.len() > 0 {
						new_directory_path.push(b'>');
					}
					new_directory_path.extend_from_slice(&name);

					let file_result = unsafe { harddisk::fat32::get_file_info(&new_directory_path) };
					if file_result.is_err() {
						return Response::Nothing;
					}

					if file_result.unwrap().is_directory {
						self.current_directory_entries = unsafe {
							harddisk::fat32::list_entries(&new_directory_path)
								.unwrap()
								.into()
						};
						self.current_directory_path = new_directory_path;
						self.selected_entry = 0;
						self.invalidate(self.used_area());
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Left,
					modifiers: Modifiers::NONE,
					..
				} => {
					// Goes to the current directory's parent directory, if it exists
					if self.current_directory_entries.len() == 0 {
						return Response::Nothing;
					}

					// Removes end of string until a directory separator is found
					// Not beautiful, but it works
					let mut new_directory_path = self.current_directory_path.clone();
					loop {
						let c = new_directory_path.pop();
						if c.is_none() || c.unwrap() == SEPARATOR_CHAR {
							break;
						}
					}

					let file_result = unsafe { harddisk::fat32::get_file_info(&new_directory_path) };
					if file_result.is_err() {
						return Response::Nothing;
					}

					if file_result.unwrap().is_directory {
						self.current_directory_entries = unsafe {
							harddisk::fat32::list_entries(&new_directory_path)
								.unwrap()
								.into()
						};
						self.current_directory_path = new_directory_path;
						self.selected_entry = 0;
						self.invalidate(self.directory_area);
					}
					Response::Nothing
				}
				KeyEvent {
					keycode: KeyCode::Enter,
					modifiers: Modifiers::NONE,
					..
				} => {
					// Appends the directory path and filename path together
					// Prompts if the full path exists
					// And returns the path to the reciever

					// Create full path
					self.full_path = self.current_directory_path.clone();
					if self.current_directory_path.len() > 0 {
						self.full_path.push(SEPARATOR_CHAR);
					}
					self.full_path.extend_from_slice(&self.current_path);

					if self.full_path.len() == 0 {
						return Response::Nothing;
					}

					// If user entered an invalid file path
					if unsafe { !harddisk::fat32::is_valid_file_path(&self.full_path) } {
						// Prompt user about invalid path
						let message_box = MessageBox::new(
							"Error".into(),
							"Invalid file path, Please enter a proper one!".into(),
							ButtonTypes::Ok,
							"".into(),
						);
						unsafe {
							display::add_widget(message_box);
						}
						return Response::Nothing;
					}

					// If file already exists, prompt user about overwriting it
					if unsafe { harddisk::fat32::get_file_info(&self.full_path).is_ok() } {
						let message_box = MessageBox::new(
							"File already exists".into(),
							"A file with this name already exists. Do you want to overwrite it?".into(),
							ButtonTypes::ConfirmCancel,
							"file_dialog:overwrite_file".into(),
						);
						unsafe {
							display::add_widget(message_box);
						}
						return Response::Nothing;
					}

					// Send event with entered path back to editor and remove this dialog
					unsafe { display::send_event(Event::Custom(&self.receiver, &self.full_path)) };
					return Response::RemoveMe;
				}
				KeyEvent {
					keycode: KeyCode::Backspace,
					modifiers: Modifiers::NONE,
					..
				} => {
					// Remove last added character
					self.current_path.pop();
					self.invalidate(self.filename_area);
					Response::Nothing
				}
				KeyEvent { char: Some(c), .. } => {
					let mut buf = [0; 4];
					let s = c.encode_utf8(&mut buf);
					if s.len() > 1 {
						// Ignore chars with a length larger than 1 to make removal simpler
						return Response::Nothing;
					}
					// Append character to filename
					self.current_path.push(buf[0]);
					self.invalidate(self.filename_area);
					Response::Nothing
				}
				// Cancels dialogs
				KeyEvent {
					keycode: KeyCode::Escape,
					modifiers: Modifiers::NONE,
					..
				} => Response::RemoveMe,
				_ => Response::Nothing,
			},
			// Handles the response from the overwrite file message box
			Event::Custom("file_dialog:overwrite_file", choice) => match choice.downcast_ref::<&str>() {
				Some(choice) => {
					// User wants to overwrite
					if *choice == "confirm" {
						// Send event with entered path back to editor and remove this dialog
						unsafe { display::send_event(Event::Custom(&self.receiver, &self.full_path)) };
						return Response::RemoveMe;
					}
					Response::Nothing
				}
				None => panic!("Wrong type for event 'file_dialog:overwrite_file'"),
			},
			Event::Custom(..) => Response::NotHandled,
		}
	}

	fn dirty(&self) -> bool {
		self.dirty
	}
}
