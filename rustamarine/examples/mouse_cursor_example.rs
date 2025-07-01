use std::ffi::CStr;

use rustamarine::{keys::KEY_Escape, opengl};

fn main() {
	let mut rustamarine = rustamarine::Rustamarine::new();
	rustamarine::opengl::load_with(|s| rustamarine.get_opengl_proc_address(s) as *const _);
	// Vertex and fragment shader sources for drawing a point
	const VERT_SRC: &CStr = cr#"
		#version 320 es
		precision mediump float;
		layout(location = 0) in vec2 position;
		void main() {
			gl_Position = vec4(position, 0.0, 1.0);
			gl_PointSize = 10.0;
		}
	"#;
	const FRAG_SRC: &CStr = cr#"
		#version 320 es
		precision mediump float;
		out vec4 color;
		void main() {
			color = vec4(1.0, 1.0, 0.0, 1.0);
		}
	"#;

	// Compile and link shader program once
	let program: u32 = unsafe {
		let vert = opengl::CreateShader(opengl::VERTEX_SHADER);
		opengl::ShaderSource(vert, 1, &VERT_SRC.as_ptr(), std::ptr::null());
		opengl::CompileShader(vert);

		let mut status = 0;
		opengl::GetShaderiv(vert, opengl::COMPILE_STATUS, &mut status);
		if status == 0 {
			panic!("Vertex shader compilation failed");
		}

		let frag = opengl::CreateShader(opengl::FRAGMENT_SHADER);
		opengl::ShaderSource(frag, 1, &FRAG_SRC.as_ptr(), std::ptr::null());
		opengl::CompileShader(frag);

		opengl::GetShaderiv(frag, opengl::COMPILE_STATUS, &mut status);
		if status == 0 {
			panic!("Fragment shader compilation failed");
		}

		let prog = opengl::CreateProgram();
		opengl::AttachShader(prog, vert);
		opengl::AttachShader(prog, frag);
		opengl::LinkProgram(prog);

		opengl::GetProgramiv(prog, opengl::LINK_STATUS, &mut status);
		if status == 0 {
			panic!("Shader program linking failed");
		}

		opengl::DeleteShader(vert);
		opengl::DeleteShader(frag);

		prog
	};

	loop {
		if rustamarine.is_key_pressed(KEY_Escape) {
			break;
		}
		let mouse_x = rustamarine.get_mouse_x();
		let mouse_y = rustamarine.get_mouse_y();
		let screens = rustamarine.screens();
		for (_i, mut screen) in screens.into_iter().enumerate() {
			let cursor_x = mouse_x;
			let cursor_y = mouse_y;

			screen.set_on_render(move |mut screen| {
				screen.use_screen();

				unsafe {
					opengl::Clear(opengl::COLOR_BUFFER_BIT);
					opengl::ClearColor(0.0, 0.0, 0.0, 1.0);

					opengl::UseProgram(program);

					// Get screen size for normalization
					let w = screen.get_width() as f32;
					let h = screen.get_height() as f32;

					// Convert cursor position to normalized device coordinates (-1..1)
					let x_ndc = (2.0 * cursor_x as f32 / w) - 1.0;
					let y_ndc = (2.0 * cursor_y as f32 / h) - 1.0;

					let point: [f32; 2] = [x_ndc, y_ndc];

					let mut vbo = 0;
					opengl::GenBuffers(1, &mut vbo);
					opengl::BindBuffer(opengl::ARRAY_BUFFER, vbo);
					opengl::BufferData(
						opengl::ARRAY_BUFFER,
						(2 * std::mem::size_of::<f32>()) as isize,
						point.as_ptr() as *const _,
						opengl::STATIC_DRAW,
					);

					opengl::EnableVertexAttribArray(0);
					opengl::VertexAttribPointer(0, 2, opengl::FLOAT, 0, 0, 0 as *const _);

					opengl::DrawArrays(opengl::POINTS, 0, 1);

					opengl::DisableVertexAttribArray(0);
					opengl::BindBuffer(opengl::ARRAY_BUFFER, 0);
					opengl::DeleteBuffers(1, &vbo);

					opengl::UseProgram(0);
				}

				screen.swap_buffers();
			});
		}
		rustamarine.poll_events();
	}
}
