pub mod drm;
#[cfg(feature = "glfw")]
pub mod glfw;
use crate::gl;

pub trait GlesContext {
    fn gles(&self) -> &gl::Gles2;
    fn swap_buffers(&self);
    fn size(&self) -> (u32, u32);
    fn should_close(&self) -> bool {
        false
    }
}