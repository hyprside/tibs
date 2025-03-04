#![allow(unsafe_op_in_unsafe_fn)]

use animation::{
    // colors::{interpolate_color_normalized, rgb_to_norm},
    easing,
    Animation,
    BasicAnimation,
    DelayAnimation,
    LoopingAnimation,
};
use clay_layout::{grow, Declaration};
use clay_skia::clay_skia_render;
use gles_context::select_and_init_gles_context;
use skia::{create_skia_surface, init_skia};
mod clay_skia;
pub mod fps_counter;
pub mod gl;
#[macro_use]
pub mod gl_errors;
pub mod animation;
pub mod gles_context;
pub mod skia;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut context = select_and_init_gles_context();
    let mut fps_counter = fps_counter::FPSCounter::new();
    
    let mut last_time = std::time::Instant::now();
    let (mut skia_context, mut skia_surface) = init_skia(context.as_mut())?;
    let (screen_width, screen_height) = context.size();
    let mut clay = clay_layout::Clay::new((screen_width as f32, screen_height as f32).into());

    while !context.should_close() {
        let (screen_width, screen_height) = context.size();
        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(last_time).as_secs_f32();
        last_time = current_time;

        // Render
        gl!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        gl!(gl::ClearColor(0., 0., 0., 1.));
        let mut c = clay.begin::<_, ()>();

        c.with(
            &Declaration::new()
                .id(c.id("red_rectangle"))
                .background_color((0xFF, 0x00, 0x00).into()),
            |_| {},
        );
        
        clay_skia_render(skia_surface.canvas(), c.end());
        
        skia_context.flush(None);
        context.swap_buffers();

        if let Some(fps) = fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
        if skia_surface.width() != screen_width as _ || skia_surface.height() != screen_height as _
        {
            skia_surface = create_skia_surface(&mut skia_context, screen_width, screen_height)?;
            clay.layout_dimensions((screen_width as f32, screen_height as f32).into());
        }
    }
    Ok(())
}
