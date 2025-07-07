#![allow(unsafe_op_in_unsafe_fn)]

use background::Background;
#[path = "components/background.rs"]
pub mod background;
#[path = "components/custom_elements.rs"]
pub mod custom_elements;
#[path = "utils/fps_counter.rs"]
pub mod fps_counter;
#[path = "utils/gl.rs"]
pub mod gl;
#[macro_use]
#[path = "utils/animation.rs"]
pub mod animation;
#[path = "components/cursor.rs"]
pub mod cursor;
#[path = "screens/loading_screen.rs"]
pub mod loading_screen;
#[path = "login/login.rs"]
pub mod login;
#[path = "utils/progress_watcher.rs"]
pub mod progress_watcher;

#[path = "login/session_manager.rs"]
pub mod session_manager;

#[path = "skia/context.rs"]
pub mod skia;

#[path = "components/textbox.rs"]
pub mod textbox;

pub mod app;
#[path = "utils/frame_pool.rs"]
pub mod frame_pool;
#[path = "utils/tty.rs"]
pub mod tty;

pub type TibsClayScope<'clay, 'render> =
	SkiaClayScope<'clay, 'render, custom_elements::CustomElements>;

use crate::{
	animation::{
		easing::{ease_in_out_circ, ease_in_quad},
		BasicAnimation, DelayAnimation,
	},
	cursor::Cursor,
	frame_pool::FramePool,
	loading_screen::LoadingScreen,
	login::{LoginManager, LoginScreen},
	session_manager::SessionManager,
	skia::clay_renderer::{create_measure_text_function, SkiaClayScope},
};
use assets_manager::AssetCache;
use skia_safe::{
	font_style::{Slant, Weight, Width},
	FontMgr, FontStyle, Typeface,
};
use std::{
	rc::Rc,
	sync::{LazyLock, Mutex},
	thread::sleep,
	time::{Duration, Instant},
};

static UBUNTU_FONT: LazyLock<Typeface> = LazyLock::new(|| {
	FontMgr::new()
		.match_family_style("UbuntuSans NF", FontStyle::normal())
		.unwrap()
});
static BOLD_UBUNTU_FONT: LazyLock<Typeface> = LazyLock::new(|| {
	FontMgr::new()
		.match_family_style("UbuntuSans NF", FontStyle::bold())
		.unwrap()
});
static MEDIUM_UBUNTU_FONT: LazyLock<Typeface> = LazyLock::new(|| {
	FontMgr::new()
		.match_family_style(
			"UbuntuSans NF",
			FontStyle::new(Weight::MEDIUM, Width::NORMAL, Slant::Upright),
		)
		.unwrap()
});
pub static FONTS: LazyLock<Vec<&Typeface>> =
	LazyLock::new(|| vec![&UBUNTU_FONT, &BOLD_UBUNTU_FONT, &MEDIUM_UBUNTU_FONT]);

fn main() -> color_eyre::Result<()> {
	color_eyre::install()?;
	env_logger::init();

	// Create clay layout
	let mut clay = clay_layout::Clay::new((0.0, 0.0).into());
	clay.set_measure_text_function(create_measure_text_function(&FONTS));
	let mut rmar = rustamarine::Rustamarine::new();
	gl::load_with(|n| rmar.get_opengl_proc_address(n));

	// Create assets
	let assets = Rc::new(AssetCache::new(
		std::env::var("TIBS_ASSETS_FOLDER").unwrap_or("assets".into()),
	)?);

	// Create app state
	let app_state = Mutex::new(app::AppState {
		boot_progress: progress_watcher::ProgressWatcher::new(),
		fps_counter: fps_counter::FPSCounter::new(),
		last_time: std::time::Instant::now(),
		scroll_velocity: (0., 0.),
		clay,
		skia: None,
		loading_screen: LoadingScreen::new(&assets),
		login_screen: LoginScreen::new(&assets),
		cursor: Cursor::new(None),
		screen_slide_animation: BasicAnimation::new("screen_slide", 1.5, ease_in_out_circ),
		show_login_screen: false,
		screen_slide_animation_progress: 0.0,
		devtools: false,
		background: Background::new(Rc::clone(&assets)),
		assets,
		should_exit: false,
		login_manager: LoginManager::new(),
		session_manager: SessionManager::new(),
		login_animation: seq!(
			BasicAnimation::new("hide_ui", 0.2, ease_in_quad),
			DelayAnimation::new(
				0.1,
				BasicAnimation::new("hide_background", 0.3, ease_in_quad)
			)
		)
		.into(),
		login_animation_direction: Default::default(),
		frame_pool: FramePool::new(),
	});
	while !app_state.lock().unwrap().should_exit {
		let mut screens = rmar.screens();
		let Some(mut screen) = screens.first_mut() else {
			rmar.poll_events();
			continue;
		};

		// Set render callback
		screen.set_on_render(|mut screen| {
			let mut app_state = app_state.lock().unwrap();
			app_state.update(&mut screen.get_rustamarine(), &mut screen);
			app_state.render(&mut screen);
		});

		rmar.poll_events();
	}
	Ok(())
}
