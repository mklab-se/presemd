use eframe::egui::{self, Pos2};

use crate::parser::{Block, Slide};
use crate::render::image_cache::ImageCache;
use crate::render::layouts::image_split;
use crate::render::text;
use crate::theme::Theme;

/// Fallback layout: render all blocks top-to-bottom, vertically centered.
/// If the slide contains one image, split into content (left) + image (right).
#[allow(clippy::too_many_arguments)]
pub fn render(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    scale: f32,
) {
    if image_split::has_image(&slide.blocks) {
        let v_padding = 80.0 * scale;
        let padded_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + v_padding, rect.top() + v_padding),
            egui::pos2(rect.right() - v_padding, rect.bottom() - v_padding),
        );
        render_with_image(
            ui,
            slide,
            theme,
            padded_rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        );
    } else {
        render_text_only(
            ui,
            slide,
            theme,
            rect,
            opacity,
            image_cache,
            reveal_step,
            scale,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_text_only(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    scale: f32,
) {
    let v_padding = 80.0 * scale;
    let content_width = rect.width() * 0.70;
    let content_left = rect.left() + (rect.width() - content_width) / 2.0;

    // Measure content height for vertical centering
    let total_height = text::measure_blocks_height(ui, &slide.blocks, theme, content_width, scale);

    let available_height = rect.height() - v_padding * 2.0;
    let start_y = if total_height < available_height {
        rect.top() + v_padding + (available_height - total_height) / 2.0
    } else {
        rect.top() + v_padding
    };

    text::draw_blocks(
        ui,
        &slide.blocks,
        theme,
        Pos2::new(content_left, start_y),
        content_width,
        opacity,
        image_cache,
        reveal_step,
        scale,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_with_image(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    padded_rect: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    scale: f32,
) {
    let (content_blocks, image_block) = image_split::split_image(&slide.blocks);
    let (left_rect, right_rect) = image_split::image_split_rects(padded_rect);

    // Measure and vertically center the content blocks
    let owned: Vec<Block> = content_blocks.iter().map(|b| (*b).clone()).collect();
    let total_height = text::measure_blocks_height(ui, &owned, theme, left_rect.width(), scale);
    let start_y = if total_height < left_rect.height() {
        left_rect.top() + (left_rect.height() - total_height) / 2.0
    } else {
        left_rect.top()
    };

    text::draw_blocks(
        ui,
        &owned,
        theme,
        Pos2::new(left_rect.left(), start_y),
        left_rect.width(),
        opacity,
        image_cache,
        reveal_step,
        scale,
    );

    // Render image in the right area
    if let Some(Block::Image {
        alt,
        path,
        directives,
    }) = image_block
    {
        text::draw_image_in_area(
            ui,
            path,
            alt,
            directives,
            theme,
            right_rect,
            opacity,
            image_cache,
        );
    }
}
