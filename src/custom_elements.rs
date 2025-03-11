use clay_layout::{math::BoundingBox, render_commands::{Custom, RenderCommand}, Color};
use skia_safe::Image;

use crate::skia_clay::clay_to_skia_color;

pub enum CustomElements {
    CheckMark(Color)
}

impl CustomElements {
    pub fn render(command: &RenderCommand<'_, Image, Self>, custom: &Custom<'_, Self>, canvas: &skia_safe::Canvas) {
        let this = custom.data;
        let BoundingBox {height, width, x, y} = command.bounding_box;
        match this {
            Self::CheckMark(color) => {
                let mut paint = skia_safe::Paint::default();
                paint.set_color(clay_to_skia_color(*color));
                paint.set_stroke_width(2.0);
                paint.set_style(skia_safe::paint::Style::Stroke);
                paint.set_stroke_cap(skia_safe::PaintCap::Round);
                paint.set_anti_alias(true);
                let mut path = skia_safe::Path::new();
                path.move_to((x + 4.0, y + 12.0));
                path.line_to((x + 9.0, y + 17.0));
                path.line_to((x + 20.0, y + 6.0));
                canvas.draw_path(&path, &paint);
            }
        }
    }
}