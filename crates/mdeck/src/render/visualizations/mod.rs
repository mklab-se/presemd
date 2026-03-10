use std::time::Instant;

pub mod bar_chart;
pub mod pie_chart;
pub mod timeline;
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
