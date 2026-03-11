use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_CORNER_SWATCH, VIZ_FONT_LEGEND, VIZ_OPACITY_BORDER_RING, VIZ_OPACITY_FILL,
    VIZ_STROKE_BORDER, VIZ_STROKE_SEPARATOR, VIZ_SWATCH_SIZE, VizReveal, assign_steps,
    parse_reveal_prefix, reveal_anim_progress,
};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DonutEntry {
    label: String,
    value: f32,
    reveal: VizReveal,
}

fn parse_donut_chart(content: &str) -> (Vec<DonutEntry>, Option<String>) {
    let mut entries = Vec::new();
    let mut center_text = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse center text directive
        if trimmed.starts_with('#') {
            if let Some(rest) = trimmed
                .strip_prefix("# center:")
                .or_else(|| trimmed.strip_prefix("#center:"))
            {
                center_text = Some(rest.trim().to_string());
            }
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
                entries.push(DonutEntry {
                    label,
                    value,
                    reveal,
                });
            }
        }
    }

    (entries, center_text)
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_donut_chart(
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
    let (entries, center_text) = parse_donut_chart(content);
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

    // Layout: donut on left side, legend on right
    let legend_width = 380.0 * scale;
    let donut_area_width = max_width - legend_width;
    let outer_radius = (donut_area_width.min(height) / 2.0 - 30.0 * scale).max(40.0 * scale);
    let inner_radius = outer_radius * 0.5; // 50% thickness (thick ring)
    let donut_cx = pos.x + donut_area_width / 2.0;
    let donut_cy = pos.y + height / 2.0;

    // Draw donut slices
    let segment_count = 360;
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
        let color = Theme::with_opacity(palette[i % palette.len()], opacity * VIZ_OPACITY_FILL);

        // Draw filled arc as triangle fan (outer arc)
        let segments = ((segment_count as f32 * (entry.value / total) * anim) as usize).max(4);
        let angle_step = sweep / segments as f32;

        for s in 0..segments {
            let a1 = angle_offset + s as f32 * angle_step;
            let a2 = angle_offset + (s + 1) as f32 * angle_step;

            // Outer edge points
            let outer1 = Pos2::new(
                donut_cx + outer_radius * a1.cos(),
                donut_cy + outer_radius * a1.sin(),
            );
            let outer2 = Pos2::new(
                donut_cx + outer_radius * a2.cos(),
                donut_cy + outer_radius * a2.sin(),
            );
            // Inner edge points
            let inner1 = Pos2::new(
                donut_cx + inner_radius * a1.cos(),
                donut_cy + inner_radius * a1.sin(),
            );
            let inner2 = Pos2::new(
                donut_cx + inner_radius * a2.cos(),
                donut_cy + inner_radius * a2.sin(),
            );

            // Draw as two triangles forming a quad
            painter.add(egui::Shape::convex_polygon(
                vec![inner1, outer1, outer2, inner2],
                color,
                Stroke::NONE,
            ));
        }

        // Separator line between slices
        let end_angle = angle_offset + sweep;
        let sep_inner = Pos2::new(
            donut_cx + (inner_radius - 1.0) * end_angle.cos(),
            donut_cy + (inner_radius - 1.0) * end_angle.sin(),
        );
        let sep_outer = Pos2::new(
            donut_cx + (outer_radius + 1.0) * end_angle.cos(),
            donut_cy + (outer_radius + 1.0) * end_angle.sin(),
        );
        painter.line_segment(
            [sep_inner, sep_outer],
            Stroke::new(VIZ_STROKE_SEPARATOR * scale, bg_color),
        );

        angle_offset += sweep;
    }

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    // Draw center hole (background color circle to create donut effect)
    painter.circle_filled(Pos2::new(donut_cx, donut_cy), inner_radius, bg_color);

    // Draw subtle border rings
    let ring_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_BORDER_RING);
    painter.circle_stroke(
        Pos2::new(donut_cx, donut_cy),
        outer_radius,
        Stroke::new(VIZ_STROKE_BORDER * scale, ring_color),
    );
    painter.circle_stroke(
        Pos2::new(donut_cx, donut_cy),
        inner_radius,
        Stroke::new(1.0 * scale, ring_color),
    );

    // Draw center text
    if let Some(ref text) = center_text {
        let center_font = FontId::proportional(theme.body_size * 1.2 * scale);
        let text_color = Theme::with_opacity(theme.foreground, opacity);
        let galley = painter.layout_no_wrap(text.clone(), center_font, text_color);
        painter.galley(
            Pos2::new(
                donut_cx - galley.rect.width() / 2.0,
                donut_cy - galley.rect.height() / 2.0,
            ),
            galley,
            text_color,
        );
    }

    // Draw legend on the right
    let legend_x = pos.x + donut_area_width + 20.0 * scale;
    let legend_item_height = 48.0 * scale;
    let total_legend_height = entries.len() as f32 * legend_item_height;
    let legend_start_y = pos.y + (height - total_legend_height) / 2.0;
    let swatch_size = VIZ_SWATCH_SIZE * scale;
    let label_font = FontId::proportional(theme.body_size * VIZ_FONT_LEGEND * scale);

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let ly = legend_start_y + i as f32 * legend_item_height;
        let color = Theme::with_opacity(palette[i % palette.len()], opacity * VIZ_OPACITY_FILL);

        // Color swatch
        let swatch_rect = egui::Rect::from_min_size(
            Pos2::new(legend_x, ly + (legend_item_height - swatch_size) / 2.0),
            egui::vec2(swatch_size, swatch_size),
        );
        painter.rect_filled(swatch_rect, VIZ_CORNER_SWATCH * scale, color);

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
    fn test_parse_donut_chart_basic() {
        let content = "- Complete: 78\n- Remaining: 22";
        let (entries, center) = parse_donut_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Complete");
        assert_eq!(entries[0].value, 78.0);
        assert_eq!(entries[1].label, "Remaining");
        assert_eq!(entries[1].value, 22.0);
        assert!(center.is_none());
    }

    #[test]
    fn test_parse_donut_chart_with_center() {
        let content = "# center: 78%\n- Complete: 78\n- Remaining: 22";
        let (entries, center) = parse_donut_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(center, Some("78%".to_string()));
    }

    #[test]
    fn test_parse_donut_chart_reveal_markers() {
        let content = "- A: 40%\n+ B: 30%\n* C: 30%";
        let (entries, _) = parse_donut_chart(content);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_donut_chart_skips_invalid() {
        let content = "# center: Done\n- Valid: 50%\n- no_value\n# comment\n- Also Valid: 50%";
        let (entries, center) = parse_donut_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(center, Some("Done".to_string()));
    }
}
