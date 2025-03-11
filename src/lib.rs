use custom_elements::CustomElements;
use skia_clay::SkiaClayScope;


pub mod fps_counter;
pub mod gl;
pub mod custom_elements;
#[macro_use]
pub mod gl_errors;
pub mod animation;
pub mod gles_context;
pub mod skia;
pub mod progress_watcher;
pub mod loading_screen;
pub mod skia_image_asset;
pub mod skia_clay;
pub type TibsClayScope<'clay, 'render> = SkiaClayScope<'clay, 'render, custom_elements::CustomElements>;