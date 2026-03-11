use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_CORNER_NODE, VIZ_FONT_SECONDARY_LABEL, VIZ_FONT_TITLE, VIZ_LABEL_REVEAL_THRESHOLD,
    VIZ_OPACITY_FILL, VizReveal, assign_steps, parse_reveal_prefix, reveal_anim_progress,
};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct FunnelEntry {
    label: String,
    value: f32,
    reveal: VizReveal,
}

fn parse_funnel_chart(content: &str) -> Vec<FunnelEntry> {
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

        // Parse "Label: 10000"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let value_str = text[colon_pos + 2..].trim().trim_end_matches('%');
            if let Ok(value) = value_str.parse::<f32>() {
                entries.push(FunnelEntry {
                    label,
                    value,
                    reveal,
                });
            }
        }
    }
    entries
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_funnel_chart(
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
    let entries = parse_funnel_chart(content);
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

    let max_value = entries.iter().map(|e| e.value).fold(0.0f32, f32::max);
    if max_value <= 0.0 {
        return height;
    }

    let n = entries.len();
    let padding = 40.0 * scale;
    let gap = 4.0 * scale;
    let total_gaps = (n.saturating_sub(1)) as f32 * gap;
    let available_height = height - padding * 2.0 - total_gaps;
    let trapezoid_height = available_height / n as f32;

    // Funnel is centered horizontally with max width for the widest bar
    let funnel_max_width = max_width - padding * 2.0;
    let min_width_ratio = 0.2; // narrowest trapezoid is at least 20% of max
    let center_x = pos.x + max_width / 2.0;

    let label_font = FontId::proportional(theme.body_size * VIZ_FONT_TITLE * scale);
    let value_font = FontId::proportional(theme.body_size * VIZ_FONT_SECONDARY_LABEL * scale);

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

        let color = Theme::with_opacity(palette[i % palette.len()], opacity * VIZ_OPACITY_FILL);

        // Width proportional to value relative to max
        let width_frac = entry.value / max_value;
        let top_width = funnel_max_width * (min_width_ratio + (1.0 - min_width_ratio) * width_frac);

        // Next entry's width (for trapezoid bottom), or a bit narrower
        let next_width_frac = entries
            .get(i + 1)
            .map(|e| e.value / max_value)
            .unwrap_or(width_frac * 0.6);
        let bottom_width =
            funnel_max_width * (min_width_ratio + (1.0 - min_width_ratio) * next_width_frac);

        let top_y = pos.y + padding + i as f32 * (trapezoid_height + gap);
        let full_h = trapezoid_height;
        let h = full_h * anim;

        // Interpolate bottom width based on animation progress
        let anim_bottom_width = top_width + (bottom_width - top_width) * anim;

        let half_top = top_width / 2.0;
        let half_bot = anim_bottom_width / 2.0;

        // Build a smooth rounded trapezoid using line segments
        // Round the corners with small arcs approximated by extra points
        let corner_r = (VIZ_CORNER_NODE * scale)
            .min(h * 0.3)
            .min((half_top - half_bot).abs() * 0.3);

        let mut points = Vec::with_capacity(20);
        // Top-left corner
        points.push(Pos2::new(center_x - half_top + corner_r, top_y));
        // Top-right corner
        points.push(Pos2::new(center_x + half_top - corner_r, top_y));
        points.push(Pos2::new(center_x + half_top, top_y + corner_r));
        // Right side slopes down
        points.push(Pos2::new(center_x + half_bot, top_y + h - corner_r));
        // Bottom-right corner
        points.push(Pos2::new(center_x + half_bot - corner_r, top_y + h));
        // Bottom-left corner
        points.push(Pos2::new(center_x - half_bot + corner_r, top_y + h));
        points.push(Pos2::new(center_x - half_bot, top_y + h - corner_r));
        // Left side slopes up
        points.push(Pos2::new(center_x - half_top, top_y + corner_r));

        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));

        // Label centered in trapezoid (only when sufficiently visible)
        if anim > VIZ_LABEL_REVEAL_THRESHOLD {
            let label_opacity =
                ((anim - VIZ_LABEL_REVEAL_THRESHOLD) / (1.0 - VIZ_LABEL_REVEAL_THRESHOLD)).min(1.0);
            let mid_y = top_y + h / 2.0;

            // Entry label
            let label_color = Theme::with_opacity(theme.foreground, opacity * label_opacity);
            let galley =
                painter.layout_no_wrap(entry.label.clone(), label_font.clone(), label_color);
            let lx = center_x - galley.rect.width() / 2.0;
            painter.galley(
                Pos2::new(lx, mid_y - galley.rect.height() - 1.0 * scale),
                galley,
                label_color,
            );

            // Value and percentage
            let pct = entry.value / max_value * 100.0;
            let val_text = if entry.value == entry.value.floor() {
                format!("{:.0} ({:.0}%)", entry.value, pct)
            } else {
                format!("{:.1} ({:.0}%)", entry.value, pct)
            };
            let val_color = Theme::with_opacity(theme.foreground, opacity * 0.7 * label_opacity);
            let val_galley = painter.layout_no_wrap(val_text, value_font.clone(), val_color);
            let vx = center_x - val_galley.rect.width() / 2.0;
            painter.galley(Pos2::new(vx, mid_y + 1.0 * scale), val_galley, val_color);
        }
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
    fn test_parse_funnel_chart_basic() {
        let content = "- Visitors: 10000\n- Signups: 5000\n- Paid: 1000";
        let entries = parse_funnel_chart(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].label, "Visitors");
        assert_eq!(entries[0].value, 10000.0);
        assert_eq!(entries[2].label, "Paid");
        assert_eq!(entries[2].value, 1000.0);
    }

    #[test]
    fn test_parse_funnel_chart_reveal_markers() {
        let content = "- Visitors: 10000\n+ Signups: 5000\n+ Activated: 2500\n+ Paid: 1000";
        let entries = parse_funnel_chart(content);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::NextStep);
        assert_eq!(entries[3].reveal, VizReveal::NextStep);
    }

    #[test]
    fn test_parse_funnel_chart_skips_comments_and_empty() {
        let content = "# comment\n\n- A: 100\n- invalid line\n- B: 50";
        let entries = parse_funnel_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "A");
        assert_eq!(entries[1].label, "B");
    }

    #[test]
    fn test_parse_funnel_chart_percentage_suffix() {
        let content = "- Top: 100%\n- Mid: 50%";
        let entries = parse_funnel_chart(content);
        assert_eq!(entries[0].value, 100.0);
        assert_eq!(entries[1].value, 50.0);
    }
}
