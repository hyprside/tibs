#![allow(unsafe_op_in_unsafe_fn)]

use gles_context::{drm::DrmGlesContext, GlesContext};
pub mod gl;
pub mod gles_context;


fn main() {
    let context = DrmGlesContext::new_from_default_card();
    let gles = context.gles();
    let mut should_close = false;
    let mut start_time = std::time::Instant::now();
    let mut frame_count = 0;

    while !should_close {
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
