use alloc::{string::String, vec::Vec};

use super::{Event, Response, Widget};
use crate::{
	gui::display::{self, Align, Color, Point, Rect, Window},
	harddisk::{self, fat32::FileInfo},
	ps2_keyboard::{KeyCode, KeyEvent, Modifiers},
	svec::SVec,
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

					if unsafe { harddisk::fat32::get_file_info(&self.current_path).is_directory } {
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
