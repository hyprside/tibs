use std::{
	ffi::{c_void, CString},
	str::FromStr,
};

mod sys;
#[cfg(feature = "opengl_loader")]
pub use sys::opengl;
mod input;
pub use input::keys;
pub mod screen;
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
