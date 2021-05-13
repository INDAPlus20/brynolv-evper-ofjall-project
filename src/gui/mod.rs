//! A widget-based GUI system.
//!
//! The core of this GUI system is the [`display`] module and the [Widget] trait.

use alloc::boxed::Box;

use bootloader::boot_info::FrameBuffer;

use self::{display::Point, widget::Widget};

pub mod widget;
#[macro_use]
pub mod display;

/// Initialize the GUI system.
///
/// This initializes the [`display`] module and adds the [Editor] widget to it.
///
/// [Editor]: widget::editor::Editor
pub unsafe fn initialize(framebuffer: FrameBuffer) {
	display::initialize(framebuffer);
	let editor = widget::editor::Editor::new();
	display::add_widget(Box::new(editor));
}
