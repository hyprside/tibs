pub mod drm;
use crate::gl;

pub trait GlesContext {
    fn gles(&self) -> &gl::Gles2;
    fn swap_buffers(&self);
    fn size(&self) -> (u32, u32);
}