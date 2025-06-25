use std::ffi::c_void;

pub trait GlesContext {
    fn swap_buffers(&mut self) -> bool;
    fn size(&self) -> (u32, u32);
    fn get_proc_address(&mut self, fn_name: &str) -> *const c_void;
    fn hint_pause_rendering(&mut self) {}
    fn hint_resume_rendering(&mut self) {}
}
