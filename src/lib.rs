use custom_elements::CustomElements;
use skia_clay::SkiaClayScope;

pub mod custom_elements;
pub mod fps_counter;
pub mod gl;
#[macro_use]
pub mod gl_errors;
pub mod animation;
pub mod gles_context;
pub mod loading_screen;
pub mod progress_watcher;
pub mod skia;
pub mod skia_clay;
pub mod skia_image_asset;
pub type TibsClayScope<'clay, 'render> =
    SkiaClayScope<'clay, 'render, custom_elements::CustomElements>;
