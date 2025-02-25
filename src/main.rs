#![allow(unsafe_op_in_unsafe_fn)]

use animation::{
    Animation, BackAndForthAnimation, BasicAnimation,
    colors::{interpolate_color_normalized, rgb_to_norm},
    easing,
};
use gl_errors::check_gl_error;
use gles_context::select_and_init_gles_context;

pub mod fps_counter;
pub mod gl;
#[macro_use]
pub mod gl_errors;
pub mod animation;
pub mod gles_context;
pub mod shader;
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let context = select_and_init_gles_context();
    let mut fps_counter = fps_counter::FPSCounter::new();
    let mut background_animation = BasicAnimation::new(
        "background",
        1.0,
        easing::ease_in_out_cubic,
    );
    let mut last_time = std::time::Instant::now();
    let mut background_color = (0.0, 0.0, 0.0);
    while !context.should_close() {
        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(last_time).as_secs_f32();
        last_time = current_time;

        gl!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        gl!(gl::ClearColor(
            background_color.0,
            background_color.1,
            background_color.2,
            1.0
        ));
        context.swap_buffers();
        for (id, progress) in background_animation.update(delta) {
            match id.as_str() {
                "background" => {
                    background_color = interpolate_color_normalized(
                        rgb_to_norm("#000000"),
                        rgb_to_norm("#0F1419"),
                        progress,
                    )
                }
                _ => {}
            }
        }
        if let Some(fps) = fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
    }
    Ok(())
}
