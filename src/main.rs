#![allow(unsafe_op_in_unsafe_fn)]

use std::{mem::ManuallyDrop, sync::LazyLock};

use assets_manager::AssetCache;
use custom_elements::CustomElements;
use gles_context::select_and_init_gles_context;
use skia::{create_skia_surface, init_skia};
use skia_safe::{FontMgr, FontStyle, Typeface};
use tibs::{
    loading_screen::LoadingScreen,
    skia_clay::{clay_skia_render, create_measure_text_function},
    *,
};
static UBUNTU_FONT: LazyLock<Typeface> = LazyLock::new(|| {
    FontMgr::new()
        .match_family_style("UbuntuSans NF", FontStyle::normal())
        .unwrap()
});
static FONTS: LazyLock<Vec<&Typeface>> = LazyLock::new(|| vec![&UBUNTU_FONT]);
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut context = select_and_init_gles_context();
    let mut fps_counter = fps_counter::FPSCounter::new();

    let mut last_time = std::time::Instant::now();
    let (mut skia_context, mut skia_surface) = init_skia(context.as_mut())?;
    let (screen_width, screen_height) = context.size();
    let mut clay = ManuallyDrop::new(clay_layout::Clay::new(
        (screen_width as f32, screen_height as f32).into(),
    ));
    clay.set_measure_text_function(create_measure_text_function(&FONTS));
    let mut boot_progress = progress_watcher::ProgressWatcher::new()?;
    let assets = AssetCache::new(std::env::var("TIBS_ASSETS_FOLDER").unwrap_or("assets".into()))?;
    let mut loading_screen = LoadingScreen::new(&assets);
    while !context.should_close() {
        let (screen_width, screen_height) = context.size();
        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(last_time).as_secs_f32();
        last_time = current_time;

        // Render
        gl!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        let mut c = clay.begin::<_, custom_elements::CustomElements>();
        let progress = boot_progress.poll_progress();
        if !progress.finished || true {
            loading_screen.render(progress, &mut c, delta);
        } else {
            todo!();
        }
        clay_skia_render(
            skia_surface.canvas(),
            c.end(),
            CustomElements::render,
            &FONTS,
        );
        drop(c);
        skia_context.flush(None);
        context.swap_buffers();

        if let Some(fps) = fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
        if skia_surface.width() != screen_width as _ || skia_surface.height() != screen_height as _
        {
            skia_surface = create_skia_surface(&mut skia_context, screen_width, screen_height)?;
            clay.set_layout_dimensions((screen_width as f32, screen_height as f32).into());
        }
    }
    Ok(())
}
