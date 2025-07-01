use clay_layout::render_commands::{Custom, RenderCommand};
use skia_safe::{Canvas, Image};
#[derive(Debug, Clone)]
pub enum CustomElements {}

impl CustomElements {
	pub fn render(
		_command: &RenderCommand<'_, Image, Self>,
		_custom: &Custom<'_, Self>,
		_canvas: &Canvas,
	) {
		// Unused
	}
}
