pub mod diagram;
pub mod image_cache;
pub mod layouts;
pub mod syntax;
pub mod text;
pub mod transition;

use std::time::Instant;

use eframe::egui;

use crate::parser::{Layout, Slide};
use crate::theme::Theme;

use image_cache::ImageCache;

/// Estimate the total content height of a slide (for scroll/overflow detection).
/// Returns (content_height, available_height) where available_height is the usable
/// area within the slide rect after padding.
pub fn measure_slide_content_height(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    scale: f32,
) -> (f32, f32) {
    let padding = 80.0 * scale;
    let available_height = rect.height() - padding * 2.0;

    // Use a content width consistent with most layouts (~75% for code/content, full for bullet)
    let content_width = match slide.layout {
        Layout::Code => rect.width() * 0.75,
        _ => rect.width() - padding * 2.0,
    };

    let content_height =
        text::measure_blocks_height(ui, &slide.blocks, theme, content_width, scale);
    (content_height, available_height)
}

/// Render a single slide using its inferred layout.
#[allow(clippy::too_many_arguments)]
pub fn render_slide(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) {
    match slide.layout {
        Layout::Title => layouts::title::render(ui, slide, theme, rect, opacity, scale),
        Layout::Section => layouts::section::render(ui, slide, theme, rect, opacity, scale),
        Layout::Quote => layouts::quote::render(ui, slide, theme, rect, opacity, scale),
        Layout::Bullet => layouts::bullet::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        ),
        Layout::Code => layouts::code::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        ),
        Layout::TwoColumn => layouts::two_column::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        ),
        Layout::Content => layouts::content::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        ),
        Layout::Image => layouts::image_slide::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        ),
        Layout::Gallery => layouts::gallery::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        ),
        Layout::Diagram => layouts::diagram::render(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            reveal_timestamp,
            scale,
        ),
    }
}
