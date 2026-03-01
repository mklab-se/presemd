use eframe::egui::{self, Pos2};

use crate::parser::{Block, Slide};
use crate::render::image_cache::ImageCache;
use crate::render::text;
use crate::theme::Theme;

/// Gallery slide layout: multiple images arranged in a grid.
/// - 2 images: side by side
/// - 3 images: top row of 2, bottom row of 1 centered
/// - 4 images: 2x2 grid
/// - 5+ images: rows of 3 (or 2 for remainder)
#[allow(clippy::too_many_arguments)]
pub fn render(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
    _reveal_step: usize,
    scale: f32,
) {
    let padding = 50.0 * scale;
    let gap = 16.0 * scale;

    // Collect heading and image blocks
    let mut heading: Option<&Block> = None;
    let mut images: Vec<&Block> = Vec::new();

    for block in &slide.blocks {
        match block {
            Block::Heading { .. } if heading.is_none() && images.is_empty() => {
                heading = Some(block);
            }
            Block::Image { .. } => {
                images.push(block);
            }
            _ => {}
        }
    }

    if images.is_empty() {
        return;
    }

    let content_width = rect.width() - padding * 2.0;
    let mut y = rect.top() + padding;

    // Draw heading if present
    if let Some(Block::Heading { level, inlines }) = heading {
        let h = text::draw_heading(
            ui,
            inlines,
            *level,
            theme,
            Pos2::new(rect.left() + padding, y),
            content_width,
            opacity,
            scale,
        );
        y += h + 20.0 * scale;
    }

    let gallery_height = rect.bottom() - y - padding;
    let gallery_left = rect.left() + padding;

    // Compute grid layout based on image count
    let cells = compute_grid(images.len(), content_width, gallery_height, gap);

    for (i, block) in images.iter().enumerate() {
        if i >= cells.len() {
            break;
        }

        let cell = &cells[i];
        let cell_rect = egui::Rect::from_min_size(
            Pos2::new(gallery_left + cell.x, y + cell.y),
            egui::vec2(cell.w, cell.h),
        );

        if let Block::Image {
            alt,
            path,
            directives,
        } = block
        {
            text::draw_image_in_area(
                ui,
                path,
                alt,
                directives,
                theme,
                cell_rect,
                opacity,
                image_cache,
            );
        }
    }
}

struct Cell {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn compute_grid(count: usize, width: f32, height: f32, gap: f32) -> Vec<Cell> {
    match count {
        0 => Vec::new(),
        1 => {
            // Single image centered
            vec![Cell {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            }]
        }
        2 => {
            // Side by side
            let cell_w = (width - gap) / 2.0;
            vec![
                Cell {
                    x: 0.0,
                    y: 0.0,
                    w: cell_w,
                    h: height,
                },
                Cell {
                    x: cell_w + gap,
                    y: 0.0,
                    w: cell_w,
                    h: height,
                },
            ]
        }
        3 => {
            // Top row: 2 images, bottom row: 1 centered
            let row_h = (height - gap) / 2.0;
            let top_w = (width - gap) / 2.0;
            let bot_w = width * 0.5; // centered, half width
            let bot_x = (width - bot_w) / 2.0;
            vec![
                Cell {
                    x: 0.0,
                    y: 0.0,
                    w: top_w,
                    h: row_h,
                },
                Cell {
                    x: top_w + gap,
                    y: 0.0,
                    w: top_w,
                    h: row_h,
                },
                Cell {
                    x: bot_x,
                    y: row_h + gap,
                    w: bot_w,
                    h: row_h,
                },
            ]
        }
        4 => {
            // 2x2 grid
            let cell_w = (width - gap) / 2.0;
            let cell_h = (height - gap) / 2.0;
            vec![
                Cell {
                    x: 0.0,
                    y: 0.0,
                    w: cell_w,
                    h: cell_h,
                },
                Cell {
                    x: cell_w + gap,
                    y: 0.0,
                    w: cell_w,
                    h: cell_h,
                },
                Cell {
                    x: 0.0,
                    y: cell_h + gap,
                    w: cell_w,
                    h: cell_h,
                },
                Cell {
                    x: cell_w + gap,
                    y: cell_h + gap,
                    w: cell_w,
                    h: cell_h,
                },
            ]
        }
        _ => {
            // Generic grid: rows of 3, last row may have fewer
            let cols = 3;
            let rows = count.div_ceil(cols);
            let cell_w = (width - (cols - 1) as f32 * gap) / cols as f32;
            let cell_h = (height - (rows - 1) as f32 * gap) / rows as f32;

            (0..count)
                .map(|i| {
                    let col = i % cols;
                    let row = i / cols;
                    // Center the last row if it has fewer items
                    let items_in_row = if row == rows - 1 {
                        count - row * cols
                    } else {
                        cols
                    };
                    let row_width = items_in_row as f32 * cell_w + (items_in_row - 1) as f32 * gap;
                    let row_offset = (width - row_width) / 2.0;
                    let col_in_row = col; // col index within this row

                    Cell {
                        x: row_offset + col_in_row as f32 * (cell_w + gap),
                        y: row as f32 * (cell_h + gap),
                        w: cell_w,
                        h: cell_h,
                    }
                })
                .collect()
        }
    }
}
