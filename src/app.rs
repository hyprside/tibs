use super::FONTS;

use crate::animation::{Animation, AnimationStateTracker, BasicAnimation};
use crate::background::Background;
use crate::cursor::Cursor;
use crate::custom_elements::CustomElements;
use crate::fps_counter::FPSCounter;
use crate::frame_pool::FramePool;
use crate::loading_screen::LoadingScreen;
use crate::login::LoginManager;
use crate::login::LoginScreen;
use crate::progress_watcher::ProgressWatcher;
use crate::session_manager::{self, SessionManager};
use crate::skia::SkiaContext;
use crate::{gl, skia};
use assets_manager::AssetCache;
use clay_layout::{fixed, grow, Declaration};
use rustamarine::screen::Screen;
use skia_safe::Rect;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
#[derive(Default, Copy, Clone)]
pub enum LoginAnimationDirection {
	FadeOut,
	#[default]
	FadeIn,
}
pub struct AppState<'a> {
	pub boot_progress: ProgressWatcher,
	pub fps_counter: FPSCounter,
	pub last_time: std::time::Instant,
	pub clay: clay_layout::Clay,
	pub skia: Option<SkiaContext>,
	pub assets: Rc<AssetCache>,
	pub loading_screen: LoadingScreen,
	pub login_screen: LoginScreen,
	pub cursor: Cursor,
	pub screen_slide_animation: BasicAnimation,
	pub show_login_screen: bool,
	pub screen_slide_animation_progress: f32,
	pub devtools: bool,
	pub background: Background,
	pub should_exit: bool,
	pub login_manager: LoginManager,
	pub session_manager: SessionManager,
	pub login_animation: AnimationStateTracker,
	pub login_animation_direction: LoginAnimationDirection,
	pub scroll_velocity: (f32, f32),
	pub frame_pool: FramePool<'a>,
}

impl AppState<'_> {
	pub fn update(&mut self, rmar: &mut rustamarine::Rustamarine, screen: &mut Screen) {
		if !self.session_manager.is_on_tibs_tty() {
			sleep(Duration::from_millis(2));
			return;
		}
		self.ensure_skia_context(screen);

		// Clamp mouse position to screen boundaries
		let screen_width = screen.get_width() as i32;
		let screen_height = screen.get_height() as i32;
		let mouse_x = rmar.get_mouse_x();
		let mouse_y = rmar.get_mouse_y();

		rmar.set_mouse_x(mouse_x.max(0).min(screen_width));
		rmar.set_mouse_y(mouse_y.max(0).min(screen_height));

		let progress = self.boot_progress.poll_progress();

		let current_time = std::time::Instant::now();

		// Handle escape key to exit
		if rmar.is_key_down(rustamarine::keys::KEY_Escape)
			&& std::env::var("TIBS_DEV_MODE") == Ok("1".to_string())
		{
			self.should_exit = true;
			return;
		}

		// Toggle devtools with Caps Lock
		if rmar.is_key_pressed(rustamarine::keys::KEY_Caps_Lock)
			&& std::env::var("TIBS_DEV_MODE") == Ok("1".to_string())
		{
			self.devtools = !self.devtools;
			self.clay.set_debug_mode(self.devtools);
		}

		// Calculate delta time
		let delta = current_time.duration_since(self.last_time).as_secs_f32();
		self.last_time = current_time;

		if rmar.is_key_down(rustamarine::keys::KEY_p)
			&& std::env::var("TIBS_DEV_MODE") == Ok("1".to_string())
		{
			self.login_animation.update(delta);
		}
		if rmar.is_key_down(rustamarine::keys::KEY_P)
			&& std::env::var("TIBS_DEV_MODE") == Ok("1".to_string())
		{
			self.login_animation.update(-delta);
		}
		// Get mouse position
		let mouse_position = (rmar.get_mouse_x() as f32, rmar.get_mouse_y() as f32);
		// Update animation
		if let Some((_, p)) = self
			.screen_slide_animation
			.update(if self.show_login_screen {
				delta
			} else {
				-delta
			})
			.get(0)
		{
			self.screen_slide_animation_progress = *p;
			self.background.time_offset = self.screen_slide_animation_progress * 5.0;
		}

		self.login_screen.update(
			&mut self.clay,
			rmar,
			&mut self.login_manager,
			&self.session_manager,
		);
		self.loading_screen.update(&progress, delta);
		// Update background
		self.background.update(delta);
		if self.session_manager.is_on_tibs_tty() {
			self.login_animation_direction = LoginAnimationDirection::FadeIn;
			if self
				.login_screen
				.authenticated_with_no_session(&self.login_manager, &self.session_manager)
				.is_some()
				&& !self.login_screen.session_open_failed()
			{
				self.login_animation_direction = LoginAnimationDirection::FadeOut;
				if self
					.login_animation
					.has_finished_this_frame("hide_background")
				{
					self
						.login_screen
						.start_session(&self.login_manager, &mut self.session_manager);
					self
						.login_manager
						.reset_login_state(self.login_screen.username());
				}
			}
		} else {
			self.login_animation_direction = LoginAnimationDirection::FadeOut;
		}
		match self.login_animation_direction {
			LoginAnimationDirection::FadeOut => {
				self.login_animation.update(delta);
			}
			LoginAnimationDirection::FadeIn => {
				self.login_animation.update(-delta);
			}
		}

		self
			.login_manager
			.get_current_login_state(self.login_screen.username());
		// Update clay pointer state
		self.clay.pointer_state(
			(mouse_position.0, mouse_position.1).into(),
			rmar.is_mouse_button_down(0),
		);
		// Update scroll containers
		// captura input de scroll cru
		let raw_scroll_x = rmar.get_mouse_scroll_x() as f32;
		let raw_scroll_y = rmar.get_mouse_scroll_y() as f32;

		// adiciona input à velocidade acumulada
		if rmar.is_key_down(rustamarine::keys::KEY_Shift_L) {
			self.scroll_velocity.0 += -raw_scroll_y * 0.05;
			self.scroll_velocity.1 += -raw_scroll_x * 0.05;
		} else {
			self.scroll_velocity.0 += -raw_scroll_x * 0.05;
			self.scroll_velocity.1 += -raw_scroll_y * 0.05;
		}

		// aplica velocidade ao clay
		self.clay.update_scroll_containers(
			false,
			(self.scroll_velocity.0, self.scroll_velocity.1).into(),
			// (0.0, -2.0).into(),
			delta,
		);

		// aplica damping/exponencial decaimento
		let damping = 12.0; // maior = mais rápido para parar
		self.scroll_velocity.0 *= (-damping * delta).exp();
		self.scroll_velocity.1 *= (-damping * delta).exp();

		// Hot reload assets
		self.assets.hot_reload();
	}
	pub fn render(&mut self, screen: &mut Screen) {
		if !self.session_manager.is_on_tibs_tty() {
			sleep(Duration::from_millis(2));
			return;
		}
		macro_rules! skia {
			() => {
				self.skia.as_mut().unwrap()
			};
		}
		skia!().use_screen(&mut *screen);
		macro_rules! canvas {
			() => {
				skia!().canvas()
			};
		}

		let rmar = screen.get_rustamarine();

		gl!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));

		canvas!().save_layer_alpha_f(
			Rect::new(
				0.,
				0.,
				screen.get_width() as f32,
				screen.get_height() as f32,
			),
			1.0
				- self
					.login_animation
					.get_animation_progress("hide_background"),
		);
		self.background.render(canvas!());
		canvas!().restore();
		canvas!().save_layer_alpha_f(
			None,
			1.0 - self.login_animation.get_animation_progress("hide_ui"),
		);
		self.render_ui(&mut *screen);
		canvas!().restore();

		let progress = self.boot_progress.poll_progress();

		if progress.finished && self.login_animation.get_animation_progress("hide_ui") < 1.0 {
			self.cursor.render(canvas!(), &rmar, "default");
		}

		skia!().flush();
		// Update FPS counter
		if let Some(fps) = self.fps_counter.tick() {
			println!("FPS: {:.2}", fps);
		}
		screen.swap_buffers();
		self.frame_pool.reset();
	}
	pub fn ensure_skia_context(&mut self, screen: &mut Screen) {
		let (screen_width, screen_height) = (screen.get_width() as u32, screen.get_height() as u32);

		if self.skia.is_none() {
			let c = SkiaContext::init_skia(screen);
			self
				.clay
				.set_layout_dimensions((screen_width as f32, screen_height as f32).into());

			screen
				.get_rustamarine()
				.set_mouse_x(screen_width as i32 / 2);
			screen
				.get_rustamarine()
				.set_mouse_y(screen_height as i32 / 2);
			self.skia = Some(c);
		} else if let Some(ctx) = &mut self.skia {
			if ctx.set_size(screen_width, screen_height) {
				self
					.clay
					.set_layout_dimensions((screen_width as f32, screen_height as f32).into());
				screen
					.get_rustamarine()
					.set_mouse_x(screen_width as i32 / 2);
				screen
					.get_rustamarine()
					.set_mouse_y(screen_height as i32 / 2);
			}
		}
	}
	fn render_ui(&mut self, screen: &mut Screen) {
		macro_rules! skia {
			() => {
				self.skia.as_mut().unwrap()
			};
		}

		let screen_height = screen.get_height() as f32;
		let mut c = self.clay.begin::<_, CustomElements>();
		let frame_pool = self.frame_pool.begin_alloc();
		let camera_y = self.screen_slide_animation_progress * screen_height;
		let progress = self.boot_progress.poll_progress();
		let rmar = screen.get_rustamarine();
		c.with(
			Declaration::new()
				.layout()
				.direction(clay_layout::layout::LayoutDirection::TopToBottom)
				.width(grow!())
				.height(grow!())
				.end()
				.clip(true, true, (0.0, -camera_y).into()),
			|c| {
				let continue_anyway_button_id = c.id("loading_continue_anyway_button");
				let continue_anyway_button_clicked =
					c.pointer_over(continue_anyway_button_id) && rmar.is_mouse_button_released(0);
				if continue_anyway_button_clicked
					|| (self.loading_screen.get_animation_progress("progress") >= 0.99
						&& !progress.has_failed_services())
				{
					self.show_login_screen = true;
				}

				c.with(
					Declaration::new()
						.layout()
						.width(grow!())
						.height(fixed!(screen_height))
						.end(),
					|c| {
						self.loading_screen.render(progress, c);
					},
				);
				c.with(
					Declaration::new()
						.layout()
						.width(grow!())
						.height(fixed!(screen_height as f32))
						.end(),
					|c| {
						self.login_screen.render(
							c,
							&self.login_manager,
							&self.session_manager,
							&frame_pool,
							&rmar,
						);
					},
				);
			},
		);
		skia::clay_renderer::clay_skia_render(
			skia!().canvas(),
			c.end(),
			CustomElements::render,
			&FONTS,
		);
	}
}
