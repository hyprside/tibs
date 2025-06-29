use std::{
    ffi::{c_void, CString},
    str::FromStr,
};

mod sys;
#[cfg(feature = "opengl_loader")]
pub use sys::opengl;

pub struct Screen<'a> {
    inner: &'a mut sys::RustamarineScreen,
}

impl<'a> Screen<'a> {
    pub fn get_rustamarine<'b>(&'b mut self) -> &'b mut sys::Rustamarine {
        unsafe { &mut *sys::rmarFromScreen(self.inner) }
    }
    pub fn use_screen(&mut self) {
        unsafe {
            sys::rmarUseScreen(self.inner);
        }
    }

    pub fn is_vblank(&self) -> bool {
        unsafe { sys::rmarIsVBlank(self.inner) }
    }
    pub fn swap_buffers(&mut self) {
        unsafe { sys::rmarSwapBuffers(self.inner) }
    }

    pub fn set_on_render<F>(&mut self, callback: F)
    where
        F: FnMut(Self) + 'a,
    {
        use std::os::raw::c_void;
        unsafe extern "C" fn rust_on_render_trampoline(
            ctx: *mut c_void,
            inner_screen: *mut sys::RustamarineScreen,
        ) {
            let closure = &mut *(ctx as *mut Box<dyn FnMut(Screen)>);
            closure(Screen {
                inner: inner_screen.as_mut().unwrap(),
            });
        }

        let boxed: Box<Box<dyn FnMut(Self)>> = Box::new(Box::new(callback));
        let ptr = Box::into_raw(boxed) as *mut c_void;
        unsafe {
            sys::rmarScreenSetOnRender(self.inner, Some(rust_on_render_trampoline), ptr);
        }
    }
}

pub struct Rustamarine {
    inner: *mut sys::Rustamarine,
}

impl Rustamarine {
    pub fn new() -> Self {
        Self {
            inner: unsafe { sys::rmarInitialize() },
        }
    }
    pub fn poll_events(&mut self) {
        unsafe { sys::rmarPollEvents(self.inner) };
    }

    pub fn screens<'a>(&'a mut self) -> Vec<Screen<'a>> {
        let screens = unsafe { sys::rmarGetScreens(self.inner) };
        if screens.count == 0 {
            return vec![];
        }
        let slice = unsafe { std::slice::from_raw_parts(screens.screens, screens.count as usize) };
        let screens_vec = slice
            .iter()
            .map(|screen_ptr| Screen {
                inner: unsafe { screen_ptr.as_mut().unwrap() },
            })
            .collect();
        unsafe { sys::rmarFreeScreens(screens) };
        screens_vec
    }
    pub fn get_opengl_proc_address(&self, name: &str) -> *const c_void {
        let name_cstring = CString::from_str(name).unwrap();
        unsafe { sys::rmarGetProcAddress(self.inner, name_cstring.as_ptr()) }
    }
}

impl Drop for Rustamarine {
    fn drop(&mut self) {
        unsafe {
            sys::rmarTearDown(self.inner);
        }
    }
}
