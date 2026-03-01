use std::time::Instant;

use eframe::egui::{self, Pos2};

use crate::parser::{Block, Slide};
use crate::render::diagram;
use crate::render::image_cache::ImageCache;
use crate::render::text;
use crate::theme::Theme;

/// Diagram slide layout: heading at top, diagram filling remaining space.
#[allow(clippy::too_many_arguments)]
pub fn render(
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
    let padding = 60.0 * scale;
    let content_width = rect.width() - padding * 2.0;
    let content_left = rect.left() + padding;
    let mut y = rect.top() + padding;

    // Find heading and diagram blocks
    let mut heading: Option<&Block> = None;
    let mut diagram_content: Option<&str> = None;

    for block in &slide.blocks {
        match block {
            Block::Heading { .. } if heading.is_none() => {
                heading = Some(block);
            }
            Block::Diagram { content } if diagram_content.is_none() => {
                diagram_content = Some(content);
            }
            _ => {}
        }
    }

    // Draw heading if present
    if let Some(Block::Heading { level, inlines }) = heading {
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
        y += h + 30.0 * scale;
    }

    // Draw diagram filling the remaining vertical space
    if let Some(content) = diagram_content {
        let remaining_height = rect.bottom() - y - padding;
        if remaining_height > 50.0 * scale {
            diagram::draw_diagram_sized(
                ui,
                content,
                theme,
                Pos2::new(content_left, y),
                content_width,
                remaining_height,
                opacity,
                image_cache,
                reveal_step,
                reveal_timestamp,
                scale,
            );
        }
    }
}
