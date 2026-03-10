use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VizReveal, assign_steps, draw_x_axis_label, draw_y_axis_label, parse_axis_label_directive,
    parse_reveal_prefix, reveal_anim_progress,
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
struct StackedSeries {
    label: String,
    values: Vec<f32>,
    reveal: VizReveal,
}

#[derive(Debug, Clone)]
struct StackedBarData {
    categories: Vec<String>,
    series: Vec<StackedSeries>,
    x_label: Option<String>,
    y_label: Option<String>,
}

fn parse_stacked_bar(content: &str) -> StackedBarData {
    let mut categories = Vec::new();
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
                .strip_prefix("# categories:")
                .or_else(|| trimmed.strip_prefix("#categories:"))
            {
                categories = rest.split(',').map(|s| s.trim().to_string()).collect();
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

        // Parse "Series Name: v1, v2, v3, ..."
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let values: Vec<f32> = text[colon_pos + 2..]
                .split(',')
                .filter_map(|s| s.trim().parse::<f32>().ok())
                .collect();
            if !values.is_empty() {
                series.push(StackedSeries {
                    label,
                    values,
                    reveal,
                });
            }
        }
    }

    StackedBarData {
        categories,
        series,
        x_label,
        y_label,
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_stacked_bar(
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
    let data = parse_stacked_bar(content);
    if data.series.is_empty() || data.categories.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    let reveals: Vec<VizReveal> = data.series.iter().map(|s| s.reveal).collect();
    let steps = assign_steps(&reveals);
    let palette = theme.edge_palette();
    let painter = ui.painter();

    let num_categories = data.categories.len();

    // Compute max stacked total across all categories
    let max_stack: f32 = (0..num_categories)
        .map(|ci| {
            data.series
                .iter()
                .map(|s| s.values.get(ci).copied().unwrap_or(0.0))
                .sum::<f32>()
        })
        .fold(0.0f32, f32::max);
    if max_stack <= 0.0 {
        return height;
    }

    // Layout
    let padding = 60.0 * scale;
    let legend_height = 40.0 * scale;
    let label_area = 40.0 * scale; // space for category labels below bars
    let y_axis_width = 50.0 * scale;
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
    let chart_left = pos.x + padding + y_axis_width + y_label_space;
    let chart_width = max_width - padding * 2.0 - y_axis_width - y_label_space;
    let chart_top = pos.y + legend_height + padding / 2.0;
    let chart_height = height - legend_height - padding - label_area - x_label_space;
    let chart_bottom = chart_top + chart_height;

    // X-axis line
    let axis_color = Theme::with_opacity(theme.foreground, opacity * 0.2);
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_bottom),
            Pos2::new(chart_left + chart_width, chart_bottom),
        ],
        Stroke::new(1.5 * scale, axis_color),
    );

    // Y-axis grid lines
    let grid_step = nice_grid_step(max_stack, 5);
    let grid_color = Theme::with_opacity(theme.foreground, opacity * 0.08);
    let grid_font = FontId::proportional(theme.body_size * 0.55 * scale);
    let mut grid_val = grid_step;
    while grid_val <= max_stack {
        let frac = grid_val / max_stack;
        let gy = chart_bottom - frac * chart_height;
        painter.line_segment(
            [
                Pos2::new(chart_left, gy),
                Pos2::new(chart_left + chart_width, gy),
            ],
            Stroke::new(0.5 * scale, grid_color),
        );
        let label = if grid_val == grid_val.floor() {
            format!("{:.0}", grid_val)
        } else {
            format!("{:.1}", grid_val)
        };
        let grid_label_color = Theme::with_opacity(theme.foreground, opacity * 0.4);
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
    }

    // Bars
    let bar_gap = 12.0 * scale;
    let total_gaps = (num_categories + 1) as f32 * bar_gap;
    let bar_width = ((chart_width - total_gaps) / num_categories as f32).max(8.0 * scale);
    let label_font = FontId::proportional(theme.body_size * 0.65 * scale);
    let value_font = FontId::proportional(theme.body_size * 0.55 * scale);

    let mut needs_repaint = false;

    // Draw category labels below bars
    let label_color = Theme::with_opacity(theme.foreground, opacity * 0.8);
    for (ci, cat_name) in data.categories.iter().enumerate() {
        let bx = chart_left + bar_gap + ci as f32 * (bar_width + bar_gap);
        let galley = painter.layout(
            cat_name.clone(),
            label_font.clone(),
            label_color,
            bar_width + bar_gap,
        );
        let lx = bx + (bar_width - galley.rect.width()) / 2.0;
        painter.galley(
            Pos2::new(lx, chart_bottom + 6.0 * scale),
            galley,
            label_color,
        );
    }

    // Draw stacked segments for each category
    for ci in 0..num_categories {
        let bx = chart_left + bar_gap + ci as f32 * (bar_width + bar_gap);
        let mut cumulative_height = 0.0f32;

        for (si, series) in data.series.iter().enumerate() {
            let step = steps.get(si).copied().unwrap_or(0);
            if step > reveal_step {
                continue;
            }

            let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
            if repaint {
                needs_repaint = true;
            }

            let val = series.values.get(ci).copied().unwrap_or(0.0);
            let full_seg_height = (val / max_stack) * chart_height;
            let seg_height = full_seg_height * anim;

            if seg_height <= 0.0 {
                cumulative_height += full_seg_height * anim;
                continue;
            }

            let color = Theme::with_opacity(palette[si % palette.len()], opacity * 0.85);
            let by = chart_bottom - cumulative_height - seg_height;

            let bar_rect =
                egui::Rect::from_min_size(Pos2::new(bx, by), egui::vec2(bar_width, seg_height));
            painter.rect_filled(bar_rect, 0.0, color);

            // Value label inside segment if tall enough
            if seg_height > 18.0 * scale && anim > 0.8 {
                let val_opacity = ((anim - 0.8) / 0.2).min(1.0);
                let val_text = if val == val.floor() {
                    format!("{:.0}", val)
                } else {
                    format!("{:.1}", val)
                };
                let val_color = Theme::with_opacity(theme.foreground, opacity * 0.7 * val_opacity);
                let val_galley = painter.layout_no_wrap(val_text, value_font.clone(), val_color);
                if val_galley.rect.width() < bar_width {
                    let vx = bx + (bar_width - val_galley.rect.width()) / 2.0;
                    let vy = by + (seg_height - val_galley.rect.height()) / 2.0;
                    painter.galley(Pos2::new(vx, vy), val_galley, val_color);
                }
            }

            cumulative_height += seg_height;
        }
    }

    // Axis labels
    let axis_label_font = FontId::proportional(theme.body_size * 0.65 * scale);
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

    // Legend at top
    let legend_font = FontId::proportional(theme.body_size * 0.65 * scale);
    let swatch_size = 18.0 * scale;
    let item_spacing = 28.0 * scale;

    let legend_items: Vec<(String, egui::Color32)> = data
        .series
        .iter()
        .enumerate()
        .filter(|(si, _)| {
            let step = steps.get(*si).copied().unwrap_or(0);
            step <= reveal_step
        })
        .map(|(si, s)| {
            let color = Theme::with_opacity(palette[si % palette.len()], opacity);
            (s.label.clone(), color)
        })
        .collect();

    if !legend_items.is_empty() {
        let mut total_w = 0.0f32;
        let galleys: Vec<_> = legend_items
            .iter()
            .map(|(name, color)| {
                let g = painter.layout_no_wrap(name.clone(), legend_font.clone(), *color);
                let w = swatch_size + 6.0 * scale + g.rect.width() + item_spacing;
                total_w += w;
                (g, *color)
            })
            .collect();
        total_w -= item_spacing;

        let legend_y = pos.y + padding / 4.0;
        let mut lx = pos.x + (max_width - total_w) / 2.0;

        for (galley, color) in galleys {
            let swatch_rect = egui::Rect::from_min_size(
                Pos2::new(lx, legend_y + (legend_height - swatch_size) / 2.0),
                egui::vec2(swatch_size, swatch_size),
            );
            painter.rect_filled(swatch_rect, 2.0 * scale, color);
            lx += swatch_size + 6.0 * scale;

            let text_y = legend_y + (legend_height - galley.rect.height()) / 2.0;
            let w = galley.rect.width();
            let text_color = Theme::with_opacity(theme.foreground, opacity);
            painter.galley(Pos2::new(lx, text_y), galley, text_color);
            lx += w + item_spacing;
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
    fn test_parse_stacked_bar_basic() {
        let content = "# categories: Q1, Q2, Q3\n- Product A: 40, 45, 50\n- Product B: 30, 35, 40";
        let data = parse_stacked_bar(content);
        assert_eq!(data.categories, vec!["Q1", "Q2", "Q3"]);
        assert_eq!(data.series.len(), 2);
        assert_eq!(data.series[0].label, "Product A");
        assert_eq!(data.series[0].values, vec![40.0, 45.0, 50.0]);
        assert_eq!(data.series[1].label, "Product B");
        assert_eq!(data.series[1].values, vec![30.0, 35.0, 40.0]);
    }

    #[test]
    fn test_parse_stacked_bar_reveal_markers() {
        let content = "# categories: A, B\n- S1: 10, 20\n+ S2: 30, 40\n* S3: 50, 60";
        let data = parse_stacked_bar(content);
        assert_eq!(data.series[0].reveal, VizReveal::Static);
        assert_eq!(data.series[1].reveal, VizReveal::NextStep);
        assert_eq!(data.series[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_stacked_bar_skips_invalid() {
        let content =
            "# categories: X, Y\n# some comment\n- Valid: 1, 2\n- no colon here\n- Also: 3, 4";
        let data = parse_stacked_bar(content);
        assert_eq!(data.categories, vec!["X", "Y"]);
        assert_eq!(data.series.len(), 2);
    }

    #[test]
    fn test_parse_stacked_bar_no_categories() {
        let content = "- Product A: 10, 20";
        let data = parse_stacked_bar(content);
        assert!(data.categories.is_empty());
        assert_eq!(data.series.len(), 1);
    }

    #[test]
    fn test_nice_grid_step() {
        assert_eq!(nice_grid_step(100.0, 5), 20.0);
        assert_eq!(nice_grid_step(50.0, 5), 10.0);
        assert_eq!(nice_grid_step(420.0, 5), 100.0);
    }
}
