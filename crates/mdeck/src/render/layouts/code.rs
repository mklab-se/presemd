use eframe::egui::{self, Pos2};

use crate::parser::{Block, Slide};
use crate::render::image_cache::ImageCache;
use crate::render::layouts::image_split;
use crate::render::text;
use crate::theme::Theme;

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
    let content_width = rect.width() * 0.75;
    let content_left = rect.left() + (rect.width() - content_width) / 2.0;

    // First pass: measure total content height
    let mut total_height = 0.0;
    for (i, block) in slide.blocks.iter().enumerate() {
        let h = measure_block_height(ui, block, theme, content_width, scale);
        total_height += h;
        if i < slide.blocks.len() - 1 {
            total_height += block_spacing(block, scale);
        }
    }

    // Vertically center
    let available_height = rect.height() - v_padding * 2.0;
    let start_y = if total_height < available_height {
        rect.top() + v_padding + (available_height - total_height) / 2.0
    } else {
        rect.top() + v_padding
    };

    let mut y = start_y;
    for (i, block) in slide.blocks.iter().enumerate() {
        match block {
            Block::Heading { level, inlines } => {
                let h = text::draw_heading(
                    ui,
                    inlines,
                    *level,
                    theme,
                    Pos2::new(content_left, y),
                    content_width,
                    opacity,
                    scale,
                );
                y += h;
            }
            Block::Paragraph { inlines } => {
                let h = text::draw_paragraph(
                    ui,
                    inlines,
                    theme,
                    Pos2::new(content_left, y),
                    content_width,
                    opacity,
                    scale,
                );
                y += h;
            }
            Block::CodeBlock {
                language,
                code,
                highlight_lines,
            } => {
                let h = text::draw_code_block(
                    ui,
                    code,
                    language.as_deref(),
                    highlight_lines,
                    theme,
                    Pos2::new(content_left, y),
                    content_width,
                    opacity,
                    scale,
                );
                y += h;
            }
            _ => {
                let h = text::draw_block(
                    ui,
                    block,
                    theme,
                    Pos2::new(content_left, y),
                    content_width,
                    opacity,
                    image_cache,
                    reveal_step,
                    scale,
                );
                y += h;
            }
        }
        if i < slide.blocks.len() - 1 {
            y += block_spacing(block, scale);
        }
    }
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

    let mut y = start_y;
    for (i, block) in content_blocks.iter().enumerate() {
        match *block {
            Block::Heading { level, inlines } => {
                let h = text::draw_heading(
                    ui,
                    inlines,
                    *level,
                    theme,
                    Pos2::new(left_rect.left(), y),
                    left_rect.width(),
                    opacity,
                    scale,
                );
                y += h;
            }
            Block::CodeBlock {
                language,
                code,
                highlight_lines,
            } => {
                let h = text::draw_code_block(
                    ui,
                    code,
                    language.as_deref(),
                    highlight_lines,
                    theme,
                    Pos2::new(left_rect.left(), y),
                    left_rect.width(),
                    opacity,
                    scale,
                );
                y += h;
            }
            Block::Paragraph { inlines } => {
                let h = text::draw_paragraph(
                    ui,
                    inlines,
                    theme,
                    Pos2::new(left_rect.left(), y),
                    left_rect.width(),
                    opacity,
                    scale,
                );
                y += h;
            }
            _ => {
                let h = text::draw_block(
                    ui,
                    block,
                    theme,
                    Pos2::new(left_rect.left(), y),
                    left_rect.width(),
                    opacity,
                    image_cache,
                    reveal_step,
                    scale,
                );
                y += h;
            }
        }
        if i < content_blocks.len() - 1 {
            y += block_spacing(block, scale);
        }
    }

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

fn block_spacing(block: &Block, scale: f32) -> f32 {
    match block {
        Block::Heading { .. } => 24.0 * scale,
        _ => 20.0 * scale,
    }
}

fn measure_block_height(
    ui: &egui::Ui,
    block: &Block,
    theme: &Theme,
    max_width: f32,
    scale: f32,
) -> f32 {
    match block {
        Block::Heading { level, inlines } => {
            let size = theme.heading_size(*level) * scale;
            let job = text::inlines_to_job(inlines, size, theme.heading_color, max_width);
            ui.painter().layout_job(job).rect.height()
        }
        Block::Paragraph { inlines } | Block::BlockQuote { inlines } => {
            let size = theme.body_size * scale;
            let job = text::inlines_to_job(inlines, size, theme.foreground, max_width);
            ui.painter().layout_job(job).rect.height()
        }
        Block::CodeBlock { code, .. } => {
            let line_count = code.lines().count().max(1);
            let line_height = theme.code_size * scale * 1.4;
            let padding = 16.0 * scale;
            line_count as f32 * line_height + padding * 2.0
        }
        _ => theme.body_size * scale * 1.5,
    }
}
