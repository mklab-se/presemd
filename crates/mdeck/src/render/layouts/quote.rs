use eframe::egui::{self, Pos2};

use crate::parser::{Block, Inline, Slide};
use crate::render::text;
use crate::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    opacity: f32,
    scale: f32,
) {
    let padding = 80.0 * scale;
    let content_rect = rect.shrink(padding);

    // Find heading, quote, and attribution
    let mut heading: Option<(u8, &Vec<Inline>)> = None;
    let mut quote_inlines: Option<&Vec<Inline>> = None;
    let mut attribution: Option<&Vec<Inline>> = None;

    for block in &slide.blocks {
        match block {
            Block::Heading { level, inlines } => heading = Some((*level, inlines)),
            Block::BlockQuote { inlines } => quote_inlines = Some(inlines),
            Block::Paragraph { inlines } => {
                if quote_inlines.is_some() {
                    attribution = Some(inlines);
                }
            }
            _ => {}
        }
    }

    let quote_size = theme.body_size * 1.3 * scale;

    // Estimate total height for vertical centering
    let mut total_height = 0.0;
    if heading.is_some() {
        total_height += theme.h2_size * scale + 40.0 * scale;
    }
    if quote_inlines.is_some() {
        total_height += quote_size * 3.0; // rough estimate
    }
    if attribution.is_some() {
        total_height += theme.body_size * scale + 20.0 * scale;
    }

    let start_y =
        (content_rect.center().y - total_height / 2.0).max(content_rect.top() + 20.0 * scale);
    let mut y = start_y;

    // Draw heading if present
    if let Some((level, inlines)) = heading {
        let h = text::draw_heading(
            ui,
            inlines,
            level,
            theme,
            Pos2::new(content_rect.left(), y),
            content_rect.width(),
            opacity,
            scale,
        );
        y += h + 40.0 * scale;
    }

    // Draw quote - centered with larger text, quotation marks inline
    if let Some(inlines) = quote_inlines {
        let color = Theme::with_opacity(theme.foreground, opacity);
        let accent = Theme::with_opacity(theme.accent, opacity);
        let quote_width = content_rect.width() * 0.8;
        let quote_x = content_rect.left() + (content_rect.width() - quote_width) / 2.0;

        // Draw left accent bar
        let bar_width = 4.0 * scale;
        let bar_x = quote_x - 16.0 * scale;

        // Build inlines with quotation marks baked in (if not already present)
        let quoted_inlines = wrap_with_quotes(inlines);
        let job = text::inlines_to_job(&quoted_inlines, quote_size, color, quote_width);
        let galley = ui.painter().layout_job(job);
        let text_height = galley.rect.height();
        let text_width = galley.rect.width();
        let text_x = quote_x + (quote_width - text_width) / 2.0;

        // Draw the quote text (marks are part of the text flow)
        ui.painter().galley(Pos2::new(text_x, y), galley, color);

        // Draw left accent bar spanning the quote text
        let bar_rect =
            egui::Rect::from_min_size(Pos2::new(bar_x, y), egui::vec2(bar_width, text_height));
        ui.painter().rect_filled(bar_rect, 2.0, accent);

        y += text_height + 30.0 * scale;
    }

    // Draw attribution - right-aligned, italic
    if let Some(inlines) = attribution {
        let color = Theme::with_opacity(theme.foreground, opacity * 0.7);
        let attr_size = theme.body_size * 0.9 * scale;

        // Strip leading -- or --- from attribution
        let cleaned = clean_attribution(inlines);
        let job = text::inlines_to_job(&cleaned, attr_size, color, content_rect.width());

        let galley = ui.painter().layout_job(job);
        let x = content_rect.right() - galley.rect.width() - 40.0 * scale;
        ui.painter().galley(Pos2::new(x, y), galley, color);
    }
}

/// Wraps quote inlines with curly quotation marks if they don't already have them.
fn wrap_with_quotes(inlines: &[Inline]) -> Vec<Inline> {
    let starts_with_quote = inlines.first().is_some_and(|first| {
        if let Inline::Text(s) = first {
            let t = s.trim_start();
            t.starts_with('\u{201C}') || t.starts_with('"')
        } else {
            false
        }
    });
    let ends_with_quote = inlines.last().is_some_and(|last| {
        if let Inline::Text(s) = last {
            let t = s.trim_end();
            t.ends_with('\u{201D}') || t.ends_with('"')
        } else {
            false
        }
    });

    if starts_with_quote && ends_with_quote {
        return inlines.to_vec();
    }

    let mut result = Vec::with_capacity(inlines.len() + 2);
    if !starts_with_quote {
        result.push(Inline::Text("\u{201C}".to_string()));
    }
    result.extend(inlines.iter().cloned());
    if !ends_with_quote {
        result.push(Inline::Text("\u{201D}".to_string()));
    }
    result
}

fn clean_attribution(inlines: &[Inline]) -> Vec<Inline> {
    let mut result = inlines.to_vec();
    if let Some(Inline::Text(s)) = result.first_mut() {
        let trimmed = s.trim_start();
        if let Some(rest) = trimmed.strip_prefix("---") {
            *s = format!("\u{2014} {}", rest.trim_start());
        } else if let Some(rest) = trimmed.strip_prefix("--") {
            *s = format!("\u{2014} {}", rest.trim_start());
        }
    }
    result
}
