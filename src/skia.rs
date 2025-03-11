use color_eyre::eyre::bail;
use skia_safe::{
    gpu::{
        backend_render_targets,
        ganesh::gl::direct_contexts,
        gl::{FramebufferInfo, Interface},
        surfaces, DirectContext,
    },
    ColorSpace, Surface,
};

use crate::{gl, gles_context::GlesContext};

pub const FRAMEBUFFER_INFO: FramebufferInfo = FramebufferInfo {
    fboid: 0,
    format: gl::RGBA8,
    protected: skia_safe::gpu::Protected::No,
};

pub fn init_skia(context: &mut dyn GlesContext) -> color_eyre::Result<(DirectContext, Surface)> {
    let Some(interface) = Interface::new_load_with(|fn_name| context.get_proc_address(fn_name))
    else {
        bail!("Failed to initialize skia (interface)");
    };
    let Some(mut skia_context) = direct_contexts::make_gl(interface, None) else {
        bail!("Failed to initialize skia (context)");
    };
    let (width, height) = context.size();
    let surface = create_skia_surface(&mut skia_context, width as u32, height as u32)?;
    Ok((skia_context, surface))
}

pub fn create_skia_surface(
    skia_context: &mut DirectContext,
    width: u32,
    height: u32,
) -> color_eyre::Result<Surface> {
    let backend_render_target =
        backend_render_targets::make_gl((width as _, height as _), 0, 0, FRAMEBUFFER_INFO);
    let Some(surface) = surfaces::wrap_backend_render_target(
        skia_context,
        &backend_render_target,
        skia_safe::gpu::SurfaceOrigin::BottomLeft,
        skia_safe::ColorType::RGBA8888,
        ColorSpace::new_srgb(),
        None,
    ) else {
        bail!("Failed to initialize skia (surface)")
    };
    Ok(surface)
}
