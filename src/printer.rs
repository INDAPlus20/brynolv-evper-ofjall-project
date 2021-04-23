use bootloader::boot_info::FrameBuffer;

const DEFAULT_FONT: [[[u8; 8]; 16]; 128] = unsafe {
    core::mem::transmute(*include_bytes!("vgafont.bin"))
};

static NONDANGLINGPOINTERMAKER: () = ();

static mut PRINTER: Printer = unsafe { Printer::uninitialized() };

pub struct Printer {
    framebuffer: FrameBuffer,
    cursor: (usize, usize),
    font: &'static [[[u8; 8]; 16]; 128]
}

impl Printer {
    const unsafe fn uninitialized() -> Self {
        Self {
            framebuffer: core::mem::transmute([0; 16]),
            cursor: (0, 0),
            font: &DEFAULT_FONT
        }
    }

    unsafe fn clear(&mut self) {
        let res_y = self.framebuffer.info().vertical_resolution;
        let res_x = self.framebuffer.info().horizontal_resolution;
        let stride = self.framebuffer.info().stride;
        let bytes_per_pixel = self.framebuffer.info().bytes_per_pixel;
        let buffer = self.framebuffer.buffer_mut();

        for y in 0..res_y {
            for x in 0..res_x {
                let i = (y * stride + x) * bytes_per_pixel;
                for b in 0..bytes_per_pixel {
                    buffer[i + b] = 0;
                }
            }
        }
    }
    
    fn print_char(&mut self, char: char) {
        let glyph = self.font[char as usize];
        let (mut cursor_x, mut cursor_y) = self.cursor;
        match char {
            '\n' => {
                cursor_y += 1;
                cursor_x = 0;
            },
            other if other < ' ' || other == '\x7F' => {},
            _ => {
                let stride = self.framebuffer.info().stride;
                let bytes_per_pixel = self.framebuffer.info().bytes_per_pixel;
                let buffer = self.framebuffer.buffer_mut();
                for y in 0..16 {
                    for x in 0..8 {
                        let color = glyph[y][x];
                        for b in 0..bytes_per_pixel {
                            buffer[((y + cursor_y * 16) * stride + (x + cursor_x * 8)) * bytes_per_pixel + b] = color;
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
        for y in res_y - 16 .. res_y {
            for x in 0..res_x {
                for b in 0..bytes_per_pixel {
                    buffer[(y * stride + x) * bytes_per_pixel + b] = 0;
                }
            }
        }
    }

    fn get_buffer_info(&mut self) -> (usize, usize, usize, usize, &mut [u8]) {
        (self.framebuffer.info().horizontal_resolution, self.framebuffer.info().vertical_resolution, self.framebuffer.info().stride, self.framebuffer.info().bytes_per_pixel, self.framebuffer.buffer_mut())
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
    PRINTER.framebuffer = framebuffer;
}

pub unsafe fn clear() {
    PRINTER.clear();
}

pub unsafe fn print_char(char: char) {
    PRINTER.print_char(char);
}

pub unsafe fn print_str(string: &str) {
    for char in string.chars() {
        PRINTER.print_char(char);
    }
}

pub unsafe fn scroll_down() {
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
