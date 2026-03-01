use crate::parser::{Block, ImageDirectives, Inline, ListItem, ListMarker};
use crate::render::diagram::draw_diagram_sized;
use crate::render::image_cache::ImageCache;
use crate::theme::Theme;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Stroke};

/// Create a LayoutJob from inline elements.
pub fn inlines_to_job(
    inlines: &[Inline],
    font_size: f32,
    color: Color32,
    max_width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = max_width;
    append_inlines(&mut job, inlines, font_size, color, false, false);
    job
}

fn append_inlines(
    job: &mut egui::text::LayoutJob,
    inlines: &[Inline],
    font_size: f32,
    color: Color32,
    bold: bool,
    italic: bool,
) {
    for inline in inlines {
        match inline {
            Inline::Text(s) => {
                let size = if bold { font_size + 1.0 } else { font_size };
                let format = egui::text::TextFormat {
                    font_id: FontId::new(size, FontFamily::Proportional),
                    color,
                    italics: italic,
                    ..Default::default()
                };
                job.append(s, 0.0, format);
            }
            Inline::Bold(children) => {
                append_inlines(job, children, font_size, color, true, italic);
            }
            Inline::Italic(children) => {
                append_inlines(job, children, font_size, color, bold, true);
            }
            Inline::Strikethrough(children) => {
                let mut inner_job = egui::text::LayoutJob::default();
                append_inlines(&mut inner_job, children, font_size, color, bold, italic);
                // Apply strikethrough to all sections
                for section in &inner_job.sections {
                    let mut format = section.format.clone();
                    format.strikethrough = Stroke::new(1.0, color);
                    job.append(&inner_job.text[section.byte_range.clone()], 0.0, format);
                }
            }
            Inline::Code(s) => {
                let format = egui::text::TextFormat {
                    font_id: FontId::new(font_size * 0.85, FontFamily::Monospace),
                    color,
                    background: Color32::from_rgba_unmultiplied(128, 128, 128, 30),
                    ..Default::default()
                };
                job.append(s, 0.0, format);
            }
            Inline::Link { text, .. } => {
                // Render link text in accent color
                let link_color = Color32::from_rgb(0x52, 0x94, 0xE2);
                append_inlines(job, text, font_size, link_color, bold, italic);
            }
        }
    }
}

/// Layout and paint inlines, returning the height used.
pub fn draw_inlines(
    ui: &egui::Ui,
    inlines: &[Inline],
    pos: Pos2,
    font_size: f32,
    color: Color32,
    max_width: f32,
) -> f32 {
    let job = inlines_to_job(inlines, font_size, color, max_width);
    let galley = ui.painter().layout_job(job);
    let height = galley.rect.height();
    ui.painter().galley(pos, galley, color);
    height
}

/// Draw a heading block. Returns height used.
#[allow(clippy::too_many_arguments)]
pub fn draw_heading(
    ui: &egui::Ui,
    inlines: &[Inline],
    level: u8,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    scale: f32,
) -> f32 {
    let size = theme.heading_size(level) * scale;
    let color = Theme::with_opacity(theme.heading_color, opacity);
    draw_inlines(ui, inlines, pos, size, color, max_width)
}

/// Draw a paragraph. Returns height used.
pub fn draw_paragraph(
    ui: &egui::Ui,
    inlines: &[Inline],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    scale: f32,
) -> f32 {
    let color = Theme::with_opacity(theme.foreground, opacity);
    draw_inlines(ui, inlines, pos, theme.body_size * scale, color, max_width)
}

/// Draw a list with incremental reveal support. Returns height used.
#[allow(clippy::too_many_arguments)]
pub fn draw_list(
    ui: &egui::Ui,
    items: &[ListItem],
    ordered: bool,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    indent_level: usize,
    reveal_step: usize,
    scale: f32,
) -> f32 {
    let mut step_counter = 0usize;
    draw_list_inner(
        ui,
        items,
        ordered,
        theme,
        pos,
        max_width,
        opacity,
        indent_level,
        reveal_step,
        &mut step_counter,
        scale,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_list_inner(
    ui: &egui::Ui,
    items: &[ListItem],
    ordered: bool,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    indent_level: usize,
    reveal_step: usize,
    step_counter: &mut usize,
    scale: f32,
) -> f32 {
    let color = Theme::with_opacity(theme.foreground, opacity);
    let indent = 30.0 * scale * indent_level as f32;
    let marker_width = 45.0 * scale;
    let item_spacing = 8.0 * scale;
    let font_size = theme.body_size * scale;
    let mut y_offset = 0.0;

    for (idx, item) in items.iter().enumerate() {
        // Compute this item's reveal step
        let item_step = match item.marker {
            ListMarker::Static | ListMarker::Ordered => 0,
            ListMarker::NextStep => {
                *step_counter += 1;
                *step_counter
            }
            ListMarker::WithPrev => *step_counter,
        };

        // Skip items not yet revealed
        if item_step > reveal_step {
            continue;
        }

        // Draw marker
        let marker_text = if ordered {
            format!("{}.", idx + 1)
        } else {
            match item.marker {
                ListMarker::Static | ListMarker::NextStep | ListMarker::WithPrev => {
                    "\u{2022}".to_string()
                }
                ListMarker::Ordered => format!("{}.", idx + 1),
            }
        };

        let marker_pos = Pos2::new(pos.x + indent, pos.y + y_offset);
        let marker_galley =
            ui.painter()
                .layout_no_wrap(marker_text, FontId::proportional(font_size), color);
        ui.painter().galley(marker_pos, marker_galley, color);

        // Draw item text
        let text_pos = Pos2::new(pos.x + indent + marker_width, pos.y + y_offset);
        let text_width = max_width - indent - marker_width;
        let text_height = draw_inlines(ui, &item.inlines, text_pos, font_size, color, text_width);

        y_offset += text_height + item_spacing;

        // Draw children
        if !item.children.is_empty() {
            let children_ordered = item
                .children
                .first()
                .is_some_and(|c| c.marker == ListMarker::Ordered);
            let child_height = draw_list_inner(
                ui,
                &item.children,
                children_ordered,
                theme,
                Pos2::new(pos.x, pos.y + y_offset),
                max_width,
                opacity,
                indent_level + 1,
                reveal_step,
                step_counter,
                scale,
            );
            y_offset += child_height;
        }
    }

    y_offset
}

/// Draw a code block with syntax highlighting. Returns height used.
#[allow(clippy::too_many_arguments)]
pub fn draw_code_block(
    ui: &egui::Ui,
    code: &str,
    language: Option<&str>,
    highlight_lines: &[usize],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    scale: f32,
) -> f32 {
    let padding = 16.0 * scale;
    let bg_color = Theme::with_opacity(theme.code_background, opacity);

    // Build syntax-highlighted layout
    let job = crate::render::syntax::highlight_code(
        code,
        language,
        theme.code_size * scale,
        opacity,
        theme,
        max_width - padding * 2.0,
    );
    let code_galley = ui.painter().layout_job(job);

    let total_height = code_galley.rect.height() + padding * 2.0;

    // Draw background
    let bg_rect = egui::Rect::from_min_size(pos, egui::vec2(max_width, total_height));
    ui.painter().rect_filled(bg_rect, 8.0 * scale, bg_color);

    // Draw line highlights using actual galley row positions
    if !highlight_lines.is_empty() {
        let accent = Theme::with_opacity(theme.accent, opacity * 0.15);
        let code_top = pos.y + padding;

        // Each row in the galley corresponds to a visual line.
        // `ends_with_newline` tells us when a source line ends.
        let mut source_line = 1usize;
        for row in &code_galley.rows {
            let row_rect = row.rect();

            if highlight_lines.contains(&source_line) {
                let hl_rect = egui::Rect::from_min_max(
                    Pos2::new(pos.x + padding * 0.5, code_top + row_rect.top()),
                    Pos2::new(
                        pos.x + max_width - padding * 0.5,
                        code_top + row_rect.bottom(),
                    ),
                );
                ui.painter().rect_filled(hl_rect, 4.0 * scale, accent);
            }

            if row.ends_with_newline {
                source_line += 1;
            }
        }
    }

    // Draw code
    let code_pos = Pos2::new(pos.x + padding, pos.y + padding);
    let fallback = Theme::with_opacity(theme.code_foreground, opacity);
    ui.painter().galley(code_pos, code_galley, fallback);

    total_height
}

/// Draw a table. Returns height used.
#[allow(clippy::too_many_arguments)]
pub fn draw_table(
    ui: &egui::Ui,
    headers: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    scale: f32,
) -> f32 {
    let color = Theme::with_opacity(theme.foreground, opacity);
    let heading_color = Theme::with_opacity(theme.heading_color, opacity);
    let accent = Theme::with_opacity(theme.accent, opacity);
    let cell_padding = 12.0 * scale;
    let row_spacing = 4.0 * scale;
    let font_size = theme.body_size * 0.85 * scale;

    let num_cols = headers.len().max(1);
    let col_width = (max_width - cell_padding * 2.0) / num_cols as f32;

    let mut y = pos.y;

    // Draw headers
    let mut max_header_height = 0.0f32;
    for (col, header) in headers.iter().enumerate() {
        let cell_pos = Pos2::new(
            pos.x + cell_padding + col as f32 * col_width,
            y + cell_padding,
        );
        let h = draw_inlines(
            ui,
            header,
            cell_pos,
            font_size,
            heading_color,
            col_width - cell_padding,
        );
        max_header_height = max_header_height.max(h);
    }
    y += max_header_height + cell_padding * 2.0;

    // Draw separator line
    let line_y = y + row_spacing / 2.0;
    ui.painter().line_segment(
        [
            Pos2::new(pos.x + cell_padding, line_y),
            Pos2::new(pos.x + max_width - cell_padding, line_y),
        ],
        Stroke::new(1.0, accent),
    );
    y += row_spacing;

    // Draw data rows
    for row in rows {
        let mut max_row_height = 0.0f32;
        for (col, cell) in row.iter().enumerate() {
            let cell_pos = Pos2::new(
                pos.x + cell_padding + col as f32 * col_width,
                y + cell_padding,
            );
            let h = draw_inlines(
                ui,
                cell,
                cell_pos,
                font_size,
                color,
                col_width - cell_padding,
            );
            max_row_height = max_row_height.max(h);
        }
        y += max_row_height + cell_padding + row_spacing;
    }

    y - pos.y
}

/// Draw a blockquote. Returns height used.
pub fn draw_blockquote(
    ui: &egui::Ui,
    inlines: &[Inline],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    scale: f32,
) -> f32 {
    let accent = Theme::with_opacity(theme.accent, opacity);
    let color = Theme::with_opacity(theme.foreground, opacity);
    let bar_width = 4.0 * scale;
    let bar_padding = 16.0 * scale;
    let font_size = theme.body_size * 1.1 * scale;

    let text_pos = Pos2::new(pos.x + bar_width + bar_padding, pos.y);
    let text_width = max_width - bar_width - bar_padding;

    let height = draw_inlines(ui, inlines, text_pos, font_size, color, text_width);

    // Draw accent bar
    let bar_rect = egui::Rect::from_min_size(pos, egui::vec2(bar_width, height));
    ui.painter().rect_filled(bar_rect, 2.0, accent);

    height
}

/// Draw all blocks in a slide sequentially. Returns total height used.
#[allow(clippy::too_many_arguments)]
pub fn draw_blocks(
    ui: &egui::Ui,
    blocks: &[Block],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    scale: f32,
) -> f32 {
    let block_spacing = 20.0 * scale;
    let mut y_offset = 0.0;

    for block in blocks {
        let block_pos = Pos2::new(pos.x, pos.y + y_offset);
        let height = draw_block(
            ui,
            block,
            theme,
            block_pos,
            max_width,
            opacity,
            image_cache,
            reveal_step,
            scale,
        );
        y_offset += height + block_spacing;
    }

    y_offset
}

/// Measure total height of a block list without drawing.
pub fn measure_blocks_height(
    ui: &egui::Ui,
    blocks: &[Block],
    theme: &Theme,
    max_width: f32,
    scale: f32,
) -> f32 {
    let block_spacing = 20.0 * scale;
    let mut total = 0.0;
    for (i, block) in blocks.iter().enumerate() {
        total += measure_single_block_height(ui, block, theme, max_width, scale);
        if i < blocks.len() - 1 {
            total += block_spacing;
        }
    }
    total
}

/// Estimate the height of a single block without drawing.
pub fn measure_single_block_height(
    ui: &egui::Ui,
    block: &Block,
    theme: &Theme,
    max_width: f32,
    scale: f32,
) -> f32 {
    match block {
        Block::Heading { level, inlines } => {
            let size = theme.heading_size(*level) * scale;
            let job = inlines_to_job(inlines, size, theme.heading_color, max_width);
            ui.painter().layout_job(job).rect.height()
        }
        Block::Paragraph { inlines } | Block::BlockQuote { inlines } => {
            let size = theme.body_size * scale;
            let job = inlines_to_job(inlines, size, theme.foreground, max_width);
            ui.painter().layout_job(job).rect.height()
        }
        Block::List { items, .. } => {
            let font_size = theme.body_size * scale;
            let item_spacing = 8.0 * scale;
            count_list_items(items) as f32 * (font_size + item_spacing)
        }
        Block::CodeBlock { code, .. } => {
            let line_count = code.lines().count().max(1);
            let line_height = theme.code_size * scale * 1.4;
            let padding = 16.0 * scale;
            line_count as f32 * line_height + padding * 2.0
        }
        Block::Table { rows, .. } => {
            let row_height = theme.body_size * scale * 1.6;
            rows.len() as f32 * row_height + 10.0 * scale
        }
        Block::HorizontalRule => 2.0 * scale,
        _ => theme.body_size * scale * 1.5,
    }
}

fn count_list_items(items: &[ListItem]) -> usize {
    let mut count = items.len();
    for item in items {
        count += count_list_items(&item.children);
    }
    count
}

/// Draw a single block. Returns height used.
#[allow(clippy::too_many_arguments)]
pub fn draw_block(
    ui: &egui::Ui,
    block: &Block,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    scale: f32,
) -> f32 {
    match block {
        Block::Heading { level, inlines } => {
            draw_heading(ui, inlines, *level, theme, pos, max_width, opacity, scale)
        }
        Block::Paragraph { inlines } => {
            draw_paragraph(ui, inlines, theme, pos, max_width, opacity, scale)
        }
        Block::List { ordered, items } => draw_list(
            ui,
            items,
            *ordered,
            theme,
            pos,
            max_width,
            opacity,
            0,
            reveal_step,
            scale,
        ),
        Block::CodeBlock {
            language,
            code,
            highlight_lines,
        } => draw_code_block(
            ui,
            code,
            language.as_deref(),
            highlight_lines,
            theme,
            pos,
            max_width,
            opacity,
            scale,
        ),
        Block::BlockQuote { inlines } => {
            draw_blockquote(ui, inlines, theme, pos, max_width, opacity, scale)
        }
        Block::Table { headers, rows } => {
            draw_table(ui, headers, rows, theme, pos, max_width, opacity, scale)
        }
        Block::Image {
            alt,
            path,
            directives,
        } => draw_image(
            ui,
            path,
            alt,
            directives,
            theme,
            pos,
            max_width,
            opacity,
            image_cache,
            scale,
        ),
        Block::Diagram { content } => draw_diagram_sized(
            ui,
            content,
            theme,
            pos,
            max_width,
            0.0,
            opacity,
            image_cache,
            reveal_step,
            None,
            scale,
        ),
        Block::HorizontalRule => {
            let color = Theme::with_opacity(theme.accent, opacity * 0.5);
            let y = pos.y + 10.0 * scale;
            ui.painter().line_segment(
                [Pos2::new(pos.x, y), Pos2::new(pos.x + max_width, y)],
                Stroke::new(1.0, color),
            );
            20.0 * scale
        }
        Block::ColumnSeparator => 0.0, // handled by two-column layout
    }
}

/// Draw an image, loading from cache. Falls back to placeholder if unavailable.
#[allow(clippy::too_many_arguments)]
pub fn draw_image(
    ui: &egui::Ui,
    path: &str,
    alt: &str,
    directives: &ImageDirectives,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    image_cache: &ImageCache,
    scale: f32,
) -> f32 {
    if let Some(texture) = image_cache.get_or_load(ui, path) {
        let tex_size = texture.size_vec2();
        let max_height = 400.0 * scale;
        let available = egui::Rect::from_min_size(pos, egui::vec2(max_width, max_height));
        let draw_rect = compute_image_rect(directives, tex_size, available);
        let alpha = (opacity * 255.0) as u8;
        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), draw_rect, uv, tint);
        draw_rect.height()
    } else {
        draw_image_placeholder(ui, alt, directives, theme, pos, max_width, opacity, scale)
    }
}

/// Draw an image with full control over the available area (used by image_slide layout).
/// Returns the actual drawn rect.
#[allow(clippy::too_many_arguments)]
pub fn draw_image_in_area(
    ui: &egui::Ui,
    path: &str,
    alt: &str,
    directives: &ImageDirectives,
    theme: &Theme,
    available: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
) -> egui::Rect {
    if let Some(texture) = image_cache.get_or_load(ui, path) {
        let tex_size = texture.size_vec2();
        let draw_rect = compute_image_rect(directives, tex_size, available);
        let alpha = (opacity * 255.0) as u8;
        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), draw_rect, uv, tint);
        draw_rect
    } else {
        let height = draw_image_placeholder(
            ui,
            alt,
            directives,
            theme,
            available.left_top(),
            available.width(),
            opacity,
            1.0,
        );
        egui::Rect::from_min_size(available.left_top(), egui::vec2(available.width(), height))
    }
}

fn compute_image_rect(
    directives: &ImageDirectives,
    tex_size: egui::Vec2,
    available: egui::Rect,
) -> egui::Rect {
    let avail_w = available.width();
    let avail_h = available.height();

    if directives.fill {
        // Cover: scale to fill, center, may crop
        let scale = (avail_w / tex_size.x).max(avail_h / tex_size.y);
        let draw_w = tex_size.x * scale;
        let draw_h = tex_size.y * scale;
        let offset_x = (avail_w - draw_w) / 2.0;
        let offset_y = (avail_h - draw_h) / 2.0;
        egui::Rect::from_min_size(
            egui::pos2(available.left() + offset_x, available.top() + offset_y),
            egui::vec2(draw_w, draw_h),
        )
    } else if let Some(ref width_str) = directives.width {
        // Explicit width
        let target_w = parse_size(width_str, avail_w);
        let scale = target_w / tex_size.x;
        let draw_h = (tex_size.y * scale).min(avail_h);
        let actual_scale = if tex_size.y * scale > avail_h {
            avail_h / tex_size.y
        } else {
            scale
        };
        let actual_w = tex_size.x * actual_scale;
        let actual_h = tex_size.y * actual_scale;
        let offset_x = (avail_w - actual_w) / 2.0;
        let offset_y = (avail_h - actual_h).max(0.0) / 2.0;
        let _ = draw_h;
        egui::Rect::from_min_size(
            egui::pos2(available.left() + offset_x, available.top() + offset_y),
            egui::vec2(actual_w, actual_h),
        )
    } else {
        // Contain: fit within available area, preserve aspect ratio
        let scale = (avail_w / tex_size.x).min(avail_h / tex_size.y).min(1.0);
        let draw_w = tex_size.x * scale;
        let draw_h = tex_size.y * scale;
        let offset_x = (avail_w - draw_w) / 2.0;
        let offset_y = (avail_h - draw_h) / 2.0;
        egui::Rect::from_min_size(
            egui::pos2(available.left() + offset_x, available.top() + offset_y),
            egui::vec2(draw_w, draw_h),
        )
    }
}

fn parse_size(s: &str, reference: f32) -> f32 {
    if let Some(pct) = s.strip_suffix('%') {
        if let Ok(v) = pct.trim().parse::<f32>() {
            return reference * v / 100.0;
        }
    }
    if let Some(px) = s.strip_suffix("px") {
        if let Ok(v) = px.trim().parse::<f32>() {
            return v;
        }
    }
    s.parse::<f32>().unwrap_or(reference * 0.8)
}

#[allow(clippy::too_many_arguments)]
pub fn draw_image_placeholder(
    ui: &egui::Ui,
    alt: &str,
    _directives: &crate::parser::ImageDirectives,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    opacity: f32,
    scale: f32,
) -> f32 {
    let height = 200.0 * scale;
    let bg = Theme::with_opacity(theme.code_background, opacity);
    let color = Theme::with_opacity(theme.foreground, opacity * 0.6);

    let rect = egui::Rect::from_min_size(pos, egui::vec2(max_width, height));
    ui.painter().rect_filled(rect, 8.0 * scale, bg);
    ui.painter().rect_stroke(
        rect,
        8.0 * scale,
        Stroke::new(1.0, color),
        egui::StrokeKind::Outside,
    );

    let label = if alt.is_empty() {
        "[Image]".to_string()
    } else {
        format!("[Image: {alt}]")
    };
    let galley = ui.painter().layout(
        label,
        FontId::proportional(theme.body_size * 0.8 * scale),
        color,
        max_width,
    );
    let text_pos = Pos2::new(
        pos.x + (max_width - galley.rect.width()) / 2.0,
        pos.y + (height - galley.rect.height()) / 2.0,
    );
    ui.painter().galley(text_pos, galley, color);

    height
}
