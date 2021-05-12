use alloc::boxed::Box;

use bootloader::boot_info::FrameBuffer;

use self::{display::Point, widget::Widget};

pub mod widget;
#[macro_use]
pub mod display;

pub unsafe fn initialize(framebuffer: FrameBuffer) {
	display::initialize(framebuffer);
	let editor = widget::editor::Editor::new();
	display::add_widget(Box::new(editor));
}
