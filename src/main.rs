#![allow(unsafe_op_in_unsafe_fn)]

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
#[path = "utils/frame_pool.rs"]
pub mod frame_pool;
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
#[path = "utils/tty.rs"]
pub mod tty;

pub mod app;
pub mod context;
pub mod gles_context;
pub mod input;

pub type TibsClayScope<'clay, 'render> =
    SkiaClayScope<'clay, 'render, custom_elements::CustomElements>;

use crate::{
    animation::{
        easing::{ease_in_out_circ, ease_in_quad},
        BasicAnimation, DelayAnimation,
    },
    background::Background,
    context::select_and_init_context,
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

    let mut context = select_and_init_context();
    let (screen_width, screen_height) = context.size();
    let mut clay = clay_layout::Clay::new((screen_width as f32, screen_height as f32).into());
    clay.set_measure_text_function(create_measure_text_function(&FONTS));

    let assets = Rc::new(AssetCache::new(
        std::env::var("TIBS_ASSETS_FOLDER").unwrap_or("assets".into()),
    )?);

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

    while {
        let state = app_state.lock().unwrap();
        !state.should_exit && !context.should_close()
    } {
        context.poll_events();

        let mut state = app_state.lock().unwrap();
        state.update(&mut *context);
        state.render(&mut *context);
    }

    Ok(())
}
