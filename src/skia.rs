use color_eyre::eyre::bail;
use skia_safe::{gpu::{backend_render_targets, ganesh::gl::direct_contexts, gl::{FramebufferInfo, Interface}, surfaces, DirectContext}, graphics, ColorSpace, Surface};

use crate::{gl, gles_context::GlesContext};

pub fn init_skia(context: &mut dyn GlesContext) -> color_eyre::Result<(DirectContext, Surface)> {
    let Some(interface) = Interface::new_load_with(|fn_name| context.get_proc_address(fn_name)) else {
        bail!("Failed to initialize skia (interface)");
    };
    println!("interface");
    let Some(mut skia_context) = direct_contexts::make_gl(interface, None) else {
        bail!("Failed to initialize skia (context)");
    };
    println!("context");
    let mut fboid = -1;
    gl!(gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid));
    dbg!(fboid);
    let mut framebuffer_info = FramebufferInfo::from_fboid(fboid as u32);
    framebuffer_info.format = gl::RGBA8;
    let (width, height) = context.size();
    let backend_render_target = backend_render_targets::make_gl((width as _, height as _), 0, 0, framebuffer_info);
    println!("backend_render_target");
    let Some(surface) = surfaces::wrap_backend_render_target(&mut skia_context, &backend_render_target, skia_safe::gpu::SurfaceOrigin::BottomLeft, skia_safe::ColorType::RGBA8888, ColorSpace::new_srgb(), None) else {
        bail!("Failed to initialize skia (surface)")
    };
    
    println!("surface");
    Ok((skia_context, surface))
}