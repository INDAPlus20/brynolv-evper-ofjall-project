use alloc::{format, string::String};

use display::{Align, Point, Window};

use super::{Event, Response, Widget};
use crate::{
	gui::{
		self,
		display::{self, Color, Rect},
	},
	ps2_keyboard::{KeyCode, KeyEvent, Modifiers},
};

pub struct MessageBox {
	size: Point,
	used_area: Rect,
	pub title: String,
	pub text: String,
	pub background_color: Color,
	pub title_bar_color: Color,
	pub title_color: Color,
	pub text_color: Color,
	pub button_color: Color,
	pub selected_button_color: Color,
	button_types: ButtonTypes2,
	dirty: bool,
	receiver: String,
}

pub enum ButtonTypes {
	None,
	Ok,
	ConfirmCancel,
}

enum ButtonTypes2 {
	None,
	Ok,
	ConfirmCancel(SelectedButton),
}

enum SelectedButton {
	Confirm,
	Cancel,
}

impl MessageBox {
	const BUTTON_HEIGHT: usize = 24;
	const BUTTON_WIDTH: usize = 72;
	const MARGIN: usize = 8;
	const TITLE_BAR_HEIGHT: usize = 32;

	pub fn new(title: String, text: String, button_types: ButtonTypes, receiver: String) -> Self {
		Self {
			size: Point::new(0, 0),
			used_area: Rect::EMPTY,
			title,
			text,
			background_color: Color::new(0x22, 0x22, 0x22),
			title_bar_color: Color::new(0x44, 0x44, 0x44),
			title_color: Color::new(0xFF, 0xFF, 0xFF),
			text_color: Color::new(0xFF, 0xFF, 0xFF),
			button_color: Color::new(0x22, 0x44, 0x22),
			selected_button_color: Color::new(0x44, 0x66, 0x44),
			button_types: match button_types {
				ButtonTypes::None => ButtonTypes2::None,
				ButtonTypes::Ok => ButtonTypes2::Ok,
				ButtonTypes::ConfirmCancel => ButtonTypes2::ConfirmCancel(SelectedButton::Confirm),
			},
			dirty: true,
			receiver,
		}
	}
}

impl Widget for MessageBox {
	fn set_size(&mut self, size: Point) {
		self.size = size;

		let char_count = self.text.chars().count();

		let max_char_per_line = (size.y - Self::MARGIN * 4) / 8;
		let line_count = (char_count + max_char_per_line - 1) / max_char_per_line;

		let text_height = line_count * 16;
		let text_width = char_count.min(max_char_per_line) * 8;

		let (button_area_height, button_area_min_width) = match self.button_types {
			ButtonTypes2::None => (0, 0),
			ButtonTypes2::Ok => (Self::BUTTON_HEIGHT + Self::MARGIN, Self::BUTTON_WIDTH),
			ButtonTypes2::ConfirmCancel(_) => (
				Self::BUTTON_HEIGHT + Self::MARGIN,
				Self::BUTTON_WIDTH * 2 + Self::MARGIN,
			),
		};

		let total_height =
			Self::TITLE_BAR_HEIGHT + Self::MARGIN + text_height + button_area_height + Self::MARGIN;
		let total_width = text_width.max(button_area_min_width) + Self::MARGIN * 2;

		self.used_area = Rect::new(
			(size.x - total_width) / 2,
			(size.y - total_height) / 2,
			total_width,
			total_height,
		);
	}

	fn draw(&mut self, mut window: Window) {
		let used_area = self.used_area;
		let title_bar_area = Rect::new(
			used_area.x,
			used_area.y,
			used_area.width,
			Self::TITLE_BAR_HEIGHT,
		);
		let main_area = Rect::new(
			used_area.x,
			used_area.y + Self::TITLE_BAR_HEIGHT,
			used_area.width,
			used_area.height - Self::TITLE_BAR_HEIGHT,
		);

		window.draw_rect(title_bar_area, self.title_bar_color);
		window.draw_rect(main_area, self.background_color);

		let title_text_area = Rect {
			y: title_bar_area.y + 8,
			..title_bar_area
		};

		window.draw_string(
			title_text_area,
			1,
			false,
			Align::Center,
			&self.title,
			self.text_color,
			self.title_bar_color,
			None,
		);

		let text_area = Rect::new(
			main_area.x + Self::MARGIN,
			main_area.y + Self::MARGIN,
			main_area.width - Self::MARGIN * 2,
			main_area.height
				- match self.button_types {
					ButtonTypes2::None => 0,
					_ => Self::BUTTON_HEIGHT + Self::MARGIN,
				} - Self::MARGIN,
		);

		window.draw_string(
			text_area,
			1,
			true,
			Align::Left,
			&self.text,
			self.text_color,
			self.background_color,
			None,
		);

		match &self.button_types {
			ButtonTypes2::None => {}
			ButtonTypes2::Ok => {
				let button_rect = Rect::new(
					used_area.x + (used_area.width - Self::BUTTON_WIDTH) / 2,
					used_area.y + used_area.height - Self::BUTTON_HEIGHT - Self::MARGIN,
					Self::BUTTON_WIDTH,
					Self::BUTTON_HEIGHT,
				);
				let text_rect = Rect {
					y: button_rect.y + 4,
					..button_rect
				};

				window.draw_rect(button_rect, self.selected_button_color);
				window.draw_string(
					text_rect,
					1,
					false,
					Align::Center,
					"OK".into(),
					self.text_color,
					self.selected_button_color,
					None,
				);
			}
			ButtonTypes2::ConfirmCancel(selected) => {
				let temp = used_area.width / 3;
				let confirm_middle = used_area.x + temp;
				let confirm_x = confirm_middle - Self::BUTTON_WIDTH / 2;

				let cancel_middle = used_area.x + used_area.width - temp;
				let cancel_x = cancel_middle - Self::BUTTON_WIDTH / 2;

				let confirm_x = used_area.x + (used_area.width - Self::MARGIN) / 2 - Self::BUTTON_WIDTH;
				let cancel_x = used_area.x + (used_area.width + Self::MARGIN) / 2;

				let confirm_rect = Rect::new(
					confirm_x,
					used_area.y + used_area.height - Self::BUTTON_HEIGHT - Self::MARGIN,
					Self::BUTTON_WIDTH,
					Self::BUTTON_HEIGHT,
				);
				let confirm_text_rect = Rect {
					y: confirm_rect.y + 4,
					..confirm_rect
				};

				let button_color = if let SelectedButton::Confirm = selected {
					self.selected_button_color
				} else {
					self.button_color
				};
				window.draw_rect(confirm_rect, button_color);
				window.draw_string(
					confirm_text_rect,
					1,
					false,
					Align::Center,
					"Confirm".into(),
					self.text_color,
					button_color,
					None,
				);

				let cancel_rect = Rect::new(
					cancel_x,
					used_area.y + used_area.height - Self::BUTTON_HEIGHT - Self::MARGIN,
					Self::BUTTON_WIDTH,
					Self::BUTTON_HEIGHT,
				);
				let cancel_text_rect = Rect {
					y: cancel_rect.y + 4,
					..cancel_rect
				};

				let button_color = if let SelectedButton::Cancel = selected {
					self.selected_button_color
				} else {
					self.button_color
				};
				window.draw_rect(cancel_rect, button_color);
				window.draw_string(
					cancel_text_rect,
					1,
					false,
					Align::Center,
					"Cancel".into(),
					self.text_color,
					button_color,
					None,
				);
			}
		}

		self.dirty = false;
	}

	fn used_area(&self) -> display::Rect {
		self.used_area
	}

	fn invalidate(&mut self, area: display::Rect) {
		self.dirty = true;
	}

	fn dirty(&self) -> bool {
		self.dirty
	}

	fn on_event(&mut self, event: Event) -> Response {
		match event {
			Event::KeyEvent(event) => match event {
				KeyEvent {
					keycode: KeyCode::Left,
					modifiers: Modifiers::NONE,
					..
				} => match &mut self.button_types {
					ButtonTypes2::ConfirmCancel(selected) => {
						if let SelectedButton::Cancel = selected {
							*selected = SelectedButton::Confirm;
							self.invalidate(self.used_area);
						}
						Response::Nothing
					}
					_ => Response::Nothing,
				},
				KeyEvent {
					keycode: KeyCode::Right,
					modifiers: Modifiers::NONE,
					..
				} => match &mut self.button_types {
					ButtonTypes2::ConfirmCancel(selected) => {
						if let SelectedButton::Confirm = selected {
							*selected = SelectedButton::Cancel;
							self.invalidate(self.used_area);
						}
						Response::Nothing
					}
					_ => Response::Nothing,
				},
				KeyEvent {
					keycode: KeyCode::Enter,
					modifiers: Modifiers::NONE,
					..
				} => match &self.button_types {
					ButtonTypes2::None => Response::Nothing,
					ButtonTypes2::Ok => {
						unsafe {
							gui::display::send_event(Event::Custom(&self.receiver, &"ok"));
						}
						Response::RemoveMe
					}
					ButtonTypes2::ConfirmCancel(selected) => {
						let data = match selected {
							SelectedButton::Confirm => &"confirm",
							SelectedButton::Cancel => &"cancel",
						};
						unsafe {
							gui::display::send_event(Event::Custom(&self.receiver, data));
						}
						Response::RemoveMe
					}
				},
				_ => Response::Nothing,
			},
			_ => Response::NotHandled,
		}
	}
}
