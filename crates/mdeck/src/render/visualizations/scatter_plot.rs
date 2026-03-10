use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};
use eframe::epaint::TextShape;

use crate::theme::Theme;

use super::{VizReveal, assign_steps, parse_reveal_prefix, reveal_anim_progress};

// ─── Utilities ──────────────────────────────────────────────────────────────

/// Compute a "nice" grid step for axis labels (1, 2, 5, 10, 20, 25, 50, 100, ...).
fn nice_grid_step(max_value: f32, target_lines: u32) -> f32 {
    let rough = max_value / target_lines as f32;
    let magnitude = 10.0f32.powf(rough.log10().floor());
    let residual = rough / magnitude;
    let nice = if residual <= 1.0 {
        1.0
    } else if residual <= 2.0 {
        2.0
    } else if residual <= 5.0 {
        5.0
    } else {
        10.0
    };
    (nice * magnitude).max(1.0)
}

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ScatterPoint {
    label: String,
    x: f32,
    y: f32,
    size: Option<f32>,
    reveal: VizReveal,
}

struct ScatterData {
    points: Vec<ScatterPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
}

fn parse_scatter_plot(content: &str) -> ScatterData {
    let mut points = Vec::new();
    let mut x_label = None;
    let mut y_label = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('#') {
            if let Some(rest) = trimmed
                .strip_prefix("# x-label:")
                .or_else(|| trimmed.strip_prefix("#x-label:"))
            {
                x_label = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed
                .strip_prefix("# y-label:")
                .or_else(|| trimmed.strip_prefix("#y-label:"))
            {
                y_label = Some(rest.trim().to_string());
            }
            continue;
        }
        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        // Parse "Label: X, Y" or "Label: X, Y (size: N)"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let rest = text[colon_pos + 2..].trim();

            // Extract optional (size: N) suffix
            let (coords_str, size) = if let Some(paren_start) = rest.find('(') {
                let coords = rest[..paren_start].trim().trim_end_matches(',').trim();
                let inner = rest[paren_start..]
                    .trim_start_matches('(')
                    .trim_end_matches(')');
                let sz = if let Some(s) = inner.strip_prefix("size:") {
                    s.trim().parse::<f32>().ok()
                } else {
                    None
                };
                (coords, sz)
            } else {
                (rest, None)
            };

            // Parse "X, Y"
            let parts: Vec<&str> = coords_str.split(',').collect();
            if parts.len() == 2 {
                if let (Ok(x), Ok(y)) = (
                    parts[0].trim().parse::<f32>(),
                    parts[1].trim().parse::<f32>(),
                ) {
                    points.push(ScatterPoint {
                        label,
                        x,
                        y,
                        size,
                        reveal,
                    });
                }
            }
        }
    }
    ScatterData {
        points,
        x_label,
        y_label,
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_scatter_plot(
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
    let data = parse_scatter_plot(content);
    let points = &data.points;
    if points.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    let reveals: Vec<VizReveal> = points.iter().map(|p| p.reveal).collect();
    let steps = assign_steps(&reveals);
    let palette = theme.edge_palette();
    let painter = ui.painter();

    // Compute data bounds
    let x_min = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let x_max = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let y_min = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let y_max = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);

    // Add some padding to data range
    let x_range = (x_max - x_min).max(1.0);
    let y_range = (y_max - y_min).max(1.0);
    let data_x_min = x_min - x_range * 0.1;
    let data_x_max = x_max + x_range * 0.1;
    let data_y_min = y_min - y_range * 0.1;
    let data_y_max = y_max + y_range * 0.1;

    // Chart area
    let padding = 60.0 * scale;
    let axis_label_space = 40.0 * scale;
    let chart_left = pos.x + padding + axis_label_space;
    let chart_right = pos.x + max_width - padding;
    let chart_top = pos.y + padding;
    let chart_bottom = pos.y + height - padding - axis_label_space;
    let chart_width = chart_right - chart_left;
    let chart_height = chart_bottom - chart_top;

    let axis_color = Theme::with_opacity(theme.foreground, opacity * 0.2);
    let grid_color = Theme::with_opacity(theme.foreground, opacity * 0.08);
    let grid_font = FontId::proportional(theme.body_size * 0.55 * scale);
    let label_font = FontId::proportional(theme.body_size * 0.6 * scale);

    // Draw axes
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_bottom),
            Pos2::new(chart_right, chart_bottom),
        ],
        Stroke::new(1.5 * scale, axis_color),
    );
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_top),
            Pos2::new(chart_left, chart_bottom),
        ],
        Stroke::new(1.5 * scale, axis_color),
    );

    // X-axis grid lines
    let x_step = nice_grid_step(data_x_max - data_x_min, 5);
    let grid_label_color = Theme::with_opacity(theme.foreground, opacity * 0.4);
    let mut gx = (data_x_min / x_step).ceil() * x_step;
    while gx <= data_x_max {
        let frac = (gx - data_x_min) / (data_x_max - data_x_min);
        let px = chart_left + frac * chart_width;
        painter.line_segment(
            [Pos2::new(px, chart_top), Pos2::new(px, chart_bottom)],
            Stroke::new(0.5 * scale, grid_color),
        );
        let label = if gx == gx.floor() {
            format!("{:.0}", gx)
        } else {
            format!("{:.1}", gx)
        };
        let galley = painter.layout_no_wrap(label, grid_font.clone(), grid_label_color);
        painter.galley(
            Pos2::new(px - galley.rect.width() / 2.0, chart_bottom + 6.0 * scale),
            galley,
            grid_label_color,
        );
        gx += x_step;
    }

    // Y-axis grid lines
    let y_step = nice_grid_step(data_y_max - data_y_min, 5);
    let mut gy = (data_y_min / y_step).ceil() * y_step;
    while gy <= data_y_max {
        let frac = (gy - data_y_min) / (data_y_max - data_y_min);
        let py = chart_bottom - frac * chart_height;
        painter.line_segment(
            [Pos2::new(chart_left, py), Pos2::new(chart_right, py)],
            Stroke::new(0.5 * scale, grid_color),
        );
        let label = if gy == gy.floor() {
            format!("{:.0}", gy)
        } else {
            format!("{:.1}", gy)
        };
        let galley = painter.layout_no_wrap(label, grid_font.clone(), grid_label_color);
        painter.galley(
            Pos2::new(
                chart_left - galley.rect.width() - 8.0 * scale,
                py - galley.rect.height() / 2.0,
            ),
            galley,
            grid_label_color,
        );
        gy += y_step;
    }

    // Draw axis labels
    let axis_label_font = FontId::proportional(theme.body_size * 0.65 * scale);
    let axis_label_color = Theme::with_opacity(theme.foreground, opacity * 0.7);

    if let Some(ref x_label_text) = data.x_label {
        let galley = painter.layout_no_wrap(
            x_label_text.clone(),
            axis_label_font.clone(),
            axis_label_color,
        );
        let lx = chart_left + (chart_width - galley.rect.width()) / 2.0;
        let ly = chart_bottom + 28.0 * scale;
        painter.galley(Pos2::new(lx, ly), galley, axis_label_color);
    }
    if let Some(ref y_label_text) = data.y_label {
        let galley = painter.layout_no_wrap(
            y_label_text.clone(),
            axis_label_font.clone(),
            axis_label_color,
        );
        let text_width = galley.rect.width();
        // Rotated 90° CCW, centered vertically along the chart axis
        let anchor_x = pos.x + padding * 0.3;
        let anchor_y = chart_top + (chart_height + text_width) / 2.0;
        let text_shape = TextShape::new(Pos2::new(anchor_x, anchor_y), galley, axis_label_color)
            .with_angle(-std::f32::consts::FRAC_PI_2);
        painter.add(text_shape);
    }

    // Draw data points
    let mut needs_repaint = false;
    let default_radius = 8.0 * scale;

    for (i, point) in points.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let fx = (point.x - data_x_min) / (data_x_max - data_x_min);
        let fy = (point.y - data_y_min) / (data_y_max - data_y_min);
        let px = chart_left + fx * chart_width;
        let py = chart_bottom - fy * chart_height;

        let radius = point.size.map_or(default_radius, |s| s * scale * 0.5) * anim;
        let color = Theme::with_opacity(palette[i % palette.len()], opacity * 0.85);

        painter.circle_filled(Pos2::new(px, py), radius, color);

        // Label near the dot
        if anim > 0.5 {
            let label_opacity = ((anim - 0.5) / 0.5).min(1.0);
            let label_color = Theme::with_opacity(theme.foreground, opacity * 0.8 * label_opacity);
            let galley =
                painter.layout_no_wrap(point.label.clone(), label_font.clone(), label_color);
            painter.galley(
                Pos2::new(px + radius + 4.0 * scale, py - galley.rect.height() / 2.0),
                galley,
                label_color,
            );
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
    fn test_parse_scatter_basic() {
        let content = "- Alice: 80, 90\n- Bob: 65, 75";
        let data = parse_scatter_plot(content);
        assert_eq!(data.points.len(), 2);
        assert_eq!(data.points[0].label, "Alice");
        assert_eq!(data.points[0].x, 80.0);
        assert_eq!(data.points[0].y, 90.0);
        assert!(data.points[0].size.is_none());
    }

    #[test]
    fn test_parse_scatter_with_size() {
        let content = "- Dave: 40, 60 (size: 30)";
        let data = parse_scatter_plot(content);
        assert_eq!(data.points.len(), 1);
        assert_eq!(data.points[0].label, "Dave");
        assert_eq!(data.points[0].x, 40.0);
        assert_eq!(data.points[0].y, 60.0);
        assert_eq!(data.points[0].size, Some(30.0));
    }

    #[test]
    fn test_parse_scatter_reveal_markers() {
        let content = "- A: 10, 20\n+ B: 30, 40\n* C: 50, 60";
        let data = parse_scatter_plot(content);
        assert_eq!(data.points[0].reveal, VizReveal::Static);
        assert_eq!(data.points[1].reveal, VizReveal::NextStep);
        assert_eq!(data.points[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_scatter_skips_invalid() {
        let content = "- Valid: 10, 20\n- Bad: only_one\n# comment\n- Also: 30, 40";
        let data = parse_scatter_plot(content);
        assert_eq!(data.points.len(), 2);
    }

    #[test]
    fn test_parse_scatter_axis_labels() {
        let content = "# x-label: Hours Studied\n# y-label: Test Score\n- Alice: 80, 90";
        let data = parse_scatter_plot(content);
        assert_eq!(data.x_label, Some("Hours Studied".to_string()));
        assert_eq!(data.y_label, Some("Test Score".to_string()));
        assert_eq!(data.points.len(), 1);
    }

    #[test]
    fn test_nice_grid_step_scatter() {
        assert_eq!(nice_grid_step(100.0, 5), 20.0);
        assert_eq!(nice_grid_step(50.0, 5), 10.0);
    }
}
