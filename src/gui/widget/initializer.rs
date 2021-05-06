use super::Widget;



pub struct Initializer<W: Widget> where W::InitData: Clone {
    inner: W,
    data: W::InitData
}

impl<W: Widget> Initializer<W> where W::InitData: Clone {
    pub const fn uninitialized(widget: W, data: W::InitData) -> Self {
        Self {
            inner: widget,
            data
        }
    }
}

impl<W: Widget> Widget for Initializer<W> where W::InitData: Clone {
    type InitData = ();

    fn initialize(&mut self, size: crate::gui::display::Point, init_data: Self::InitData) {
        self.inner.initialize(size, self.data.clone());
    }

    fn draw(&mut self, window: crate::gui::display::Window) {
        self.inner.draw(window);
    }

    fn used_area(&self) -> crate::gui::display::Rect {
        self.inner.used_area()
    }

    fn invalidate(&mut self, area: crate::gui::display::Rect) {
        self.inner.invalidate(area);
    }

    fn on_event(&mut self, event: super::Event) -> super::Response {
        self.inner.on_event(event)
    }

    fn dirty(&self) -> bool {
        self.inner.dirty()
    }
}