use bootloader::boot_info::FrameBuffer;

/// A glyph or character is 8*16 pixels
type Glyph = [[u8; 8]; 16];

/// Monospace pixelfont made by @Elekrisk
const DEFAULT_FONT: [Glyph; 128] = unsafe { core::mem::transmute(*include_bytes!("vgafont.bin")) };

static mut PRINTER: Printer = unsafe { Printer::uninitialized() };

pub struct Printer {
    framebuffer: FrameBuffer,
    cursor: (usize, usize),
    font: &'static [Glyph; 128],
    initialized: bool,
}

impl Printer {
    const unsafe fn uninitialized() -> Self {
        Self {
            framebuffer: core::mem::transmute([0; 16]),
            cursor: (0, 0),
            font: &DEFAULT_FONT,
            initialized: false,
        }
    }

    /// Clears the screen by setting every byte to zero.
    unsafe fn clear(&mut self) {
        let (res_x, res_y, stride, bytes_per_pixel, buffer) = self.get_buffer_info();
        for y in 0..res_y {
            for x in 0..res_x {
                let i = (y * stride + x) * bytes_per_pixel;
                for b in 0..bytes_per_pixel {
                    buffer[i + b] = 0;
                }
            }
        }
    }

    /// Prints a single ASCII character at the current cursor position.
    fn print_char(&mut self, char: char) {
        let glyph = self.font[char as usize];
        let (mut cursor_x, mut cursor_y) = self.cursor;
        match char {
            '\n' => {
                cursor_y += 1;
                cursor_x = 0;
            }
            other if other < ' ' || other == '\x7F' => {}
            _ => {
                let (_, _, stride, bytes_per_pixel, buffer) = self.get_buffer_info();
                for y in 0..16 {
                    for x in 0..8 {
                        let color = glyph[y][x];
                        for b in 0..bytes_per_pixel {
                            buffer[((y + cursor_y * 16) * stride + (x + cursor_x * 8))
                                * bytes_per_pixel
                                + b] = color;
                        }
                    }
                }
                let chars_per_line = self.framebuffer.info().horizontal_resolution / 8;
                cursor_x += 1;
                if cursor_x >= chars_per_line {
                    cursor_y += 1;
                    cursor_x = 0;
                }
            }
        }
        let line_count = self.framebuffer.info().vertical_resolution / 16;
        if cursor_y >= line_count {
            self.scroll_down();
            cursor_y -= 1;
        }

        self.cursor = (cursor_x, cursor_y);
    }

    /// Scrolls down the screen one text row.
    /// TODO: remember offscreen lines for later retrival.
    fn scroll_down(&mut self) {
        let (res_x, res_y, stride, bytes_per_pixel, buffer) = self.get_buffer_info();
        for y in 16..res_y {
            for x in 0..res_x {
                for b in 0..bytes_per_pixel {
                    let value = buffer[(y * stride + x) * bytes_per_pixel + b];
                    buffer[((y - 16) * stride + x) * bytes_per_pixel + b] = value;
                }
            }
        }
        for y in res_y - 16..res_y {
            for x in 0..res_x {
                for b in 0..bytes_per_pixel {
                    buffer[(y * stride + x) * bytes_per_pixel + b] = 0;
                }
            }
        }
    }

    /// Returns (x, y, stride, bytes_per_pixel, buffer)
    fn get_buffer_info(&mut self) -> (usize, usize, usize, usize, &mut [u8]) {
        (
            self.framebuffer.info().horizontal_resolution,
            self.framebuffer.info().vertical_resolution,
            self.framebuffer.info().stride,
            self.framebuffer.info().bytes_per_pixel,
            self.framebuffer.buffer_mut(),
        )
    }
}

impl core::fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            print_str(s);
        }
        Ok(())
    }
}

/// ## Safety
/// Framebuffer must be a valid framebuffer.
/// Call this first.
/// Only one Writer instance should ever be in existance.
pub unsafe fn initialize(framebuffer: FrameBuffer) {
    if PRINTER.initialized {
        panic!("PRINTER already initialized!");
    }
    PRINTER.framebuffer = framebuffer;
    PRINTER.initialized = true;
}

/// Clears the screen by setting every byte in the buffer to 0 and resets the cursor.
pub unsafe fn clear() {
    if !PRINTER.initialized {
        panic!("Printer not initialized!");
    }
    PRINTER.clear();
    PRINTER.cursor = (0, 0);
}

/// Use `print!()` macro or `print_str` instead.
/*pub unsafe fn print_char(char: char) {
    PRINTER.print_char(char);
}*/

/// Prints the input string (assuming ASCII)
pub unsafe fn print_str(string: &str) {
    if !PRINTER.initialized {
        panic!("Printer not initialized!");
    }
    for char in string.chars() {
        PRINTER.print_char(char);
    }
}

/// Scrolls entire screen down one text row.
/// **WARNING** rows going offscreen are gone from memory.
pub unsafe fn scroll_down() {
    if !PRINTER.initialized {
        panic!("Printer not initialized!");
    }
    PRINTER.scroll_down();
    PRINTER.cursor.1 -= 1;
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::printer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    unsafe {
        PRINTER.write_fmt(args).unwrap();
    }
}
