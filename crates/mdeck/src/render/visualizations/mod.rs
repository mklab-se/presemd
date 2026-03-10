use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Pos2};
use eframe::epaint::TextShape;

pub mod bar_chart;
pub mod donut_chart;
pub mod funnel_chart;
pub mod gantt_chart;
pub mod kpi_cards;
pub mod line_chart;
pub mod org_chart;
pub mod pie_chart;
pub mod progress_bars;
pub mod radar_chart;
pub mod scatter_plot;
pub mod stacked_bar;
pub mod timeline;
pub mod venn_diagram;
pub mod word_cloud;

const REVEAL_ANIMATION_DURATION: f32 = 0.4; // seconds

/// Compute eased animation progress (0.0→1.0) for an element revealed at `item_step`.
/// Returns `(progress, needs_repaint)`.
pub fn reveal_anim_progress(
    item_step: usize,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
) -> (f32, bool) {
    // Only animate items that just appeared on the current step
    if item_step == reveal_step && item_step > 0 {
        if let Some(ts) = reveal_timestamp {
            let elapsed = ts.elapsed().as_secs_f32();
            let t = (elapsed / REVEAL_ANIMATION_DURATION).min(1.0);
            // Ease-in-out quadratic
            let eased = if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0_f32 * t + 2.0).powi(2) / 2.0
            };
            return (eased, t < 1.0);
        }
    }
    (1.0, false)
}

/// Reveal marker for visualization elements (mirrors diagram semantics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VizReveal {
    /// Always visible (prefix `-` or no prefix).
    Static,
    /// Appears on the next reveal step (prefix `+`).
    NextStep,
    /// Appears together with the previous `+` element (prefix `*`).
    WithPrev,
}

/// Parse a line's reveal prefix, returning the trimmed content and its reveal marker.
pub fn parse_reveal_prefix(line: &str) -> (&str, VizReveal) {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("+ ") {
        (rest, VizReveal::NextStep)
    } else if let Some(rest) = trimmed.strip_prefix("* ") {
        (rest, VizReveal::WithPrev)
    } else if let Some(rest) = trimmed.strip_prefix("- ") {
        (rest, VizReveal::Static)
    } else {
        (trimmed, VizReveal::Static)
    }
}

/// Count the number of `+` (NextStep) markers in a visualization content string.
pub fn count_viz_steps(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.starts_with("+ ")
        })
        .count()
}

/// Assign reveal step numbers to items based on their reveal markers.
/// Returns a Vec of step numbers (0 = always visible).
pub fn assign_steps(reveals: &[VizReveal]) -> Vec<usize> {
    let mut step_counter = 0usize;
    reveals
        .iter()
        .map(|r| match r {
            VizReveal::Static => 0,
            VizReveal::NextStep => {
                step_counter += 1;
                step_counter
            }
            VizReveal::WithPrev => step_counter,
        })
        .collect()
}

/// Draw a horizontal axis label centered below the chart area.
pub fn draw_x_axis_label(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    chart_left: f32,
    chart_width: f32,
    y: f32,
) {
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let lx = chart_left + (chart_width - galley.rect.width()) / 2.0;
    painter.galley(Pos2::new(lx, y), galley, color);
}

/// Draw a vertical axis label rotated 90° CCW, centered along the chart's Y axis.
pub fn draw_y_axis_label(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    x: f32,
    chart_top: f32,
    chart_height: f32,
) {
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let text_width = galley.rect.width();
    // Place anchor so that the rotated text is vertically centered
    // After -90° rotation around anchor, text extends upward from anchor
    let anchor_x = x;
    let anchor_y = chart_top + (chart_height + text_width) / 2.0;
    let text_shape = TextShape::new(Pos2::new(anchor_x, anchor_y), galley, color)
        .with_angle(-std::f32::consts::FRAC_PI_2);
    painter.add(text_shape);
}

/// Parse an axis label directive from a comment line.
/// Returns Some((key, value)) for lines like "# x-label: Foo" or "# y-label: Bar".
pub fn parse_axis_label_directive(trimmed: &str) -> Option<(&str, String)> {
    for key in &["x-label", "y-label"] {
        let prefixed = format!("# {key}:");
        let compact = format!("#{key}:");
        if let Some(rest) = trimmed.strip_prefix(prefixed.as_str()) {
            return Some((key, rest.trim().to_string()));
        }
        if let Some(rest) = trimmed.strip_prefix(compact.as_str()) {
            return Some((key, rest.trim().to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_reveal_prefix() {
        assert_eq!(parse_reveal_prefix("- foo"), ("foo", VizReveal::Static));
        assert_eq!(parse_reveal_prefix("+ bar"), ("bar", VizReveal::NextStep));
        assert_eq!(parse_reveal_prefix("* baz"), ("baz", VizReveal::WithPrev));
        assert_eq!(parse_reveal_prefix("plain"), ("plain", VizReveal::Static));
    }

    #[test]
    fn test_count_viz_steps() {
        let content = "- A\n+ B\n+ C\n* D";
        assert_eq!(count_viz_steps(content), 2);
    }

    #[test]
    fn test_count_viz_steps_skips_comments() {
        let content = "# comment\n+ A\n# another\n+ B";
        assert_eq!(count_viz_steps(content), 2);
    }

    #[test]
    fn test_assign_steps() {
        let reveals = vec![
            VizReveal::Static,
            VizReveal::NextStep,
            VizReveal::NextStep,
            VizReveal::WithPrev,
            VizReveal::NextStep,
        ];
        assert_eq!(assign_steps(&reveals), vec![0, 1, 2, 2, 3]);
    }
}
