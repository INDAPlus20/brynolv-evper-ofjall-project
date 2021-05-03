use core::fmt::Write;

use crate::{gui::display::{Point, Rect, Window}, svec::SVec};

use super::Widget;
use super::Event;
use super::Response;
use super::KeyEvent;
use crate::ps2_keyboard::{KeyCode, Modifiers};

pub static mut PRINTER_WIDGET: PrinterWidget = PrinterWidget::uninitialized();

pub struct PrinterWidget {
    /// The width in chars
    width: usize,
    /// The height in chars
    height: usize,
    char_buffer: SVec<SVec<char, 256>, { Self::BUFFER_LINE_COUNT }>,
    /// How many lines the printer has scrolled down
    scroll: usize,
    max_scroll: usize,
    /// Cursor, using buffer-local coordinates
    cursor: Point,
    dirty: bool,
    invalidated: Rect
}

impl PrinterWidget {
    const BUFFER_LINE_COUNT: usize = 128;

    const fn uninitialized() -> Self {
        Self {
            width: 0,
            height: 0,
            char_buffer: SVec::new(),
            scroll: 0,
            max_scroll: 0,
            cursor: Point::new(0, 0),
            dirty: false,
            invalidated: Rect::new(0, 0, 0, 0),
        }
    }

    pub fn initialize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.scroll = 0;
        self.max_scroll = 0;

        self.char_buffer.clear_without_drop();
        for y in 0..height.min(Self::BUFFER_LINE_COUNT) {
            let mut row = SVec::new();
            for x in 0..width {
                row.push('\x00');
            }
            self.char_buffer.push(row);
        }
        self.cursor = Point::new(0, 0);
        self.dirty = true;
    }

    fn print_char(&mut self, char: char) {
        match char {
            '\n' => {
                self.cursor.x = 0;
                self.cursor.y += 1;
            }
            _ => {
                self.invalidate(Rect::new(self.cursor.x * 8, self.cursor.y * 16, 8, 16));
                let current_row = &mut self.char_buffer[self.cursor.y];
                while current_row.len() + 1 < self.cursor.x {
                    current_row.push('\x00');
                }
                current_row[self.cursor.x] = char;

                self.cursor.x += 1;
                if self.cursor.x >= self.width {
                    self.cursor.x = 0;
                    self.cursor.y += 1;
                }
            }
        }
        if self.cursor.y.saturating_sub(self.scroll) >= self.height || self.cursor.y >= self.char_buffer.len() {
            self.scroll_down();
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    fn scroll_down(&mut self) {
        let lines_below_screen = Self::BUFFER_LINE_COUNT.saturating_sub(self.scroll).saturating_sub(self.height);
        if lines_below_screen > 0 {
            self.scroll += 1;
            self.max_scroll = self.scroll.max(self.max_scroll);
        } else {
            self.char_buffer.remove(0);
            if self.cursor.y > 0 {
                self.cursor.y -= 1;
            }
        }
        
        self.char_buffer.push(SVec::with_length('\x00', self.width));
        self.invalidate(self.used_area());
    }

    fn print_str(&mut self, str: &str) {
        for char in str.chars() {
            self.print_char(char);
        }
    }
}

impl Widget for PrinterWidget {
    fn draw(&mut self, mut window: Window) {

        let invalid = self.invalidated;
        let start_x = invalid.x / 8;
        let start_y = invalid.y / 16;
        let end_x = (invalid.x + invalid.width) / 8 + 1;
        let end_y = (invalid.y + invalid.height) / 8 + 1;

        let end_y = self.height
            .min(self.char_buffer.len() - self.scroll)
            .min(end_y);

        for y in start_y..end_y {
            let row = &self.char_buffer[self.scroll + y];
            for x in start_x..row.len().min(end_x) {
                window.draw_char(Point { x: x * 8, y: y * 16 }, 1, row[x], None);
            }
        }
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn used_area(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: self.width * 8,
            height: self.height * 16
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
            Event::Custom("print", msg) => match msg.downcast_ref::<&str>() {
                Some(msg) => {
                    self.print_str(msg);
                    Response::Nothing
                },
                None => match msg.downcast_ref::<char>() {
                    Some(msg) => {
                        self.print_char(*msg);
                        Response::Nothing
                    },
                    None => panic!("Invalid 'print' event payload")
                },
            },
            Event::Custom("println", msg) => match msg.downcast_ref::<&str>() {
                Some(msg) => {
                    self.print_str(msg);
                    self.print_char('\n');
                    Response::Nothing
                },
                None => match msg.downcast_ref::<char>() {
                    Some(msg) => {
                        self.print_char(*msg);
                        Response::Nothing
                    },
                    None => panic!("Invalid 'print' event payload")
                },
            },
            Event::KeyEvent(k) => match k {
                KeyEvent { keycode: KeyCode::Up, modifiers: Modifiers::SHIFT, .. } => {
                    self.scroll_up();
                    Response::Nothing
                }
                KeyEvent { keycode: KeyCode::Down, modifiers: Modifiers::SHIFT, .. } => {
                    if self.scroll < self.max_scroll {
                        self.scroll_down();
                    }
                    Response::Nothing
                }
                KeyEvent { .. } => Response::NotHandled
            }
            _ => Response::NotHandled
        }
    }
}

impl Write for PrinterWidget {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.print_str(s);
        Ok(())
    }
}

