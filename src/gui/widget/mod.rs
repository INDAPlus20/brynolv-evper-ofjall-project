pub mod container;
pub mod editor;
pub mod file_dialog;
pub mod message_box;

use core::any::Any;

use super::display::{Point, Rect, Window};
use crate::ps2_keyboard::KeyEvent;

pub trait Widget {
	/// Set's the size of the widget.
	fn set_size(&mut self, size: Point);

	/// Draw the widget to the given window.
	/// The widget decides which parts of itself needs redrawing.
	fn draw(&mut self, window: Window);

	/// Return the area which the widget draws to.
	/// This may depend on runtime values, but should never change after the widget
	/// has been created.
	/// Transparency is currently not supported.
	fn used_area(&self) -> Rect;

	/// Tell the widget that the given area has been clobbered, and the
	/// widget should redraw this section in the next call to `draw`.
	fn invalidate(&mut self, area: Rect);

	/// Send an event to the widget, returning wether the event
	/// was handled (`true`) or not (`false`).
	fn on_event(&mut self, event: Event) -> Response {
		Response::NotHandled
	}

	/// Returns wether the widget needs redrawing.
	fn dirty(&self) -> bool;
}

/// Returned from `Widget::on_event`. Specifies how the GUI system should proceed.
pub enum Response {
	/// Continue passing the event to other widgets.
	NotHandled,
	/// Don't pass the event to other widgets.
	Nothing,
	/// Don't pass the event to other widgets, and remove this widget.
	RemoveMe,
}

#[derive(Clone)]
pub enum Event<'a> {
	KeyEvent(KeyEvent),
	Custom(&'a str, &'a dyn Any),
}
