use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_CORNER_TRACK, VIZ_FONT_PRIMARY_LABEL, VIZ_FONT_TITLE, VIZ_OPACITY_FILL, VIZ_OPACITY_GRID,
    VIZ_OPACITY_LABEL, VIZ_STROKE_BORDER, VizReveal, assign_steps, parse_reveal_prefix,
    reveal_anim_progress,
};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ProgressEntry {
    label: String,
    value: f32,
    reveal: VizReveal,
}

fn parse_progress_bars(content: &str) -> Vec<ProgressEntry> {
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

        // Parse "Label: 75%" or "Label: 75"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let value_str = text[colon_pos + 2..].trim().trim_end_matches('%');
            if let Ok(value) = value_str.parse::<f32>() {
                entries.push(ProgressEntry {
                    label,
                    value: value.clamp(0.0, 100.0),
                    reveal,
                });
            }
        }
    }
    entries
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_progress_bars(
    ui: &egui::Ui,
    content: &str,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) -> f32 {
    let entries = parse_progress_bars(content);
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
    let label_font = FontId::proportional(theme.body_size * VIZ_FONT_TITLE * scale);
    let pct_font = FontId::proportional(theme.body_size * VIZ_FONT_PRIMARY_LABEL * scale);

    // Layout — use generous space for readability from distance
    let padding = 30.0 * scale;
    let label_width = 200.0 * scale;
    let pct_width = 90.0 * scale;
    // Scale bar height to fill available space while maintaining breathing room
    let available_height = height - padding * 2.0;
    let max_bar_height = 44.0 * scale;
    let min_bar_height = 28.0 * scale;
    let row_spacing = 20.0 * scale;
    let bar_height = ((available_height - (n as f32 - 1.0) * row_spacing) / n as f32)
        .clamp(min_bar_height, max_bar_height);
    let total_rows_height = n as f32 * (bar_height + row_spacing) - row_spacing;
    let start_y = pos.y + (height - total_rows_height) / 2.0;
    let bar_left = pos.x + padding + label_width + 12.0 * scale;
    let bar_width = max_width - padding * 2.0 - label_width - 12.0 * scale - pct_width;

    let mut needs_repaint = false;

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let row_y = start_y + i as f32 * (bar_height + row_spacing);

        // Label on the left
        let label_color = Theme::with_opacity(theme.foreground, opacity * 0.9);
        let galley = painter.layout_no_wrap(entry.label.clone(), label_font.clone(), label_color);
        let label_y = row_y + (bar_height - galley.rect.height()) / 2.0;
        let label_x = bar_left - 12.0 * scale - galley.rect.width();
        painter.galley(Pos2::new(label_x, label_y), galley, label_color);

        // Track background
        let track_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_GRID);
        let track_rect = egui::Rect::from_min_size(
            Pos2::new(bar_left, row_y),
            egui::vec2(bar_width, bar_height),
        );
        painter.rect_filled(track_rect, VIZ_CORNER_TRACK * scale, track_color);

        // Fill bar
        let color = palette[i % palette.len()];
        let fill_color = Theme::with_opacity(color, opacity * VIZ_OPACITY_FILL);
        let fill_frac = (entry.value / 100.0) * anim;
        let fill_width = bar_width * fill_frac;
        if fill_width > 0.0 {
            let fill_rect = egui::Rect::from_min_size(
                Pos2::new(bar_left, row_y),
                egui::vec2(fill_width, bar_height),
            );
            painter.rect_filled(fill_rect, VIZ_CORNER_TRACK * scale, fill_color);
        }

        // Subtle border on track
        let border_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_GRID);
        painter.rect_stroke(
            track_rect,
            VIZ_CORNER_TRACK * scale,
            Stroke::new(VIZ_STROKE_BORDER * scale, border_color),
            egui::StrokeKind::Outside,
        );

        // Percentage on the right
        let pct_text = format!("{:.0}%", entry.value);
        let pct_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_LABEL);
        let pct_galley = painter.layout_no_wrap(pct_text, pct_font.clone(), pct_color);
        let pct_y = row_y + (bar_height - pct_galley.rect.height()) / 2.0;
        let pct_x = bar_left + bar_width + 12.0 * scale;
        painter.galley(Pos2::new(pct_x, pct_y), pct_galley, pct_color);
    }

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    height
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress_bars_basic() {
        let content = "- Design: 100%\n- Frontend: 75%";
        let entries = parse_progress_bars(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Design");
        assert_eq!(entries[0].value, 100.0);
        assert_eq!(entries[1].label, "Frontend");
        assert_eq!(entries[1].value, 75.0);
    }

    #[test]
    fn test_parse_progress_bars_without_percent() {
        let content = "- A: 50\n- B: 80";
        let entries = parse_progress_bars(content);
        assert_eq!(entries[0].value, 50.0);
        assert_eq!(entries[1].value, 80.0);
    }

    #[test]
    fn test_parse_progress_bars_clamped() {
        let content = "- Over: 150%\n- Under: -10";
        let entries = parse_progress_bars(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].value, 100.0); // clamped to max
        assert_eq!(entries[1].value, 0.0); // clamped to min
    }

    #[test]
    fn test_parse_progress_bars_reveal_markers() {
        let content = "- A: 100%\n+ B: 75%\n* C: 50%";
        let entries = parse_progress_bars(content);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_progress_bars_skips_invalid() {
        let content = "- Valid: 50%\n- no_value\n# comment\n- Also: 30%";
        let entries = parse_progress_bars(content);
        assert_eq!(entries.len(), 2);
    }
}
