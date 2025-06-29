use rustamarine::opengl;

fn main() {
	let mut rustamarine = rustamarine::Rustamarine::new();
	rustamarine::opengl::load_with(|s| rustamarine.get_opengl_proc_address(s) as *const _);
	loop {
		let screens = rustamarine.screens();
		for (i, mut screen) in screens.into_iter().enumerate() {
			use rand::Rng;
			screen.set_on_render(move |mut screen| {
				let mut rng = rand::rng();

				let r: f32 = rng.random_range(0.0..1.0);
				let g: f32 = rng.random_range(0.0..1.0);
				let b: f32 = rng.random_range(0.0..1.0);
				println!("Rendering screen {i} with color ({r}, {g}, {b})");
				screen.use_screen();
				unsafe { opengl::ClearColor(r, g, b, 1.0); }
				unsafe { opengl::Clear(opengl::COLOR_BUFFER_BIT); }
				screen.swap_buffers();
			});
		}
		rustamarine.poll_events();
	}
}
