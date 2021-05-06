use crate::gui::display::{Color, Point, Rect};

use super::Widget;



pub struct MessageBox<'a> {
    size: Point,
    used_area: Rect,
    pub title: &'a str,
    pub text: &'a str,
    pub background_color: Color,
    pub title_bar_color: Color,
    pub title_color: Color,
    pub text_color: Color,
    dirty: bool
}

impl<'a> MessageBox<'a> {
    pub fn uninitialized() -> Self {
        Self {
            size: Point::new(0, 0),
            used_area: Rect::EMPTY,
            title: "",
            text: "",
            background_color: Color::new(0x22, 0x22, 0x22),
            title_bar_color: Color::new(0x44, 0x44, 0x44),
            title_color: Color::new(0xFF, 0xFF, 0xFF),
            text_color: Color::new(0xFF, 0xFF, 0xFF),
            dirty: true
        }
    }
}

impl<'a> Widget for MessageBox<'a> {
    type InitData = ();

    fn initialize(&mut self, size: crate::gui::display::Point, _: Self::InitData) {
        self.size = size;
    }

    fn draw(&mut self, mut window: crate::gui::display::Window) {
        let title_char_count = self.title.chars().count();
        let text_char_count = self.text.chars().count();
        let max_char_width = (self.size.x / 8).saturating_sub(1);
        let title_rows = ((title_char_count + max_char_width - 1) / max_char_width).max(1);
        let text_rows = ((text_char_count + max_char_width - 1) / max_char_width).max(1);

        let title_bar_height = title_rows * 16 + 16;
        let text_area_height = self.size.y - title_bar_height;

        assert!(text_area_height >= text_rows * 16);

        window.draw_rect(Rect::new(0, 0, self.size.x, title_bar_height), self.title_bar_color);
        window.draw_rect(Rect::new(0, title_bar_height, self.size.x, text_area_height), self.background_color);

        let mut title_chars = self.title.chars();

        for title_row in 0..title_rows {
            let text = title_chars.by_ref().take(max_char_width);
            let text_count = (title_char_count - max_char_width * title_row).min(max_char_width);
            let text_width = text_count * 8;
            let mut x = (self.size.x - text_width) / 2;
            let y = title_row * 16 + 8;
            for c in text {
                window.draw_char(Point::new(x, y), 1, c, self.title_color, self.title_bar_color, None);
                x += 8;
            }
        }

        let max_text_line_length = text_char_count.min(max_char_width);
        let mut text_chars = self.text.chars();

        for text_row in 0..text_rows {
            let text = text_chars.by_ref().take(max_char_width);
            let mut x = (self.size.x - max_text_line_length * 8) / 2;
            let y = (text_area_height - text_rows * 16) / 2 + text_row * 16 + title_bar_height;
            for c in text {
                window.draw_char(Point::new(x, y), 1, c, self.text_color, self.background_color, None);
                x += 8;
            }
        }
    }

    fn used_area(&self) -> crate::gui::display::Rect {
        Rect::new(0, 0, self.size.x, self.size.y)
    }

    fn invalidate(&mut self, area: crate::gui::display::Rect) {
        self.dirty = true;
    }

    fn dirty(&self) -> bool  {
        self.dirty
    }
}