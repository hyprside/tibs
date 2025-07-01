use rustamarine::{keys::KEY_Escape, opengl};

fn main() {
	let mut rustamarine = rustamarine::Rustamarine::new();
	rustamarine::opengl::load_with(|s| rustamarine.get_opengl_proc_address(s) as *const _);
	loop {
		if rustamarine.is_key_pressed(KEY_Escape) {
			break;
		}
		for mut screen in rustamarine.screens() {
			use rand::Rng;
			screen.set_on_render(move |mut screen| {
				let mut rng = rand::rng();

				let r: f32 = rng.random_range(0.0..1.0);
				let g: f32 = rng.random_range(0.0..1.0);
				let b: f32 = rng.random_range(0.0..1.0);
				screen.use_screen();

				unsafe {
					opengl::Clear(opengl::COLOR_BUFFER_BIT);
				}
				unsafe {
					opengl::ClearColor(r, g, b, 1.0);
				}
				screen.swap_buffers();
			});
		}
		rustamarine.poll_events();
	}
}
