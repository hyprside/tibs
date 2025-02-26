#![allow(unsafe_op_in_unsafe_fn)]

use animation::{
    colors::{interpolate_color_normalized, rgb_to_norm},
    easing, Animation, BasicAnimation, DelayAnimation, LoopingAnimation,
};
use gles_context::select_and_init_gles_context;
use skia::{create_skia_surface, init_skia};
use skia_safe::{Color, Color4f, Data, Image, Paint, Rect};

pub mod fps_counter;
pub mod gl;
#[macro_use]
pub mod gl_errors;
pub mod animation;
pub mod gles_context;
pub mod skia;
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut context = select_and_init_gles_context();
    let mut fps_counter = fps_counter::FPSCounter::new();
    let mut background_animation = all!(
        BasicAnimation::new("background", 1., easing::ease_in_out_cubic,),
        seq!(
            DelayAnimation::new(
                0.5,
                BasicAnimation::new("logo", 0.5, easing::ease_out_back,)
            ),
            all!(
                LoopingAnimation::infinite(BasicAnimation::new(
                    "progress_bar",
                    10.0,
                    easing::linear
                )),
                BasicAnimation::new("progress_bar_opacity", 0.5, easing::ease_in_out_cubic)
            )
        )
    );
    let mut last_time = std::time::Instant::now();
    let mut background_color = (0.0, 0.0, 0.0);
    let mut logo_alpha = 0.;
    let (mut skia_context, mut skia_surface) = init_skia(context.as_mut())?;
    let logo_image =
        Image::from_encoded(unsafe { Data::new_bytes(include_bytes!("assets/logo.png")) })
            .expect("assets/logo.png is invalid");
    let mut progress_bar = 0.0;
    let mut progress_bar_opacity = 0.0;
    while !context.should_close() {
        let (screen_width, screen_height) = context.size();
        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(last_time).as_secs_f32();
        last_time = current_time;

        // Render
        gl!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        gl!(gl::ClearColor(
            background_color.0,
            background_color.1,
            background_color.2,
            1.0
        ));
        let skia_canvas = skia_surface.canvas();
        let logo_size = 100. * logo_alpha;
        skia_canvas.draw_image_rect(
            &logo_image,
            None,
            Rect::from_xywh(
                ((screen_width as f32 / 2.) - logo_size / 2.) as _,
                ((screen_height as f32 / 2.) - logo_size / 2.) as _,
                logo_size,
                logo_size,
            ),
            &Paint::new(&Color4f::new(1.0, 1.0, 1.0, logo_alpha), None),
        );
        {
            let mut paint = Paint::default();
            paint.set_color(Color::WHITE.with_a((progress_bar_opacity * 255.) as u8));
            paint.set_stroke_width(8.0);
            paint.set_stroke_cap(skia_safe::PaintCap::Round);

            let progress_bar_width = 150.0;
            let margin = 30.0;
            let start_x = (screen_width as f32 / 2.) - progress_bar_width / 2.;
            let end_x = start_x + progress_bar_width * progress_bar;
            let y = (screen_height as f32 / 2.) + 100. / 2. + margin;

            skia_canvas.draw_line((start_x, y), (end_x, y), &paint);
        }
        skia_context.flush(None);
        context.swap_buffers();

        // update
        for (id, progress) in background_animation.update(delta) {
            match id.as_str() {
                "background" => {
                    background_color = interpolate_color_normalized(
                        rgb_to_norm("#000000"),
                        rgb_to_norm("#0F1419"),
                        progress,
                    )
                }
                "logo" => logo_alpha = progress,
                "progress_bar" => progress_bar = progress,
                "progress_bar_opacity" => progress_bar_opacity = progress,
                _ => {}
            }
        }

        if let Some(fps) = fps_counter.tick() {
            println!("FPS: {:.2}", fps);
        }
        if skia_surface.width() != screen_width as _ || skia_surface.height() != screen_height as _
        {
            skia_surface = create_skia_surface(&mut skia_context, screen_width, screen_height)?;
        }
    }
    Ok(())
}
