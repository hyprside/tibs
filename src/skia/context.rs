use color_eyre::eyre::{bail, Result};
use rustamarine::{screen::Screen, Rustamarine};
use std::collections::HashMap;

use skia_safe::{
	gpu::{
		backend_render_targets,
		ganesh::gl::direct_contexts,
		gl::{FramebufferInfo, Interface},
		surfaces, DirectContext,
	},
	Canvas, ColorSpace, Surface,
};
pub mod clay_renderer;
use crate::gl;

pub mod asset_loaders {
	mod image;
	pub use image::*;
	mod shader;
	pub use shader::*;
}

pub struct SkiaContext {
	context: DirectContext,
	surfaces: HashMap<u32, Surface>,
	width: u32,
	height: u32,
	current_fboid: u32,
}

impl SkiaContext {
	pub fn init_skia(screen: &mut Screen) -> Self {
		let interface =
			Interface::new_load_with(|fn_name| screen.get_rustamarine().get_opengl_proc_address(fn_name))
				.expect("Failed to initialize skia (interface)");

		let context =
			direct_contexts::make_gl(interface, None).expect("Failed to initialize skia (context)");

		let (width, height) = (screen.get_width(), screen.get_height());

		Self {
			context,
			surfaces: HashMap::new(),
			width,
			height,
			current_fboid: 0,
		}
	}

	pub fn set_size(&mut self, screen_width: u32, screen_height: u32) -> bool {
		if self.width != screen_width || self.height != screen_height {
			self.width = screen_width;
			self.height = screen_height;
			self.surfaces.clear(); // forçar recriação
			true
		} else {
			false
		}
	}

	pub fn use_screen(&mut self, screen: &mut Screen) {
		let fboid = screen.use_screen(); // <- obtém o framebuffer do sistema
		self.current_fboid = fboid;

		self.surfaces.entry(fboid).or_insert_with(|| {
			create_skia_surface(&mut self.context, self.width, self.height, fboid)
				.expect("Failed to create Skia surface")
		});
	}

	pub fn canvas(&mut self) -> &Canvas {
		self
			.surfaces
			.get_mut(&self.current_fboid)
			.expect("No surface available for current framebuffer")
			.canvas()
	}

	pub fn flush(&mut self) {
		self.context.flush(None);
	}
}

fn create_skia_surface(
	skia_context: &mut DirectContext,
	width: u32,
	height: u32,
	fboid: u32,
) -> Result<Surface, &'static str> {
	let framebuffer_info = FramebufferInfo {
		fboid,
		format: gl::RGBA8,
		protected: skia_safe::gpu::Protected::No,
	};

	let backend_render_target =
		backend_render_targets::make_gl((width as _, height as _), 0, 0, framebuffer_info);

	surfaces::wrap_backend_render_target(
		skia_context,
		&backend_render_target,
		skia_safe::gpu::SurfaceOrigin::TopLeft,
		skia_safe::ColorType::RGBA8888,
		ColorSpace::new_srgb(),
		None,
	)
	.ok_or("Failed to wrap backend render target")
}
