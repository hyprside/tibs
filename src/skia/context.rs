use color_eyre::eyre::Result;

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
use crate::gles_context::GlesContext;

pub mod asset_loaders {
    mod image;
    pub use image::*;
    mod shader;
    pub use shader::*;
}

pub struct SkiaContext {
    context: DirectContext,
    surface: Surface,
    width: u32,
    height: u32,
}

impl SkiaContext {
    pub fn init_skia(context: &mut dyn GlesContext) -> Self {
        let interface = Interface::new_load_with(|fn_name| context.get_proc_address(fn_name))
            .expect("Failed to initialize skia (interface)");

        let mut skia_context =
            direct_contexts::make_gl(interface, None).expect("Failed to initialize skia (context)");

        let (width, height) = context.size();
        let surface = create_skia_surface(&mut skia_context, width, height, 0)
            .expect("Failed to create Skia surface");

        Self {
            context: skia_context,
            surface,
            width,
            height,
        }
    }

    pub fn set_size(&mut self, screen_width: u32, screen_height: u32) -> bool {
        if self.width != screen_width || self.height != screen_height {
            self.width = screen_width;
            self.height = screen_height;
            self.surface = create_skia_surface(&mut self.context, screen_width, screen_height, 0)
                .expect("Failed to recreate Skia surface");
            true
        } else {
            false
        }
    }

    pub fn canvas(&mut self) -> &Canvas {
        self.surface.canvas()
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
		skia_safe::gpu::SurfaceOrigin::BottomLeft,
		skia_safe::ColorType::RGBA8888,
		ColorSpace::new_srgb(),
		None,
    )
    .ok_or("Failed to wrap backend render target")
}
