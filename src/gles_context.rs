use std::ffi::c_void;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderTargetOrigin {
    TopLeft,
    BottomLeft,
}

pub trait GlesContext {
    fn swap_buffers(&mut self) -> bool;
    fn size(&self) -> (u32, u32);
    fn get_proc_address(&mut self, fn_name: &str) -> *const c_void;
    fn framebuffer_id(&self) -> u32 {
        0
    }
    fn render_target_origin(&self) -> RenderTargetOrigin {
        RenderTargetOrigin::BottomLeft
    }
    fn hint_pause_rendering(&mut self) {}
    fn hint_resume_rendering(&mut self) {}
}
