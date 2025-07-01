#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

#[cfg(feature = "opengl_loader")]
pub mod opengl {
	include!(concat!(env!("OUT_DIR"), "/opengl_bindings.rs"));
}
include!(concat!(env!("OUT_DIR"), "/rustamarine_bindings.rs"));

#[no_mangle]
pub extern "C" fn rmarFreeRustClosure(ptr: *mut std::ffi::c_void) {
	if !ptr.is_null() {
		unsafe {
			let _: Box<Box<dyn FnMut()>> = Box::from_raw(ptr as *mut Box<dyn FnMut()>);
		}
	}
}
