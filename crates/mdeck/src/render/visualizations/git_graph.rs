use eframe::egui::{self, FontId, Pos2, Stroke};
use eframe::epaint::CubicBezierShape;

use crate::theme::Theme;

use super::{
    VIZ_FONT_PRIMARY_LABEL, VIZ_FONT_SECONDARY_LABEL, VizReveal, assign_steps, parse_reveal_prefix,
};

// ─── Data model ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum GitGraphItem {
    Branch {
        name: String,
        from: Option<String>,
        reveal: VizReveal,
    },
    Commit {
        branch: String,
        message: String,
        reveal: VizReveal,
    },
    Merge {
        source: String,
        target: String,
        label: String,
        reveal: VizReveal,
    },
}

// ─── Parsing ────────────────────────────────────────────────────────────────

fn parse_gitgraph(content: &str) -> Vec<GitGraphItem> {
    let mut items = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        let lower = text.to_lowercase();

        if lower.starts_with("branch ") {
            let rest = &text["branch ".len()..];
            if let Some(from_idx) = rest.to_lowercase().find(" from ") {
                let name = rest[..from_idx].trim().to_string();
                let from = rest[from_idx + " from ".len()..].trim().to_string();
                items.push(GitGraphItem::Branch {
                    name,
                    from: Some(from),
                    reveal,
                });
            } else {
                items.push(GitGraphItem::Branch {
                    name: rest.trim().to_string(),
                    from: None,
                    reveal,
                });
            }
        } else if lower.starts_with("commit ") {
            let rest = &text["commit ".len()..];
            if let Some(colon) = rest.find(": ") {
                let branch = rest[..colon].trim().to_string();
                let message = rest[colon + 2..].trim().trim_matches('"').to_string();
                items.push(GitGraphItem::Commit {
                    branch,
                    message,
                    reveal,
                });
            }
        } else if lower.starts_with("merge ") {
            let rest = &text["merge ".len()..];
            // Parse: source -> target: "label"
            if let Some(arrow) = rest.find(" -> ") {
                let source = rest[..arrow].trim().to_string();
                let after_arrow = &rest[arrow + " -> ".len()..];
                let (target, label) = if let Some(colon) = after_arrow.find(": ") {
                    (
                        after_arrow[..colon].trim().to_string(),
                        after_arrow[colon + 2..]
                            .trim()
                            .trim_matches('"')
                            .to_string(),
                    )
                } else {
                    (after_arrow.trim().to_string(), String::new())
                };
                items.push(GitGraphItem::Merge {
                    source,
                    target,
                    label,
                    reveal,
                });
            }
        }
    }
    items
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_gitgraph(
    ui: &egui::Ui,
    content: &str,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    reveal_step: usize,
    scale: f32,
) -> f32 {
    let items = parse_gitgraph(content);
    if items.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    // Assign reveal steps
    let reveals: Vec<VizReveal> = items
        .iter()
        .map(|item| match item {
            GitGraphItem::Branch { reveal, .. }
            | GitGraphItem::Commit { reveal, .. }
            | GitGraphItem::Merge { reveal, .. } => *reveal,
        })
        .collect();
    let steps = assign_steps(&reveals);

    let palette = theme.edge_palette();
    let painter = ui.painter();

    // Build branch order (order of first appearance) and assign colors
    let mut branch_order: Vec<String> = Vec::new();
    for item in &items {
        let name = match item {
            GitGraphItem::Branch { name, .. } => name.clone(),
            GitGraphItem::Commit { branch, .. } => branch.clone(),
            GitGraphItem::Merge { source, target, .. } => {
                // Ensure both branches exist in order
                if !branch_order.contains(source) {
                    branch_order.push(source.clone());
                }
                if !branch_order.contains(target) {
                    branch_order.push(target.clone());
                }
                continue;
            }
        };
        if !branch_order.contains(&name) {
            branch_order.push(name);
        }
    }

    let num_branches = branch_order.len().max(1);

    // Layout dimensions
    let label_margin = 130.0 * scale;
    let right_margin = 40.0 * scale;
    let top_margin = 30.0 * scale;
    let bottom_margin = 30.0 * scale;
    let usable_width = max_width - label_margin - right_margin;
    let usable_height = height - top_margin - bottom_margin;
    let lane_spacing = if num_branches > 1 {
        usable_height / (num_branches - 1) as f32
    } else {
        0.0
    };

    // Y position for each branch lane
    let branch_y = |name: &str| -> f32 {
        let idx = branch_order.iter().position(|b| b == name).unwrap_or(0);
        if num_branches == 1 {
            pos.y + top_margin + usable_height / 2.0
        } else {
            pos.y + top_margin + idx as f32 * lane_spacing
        }
    };

    // Color for each branch
    let branch_color = |name: &str, op: f32| -> egui::Color32 {
        let idx = branch_order.iter().position(|b| b == name).unwrap_or(0);
        Theme::with_opacity(palette[idx % palette.len()], op)
    };

    let total_events = items.len().max(1);
    let event_spacing = usable_width / total_events as f32;

    // Compute X position for each item
    let item_x = |idx: usize| -> f32 { pos.x + label_margin + event_spacing * (idx as f32 + 0.5) };

    // Track where each branch starts and ends (X range) for drawing lane lines
    let mut branch_start_x: std::collections::HashMap<String, f32> =
        std::collections::HashMap::new();
    let mut branch_end_x: std::collections::HashMap<String, f32> = std::collections::HashMap::new();

    // First pass: determine branch extents
    for (i, item) in items.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }
        let x = item_x(i);
        match item {
            GitGraphItem::Branch { name, .. } => {
                branch_start_x.entry(name.clone()).or_insert(x);
                branch_end_x
                    .entry(name.clone())
                    .and_modify(|e| *e = e.max(x))
                    .or_insert(x);
            }
            GitGraphItem::Commit { branch, .. } => {
                branch_start_x.entry(branch.clone()).or_insert(x);
                branch_end_x
                    .entry(branch.clone())
                    .and_modify(|e| *e = e.max(x))
                    .or_insert(x);
            }
            GitGraphItem::Merge { source, target, .. } => {
                // Source branch ends here, target branch continues
                branch_end_x
                    .entry(source.clone())
                    .and_modify(|e| *e = e.max(x))
                    .or_insert(x);
                branch_end_x
                    .entry(target.clone())
                    .and_modify(|e| *e = e.max(x))
                    .or_insert(x);
            }
        }
    }

    // Thick lines like the reference image
    let line_width = 3.5 * scale;
    let curve_width = 3.0 * scale;
    let dot_radius = 10.0 * scale;
    let arrow_size = 8.0 * scale;

    // Collect commit positions per branch for drawing arrows between them
    let mut branch_events: std::collections::HashMap<String, Vec<f32>> =
        std::collections::HashMap::new();

    // First pass: collect all event X positions per branch
    for (i, item) in items.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }
        let x = item_x(i);
        match item {
            GitGraphItem::Branch { name, .. } => {
                branch_events.entry(name.clone()).or_default().push(x);
            }
            GitGraphItem::Commit { branch, .. } => {
                branch_events.entry(branch.clone()).or_default().push(x);
            }
            GitGraphItem::Merge { target, .. } => {
                branch_events.entry(target.clone()).or_default().push(x);
            }
        }
    }

    // Draw arrows between consecutive commits on each branch
    for branch in &branch_order {
        let Some(positions) = branch_events.get(branch) else {
            continue;
        };
        if positions.len() < 2 {
            continue;
        }
        let y = branch_y(branch);
        let color = branch_color(branch, opacity);

        for pair in positions.windows(2) {
            let x1 = pair[0] + dot_radius;
            let x2 = pair[1] - dot_radius;
            if x2 > x1 + arrow_size {
                // Line segment
                painter.line_segment(
                    [Pos2::new(x1, y), Pos2::new(x2, y)],
                    Stroke::new(line_width, color),
                );
                // Arrowhead
                draw_arrowhead(painter, Pos2::new(x2, y), arrow_size, 0.0, color);
            } else if x2 > x1 {
                painter.line_segment(
                    [Pos2::new(x1, y), Pos2::new(x2, y)],
                    Stroke::new(line_width, color),
                );
            }
        }
    }

    let label_font = FontId::proportional(theme.body_size * VIZ_FONT_PRIMARY_LABEL * scale);
    let msg_font = FontId::proportional(theme.body_size * VIZ_FONT_SECONDARY_LABEL * scale);

    // Draw branch labels near the first event on each branch
    for branch in &branch_order {
        let Some(positions) = branch_events.get(branch) else {
            continue;
        };
        let y = branch_y(branch);
        let bcolor = branch_color(branch, opacity);
        let galley = painter.layout_no_wrap(branch.clone(), label_font.clone(), bcolor);
        // Position label to the left of the first dot, or above/below if it's a child branch
        let first_x = positions[0];
        let text_x = first_x - galley.rect.width() - dot_radius - 8.0 * scale;
        let text_y = y - galley.rect.height() / 2.0;
        painter.galley(Pos2::new(text_x, text_y), galley, bcolor);
    }

    // Draw events (dots, fork curves, merge curves, labels)
    for (i, item) in items.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }
        let x = item_x(i);

        match item {
            GitGraphItem::Branch { name, from, .. } => {
                let y = branch_y(name);
                let color = branch_color(name, opacity);

                // Draw fork S-curve from parent branch
                if let Some(parent) = from {
                    let parent_y = branch_y(parent);
                    let parent_color = branch_color(name, opacity * 0.8);
                    // S-curve: horizontal out from parent, then vertical, then horizontal into child
                    let mid_x = x - event_spacing * 0.35;
                    let bezier = CubicBezierShape::from_points_stroke(
                        [
                            Pos2::new(x - event_spacing * 0.5, parent_y),
                            Pos2::new(mid_x, parent_y),
                            Pos2::new(mid_x, y),
                            Pos2::new(x, y),
                        ],
                        false,
                        egui::Color32::TRANSPARENT,
                        Stroke::new(curve_width, parent_color),
                    );
                    painter.add(bezier);
                }

                // Commit dot with ring
                painter.circle_filled(Pos2::new(x, y), dot_radius, color);
                let ring_color = Theme::with_opacity(color, opacity * 0.3);
                painter.circle_stroke(
                    Pos2::new(x, y),
                    dot_radius + 2.0 * scale,
                    Stroke::new(1.5 * scale, ring_color),
                );
            }
            GitGraphItem::Commit {
                branch, message, ..
            } => {
                let y = branch_y(branch);
                let color = branch_color(branch, opacity);

                // Commit dot with ring
                painter.circle_filled(Pos2::new(x, y), dot_radius, color);
                let ring_color = Theme::with_opacity(color, opacity * 0.3);
                painter.circle_stroke(
                    Pos2::new(x, y),
                    dot_radius + 2.0 * scale,
                    Stroke::new(1.5 * scale, ring_color),
                );

                // Commit message label
                if !message.is_empty() {
                    let msg_color = Theme::with_opacity(theme.foreground, opacity * 0.75);
                    let galley =
                        painter.layout_no_wrap(message.clone(), msg_font.clone(), msg_color);
                    let text_x = x - galley.rect.width() / 2.0;
                    let text_y = y - dot_radius - galley.rect.height() - 6.0 * scale;
                    painter.galley(Pos2::new(text_x, text_y), galley, msg_color);
                }
            }
            GitGraphItem::Merge {
                source,
                target,
                label,
                ..
            } => {
                let source_y = branch_y(source);
                let target_y = branch_y(target);
                let merge_color = branch_color(source, opacity * 0.8);

                // Dot on target branch at merge point
                let target_color = branch_color(target, opacity);
                painter.circle_filled(Pos2::new(x, target_y), dot_radius, target_color);
                let ring_color = Theme::with_opacity(target_color, opacity * 0.3);
                painter.circle_stroke(
                    Pos2::new(x, target_y),
                    dot_radius + 2.0 * scale,
                    Stroke::new(1.5 * scale, ring_color),
                );

                // S-curve merge line from source to target
                let mid_x = x - event_spacing * 0.25;
                let bezier = CubicBezierShape::from_points_stroke(
                    [
                        Pos2::new(x - event_spacing * 0.4, source_y),
                        Pos2::new(mid_x, source_y),
                        Pos2::new(mid_x, target_y),
                        Pos2::new(x - dot_radius, target_y),
                    ],
                    false,
                    egui::Color32::TRANSPARENT,
                    Stroke::new(curve_width, merge_color),
                );
                painter.add(bezier);

                // Merge label
                if !label.is_empty() {
                    let mid_y = (source_y + target_y) / 2.0;
                    let lbl_color = Theme::with_opacity(theme.foreground, opacity * 0.65);
                    let galley = painter.layout_no_wrap(label.clone(), msg_font.clone(), lbl_color);
                    let text_x = x + dot_radius + 6.0 * scale;
                    let text_y = mid_y - galley.rect.height() / 2.0;
                    painter.galley(Pos2::new(text_x, text_y), galley, lbl_color);
                }
            }
        }
    }

    height
}

/// Draw a small arrowhead pointing in a direction.
fn draw_arrowhead(
    painter: &egui::Painter,
    tip: Pos2,
    size: f32,
    _angle: f32,
    color: egui::Color32,
) {
    // Right-pointing arrowhead
    let left_top = Pos2::new(tip.x - size, tip.y - size * 0.5);
    let left_bot = Pos2::new(tip.x - size, tip.y + size * 0.5);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left_top, left_bot],
        color,
        Stroke::NONE,
    ));
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_branch_simple() {
        let content = "- branch main\n- branch develop from main";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 2);
        match &items[0] {
            GitGraphItem::Branch { name, from, .. } => {
                assert_eq!(name, "main");
                assert!(from.is_none());
            }
            _ => panic!("Expected Branch"),
        }
        match &items[1] {
            GitGraphItem::Branch { name, from, .. } => {
                assert_eq!(name, "develop");
                assert_eq!(from.as_deref(), Some("main"));
            }
            _ => panic!("Expected Branch"),
        }
    }

    #[test]
    fn test_parse_commit() {
        let content = "- commit develop: \"Initial setup\"";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 1);
        match &items[0] {
            GitGraphItem::Commit {
                branch, message, ..
            } => {
                assert_eq!(branch, "develop");
                assert_eq!(message, "Initial setup");
            }
            _ => panic!("Expected Commit"),
        }
    }

    #[test]
    fn test_parse_merge() {
        let content = "- merge feature/login -> develop: \"PR #42\"";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 1);
        match &items[0] {
            GitGraphItem::Merge {
                source,
                target,
                label,
                ..
            } => {
                assert_eq!(source, "feature/login");
                assert_eq!(target, "develop");
                assert_eq!(label, "PR #42");
            }
            _ => panic!("Expected Merge"),
        }
    }

    #[test]
    fn test_parse_merge_no_label() {
        let content = "- merge feature/auth -> develop";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 1);
        match &items[0] {
            GitGraphItem::Merge {
                source,
                target,
                label,
                ..
            } => {
                assert_eq!(source, "feature/auth");
                assert_eq!(target, "develop");
                assert!(label.is_empty());
            }
            _ => panic!("Expected Merge"),
        }
    }

    #[test]
    fn test_parse_reveal_markers() {
        let content = "- branch main\n+ branch develop from main\n* commit develop: \"Init\"";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 3);
        match &items[0] {
            GitGraphItem::Branch { reveal, .. } => assert_eq!(*reveal, VizReveal::Static),
            _ => panic!(),
        }
        match &items[1] {
            GitGraphItem::Branch { reveal, .. } => assert_eq!(*reveal, VizReveal::NextStep),
            _ => panic!(),
        }
        match &items[2] {
            GitGraphItem::Commit { reveal, .. } => assert_eq!(*reveal, VizReveal::WithPrev),
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_ignores_comments_and_blanks() {
        let content =
            "# This is a comment\n\n- branch main\n# Another comment\n- branch develop from main";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_parse_full_gitflow() {
        let content = "\
- branch main
- branch develop from main
+ branch feature/login from develop
+ commit feature/login: \"Add form\"
+ merge feature/login -> develop
+ branch release/1.0 from develop
+ merge release/1.0 -> main: \"v1.0\"
* merge release/1.0 -> develop";
        let items = parse_gitgraph(content);
        assert_eq!(items.len(), 8);
    }
}
