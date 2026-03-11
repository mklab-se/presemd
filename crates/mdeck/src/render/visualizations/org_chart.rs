use std::collections::HashMap;
use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{
    VIZ_CORNER_NODE, VIZ_FONT_PRIMARY_LABEL, VIZ_STROKE_BORDER, VIZ_STROKE_SEPARATOR, VizReveal,
    assign_steps, parse_reveal_prefix, reveal_anim_progress,
};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct OrgEdge {
    parent: String,
    child: String,
    reveal: VizReveal,
}

fn parse_org_chart(content: &str) -> (Vec<String>, Vec<OrgEdge>) {
    let mut roots = Vec::new();
    let mut edges = Vec::new();
    let mut seen_nodes: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        if let Some(arrow_pos) = text.find(" -> ") {
            let parent = text[..arrow_pos].trim().to_string();
            let child = text[arrow_pos + 4..].trim().to_string();
            if !seen_nodes.contains(&parent) {
                seen_nodes.push(parent.clone());
            }
            if !seen_nodes.contains(&child) {
                seen_nodes.push(child.clone());
            }
            edges.push(OrgEdge {
                parent,
                child,
                reveal,
            });
        } else {
            // Root node declaration
            let node = text.to_string();
            if !seen_nodes.contains(&node) {
                seen_nodes.push(node.clone());
            }
            if !roots.contains(&node) {
                roots.push(node);
            }
        }
    }

    // If no explicit roots, find nodes that are never children
    if roots.is_empty() {
        let children: Vec<&str> = edges.iter().map(|e| e.child.as_str()).collect();
        for node in &seen_nodes {
            if !children.contains(&node.as_str()) {
                roots.push(node.clone());
            }
        }
    }

    (roots, edges)
}

// ─── Tree layout ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LayoutNode {
    label: String,
    x: f32,
    y: f32,
    depth: usize,
    #[allow(dead_code)]
    child_count: usize,
}

fn build_layout(
    roots: &[String],
    edges: &[OrgEdge],
    area_width: f32,
    area_height: f32,
) -> Vec<LayoutNode> {
    // Build children map
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        children_map
            .entry(edge.parent.clone())
            .or_default()
            .push(edge.child.clone());
    }

    // BFS to compute depth and collect nodes per level
    let mut levels: Vec<Vec<String>> = Vec::new();
    let mut visited: HashMap<String, usize> = HashMap::new();

    let mut queue: Vec<(String, usize)> = Vec::new();
    for root in roots {
        queue.push((root.clone(), 0));
    }

    while let Some((node, depth)) = queue.first().cloned() {
        queue.remove(0);
        if visited.contains_key(&node) {
            continue;
        }
        visited.insert(node.clone(), depth);
        while levels.len() <= depth {
            levels.push(Vec::new());
        }
        levels[depth].push(node.clone());
        if let Some(children) = children_map.get(&node) {
            for child in children {
                if !visited.contains_key(child) {
                    queue.push((child.clone(), depth + 1));
                }
            }
        }
    }

    let num_levels = levels.len().max(1);
    let level_height = area_height / num_levels as f32;

    let mut layout_nodes = Vec::new();
    for (depth, level) in levels.iter().enumerate() {
        let n = level.len();
        let spacing = area_width / (n + 1) as f32;
        for (i, node) in level.iter().enumerate() {
            let child_count = children_map.get(node).map_or(0, |c| c.len());
            layout_nodes.push(LayoutNode {
                label: node.clone(),
                x: spacing * (i + 1) as f32,
                y: level_height * depth as f32 + level_height * 0.5,
                depth,
                child_count,
            });
        }
    }

    layout_nodes
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_org_chart(
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
    let (roots, edges) = parse_org_chart(content);
    if roots.is_empty() && edges.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    // Assign reveal steps: root declarations are static, edges carry reveal markers
    // We assign steps per edge, and nodes inherit the step of the edge that introduces them
    let mut all_reveals: Vec<VizReveal> = Vec::new();
    // First, root nodes (always static if declared explicitly)
    for _root in &roots {
        all_reveals.push(VizReveal::Static);
    }
    // Then edges
    let edge_start = all_reveals.len();
    for edge in &edges {
        all_reveals.push(edge.reveal);
    }
    let steps = assign_steps(&all_reveals);

    // Build node step map: a node is visible when its introducing element is visible
    let mut node_step: HashMap<String, usize> = HashMap::new();
    for (i, root) in roots.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        node_step
            .entry(root.clone())
            .and_modify(|s| *s = (*s).min(step))
            .or_insert(step);
    }
    for (j, edge) in edges.iter().enumerate() {
        let step = steps.get(edge_start + j).copied().unwrap_or(0);
        // Parent is at least visible at this step
        node_step
            .entry(edge.parent.clone())
            .and_modify(|s| *s = (*s).min(step))
            .or_insert(step);
        // Child appears at this step
        node_step
            .entry(edge.child.clone())
            .and_modify(|s| *s = (*s).min(step))
            .or_insert(step);
    }

    let palette = theme.edge_palette();
    let painter = ui.painter();
    let label_font = FontId::proportional(theme.body_size * VIZ_FONT_PRIMARY_LABEL * scale);

    let padding = 40.0 * scale;
    let layout = build_layout(
        &roots,
        &edges,
        max_width - padding * 2.0,
        height - padding * 2.0,
    );

    // Build position lookup
    let node_positions: HashMap<String, (f32, f32)> = layout
        .iter()
        .map(|n| {
            (
                n.label.clone(),
                (pos.x + padding + n.x, pos.y + padding + n.y),
            )
        })
        .collect();

    let min_node_w = 120.0 * scale;
    let node_h_padding = 16.0 * scale;
    let node_w_padding = 24.0 * scale;
    let corner_radius = VIZ_CORNER_NODE * scale;

    // Pre-compute node sizes based on label text width
    let mut node_sizes: HashMap<String, (f32, f32)> = HashMap::new();
    for node in &layout {
        let text_color = Theme::with_opacity(theme.foreground, opacity);
        let galley = painter.layout_no_wrap(node.label.clone(), label_font.clone(), text_color);
        let w = (galley.rect.width() + node_w_padding * 2.0).max(min_node_w);
        let h = galley.rect.height() + node_h_padding * 2.0;
        node_sizes.insert(node.label.clone(), (w, h));
    }

    let mut needs_repaint = false;

    // Draw edges first (behind nodes)
    for (j, edge) in edges.iter().enumerate() {
        let step = steps.get(edge_start + j).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        if let (Some(&(px, py)), Some(&(cx, cy))) = (
            node_positions.get(&edge.parent),
            node_positions.get(&edge.child),
        ) {
            let edge_color = Theme::with_opacity(theme.foreground, opacity * 0.3 * anim);
            let stroke = Stroke::new(VIZ_STROKE_SEPARATOR * scale, edge_color);

            let parent_h = node_sizes
                .get(&edge.parent)
                .map(|s| s.1)
                .unwrap_or(40.0 * scale);
            let child_h = node_sizes
                .get(&edge.child)
                .map(|s| s.1)
                .unwrap_or(40.0 * scale);

            // Right-angle connector: parent bottom -> mid-y -> child top
            let p_bottom = py + parent_h / 2.0;
            let c_top = cy - child_h / 2.0;
            let mid_y = (p_bottom + c_top) / 2.0;

            painter.line_segment([Pos2::new(px, p_bottom), Pos2::new(px, mid_y)], stroke);
            painter.line_segment([Pos2::new(px, mid_y), Pos2::new(cx, mid_y)], stroke);
            painter.line_segment([Pos2::new(cx, mid_y), Pos2::new(cx, c_top)], stroke);
        }
    }

    // Draw nodes
    for node in &layout {
        let step = node_step.get(&node.label).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let (nx, ny) = node_positions
            .get(&node.label)
            .copied()
            .unwrap_or((0.0, 0.0));
        let (node_w, node_h) = node_sizes
            .get(&node.label)
            .copied()
            .unwrap_or((min_node_w, 40.0 * scale));

        let color_idx = node.depth % palette.len();
        let bg_color = Theme::with_opacity(palette[color_idx], opacity * 0.15 * anim);
        let border_color = Theme::with_opacity(palette[color_idx], opacity * 0.6 * anim);

        let rect = egui::Rect::from_center_size(Pos2::new(nx, ny), egui::vec2(node_w, node_h));
        painter.rect_filled(rect, corner_radius, bg_color);
        painter.rect_stroke(
            rect,
            corner_radius,
            Stroke::new(VIZ_STROKE_BORDER * scale, border_color),
            egui::StrokeKind::Outside,
        );

        // Label
        let text_color = Theme::with_opacity(theme.foreground, opacity * anim);
        let galley = painter.layout_no_wrap(node.label.clone(), label_font.clone(), text_color);
        let tx = nx - galley.rect.width() / 2.0;
        let ty = ny - galley.rect.height() / 2.0;
        painter.galley(Pos2::new(tx, ty), galley, text_color);
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
    fn test_parse_org_chart_basic() {
        let content = "- CEO\n- CEO -> CTO\n- CEO -> CFO";
        let (roots, edges) = parse_org_chart(content);
        assert_eq!(roots, vec!["CEO"]);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].parent, "CEO");
        assert_eq!(edges[0].child, "CTO");
        assert_eq!(edges[1].parent, "CEO");
        assert_eq!(edges[1].child, "CFO");
    }

    #[test]
    fn test_parse_org_chart_implicit_root() {
        let content = "- CEO -> CTO\n- CEO -> CFO";
        let (roots, edges) = parse_org_chart(content);
        // CEO is never a child, so it becomes root
        assert_eq!(roots, vec!["CEO"]);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_parse_org_chart_reveal_markers() {
        let content = "- CEO\n- CEO -> CTO\n+ CTO -> VP Engineering";
        let (_, edges) = parse_org_chart(content);
        assert_eq!(edges[0].reveal, VizReveal::Static);
        assert_eq!(edges[1].reveal, VizReveal::NextStep);
    }

    #[test]
    fn test_parse_org_chart_skips_comments() {
        let content = "# header\n- CEO\n# note\n- CEO -> CTO";
        let (roots, edges) = parse_org_chart(content);
        assert_eq!(roots, vec!["CEO"]);
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_build_layout_depths() {
        let roots = vec!["CEO".to_string()];
        let edges = vec![
            OrgEdge {
                parent: "CEO".to_string(),
                child: "CTO".to_string(),
                reveal: VizReveal::Static,
            },
            OrgEdge {
                parent: "CTO".to_string(),
                child: "VP".to_string(),
                reveal: VizReveal::Static,
            },
        ];
        let layout = build_layout(&roots, &edges, 800.0, 600.0);
        assert_eq!(layout.len(), 3);
        let ceo = layout.iter().find(|n| n.label == "CEO").unwrap();
        let cto = layout.iter().find(|n| n.label == "CTO").unwrap();
        let vp = layout.iter().find(|n| n.label == "VP").unwrap();
        assert_eq!(ceo.depth, 0);
        assert_eq!(cto.depth, 1);
        assert_eq!(vp.depth, 2);
    }
}
