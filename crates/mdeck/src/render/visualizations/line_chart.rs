use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_DOT_RADIUS, VIZ_FONT_AXIS_LABEL, VIZ_FONT_GRID_LABEL, VIZ_FONT_LEGEND, VIZ_OPACITY_AXIS,
    VIZ_OPACITY_FILL, VIZ_OPACITY_GRID, VIZ_OPACITY_GRID_LABEL, VIZ_STROKE_AXIS,
    VIZ_STROKE_DATA_LINE, VIZ_STROKE_GRID, VIZ_SWATCH_SIZE, VizReveal, assign_steps,
    draw_x_axis_label, draw_y_axis_label, parse_axis_label_directive, parse_reveal_prefix,
    reveal_anim_progress,
};

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
struct LineSeries {
    label: String,
    values: Vec<f32>,
    reveal: VizReveal,
}

struct LineChartData {
    x_labels: Vec<String>,
    series: Vec<LineSeries>,
    x_label: Option<String>,
    y_label: Option<String>,
}

fn parse_line_chart(content: &str) -> LineChartData {
    let mut x_labels = Vec::new();
    let mut series = Vec::new();
    let mut x_label = None;
    let mut y_label = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse directives
        if trimmed.starts_with('#') {
            if let Some(rest) = trimmed
                .strip_prefix("# x-labels:")
                .or_else(|| trimmed.strip_prefix("#x-labels:"))
            {
                x_labels = rest.split(',').map(|s| s.trim().to_string()).collect();
            } else if let Some((key, val)) = parse_axis_label_directive(trimmed) {
                match key {
                    "x-label" => x_label = Some(val),
                    "y-label" => y_label = Some(val),
                    _ => {}
                }
            }
            continue;
        }

        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        // Parse "Label: 100, 150, 200, 280"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let values_str = &text[colon_pos + 2..];
            let values: Vec<f32> = values_str
                .split(',')
                .filter_map(|s| s.trim().parse::<f32>().ok())
                .collect();
            if !values.is_empty() {
                series.push(LineSeries {
                    label,
                    values,
                    reveal,
                });
            }
        }
    }

    LineChartData {
        x_labels,
        series,
        x_label,
        y_label,
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_line_chart(
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
    let data = parse_line_chart(content);
    let x_labels = &data.x_labels;
    let series = &data.series;
    if series.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    let reveals: Vec<VizReveal> = series.iter().map(|s| s.reveal).collect();
    let steps = assign_steps(&reveals);
    let palette = theme.edge_palette();
    let painter = ui.painter();

    // Find global max value across all series
    let max_value = series
        .iter()
        .flat_map(|s| s.values.iter())
        .copied()
        .fold(0.0f32, f32::max);
    if max_value <= 0.0 {
        return height;
    }

    // Find max number of data points
    let max_points = series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    if max_points == 0 {
        return height;
    }

    // Layout
    let padding = 60.0 * scale;
    let label_area = 50.0 * scale; // space for x-axis labels below
    let legend_width = 200.0 * scale;
    let y_axis_label_width = 60.0 * scale;
    let y_label_space = if data.y_label.is_some() {
        25.0 * scale
    } else {
        0.0
    };
    let x_label_space = if data.x_label.is_some() {
        30.0 * scale
    } else {
        0.0
    };
    let chart_left = pos.x + padding + y_axis_label_width + y_label_space;
    let chart_top = pos.y + padding;
    let chart_width = max_width - padding * 2.0 - y_axis_label_width - y_label_space - legend_width;
    let chart_height = height - padding * 2.0 - label_area - x_label_space;
    let chart_bottom = chart_top + chart_height;

    // Draw grid lines with nice numbers
    let grid_step = nice_grid_step(max_value, 5);
    let grid_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_GRID);
    let grid_font = FontId::proportional(theme.body_size * VIZ_FONT_GRID_LABEL * scale);
    let grid_label_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_GRID_LABEL);

    let mut grid_val = 0.0;
    while grid_val <= max_value + grid_step * 0.5 {
        let frac = grid_val / max_value;
        let gy = chart_bottom - frac * chart_height;
        if grid_val > 0.0 {
            painter.line_segment(
                [
                    Pos2::new(chart_left, gy),
                    Pos2::new(chart_left + chart_width, gy),
                ],
                Stroke::new(VIZ_STROKE_GRID * scale, grid_color),
            );
        }
        let label = if grid_val == grid_val.floor() {
            format!("{:.0}", grid_val)
        } else {
            format!("{:.1}", grid_val)
        };
        let galley = painter.layout_no_wrap(label, grid_font.clone(), grid_label_color);
        painter.galley(
            Pos2::new(
                chart_left - galley.rect.width() - 8.0 * scale,
                gy - galley.rect.height() / 2.0,
            ),
            galley,
            grid_label_color,
        );
        grid_val += grid_step;
        if grid_val > max_value * 1.01 && grid_val - grid_step < max_value {
            break;
        }
    }

    // Draw axes
    let axis_color = Theme::with_opacity(theme.foreground, opacity * VIZ_OPACITY_AXIS);
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_bottom),
            Pos2::new(chart_left + chart_width, chart_bottom),
        ],
        Stroke::new(VIZ_STROKE_AXIS * scale, axis_color),
    );
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_top),
            Pos2::new(chart_left, chart_bottom),
        ],
        Stroke::new(VIZ_STROKE_AXIS * scale, axis_color),
    );

    // Draw x-axis labels
    let x_label_font = FontId::proportional(theme.body_size * VIZ_FONT_GRID_LABEL * scale);
    let x_label_color = Theme::with_opacity(theme.foreground, opacity * 0.7);
    for (i, label) in x_labels.iter().enumerate().take(max_points) {
        let x = chart_left + (i as f32 / (max_points - 1).max(1) as f32) * chart_width;
        let galley = painter.layout_no_wrap(label.clone(), x_label_font.clone(), x_label_color);
        painter.galley(
            Pos2::new(x - galley.rect.width() / 2.0, chart_bottom + 8.0 * scale),
            galley,
            x_label_color,
        );
    }

    // Draw series lines
    let mut needs_repaint = false;
    let dot_radius = VIZ_DOT_RADIUS * scale;

    for (si, s) in series.iter().enumerate() {
        let step = steps.get(si).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let color = Theme::with_opacity(palette[si % palette.len()], opacity * VIZ_OPACITY_FILL);
        let n_points = s.values.len();
        if n_points == 0 {
            continue;
        }

        // Compute data point positions
        let points: Vec<Pos2> = s
            .values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let x = chart_left + (i as f32 / (max_points - 1).max(1) as f32) * chart_width;
                let y = chart_bottom - (v / max_value) * chart_height;
                Pos2::new(x, y)
            })
            .collect();

        // Clip line drawing by anim_progress * total_width
        let clip_x = chart_left + anim * chart_width;

        // Draw line segments
        for i in 0..points.len() - 1 {
            let p1 = points[i];
            let p2 = points[i + 1];

            if p1.x > clip_x {
                break;
            }

            let draw_p2 = if p2.x > clip_x {
                // Interpolate to clip boundary
                let t = (clip_x - p1.x) / (p2.x - p1.x);
                Pos2::new(clip_x, p1.y + t * (p2.y - p1.y))
            } else {
                p2
            };

            painter.line_segment(
                [p1, draw_p2],
                Stroke::new(VIZ_STROKE_DATA_LINE * scale, color),
            );
        }

        // Draw dots at data points (only those within clip range)
        for &pt in &points {
            if pt.x > clip_x + 0.5 {
                break;
            }
            painter.circle_filled(pt, dot_radius, color);
        }
    }

    // Axis labels
    let axis_label_font = FontId::proportional(theme.body_size * VIZ_FONT_AXIS_LABEL * scale);
    let axis_label_color = Theme::with_opacity(theme.foreground, opacity * 0.7);
    if let Some(ref text) = data.x_label {
        draw_x_axis_label(
            painter,
            text,
            axis_label_font.clone(),
            axis_label_color,
            chart_left,
            chart_width,
            chart_bottom + label_area + 4.0 * scale,
        );
    }
    if let Some(ref text) = data.y_label {
        draw_y_axis_label(
            painter,
            text,
            axis_label_font,
            axis_label_color,
            pos.x + padding * 0.3,
            chart_top,
            chart_height,
        );
    }

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    // Draw legend at top-right
    let legend_x = pos.x + max_width - legend_width;
    let legend_font = FontId::proportional(theme.body_size * VIZ_FONT_LEGEND * scale);
    let legend_item_height = 32.0 * scale;
    let legend_start_y = chart_top;
    let swatch_width = VIZ_SWATCH_SIZE * scale;
    let swatch_height = 3.0 * scale;

    for (si, s) in series.iter().enumerate() {
        let step = steps.get(si).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let ly = legend_start_y + si as f32 * legend_item_height;
        let color = Theme::with_opacity(palette[si % palette.len()], opacity * VIZ_OPACITY_FILL);
        let text_color = Theme::with_opacity(theme.foreground, opacity);

        // Color swatch (line style)
        let swatch_y = ly + legend_item_height / 2.0;
        painter.line_segment(
            [
                Pos2::new(legend_x, swatch_y),
                Pos2::new(legend_x + swatch_width, swatch_y),
            ],
            Stroke::new(VIZ_STROKE_DATA_LINE * scale, color),
        );
        painter.circle_filled(
            Pos2::new(legend_x + swatch_width / 2.0, swatch_y),
            dot_radius * 0.8,
            color,
        );

        // Label
        let galley = painter.layout_no_wrap(s.label.clone(), legend_font.clone(), text_color);
        painter.galley(
            Pos2::new(
                legend_x + swatch_width + 8.0 * scale,
                ly + (legend_item_height - galley.rect.height()) / 2.0,
            ),
            galley,
            text_color,
        );
    }

    let _ = swatch_height; // suppress unused warning

    height
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_chart_basic() {
        let content = "# x-labels: Q1, Q2, Q3, Q4\n- Revenue: 100, 150, 200, 280";
        let data = parse_line_chart(content);
        assert_eq!(data.x_labels, vec!["Q1", "Q2", "Q3", "Q4"]);
        assert_eq!(data.series.len(), 1);
        assert_eq!(data.series[0].label, "Revenue");
        assert_eq!(data.series[0].values, vec![100.0, 150.0, 200.0, 280.0]);
    }

    #[test]
    fn test_parse_line_chart_multiple_series() {
        let content = "# x-labels: A, B, C\n- Revenue: 100, 150, 200\n+ Costs: 80, 90, 120";
        let data = parse_line_chart(content);
        assert_eq!(data.x_labels.len(), 3);
        assert_eq!(data.series.len(), 2);
        assert_eq!(data.series[0].label, "Revenue");
        assert_eq!(data.series[0].reveal, VizReveal::Static);
        assert_eq!(data.series[1].label, "Costs");
        assert_eq!(data.series[1].reveal, VizReveal::NextStep);
    }

    #[test]
    fn test_parse_line_chart_no_labels() {
        let content = "- Sales: 10, 20, 30";
        let data = parse_line_chart(content);
        assert!(data.x_labels.is_empty());
        assert_eq!(data.series.len(), 1);
        assert_eq!(data.series[0].values, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn test_parse_line_chart_skips_invalid() {
        let content =
            "# x-labels: A, B\n- Valid: 10, 20\n- no_colon_values\n# comment\n- Also: 30, 40";
        let data = parse_line_chart(content);
        assert_eq!(data.series.len(), 2);
    }

    #[test]
    fn test_parse_line_chart_axis_labels() {
        let content =
            "# x-label: Quarter\n# y-label: Revenue ($M)\n# x-labels: Q1, Q2\n- Sales: 10, 20";
        let data = parse_line_chart(content);
        assert_eq!(data.x_label, Some("Quarter".to_string()));
        assert_eq!(data.y_label, Some("Revenue ($M)".to_string()));
        assert_eq!(data.series.len(), 1);
    }
}
