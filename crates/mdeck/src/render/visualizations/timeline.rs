use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_FONT_PRIMARY_LABEL, VIZ_FONT_SECONDARY_LABEL, VIZ_OPACITY_AXIS, VIZ_OPACITY_LABEL,
    VIZ_STROKE_CONNECTOR, VIZ_STROKE_SEPARATOR, VIZ_TIMELINE_DOT, VizReveal, assign_steps,
    parse_reveal_prefix,
};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct TimelineEntry {
    date: String,
    description: String,
    reveal: VizReveal,
}

fn parse_timeline(content: &str) -> Vec<TimelineEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        // Parse "Date: Description"
        if let Some(colon_pos) = text.find(": ") {
            let date = text[..colon_pos].trim().to_string();
            let description = text[colon_pos + 2..].trim().to_string();
            entries.push(TimelineEntry {
                date,
                description,
                reveal,
            });
        } else {
            // No colon — treat whole line as description
            entries.push(TimelineEntry {
                date: String::new(),
                description: text.to_string(),
                reveal,
            });
        }
    }
    entries
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_timeline(
    ui: &egui::Ui,
    content: &str,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    reveal_step: usize,
    scale: f32,
) -> f32 {
    let entries = parse_timeline(content);
    if entries.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    let reveals: Vec<VizReveal> = entries.iter().map(|e| e.reveal).collect();
    let steps = assign_steps(&reveals);
    let palette = theme.edge_palette();
    let painter = ui.painter();

    let n = entries.len();
    let line_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_AXIS);

    // Horizontal layout: events spaced along a central line
    let line_y = pos.y + height * 0.5;
    let margin = 80.0 * scale;
    let usable_width = max_width - margin * 2.0;
    let spacing = if n > 1 {
        usable_width / (n - 1) as f32
    } else {
        0.0
    };

    // Draw the main timeline line
    let line_start = Pos2::new(pos.x + margin * 0.5, line_y);
    let line_end = Pos2::new(pos.x + max_width - margin * 0.5, line_y);
    painter.line_segment(
        [line_start, line_end],
        Stroke::new(VIZ_STROKE_SEPARATOR * scale, line_color),
    );

    let date_font = FontId::proportional(theme.body_size * VIZ_FONT_PRIMARY_LABEL * scale);
    let desc_font = FontId::proportional(theme.body_size * VIZ_FONT_SECONDARY_LABEL * scale);
    let dot_radius = VIZ_TIMELINE_DOT * scale;

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let x = if n > 1 {
            pos.x + margin + i as f32 * spacing
        } else {
            pos.x + max_width / 2.0
        };

        let color = Theme::with_opacity(palette[i % palette.len()], opacity);
        let alternate_above = i % 2 == 0;

        // Dot on the line
        painter.circle_filled(Pos2::new(x, line_y), dot_radius, color);
        // White inner dot
        let inner_color = Theme::with_opacity(theme.background, opacity);
        painter.circle_filled(Pos2::new(x, line_y), dot_radius * 0.4, inner_color);

        // Connector line
        let connector_len = 50.0 * scale;
        let (connector_start, connector_end, text_anchor_y) = if alternate_above {
            (
                Pos2::new(x, line_y - dot_radius),
                Pos2::new(x, line_y - dot_radius - connector_len),
                line_y - dot_radius - connector_len,
            )
        } else {
            (
                Pos2::new(x, line_y + dot_radius),
                Pos2::new(x, line_y + dot_radius + connector_len),
                line_y + dot_radius + connector_len,
            )
        };
        painter.line_segment(
            [connector_start, connector_end],
            Stroke::new(
                VIZ_STROKE_CONNECTOR * scale,
                Theme::with_opacity(color, opacity * 0.6),
            ),
        );

        // Date label
        let date_color = Theme::with_opacity(theme.heading_color, opacity);
        let max_label_width = spacing.max(120.0 * scale);
        let date_galley = painter.layout(
            entry.date.clone(),
            date_font.clone(),
            date_color,
            max_label_width,
        );
        let date_w = date_galley.rect.width();
        let date_h = date_galley.rect.height();

        // Description label
        let desc_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_LABEL);
        let desc_galley = painter.layout(
            entry.description.clone(),
            desc_font.clone(),
            desc_color,
            max_label_width,
        );
        let desc_w = desc_galley.rect.width();
        let desc_h = desc_galley.rect.height();

        if alternate_above {
            // Text above: description first (higher), then date
            let desc_y = text_anchor_y - desc_h - date_h - 4.0 * scale;
            let date_y = text_anchor_y - date_h;
            painter.galley(Pos2::new(x - desc_w / 2.0, desc_y), desc_galley, desc_color);
            painter.galley(Pos2::new(x - date_w / 2.0, date_y), date_galley, date_color);
        } else {
            // Text below: date first, then description
            let date_y = text_anchor_y + 4.0 * scale;
            let desc_y = date_y + date_h + 2.0 * scale;
            painter.galley(Pos2::new(x - date_w / 2.0, date_y), date_galley, date_color);
            painter.galley(Pos2::new(x - desc_w / 2.0, desc_y), desc_galley, desc_color);
        }
    }

    height
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timeline_basic() {
        let content = "- 2000: Y2K Bug\n+ 2007: iPhone Released";
        let entries = parse_timeline(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].date, "2000");
        assert_eq!(entries[0].description, "Y2K Bug");
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].date, "2007");
        assert_eq!(entries[1].description, "iPhone Released");
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
    }

    #[test]
    fn test_parse_timeline_no_description() {
        let content = "- Just a label";
        let entries = parse_timeline(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "");
        assert_eq!(entries[0].description, "Just a label");
    }

    #[test]
    fn test_parse_timeline_skips_comments() {
        let content = "# header\n- 2020: Event\n# note";
        let entries = parse_timeline(content);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_parse_timeline_reveal_markers() {
        let content = "- A: first\n+ B: second\n* C: third";
        let entries = parse_timeline(content);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::WithPrev);
    }
}
