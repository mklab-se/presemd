use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Pos2, Stroke};

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Orientation {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone)]
struct BarEntry {
    label: String,
    value: f32,
    reveal: VizReveal,
}

fn parse_bar_chart(content: &str) -> (Vec<BarEntry>, Orientation) {
    let mut entries = Vec::new();
    let mut orientation = Orientation::Vertical;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse directives from comments
        if trimmed.starts_with('#') {
            if let Some(rest) = trimmed
                .strip_prefix("# orientation:")
                .or_else(|| trimmed.strip_prefix("#orientation:"))
            {
                let val = rest.trim();
                if val.eq_ignore_ascii_case("horizontal") {
                    orientation = Orientation::Horizontal;
                } else if val.eq_ignore_ascii_case("vertical") {
                    orientation = Orientation::Vertical;
                }
            }
            continue;
        }

        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        // Parse "Label: 40" or "Label: 40%"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let value_str = text[colon_pos + 2..].trim().trim_end_matches('%');
            if let Ok(value) = value_str.parse::<f32>() {
                entries.push(BarEntry {
                    label,
                    value,
                    reveal,
                });
            }
        }
    }

    (entries, orientation)
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_bar_chart(
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
    let (entries, orientation) = parse_bar_chart(content);
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

    let needs_repaint = match orientation {
        Orientation::Vertical => draw_vertical(
            painter,
            &entries,
            &steps,
            &palette,
            theme,
            pos,
            max_width,
            height,
            max_value,
            opacity,
            reveal_step,
            reveal_timestamp,
            scale,
        ),
        Orientation::Horizontal => draw_horizontal(
            painter,
            &entries,
            &steps,
            &palette,
            theme,
            pos,
            max_width,
            height,
            max_value,
            opacity,
            reveal_step,
            reveal_timestamp,
            scale,
        ),
    };

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    height
}

#[allow(clippy::too_many_arguments)]
fn draw_vertical(
    painter: &egui::Painter,
    entries: &[BarEntry],
    steps: &[usize],
    palette: &[Color32],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    height: f32,
    max_value: f32,
    opacity: f32,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) -> bool {
    let mut needs_repaint = false;
    let n = entries.len();
    let padding = 60.0 * scale;
    let label_area = 40.0 * scale; // space for labels below bars
    let value_area = 30.0 * scale; // space for value labels above bars
    let chart_height = height - padding - label_area - value_area;
    let chart_bottom = pos.y + padding + value_area + chart_height;
    let chart_left = pos.x + padding;
    let chart_width = max_width - padding * 2.0;

    // Axis line
    let axis_color = Theme::with_opacity(theme.foreground, opacity * 0.2);
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_bottom),
            Pos2::new(chart_left + chart_width, chart_bottom),
        ],
        Stroke::new(1.5 * scale, axis_color),
    );

    // Grid lines with nice round numbers
    let grid_step = nice_grid_step(max_value, 5);
    let grid_color = Theme::with_opacity(theme.foreground, opacity * 0.08);
    let grid_font = FontId::proportional(theme.body_size * 0.4 * scale);
    let mut grid_val = grid_step;
    while grid_val <= max_value {
        let frac = grid_val / max_value;
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
    let total_gaps = (n + 1) as f32 * bar_gap;
    let bar_width = ((chart_width - total_gaps) / n as f32).max(8.0 * scale);
    let label_font = FontId::proportional(theme.body_size * 0.6 * scale);
    let value_font = FontId::proportional(theme.body_size * 0.55 * scale);

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let color = Theme::with_opacity(palette[i % palette.len()], opacity * 0.85);
        let full_bar_height = (entry.value / max_value) * chart_height;
        let bar_height = full_bar_height * anim;
        let bx = chart_left + bar_gap + i as f32 * (bar_width + bar_gap);
        let by = chart_bottom - bar_height;

        // Bar with rounded corners
        let bar_rect =
            egui::Rect::from_min_size(Pos2::new(bx, by), egui::vec2(bar_width, bar_height));
        painter.rect_filled(bar_rect, 4.0 * scale, color);

        // Value label above bar (only show when animation is near-complete)
        if anim > 0.9 {
            let val_text = if entry.value == entry.value.floor() {
                format!("{:.0}", entry.value)
            } else {
                format!("{:.1}", entry.value)
            };
            let val_opacity = ((anim - 0.9) / 0.1).min(1.0); // fade in during last 10%
            let val_color = Theme::with_opacity(theme.foreground, opacity * 0.7 * val_opacity);
            let val_galley = painter.layout_no_wrap(val_text, value_font.clone(), val_color);
            let val_x = bx + (bar_width - val_galley.rect.width()) / 2.0;
            painter.galley(
                Pos2::new(val_x, by - val_galley.rect.height() - 4.0 * scale),
                val_galley,
                val_color,
            );
        }

        // Category label below bar
        let label_color = Theme::with_opacity(theme.foreground, opacity * 0.8);
        let galley = painter.layout(
            entry.label.clone(),
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

    needs_repaint
}

#[allow(clippy::too_many_arguments)]
fn draw_horizontal(
    painter: &egui::Painter,
    entries: &[BarEntry],
    steps: &[usize],
    palette: &[Color32],
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    height: f32,
    max_value: f32,
    opacity: f32,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) -> bool {
    let mut needs_repaint = false;
    let n = entries.len();
    let padding = 40.0 * scale;
    let label_area = 140.0 * scale; // space for labels on the left
    let value_area = 60.0 * scale; // space for value labels on the right
    let chart_left = pos.x + padding + label_area;
    let chart_width = max_width - padding * 2.0 - label_area - value_area;
    let chart_top = pos.y + padding;
    let chart_height = height - padding * 2.0;

    // Axis line (vertical)
    let axis_color = Theme::with_opacity(theme.foreground, opacity * 0.2);
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_top),
            Pos2::new(chart_left, chart_top + chart_height),
        ],
        Stroke::new(1.5 * scale, axis_color),
    );

    // Bars
    let bar_gap = 10.0 * scale;
    let total_gaps = (n + 1) as f32 * bar_gap;
    let bar_height = ((chart_height - total_gaps) / n as f32).max(8.0 * scale);
    let label_font = FontId::proportional(theme.body_size * 0.6 * scale);
    let value_font = FontId::proportional(theme.body_size * 0.55 * scale);

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let color = Theme::with_opacity(palette[i % palette.len()], opacity * 0.85);
        let full_bar_w = (entry.value / max_value) * chart_width;
        let bar_w = full_bar_w * anim;
        let by = chart_top + bar_gap + i as f32 * (bar_height + bar_gap);

        // Bar with rounded corners
        let bar_rect =
            egui::Rect::from_min_size(Pos2::new(chart_left, by), egui::vec2(bar_w, bar_height));
        painter.rect_filled(bar_rect, 4.0 * scale, color);

        // Category label on the left
        let label_color = Theme::with_opacity(theme.foreground, opacity * 0.8);
        let galley = painter.layout_no_wrap(entry.label.clone(), label_font.clone(), label_color);
        let lx = chart_left - galley.rect.width() - 10.0 * scale;
        let ly = by + (bar_height - galley.rect.height()) / 2.0;
        painter.galley(Pos2::new(lx, ly), galley, label_color);

        // Value label to the right of bar (fade in near end of animation)
        if anim > 0.9 {
            let val_text = if entry.value == entry.value.floor() {
                format!("{:.0}", entry.value)
            } else {
                format!("{:.1}", entry.value)
            };
            let val_opacity = ((anim - 0.9) / 0.1).min(1.0);
            let val_color = Theme::with_opacity(theme.foreground, opacity * 0.7 * val_opacity);
            let val_galley = painter.layout_no_wrap(val_text, value_font.clone(), val_color);
            let vx = chart_left + bar_w + 8.0 * scale;
            let vy = by + (bar_height - val_galley.rect.height()) / 2.0;
            painter.galley(Pos2::new(vx, vy), val_galley, val_color);
        }
    }

    needs_repaint
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bar_chart_basic() {
        let content = "- Sales: 40\n- Costs: 25";
        let (entries, orientation) = parse_bar_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Sales");
        assert_eq!(entries[0].value, 40.0);
        assert_eq!(orientation, Orientation::Vertical);
    }

    #[test]
    fn test_parse_bar_chart_horizontal() {
        let content = "# orientation: horizontal\n- A: 10\n- B: 20";
        let (entries, orientation) = parse_bar_chart(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(orientation, Orientation::Horizontal);
    }

    #[test]
    fn test_parse_bar_chart_percentage_suffix() {
        let content = "- A: 40%\n- B: 60%";
        let (entries, _) = parse_bar_chart(content);
        assert_eq!(entries[0].value, 40.0);
        assert_eq!(entries[1].value, 60.0);
    }

    #[test]
    fn test_parse_bar_chart_reveal_markers() {
        let content = "- A: 10\n+ B: 20\n* C: 30";
        let (entries, _) = parse_bar_chart(content);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_bar_chart_skips_invalid() {
        let content = "- Valid: 50\n- no_value\n# comment\n- Also: 30";
        let (entries, _) = parse_bar_chart(content);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_parse_bar_chart_decimal_values() {
        let content = "- A: 3.14\n- B: 2.71";
        let (entries, _) = parse_bar_chart(content);
        assert!((entries[0].value - 3.14).abs() < 0.001);
        assert!((entries[1].value - 2.71).abs() < 0.001);
    }

    #[test]
    fn test_nice_grid_step() {
        assert_eq!(nice_grid_step(100.0, 5), 20.0);
        assert_eq!(nice_grid_step(65.0, 5), 20.0);
        assert_eq!(nice_grid_step(95.0, 5), 20.0);
        assert_eq!(nice_grid_step(50.0, 5), 10.0);
        assert_eq!(nice_grid_step(420.0, 5), 100.0);
        assert_eq!(nice_grid_step(10.0, 5), 2.0);
    }
}
