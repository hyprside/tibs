use glfw::{Action, Context, Glfw, Key, PWindow, WindowHint};
use std::cell::RefCell;
use std::ffi::c_void;
use crate::gl;

use super::GlesContext;

pub struct GlfwGlesContext {
    glfw: RefCell<Glfw>,
    window: RefCell<PWindow>
}

impl GlfwGlesContext {
    pub fn new(width: u32, height: u32, title: &str) -> Self {
        let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();

        glfw.window_hint(WindowHint::ContextVersion(3, 0));
        glfw.window_hint(WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
        glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
        glfw.window_hint(WindowHint::ClientApi(glfw::ClientApiHint::OpenGlEs));
        glfw.window_hint(WindowHint::ContextCreationApi(glfw::ContextCreationApi::Egl));
        let (mut window, _) = glfw.with_primary_monitor(|glfw, m| {
            glfw.create_window(width, height, title,
                m.map_or(glfw::WindowMode::Windowed, |m| glfw::WindowMode::FullScreen(m)))
            }).expect("Failed to create GLFW window.");

        window.make_current();
        window.set_key_polling(true);
        window.set_cursor_mode(glfw::CursorMode::Hidden);
        let mut context = GlfwGlesContext {
            glfw: RefCell::new(glfw),
            window: RefCell::new(window)
        };
        gl::load_with(|symbol| context.get_proc_address(symbol));
        println!("loaded gl crate");
        context
    }
}

impl GlesContext for GlfwGlesContext {

    fn get_proc_address(&mut self, fn_name: &str) -> *const c_void {
        self.window.borrow_mut().get_proc_address(fn_name)
    }
    fn swap_buffers(&self) {
        self.glfw.borrow_mut().poll_events();
        let mut window = self.window.borrow_mut();
        window.swap_buffers();
        if window.get_key(Key::Escape) == Action::Press {
            window.set_should_close(true);
        }
        let (width, height) = window.get_size();
        unsafe {gl::Viewport(0, 0, width, height)};
    }

    fn size(&self) -> (u32, u32) {
        let (width, height) = self.window.borrow().get_size();
        (width as u32, height as u32)
    }
    fn should_close(&self) -> bool {
        self.window.borrow().should_close()
    }
}
