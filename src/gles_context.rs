use std::ffi::c_void;

pub mod drm;
#[cfg(feature = "glfw")]
pub mod glfw;

pub trait GlesContext {
    fn swap_buffers(&self);
    fn size(&self) -> (u32, u32);
    fn should_close(&self) -> bool {
        false
    }
    fn get_proc_address(&mut self, fn_name: &str) -> *const c_void;
}
pub fn select_and_init_gles_context() -> Box<dyn GlesContext> {
    let display_is_defined = std::env::var("DISPLAY").is_ok();
    let context: Box<dyn GlesContext> = 'a: {
        if display_is_defined {
            #[cfg(not(feature = "glfw"))]
            println!("[WARN] GLFW feature is not enabled, ignoring DISPLAY variable");
            #[cfg(feature = "glfw")]
            break 'a Box::new(glfw::GlfwGlesContext::new("Tiago's Incredible Boot Screen"))
        }
        Box::new(drm::DrmGlesContext::new_from_default_card())
    };

    context
}