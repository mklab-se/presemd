use eframe::egui;

use crate::parser::Block;

/// Split slide blocks into content blocks and an optional side image.
/// Returns (content_blocks, image_block) where image_block is the first
/// Image block found (if any).
pub fn split_image(blocks: &[Block]) -> (Vec<&Block>, Option<&Block>) {
    let mut content = Vec::new();
    let mut image = None;

    for block in blocks {
        if image.is_none() {
            if let Block::Image { .. } = block {
                image = Some(block);
                continue;
            }
        }
        content.push(block);
    }

    (content, image)
}

/// Check if any block in the slide is an image.
pub fn has_image(blocks: &[Block]) -> bool {
    blocks.iter().any(|b| matches!(b, Block::Image { .. }))
}

/// Calculate left/right rects for a content+image split layout.
/// Content gets 55% width, image gets 40% width, with a 5% gap.
pub fn image_split_rects(content_rect: egui::Rect) -> (egui::Rect, egui::Rect) {
    let total_width = content_rect.width();
    let content_width = total_width * 0.55;
    let image_width = total_width * 0.40;
    let gap = total_width * 0.05;

    let left = egui::Rect::from_min_size(
        content_rect.min,
        egui::vec2(content_width, content_rect.height()),
    );

    let right = egui::Rect::from_min_size(
        egui::pos2(
            content_rect.left() + content_width + gap,
            content_rect.top(),
        ),
        egui::vec2(image_width, content_rect.height()),
    );

    (left, right)
}
