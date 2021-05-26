use core::mem::MaybeUninit;

use super::Widget;
use crate::gui::display::{Point, Rect, Window};

pub struct Container<W: Widget> {
	inner: W,
	inner_size: Point,
	outer_size: Point,
}

impl<W: Widget> Container<W> {
	pub const fn new(widget: W, inner_size: Point) -> Self {
		Self {
			inner: widget,
			inner_size,
			outer_size: Point::new(0, 0),
		}
	}

	fn get_inner_area(&self) -> Rect {
		let x = (self.outer_size.x - self.inner_size.x) / 2;
		let y = (self.outer_size.y - self.inner_size.y) / 2;
		Rect::new(x, y, self.inner_size.x, self.inner_size.y)
	}
}

impl<W: Widget> Widget for Container<W> {
	fn set_size(&mut self, outer_size: Point) {
		self.outer_size = outer_size;
		self.inner.set_size(self.inner_size);
	}

	fn draw(&mut self, mut window: Window) {
		let inner_area = self.get_inner_area();
		let subwindow = window.subwindow(inner_area);
		self.inner.draw(subwindow);
	}

	fn used_area(&self) -> crate::gui::display::Rect {
		self.get_inner_area()
	}

	fn invalidate(&mut self, area: crate::gui::display::Rect) {
		let area = Rect::intersection(self.get_inner_area(), area);
		if area.width > 0 && area.height > 0 {
			let area = Rect::new(0, 0, area.width, area.height);
			self.inner.invalidate(area);
		}
	}

	fn on_event(&mut self, event: super::Event) -> super::Response {
		self.inner.on_event(event)
	}

	fn dirty(&self) -> bool {
		self.inner.dirty()
	}
}
