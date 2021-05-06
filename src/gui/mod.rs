use bootloader::boot_info::FrameBuffer;

use self::{display::Point, widget::Widget};


pub mod widget;
#[macro_use]
pub mod display;

pub unsafe fn initialize(framebuffer: FrameBuffer) {
    let info = framebuffer.info();
    display::initialize(framebuffer);
    widget::editor::EDITOR.initialize(Point::new(info.horizontal_resolution, info.vertical_resolution), ());
    display::add_initialized_widget(&mut widget::editor::EDITOR);
}