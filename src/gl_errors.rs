use crate::gl;

pub fn check_gl_error() {
    unsafe {
        let mut error = gl::GetError();
        while error != gl::NO_ERROR {
            let error_message = match error {
                gl::INVALID_ENUM => "INVALID_ENUM",
                gl::INVALID_VALUE => "INVALID_VALUE",
                gl::INVALID_OPERATION => "INVALID_OPERATION",
                gl::OUT_OF_MEMORY => "OUT_OF_MEMORY",
                _ => "UNKNOWN_ERROR",
            };
            let backtrace = std::backtrace::Backtrace::capture();
            println!("OpenGL Error: {}", error_message);
            println!("{:#?}", backtrace);
            error = gl::GetError();
        }
    }
}

#[macro_export]
macro_rules! gl {
    (gl::$call:ident ( $($arg:expr),* )) => {{
        let result = unsafe { gl::$call($($arg),*) };
        $crate::gl_errors::check_gl_error();
        result
    }};
}
