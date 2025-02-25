#![allow(unsafe_op_in_unsafe_fn)]

use gles_context::{drm::DrmGlesContext, GlesContext};
#[cfg(feature = "glfw")]
use gles_context::glfw::GlfwGlesContext;
pub mod gl;
pub mod gles_context;


fn main() {
    let display_is_defined = std::env::var("DISPLAY").is_ok();
    #[cfg(feature = "glfw")]
    let context: Box<dyn GlesContext> = if display_is_defined {
        Box::new(GlfwGlesContext::new(1920 / 2, 1080 / 2, "Tiago's Incredible Boot Screen"))
    } else {
        Box::new(DrmGlesContext::new_from_default_card())
    };
    #[cfg(not(feature = "glfw"))]
    let context: Box<dyn GlesContext> = {
        if display_is_defined {
            println!("GLFW feature is not enabled, ignoring DISPLAY variable");
        }
        Box::new(DrmGlesContext::new_from_default_card())
    };

    let gles = context.gles();
    let mut start_time = std::time::Instant::now();
    let mut frame_count = 0;

    while !context.should_close() {
        unsafe {
            gles.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gles.ClearColor(1.0, 1.0, 1.0, 1.0);
            context.swap_buffers();
        }

        frame_count += 1;
        let elapsed = start_time.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            let fps = frame_count as f32 / elapsed;
            println!("FPS: {:.2}", fps);
            frame_count = 0;
            start_time = std::time::Instant::now();
        }
    }
}
