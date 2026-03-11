use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_FONT_PRIMARY_LABEL, VIZ_FONT_SECONDARY_LABEL, VIZ_STROKE_SEPARATOR, VizReveal,
    assign_steps, parse_reveal_prefix, reveal_anim_progress,
};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct VennCircle {
    label: String,
    size: f32,
    reveal: VizReveal,
}

#[derive(Debug, Clone)]
struct VennIntersection {
    sets: Vec<String>,
    label: String,
    reveal: VizReveal,
}

fn parse_venn_diagram(content: &str) -> (Vec<VennCircle>, Vec<VennIntersection>) {
    let mut circles = Vec::new();
    let mut intersections = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        if text.contains(" & ") {
            // Intersection line: "A & B: Label"
            if let Some(colon_pos) = text.find(": ") {
                let sets_part = &text[..colon_pos];
                let label = text[colon_pos + 2..].trim().to_string();
                let sets: Vec<String> = sets_part
                    .split(" & ")
                    .map(|s| s.trim().to_string())
                    .collect();
                intersections.push(VennIntersection {
                    sets,
                    label,
                    reveal,
                });
            }
        } else {
            // Circle line: "Label (size: N)" or "Label"
            let (label, size) = if let Some(paren_start) = text.find('(') {
                let label = text[..paren_start].trim().to_string();
                let inner = text[paren_start..]
                    .trim_start_matches('(')
                    .trim_end_matches(')');
                let size = if let Some(rest) = inner.strip_prefix("size:") {
                    rest.trim().parse::<f32>().unwrap_or(30.0)
                } else {
                    30.0
                };
                (label, size)
            } else {
                // Could be "Label: value" format
                if let Some(colon_pos) = text.find(": ") {
                    let label = text[..colon_pos].trim().to_string();
                    let val_str = text[colon_pos + 2..].trim();
                    // Check if it looks like a size value (not an intersection)
                    if let Ok(v) = val_str.parse::<f32>() {
                        (label, v)
                    } else {
                        (text.to_string(), 30.0)
                    }
                } else {
                    (text.to_string(), 30.0)
                }
            };
            circles.push(VennCircle {
                label,
                size,
                reveal,
            });
        }
    }

    (circles, intersections)
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_venn_diagram(
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
    let (circles, intersections) = parse_venn_diagram(content);
    if circles.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    // Assign reveal steps for circles and intersections together
    let mut all_reveals: Vec<VizReveal> = circles.iter().map(|c| c.reveal).collect();
    let intersection_start = all_reveals.len();
    for inter in &intersections {
        all_reveals.push(inter.reveal);
    }
    let steps = assign_steps(&all_reveals);
    let palette = theme.edge_palette();
    let painter = ui.painter();

    let cx = pos.x + max_width / 2.0;
    let cy = pos.y + height / 2.0;

    // Compute circle radii proportional to size values
    let max_size: f32 = circles.iter().map(|c| c.size).fold(0.0f32, f32::max);
    let max_radius = (max_width.min(height) / 2.0 - 60.0 * scale).max(40.0 * scale);
    let radii: Vec<f32> = circles
        .iter()
        .map(|c| (c.size / max_size).sqrt() * max_radius * 0.7)
        .collect();

    // Compute circle centers based on count
    let centers: Vec<Pos2> = match circles.len() {
        1 => vec![Pos2::new(cx, cy)],
        2 => {
            let overlap = radii[0].min(radii[1]) * 0.6;
            let dist = radii[0] + radii[1] - overlap;
            vec![
                Pos2::new(cx - dist / 2.0, cy),
                Pos2::new(cx + dist / 2.0, cy),
            ]
        }
        _ => {
            // Triangular arrangement for 3+ circles
            let base_dist = max_radius * 0.7;
            let mut positions = Vec::new();
            let n = circles.len();
            for i in 0..n {
                let angle = -std::f32::consts::FRAC_PI_2
                    + (i as f32 / n as f32) * 2.0 * std::f32::consts::PI;
                positions.push(Pos2::new(
                    cx + base_dist * angle.cos(),
                    cy + base_dist * angle.sin(),
                ));
            }
            positions
        }
    };

    let mut needs_repaint = false;
    let label_font = FontId::proportional(theme.body_size * VIZ_FONT_PRIMARY_LABEL * scale);
    let inter_font = FontId::proportional(theme.body_size * VIZ_FONT_SECONDARY_LABEL * scale);

    // Draw circles
    for (i, circle) in circles.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let color = palette[i % palette.len()];
        let fill_color = Theme::with_opacity(color, opacity * 0.25 * anim);
        let stroke_color = Theme::with_opacity(color, opacity * anim);
        let radius = radii[i] * anim;
        let center = centers[i];

        painter.circle_filled(center, radius, fill_color);
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(VIZ_STROKE_SEPARATOR * scale, stroke_color),
        );

        // Label in the non-overlapping region (offset away from center)
        let label_offset_x = (center.x - cx) * 0.4;
        let label_offset_y = (center.y - cy) * 0.4;
        let label_color = Theme::with_opacity(theme.foreground, opacity * anim);
        let galley = painter.layout_no_wrap(circle.label.clone(), label_font.clone(), label_color);
        let lx = center.x + label_offset_x - galley.rect.width() / 2.0;
        let ly = center.y + label_offset_y - galley.rect.height() / 2.0;
        painter.galley(Pos2::new(lx, ly), galley, label_color);
    }

    // Draw intersection labels
    for (j, inter) in intersections.iter().enumerate() {
        let step = steps.get(intersection_start + j).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        // Find the center point of the intersection (average of matching circle centers)
        let mut inter_x = 0.0;
        let mut inter_y = 0.0;
        let mut count = 0;
        for set_name in &inter.sets {
            for (i, circle) in circles.iter().enumerate() {
                if &circle.label == set_name {
                    inter_x += centers[i].x;
                    inter_y += centers[i].y;
                    count += 1;
                }
            }
        }
        if count > 0 {
            inter_x /= count as f32;
            inter_y /= count as f32;

            let label_color = Theme::with_opacity(theme.foreground, opacity * anim);
            let galley =
                painter.layout_no_wrap(inter.label.clone(), inter_font.clone(), label_color);
            let lx = inter_x - galley.rect.width() / 2.0;
            let ly = inter_y - galley.rect.height() / 2.0;
            painter.galley(Pos2::new(lx, ly), galley, label_color);
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
    fn test_parse_venn_two_circles() {
        let content = "- Frontend (size: 40)\n- Backend (size: 35)";
        let (circles, intersections) = parse_venn_diagram(content);
        assert_eq!(circles.len(), 2);
        assert_eq!(circles[0].label, "Frontend");
        assert_eq!(circles[0].size, 40.0);
        assert_eq!(circles[1].label, "Backend");
        assert_eq!(circles[1].size, 35.0);
        assert!(intersections.is_empty());
    }

    #[test]
    fn test_parse_venn_with_intersections() {
        let content =
            "- Frontend (size: 40)\n- Backend (size: 35)\n+ Frontend & Backend: Fullstack";
        let (circles, intersections) = parse_venn_diagram(content);
        assert_eq!(circles.len(), 2);
        assert_eq!(intersections.len(), 1);
        assert_eq!(intersections[0].sets, vec!["Frontend", "Backend"]);
        assert_eq!(intersections[0].label, "Fullstack");
        assert_eq!(intersections[0].reveal, VizReveal::NextStep);
    }

    #[test]
    fn test_parse_venn_three_circles() {
        let content = "- A (size: 30)\n- B (size: 25)\n- C (size: 20)\n+ A & B: AB\n+ B & C: BC";
        let (circles, intersections) = parse_venn_diagram(content);
        assert_eq!(circles.len(), 3);
        assert_eq!(intersections.len(), 2);
    }

    #[test]
    fn test_parse_venn_no_size() {
        let content = "- Frontend\n- Backend";
        let (circles, _) = parse_venn_diagram(content);
        assert_eq!(circles.len(), 2);
        assert_eq!(circles[0].size, 30.0); // default
        assert_eq!(circles[1].size, 30.0);
    }

    #[test]
    fn test_parse_venn_skips_comments() {
        let content = "# header\n- A (size: 10)\n# note\n- B (size: 20)";
        let (circles, _) = parse_venn_diagram(content);
        assert_eq!(circles.len(), 2);
    }
}
