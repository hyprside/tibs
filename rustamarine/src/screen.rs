use std::{marker::PhantomData, mem::ManuallyDrop};

use crate::{Rustamarine, RustamarineRef};

use super::sys;
pub struct Screen<'a> {
	inner: &'a mut sys::RustamarineScreen,
}

impl<'a> Screen<'a> {
	pub fn get_rustamarine(&mut self) -> RustamarineRef<'a> {
		crate::RustamarineRef {
			inner: ManuallyDrop::new(Rustamarine {
				inner: unsafe { &mut *sys::rmarFromScreen(self.inner) },
			}),
			_e: PhantomData,
		}
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
	pub fn get_width(&self) -> u32 {
		unsafe { sys::rmarScreenGetWidth(self.inner) }
	}

	pub fn get_height(&self) -> u32 {
		unsafe { sys::rmarScreenGetHeight(self.inner) }
	}

	pub fn get_refresh_rate(&self) -> f32 {
		unsafe { sys::rmarScreenGetRefreshRate(self.inner) }
	}

	pub fn get_name(&self) -> &str {
		unsafe {
			let c_str = sys::rmarScreenGetName(self.inner);
			std::ffi::CStr::from_ptr(c_str).to_str().unwrap_or_default()
		}
	}

	pub fn is_enabled(&self) -> bool {
		unsafe { sys::rmarScreenIsEnabled(self.inner) }
	}
}
impl super::Rustamarine {
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
}
