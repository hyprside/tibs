#![allow(unsafe_op_in_unsafe_fn)]
use std::backtrace;

use gles_context::select_and_init_gles_context;

pub mod fps_counter;
pub mod gl;
pub mod gles_context;

fn main() {
    let context = select_and_init_gles_context();
    let gles = context.gles();
    let mut fps_counter = fps_counter::FPSCounter::new();
    let vbo = create_triangle_buffer(gles);
    
    let mut vao = 0;
    unsafe {

        gles.GenVertexArrays(1, &mut vao);
        gles.BindVertexArray(vao);
        gles.BindBuffer(gl::ARRAY_BUFFER, vbo);
        check_gl_error(gles);
        gles.VertexAttribPointer(
            0,
            3,
            gl::FLOAT,
            gl::FALSE,
            3 * std::mem::size_of::<f32>() as i32,
            std::ptr::null(),
        );
        check_gl_error(gles);
    }
    let program = create_shader_program(gles);
    check_gl_error(context.gles());
    while !context.should_close() {
        unsafe {
            gles.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            check_gl_error(context.gles());
            gles.ClearColor(1.0, 0.0, 0.0, 1.0);
            check_gl_error(context.gles());
            gles.UseProgram(program);
            check_gl_error(context.gles());
            render_triangle(gles);
            gles.Flush();
            context.swap_buffers();
        }
        if let Some(fps) = fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
    }
}

fn create_shader_program(gles: &gl::Gles2) -> u32 {
    let vertex_shader_source = r#"#version 100
        attribute vec3 position;
        void main() {
            gl_Position = vec4(position, 1.0);
        }
    "#;

    let fragment_shader_source = r#"#version 100
        void main() {
            gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);
        }
    "#;

    unsafe fn compile_shader(gles: &gl::Gles2, source: &str, shader_type: u32) -> u32 {
        let shader = gles.CreateShader(shader_type);
        let cstr = std::ffi::CString::new(source).unwrap();
        let source = cstr.as_bytes_with_nul();
        let sources = [source.as_ptr()];
        gles.ShaderSource(shader, 1, sources.as_ptr().cast(), std::ptr::null());
        check_gl_error(gles);
        gles.CompileShader(shader);
        check_gl_error(gles);

        let mut success = 0;
        gles.GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        check_gl_error(gles);
        if success == 0 {
            let mut len = 0;
            gles.GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            check_gl_error(gles);
            let mut buffer = vec![0u8; len as usize];
            gles.GetShaderInfoLog(
                shader,
                len,
                std::ptr::null_mut(),
                buffer.as_mut_ptr() as *mut i8,
            );
            check_gl_error(gles);
            panic!(
                "Shader compilation failed: {}",
                String::from_utf8_lossy(&buffer)
            );
        }

        shader
    }

    let vertex_shader = unsafe { compile_shader(gles, vertex_shader_source, gl::VERTEX_SHADER) };
    let fragment_shader =
        unsafe { compile_shader(gles, fragment_shader_source, gl::FRAGMENT_SHADER) };

    let program = unsafe { gles.CreateProgram() };
    check_gl_error(gles);
    unsafe {
        gles.AttachShader(program, vertex_shader);
        check_gl_error(gles);
        gles.AttachShader(program, fragment_shader);
        check_gl_error(gles);
        gles.LinkProgram(program);
        check_gl_error(gles);
        let mut success = 0;
        gles.GetProgramiv(program, gl::LINK_STATUS, &mut success);
        check_gl_error(gles);
        if success == 0 {
            let mut len = 0;
            gles.GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
            check_gl_error(gles);
            let mut buffer = vec![0u8; len as usize];
            gles.GetProgramInfoLog(
                program,
                len,
                std::ptr::null_mut(),
                buffer.as_mut_ptr() as *mut i8,
            );
            check_gl_error(gles);
            panic!(
                "Program linking failed: {}",
                String::from_utf8_lossy(&buffer)
            );
        }

        gles.DeleteShader(vertex_shader);
        check_gl_error(gles);
        gles.DeleteShader(fragment_shader);
        check_gl_error(gles);
    }

    program
}
fn create_triangle_buffer(gles: &gl::Gles2) -> u32 {
    let vertices: [f32; 9] = [
        0.0, 0.5, 0.0, // Top vertex
        -0.5, -0.5, 0.0, // Bottom left vertex
        0.5, -0.5, 0.0, // Bottom right vertex
    ];

    let mut vbo: u32 = 0;
    unsafe {
        gles.GenBuffers(1, &mut vbo);
        check_gl_error(gles);

        gles.BindBuffer(gl::ARRAY_BUFFER, vbo);
        check_gl_error(gles);

        gles.BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const _,
            gl::STATIC_DRAW,
        );
        check_gl_error(gles);
    }
    vbo
}

fn render_triangle(gles: &gl::Gles2) {
    unsafe {
        gles.EnableVertexAttribArray(0);
        check_gl_error(gles);
        gles.DrawArrays(gl::TRIANGLES, 0, 3);
        check_gl_error(gles);
        gles.DisableVertexAttribArray(0);
    }
}

fn check_gl_error(gles: &gl::Gles2) {
    unsafe {
        let mut error = gles.GetError();
        while error != gl::NO_ERROR {
            let error_message = match error {
                gl::INVALID_ENUM => "INVALID_ENUM",
                gl::INVALID_VALUE => "INVALID_VALUE",
                gl::INVALID_OPERATION => "INVALID_OPERATION",
                gl::OUT_OF_MEMORY => "OUT_OF_MEMORY",
                _ => "UNKNOWN_ERROR",
            };
            panic!("OpenGL Error: {}", error_message);
            error = gles.GetError();
        }
    }
}
