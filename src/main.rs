#![allow(unsafe_op_in_unsafe_fn)]
use gles_context::select_and_init_gles_context;

pub mod gl;
pub mod gles_context;
pub mod fps_counter;

fn main() {
    let context = select_and_init_gles_context();
    let gles = context.gles();
    let mut fps_counter = fps_counter::FPSCounter::new();

    while !context.should_close() {
        unsafe {
            gles.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gles.ClearColor(1.0, 1.0, 1.0, 1.0);
            context.swap_buffers();
        }
        if let Some(fps) = fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
    }
}
