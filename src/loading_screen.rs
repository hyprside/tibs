use std::{collections::HashMap, sync::mpsc::Sender};

use clay_layout::{
    elements::{FloatingAttachPointType, FloatingAttachToElement}, fixed, grow, layout::{Alignment, LayoutAlignmentX, LayoutAlignmentY, LayoutDirection, Sizing}, ClayLayoutScope, Color, Declaration
};

use crate::{
    all,
    animation::{
        self, colors::interpolate_color, easing::{ease_out_elastic, ease_out_quad}, Animation, BasicAnimation, ParallelAnimation, ProgressBarAnimation
    },
    custom_elements::CustomElements,
    seq,
    start_progress::ProgressData,
};
pub struct LoadingScreen {
    animation: ParallelAnimation,
    progress_bar_sender: Sender<f32>,
    animations_state: HashMap<String, f32>,
    checkmark: CustomElements
}

impl LoadingScreen {
    
    pub fn new() -> Self {
        let (progress_bar_animation, progress_bar_sender) = ProgressBarAnimation::new("progress", 2.5);
        Self {
            animation: all!(
                seq!(
                    BasicAnimation::new("bg", 0.25, ease_out_quad),
                    BasicAnimation::new("logo", 2.0, ease_out_elastic)
                ),
                seq!(progress_bar_animation, BasicAnimation::new("success", 0.25, ease_out_quad))
            ),
            progress_bar_sender,
            animations_state: HashMap::new(),
            checkmark: CustomElements::CheckMark((0.0, 0.0, 0.0, 0.0).into())
        }
    }
    pub fn render<'clay, 'render>(
        &'render mut self,
        progress: &'render ProgressData,
        c: &mut ClayLayoutScope<'clay, 'render, skia_safe::Image, CustomElements>,
        logo: impl Into<Option<&'render skia_safe::Image>>,
        delta_time: f32,
    ) where
        'clay: 'render,
    {
        self.progress_bar_sender
            .send(progress.get_percentage())
            .unwrap();
        self.animations_state
            .extend(self.animation.update(delta_time));
        let success_animation = self.animations_state.get("success").copied().unwrap_or(0.0);
        self.checkmark = CustomElements::CheckMark(Color::u_rgba(0x4C, 0xE3, 0xA2, (255.*success_animation) as u8));
        c.with(
            Declaration::new()
                .layout()
                .child_alignment(Alignment::new(
                    LayoutAlignmentX::Center,
                    LayoutAlignmentY::Center,
                ))
                .child_gap(52)
                .direction(LayoutDirection::TopToBottom)
                .width(grow!())
                .height(grow!())
                .end()
                .background_color(
                    animation::colors::interpolate_color(
                        (0, 0, 0),
                        (0x0F, 0x14, 0x19),
                        self.animations_state["bg"],
                    )
                    .into(),
                ),
            |c| {
                if let Some(logo) = logo.into() {
                    c.with(
                        Declaration::new()
                            .layout()
                            .child_alignment(Alignment::new(
                                LayoutAlignmentX::Center,
                                LayoutAlignmentY::Center,
                            ))
                            .height(fixed!(183.0))
                            .width(fixed!(183.0))
                            .end(),
                        |c| {
                            c.with(
                                Declaration::new()
                                    .image()
                                    .data(logo)
                                    .source_dimensions({
                                        let dimensions = logo.dimensions();
                                        (dimensions.width as f32, dimensions.height as f32).into()
                                    })
                                    .end()
                                    .layout()
                                    .width(Sizing::Fixed(self.animations_state.get("logo").copied().unwrap_or(0.0)*183.))
                                    .end(),
                                |_| {},
                            )
                        },
                    );
                }
                c.with(
                    Declaration::new()
                        .layout()
                        .width(fixed!(242.0))
                        .end()
                        .background_color((0x18, 0x1F, 0x3F).into())
                        .corner_radius()
                        .all(9999.0)
                        .end(),
                    |c| {
                        c.with(
                            Declaration::new()
                                .floating()
                                .attach_to(FloatingAttachToElement::Parent)
                                .attach_points(FloatingAttachPointType::RightCenter, FloatingAttachPointType::LeftCenter)
                                .offset((-24., 0.).into())
                                .dimensions((12., 12.).into())
                                .end()
                                .custom_element(&self.checkmark),
                            |_| {}
                        );
                        c.with(
                            Declaration::new()
                                .layout()
                                .width(Sizing::Percent(
                                    self.animations_state.get("progress").copied().unwrap_or(0.),
                                ))
                                .height(fixed!(5.0))
                                .end()
                                .background_color(interpolate_color((0xFF, 0xFF, 0xFF), (0x4C, 0xE3, 0xA2), success_animation).into())
                                .corner_radius()
                                .all(5.0)
                                .end(),
                            |_| {},
                        );
                    },
                );
            },
        );
    }
}
