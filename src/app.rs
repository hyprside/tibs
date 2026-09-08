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
use crate::skia::SkiaContext;
use crate::{gl, skia};
use assets_manager::AssetCache;
use clay_layout::{fixed, grow, Declaration};
use skia_safe::Rect;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use tibs_service_definitions::SessionManager;
use xkbcommon::xkb::Keysym;

use crate::context::TibsContext;
use crate::input::MouseButton;
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
    pub session_manager: Box<dyn SessionManager>,
    pub last_login_session_active: Option<bool>,
    pub login_animation: AnimationStateTracker,
    pub login_animation_direction: LoginAnimationDirection,
    pub scroll_velocity: (f32, f32),
    pub frame_pool: FramePool<'a>,
}

impl AppState<'_> {
    pub fn update(&mut self, context: &mut dyn TibsContext) {
        let login_session_active = self.session_manager.is_login_session_active();
        if self.last_login_session_active != Some(login_session_active) {
            log::info!("Login session active state changed: {login_session_active}");
            self.last_login_session_active = Some(login_session_active);
        }
        if !login_session_active {
            sleep(Duration::from_millis(2));
            return;
        }
        self.ensure_skia_context(context);

        let progress = self.boot_progress.poll_progress();

        let current_time = std::time::Instant::now();

        // Handle escape key to exit
        if context.is_key_down(Keysym::Escape)
            && std::env::var("TIBS_DEV_MODE") == Ok("1".to_string())
        {
            self.should_exit = true;
            return;
        }

        // Toggle devtools with Caps Lock
        if context.is_key_pressed(Keysym::Caps_Lock)
            && std::env::var("TIBS_DEV_MODE") == Ok("1".to_string())
        {
            self.devtools = !self.devtools;
            log::info!("Devtools toggled: {}", self.devtools);
            self.clay.set_debug_mode(self.devtools);
        }

        // Calculate delta time
        let delta = current_time.duration_since(self.last_time).as_secs_f32();
        self.last_time = current_time;

        if context.is_key_down(Keysym::p) && std::env::var("TIBS_DEV_MODE") == Ok("1".to_string()) {
            self.login_animation.update(delta);
        }
        if context.is_key_down(Keysym::P) && std::env::var("TIBS_DEV_MODE") == Ok("1".to_string()) {
            self.login_animation.update(-delta);
        }
        // Get mouse position
        let mouse_position = context.mouse_position();
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
            context.as_input(),
            &mut self.login_manager,
            &*self.session_manager,
        );
        self.loading_screen.update(&progress, delta);
        // Update background
        self.background.update(delta);
        if self.session_manager.is_login_session_active() {
            self.login_animation_direction = LoginAnimationDirection::FadeIn;
            if self
                .login_screen
                .authenticated_with_no_session(&self.login_manager, &*self.session_manager)
                .is_some()
                && !self.login_screen.session_open_failed()
            {
                self.login_animation_direction = LoginAnimationDirection::FadeOut;
                if self
                    .login_animation
                    .has_finished_this_frame("hide_background")
                {
                    self.login_screen
                        .start_session(&self.login_manager, &*self.session_manager);
                    self.login_manager
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

        self.login_manager
            .get_current_login_state(self.login_screen.username());
        // Update clay pointer state
        self.clay.pointer_state(
            (mouse_position.0, mouse_position.1).into(),
            context.is_mouse_button_down(MouseButton::Left),
        );
        // Update scroll containers
        let (raw_scroll_x, raw_scroll_y) = context.mouse_wheel();

        // adiciona input à velocidade acumulada
        if context.is_key_down(Keysym::Shift_L) {
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
    pub fn render(&mut self, context: &mut dyn TibsContext) {
        if !self.session_manager.is_login_session_active() {
            sleep(Duration::from_millis(2));
            return;
        }
        macro_rules! skia {
            () => {
                self.skia.as_mut().unwrap()
            };
        }
        macro_rules! canvas {
            () => {
                skia!().canvas()
            };
        }
        let (screen_width, screen_height) = context.size();

        gl!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));

        canvas!().save_layer_alpha_f(
            Rect::new(0., 0., screen_width as f32, screen_height as f32),
            1.0 - self
                .login_animation
                .get_animation_progress("hide_background"),
        );
        self.background.render(canvas!());
        canvas!().restore();
        canvas!().save_layer_alpha_f(
            Rect::new(0., 0., screen_width as f32, screen_height as f32),
            1.0 - self.login_animation.get_animation_progress("hide_ui"),
        );
        self.render_ui(context);
        canvas!().restore();

        let progress = self.boot_progress.poll_progress();

        if progress.finished && self.login_animation.get_animation_progress("hide_ui") < 1.0 {
            self.cursor.render(canvas!(), context.as_input(), "default");
        }

        skia!().flush();
        // Update FPS counter
        if let Some(fps) = self.fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
        context.swap_buffers();
        self.frame_pool.reset();
    }
    pub fn ensure_skia_context(&mut self, context: &mut dyn TibsContext) {
        let (screen_width, screen_height) = context.size();

        if self.skia.is_none() {
            let c = SkiaContext::init_skia(context.as_gles_context_mut());
            self.clay
                .set_layout_dimensions((screen_width as f32, screen_height as f32).into());
            self.skia = Some(c);
        } else if let Some(ctx) = &mut self.skia {
            if ctx.set_render_target(
                screen_width,
                screen_height,
                context.as_gles_context().framebuffer_id(),
            ) {
                self.clay
                    .set_layout_dimensions((screen_width as f32, screen_height as f32).into());
            }
        }
    }
    fn render_ui(&mut self, context: &mut dyn TibsContext) {
        macro_rules! skia {
            () => {
                self.skia.as_mut().unwrap()
            };
        }

        let screen_height = context.size().1 as f32;
        let progress = self.boot_progress.poll_progress().clone();
        let mut c = self.clay.begin::<_, CustomElements>();
        let frame_pool = self.frame_pool.begin_alloc();
        let camera_y = self.screen_slide_animation_progress * screen_height;
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
                let continue_anyway_button_clicked = c.pointer_over(continue_anyway_button_id)
                    && context.is_mouse_button_released(MouseButton::Left);
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
                        self.loading_screen.render(&progress, c);
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
                            &*self.session_manager,
                            &frame_pool,
                            context.as_input(),
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
