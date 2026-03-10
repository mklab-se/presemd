use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{VizReveal, assign_steps, parse_reveal_prefix, reveal_anim_progress};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct PieEntry {
    label: String,
    value: f32,
    reveal: VizReveal,
}

fn parse_pie_chart(content: &str) -> Vec<PieEntry> {
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

        // Parse "Label: 40%" or "Label: 40"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let value_str = text[colon_pos + 2..].trim().trim_end_matches('%');
            if let Ok(value) = value_str.parse::<f32>() {
                entries.push(PieEntry {
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
pub fn draw_pie_chart(
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
    let entries = parse_pie_chart(content);
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

    // Compute total for percentages
    let total: f32 = entries.iter().map(|e| e.value).sum();
    if total <= 0.0 {
        return height;
    }

    // Layout: pie on left side, legend on right
    let legend_width = 380.0 * scale;
    let pie_area_width = max_width - legend_width;
    let pie_radius = (pie_area_width.min(height) / 2.0 - 30.0 * scale).max(40.0 * scale);
    let pie_cx = pos.x + pie_area_width / 2.0;
    let pie_cy = pos.y + height / 2.0;

    // Draw pie slices
    let segment_count = 360; // segments per full circle for smooth arcs
    let mut angle_offset = -std::f32::consts::FRAC_PI_2; // start at top
    let mut needs_repaint = false;

    let bg_color = Theme::with_opacity(theme.background, opacity);

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let full_sweep = (entry.value / total) * 2.0 * std::f32::consts::PI;
        let sweep = full_sweep * anim;
        let color = Theme::with_opacity(palette[i % palette.len()], opacity * 0.85);

        // Draw filled arc as triangle fan with enough segments for smooth curves
        let segments = ((segment_count as f32 * (entry.value / total) * anim) as usize).max(4);
        let angle_step = sweep / segments as f32;

        for s in 0..segments {
            let a1 = angle_offset + s as f32 * angle_step;
            let a2 = angle_offset + (s + 1) as f32 * angle_step;
            let p1 = Pos2::new(
                pie_cx + pie_radius * a1.cos(),
                pie_cy + pie_radius * a1.sin(),
            );
            let p2 = Pos2::new(
                pie_cx + pie_radius * a2.cos(),
                pie_cy + pie_radius * a2.sin(),
            );
            let center = Pos2::new(pie_cx, pie_cy);

            painter.add(egui::Shape::convex_polygon(
                vec![center, p1, p2],
                color,
                Stroke::NONE,
            ));
        }

        // Separator line between slices
        let end_angle = angle_offset + sweep;
        let sep_end = Pos2::new(
            pie_cx + (pie_radius + 1.0) * end_angle.cos(),
            pie_cy + (pie_radius + 1.0) * end_angle.sin(),
        );
        painter.line_segment(
            [Pos2::new(pie_cx, pie_cy), sep_end],
            Stroke::new(2.0 * scale, bg_color),
        );

        angle_offset += sweep;
    }

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    // Draw subtle border ring
    let ring_color = Theme::with_opacity(theme.foreground, opacity * 0.15);
    painter.circle_stroke(
        Pos2::new(pie_cx, pie_cy),
        pie_radius,
        Stroke::new(1.5 * scale, ring_color),
    );

    // Draw legend on the right
    let legend_x = pos.x + pie_area_width + 20.0 * scale;
    let legend_item_height = 48.0 * scale;
    let total_legend_height = entries.len() as f32 * legend_item_height;
    let legend_start_y = pos.y + (height - total_legend_height) / 2.0;
    let swatch_size = 20.0 * scale;
    let label_font = FontId::proportional(theme.body_size * 0.65 * scale);

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let ly = legend_start_y + i as f32 * legend_item_height;
        let color = Theme::with_opacity(palette[i % palette.len()], opacity * 0.85);

        // Color swatch
        let swatch_rect = egui::Rect::from_min_size(
            Pos2::new(legend_x, ly + (legend_item_height - swatch_size) / 2.0),
            egui::vec2(swatch_size, swatch_size),
        );
        painter.rect_filled(swatch_rect, 3.0 * scale, color);

        // Label + percentage
        let pct = entry.value / total * 100.0;
        let label_text = format!("{} ({:.0}%)", entry.label, pct);
        let text_color = Theme::with_opacity(theme.foreground, opacity);
        let galley = painter.layout_no_wrap(label_text, label_font.clone(), text_color);
        let text_y = ly + (legend_item_height - galley.rect.height()) / 2.0;
        painter.galley(
            Pos2::new(legend_x + swatch_size + 10.0 * scale, text_y),
            galley,
            text_color,
        );
    }

    height
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pie_chart_percentages() {
        let content = "- Category A: 40%\n- Category B: 25%";
        let entries = parse_pie_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Category A");
        assert_eq!(entries[0].value, 40.0);
        assert_eq!(entries[1].label, "Category B");
        assert_eq!(entries[1].value, 25.0);
    }

    #[test]
    fn test_parse_pie_chart_raw_values() {
        let content = "- Sales: 100\n- Costs: 60";
        let entries = parse_pie_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].value, 100.0);
        assert_eq!(entries[1].value, 60.0);
    }

    #[test]
    fn test_parse_pie_chart_reveal_markers() {
        let content = "- A: 40%\n+ B: 30%\n* C: 30%";
        let entries = parse_pie_chart(content);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_pie_chart_skips_invalid() {
        let content = "- Valid: 50%\n- no_value\n# comment\n- Also Valid: 50%";
        let entries = parse_pie_chart(content);
        assert_eq!(entries.len(), 2);
    }
}
