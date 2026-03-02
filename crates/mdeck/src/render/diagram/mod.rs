pub mod routing;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use std::time::Instant;

use crate::render::image_cache::ImageCache;
use crate::theme::Theme;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Stroke};

// ─── Route cache ────────────────────────────────────────────────────────────

// Thread-local cache for routing results. Routing is expensive (A* search with
// rayon parallelism per edge) and the inputs rarely change between frames.
// We cache the output keyed by a hash of (nodes, edges, config).
thread_local! {
    static ROUTE_CACHE: RefCell<Option<RouteCacheEntry>> = const { RefCell::new(None) };
}

struct RouteCacheEntry {
    key: u64,
    output: routing::types::RoutingOutput,
}

/// Routing weights loaded once from config at startup.
static ROUTING_WEIGHTS: LazyLock<routing::types::CostWeights> = LazyLock::new(|| {
    crate::config::Config::load_or_default()
        .routing
        .unwrap_or_default()
        .to_cost_weights()
});

/// Compute a hash key for the routing inputs.
fn route_cache_key(
    nodes: &[routing::types::DiagramNode],
    edges: &[routing::types::DiagramEdge],
    config: &routing::types::RoutingConfig,
) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    for n in nodes {
        n.name.hash(&mut hasher);
        n.col.hash(&mut hasher);
        n.row.hash(&mut hasher);
    }
    for e in edges {
        e.source.hash(&mut hasher);
        e.target.hash(&mut hasher);
        e.label.hash(&mut hasher);
    }
    config.h_lane_capacity.hash(&mut hasher);
    config.v_lane_capacity.hash(&mut hasher);
    config.weights.length.to_bits().hash(&mut hasher);
    config.weights.turn.to_bits().hash(&mut hasher);
    config.weights.lane_change.to_bits().hash(&mut hasher);
    config.weights.crossing.to_bits().hash(&mut hasher);
    hasher.finish()
}

// ─── Diagram data structures ─────────────────────────────────────────────────

/// Reveal marker for diagram elements (mirrors ListMarker semantics).
#[derive(Debug, Clone, Copy, PartialEq)]
enum DiagramReveal {
    /// Always visible (prefix `-` or no prefix).
    Static,
    /// Appears on the next reveal step (prefix `+`).
    NextStep,
    /// Appears together with the previous `+` element (prefix `*`).
    WithPrev,
}

struct DiagramNode {
    name: String,
    label: String,
    icon: String,
    grid_pos: Option<(u32, u32)>,
    reveal: DiagramReveal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ArrowKind {
    Forward,       // ->
    Reverse,       // <-
    Bidirectional, // <->
    DashedLine,    // --
    DashedArrow,   // -->
}

struct DiagramEdge {
    from: String,
    to: String,
    label: String,
    arrow: ArrowKind,
    reveal: DiagramReveal,
}

// ─── Orthogonal routing ─────────────────────────────────────────────────────

/// Information about the grid layout for routing.
///
/// Corridors live in the gaps between grid cells:
///   - Horizontal corridor `i` runs at y = origin_y + i * cell_h (between row i-1 and row i)
///   - Vertical corridor `j` runs at x = origin_x + j * cell_w (between col j-1 and col j)
///
/// Corridor index 0 is the edge before the first row/col; index N is after the last.
struct GridInfo {
    cols: usize,
    rows: usize,
    cell_w: f32,
    cell_h: f32,
    origin_x: f32,
    origin_y: f32,
    /// Grid cells that contain a node (0-indexed: col 0..cols-1, row 0..rows-1).
    occupied: HashSet<(usize, usize)>,
}

impl GridInfo {
    /// Y position of horizontal corridor at given index (raw cell boundary).
    #[cfg(test)]
    fn h_corridor_y(&self, index: usize) -> f32 {
        self.origin_y + index as f32 * self.cell_h
    }

    /// X position of vertical corridor at given index (raw cell boundary).
    #[cfg(test)]
    fn v_corridor_x(&self, index: usize) -> f32 {
        self.origin_x + index as f32 * self.cell_w
    }

    /// Return the grid cell (col, row) containing a point, if within bounds.
    fn cell_at(&self, pos: Pos2) -> Option<(usize, usize)> {
        let col = ((pos.x - self.origin_x) / self.cell_w).floor() as isize;
        let row = ((pos.y - self.origin_y) / self.cell_h).floor() as isize;
        if col >= 0 && (col as usize) < self.cols && row >= 0 && (row as usize) < self.rows {
            Some((col as usize, row as usize))
        } else {
            None
        }
    }

    /// Check if a grid cell has no node in it.
    #[cfg(test)]
    fn is_cell_empty(&self, col: usize, row: usize) -> bool {
        !self.occupied.contains(&(col, row))
    }
}

/// Which face of a node to exit/enter from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Face {
    Right,
    Left,
    Bottom,
    Top,
}

/// Compute a point on a node face, offset from center by `port_offset`.
/// For Left/Right faces, offset shifts Y. For Top/Bottom faces, offset shifts X.
fn face_point_with_port(rect: &egui::Rect, face: Face, port_offset: f32) -> Pos2 {
    let c = rect.center();
    match face {
        Face::Right => Pos2::new(rect.right(), c.y + port_offset),
        Face::Left => Pos2::new(rect.left(), c.y + port_offset),
        Face::Bottom => Pos2::new(c.x + port_offset, rect.bottom()),
        Face::Top => Pos2::new(c.x + port_offset, rect.top()),
    }
}

/// Choose the best exit face from `from_rect` toward `to_rect` center.
fn choose_exit_face(from_rect: &egui::Rect, to_center: Pos2) -> Face {
    let c = from_rect.center();
    let dx = to_center.x - c.x;
    let dy = to_center.y - c.y;
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { Face::Right } else { Face::Left }
    } else if dy >= 0.0 {
        Face::Bottom
    } else {
        Face::Top
    }
}

/// Choose the best entry face on `to_rect` coming from `from_center`.
fn choose_entry_face(to_rect: &egui::Rect, from_center: Pos2) -> Face {
    let c = to_rect.center();
    let dx = from_center.x - c.x;
    let dy = from_center.y - c.y;
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { Face::Right } else { Face::Left }
    } else if dy >= 0.0 {
        Face::Bottom
    } else {
        Face::Top
    }
}

/// Compute a ramp point offset from a face point.
fn ramp_from_face(rect: &egui::Rect, face: Face, port_offset: f32, node_margin: f32) -> Pos2 {
    let fp = face_point_with_port(rect, face, port_offset);
    match face {
        Face::Right => Pos2::new(fp.x + node_margin, fp.y),
        Face::Left => Pos2::new(fp.x - node_margin, fp.y),
        Face::Bottom => Pos2::new(fp.x, fp.y + node_margin),
        Face::Top => Pos2::new(fp.x, fp.y - node_margin),
    }
}

/// Map a routing::Direction to the corresponding Face.
fn direction_to_face(dir: routing::types::Direction) -> Face {
    match dir {
        routing::types::Direction::North => Face::Top,
        routing::types::Direction::South => Face::Bottom,
        routing::types::Direction::East => Face::Right,
        routing::types::Direction::West => Face::Left,
    }
}

// ─── New routing engine integration ─────────────────────────────────────────

/// Compute lane capacity for horizontal corridors (edges travel left/right).
/// The gap available is the vertical space between node edges.
fn compute_h_capacity(
    grid: &GridInfo,
    node_rects: &HashMap<String, egui::Rect>,
    lane_spacing: f32,
) -> i32 {
    // Find the minimum vertical gap between any two adjacent rows
    let mut min_gap = f32::MAX;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            // Check if this cell has a node
            if let Some(rect) = find_rect_at(grid, node_rects, col, row) {
                let node_h = rect.height();
                let gap = grid.cell_h - node_h;
                min_gap = min_gap.min(gap);
            }
        }
    }
    if !min_gap.is_finite() || min_gap <= 0.0 || lane_spacing <= 0.0 {
        return 3; // sensible default
    }
    let capacity = (min_gap / lane_spacing).floor() as i32;
    capacity.max(1)
}

/// Compute lane capacity for vertical corridors (edges travel up/down).
/// The gap available is the horizontal space between node edges.
fn compute_v_capacity(
    grid: &GridInfo,
    node_rects: &HashMap<String, egui::Rect>,
    lane_spacing: f32,
) -> i32 {
    let mut min_gap = f32::MAX;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            if let Some(rect) = find_rect_at(grid, node_rects, col, row) {
                let node_w = rect.width();
                let gap = grid.cell_w - node_w;
                min_gap = min_gap.min(gap);
            }
        }
    }
    if !min_gap.is_finite() || min_gap <= 0.0 || lane_spacing <= 0.0 {
        return 3;
    }
    let capacity = (min_gap / lane_spacing).floor() as i32;
    capacity.max(1)
}

/// Find the rect of a node at a specific 0-indexed grid cell.
fn find_rect_at(
    grid: &GridInfo,
    node_rects: &HashMap<String, egui::Rect>,
    col: usize,
    row: usize,
) -> Option<egui::Rect> {
    if !grid.occupied.contains(&(col, row)) {
        return None;
    }
    // Find rect whose center falls in this cell
    let cell_center_x = grid.origin_x + (col as f32 + 0.5) * grid.cell_w;
    let cell_center_y = grid.origin_y + (row as f32 + 0.5) * grid.cell_h;
    let cell_center = Pos2::new(cell_center_x, cell_center_y);
    node_rects
        .values()
        .find(|r| {
            let c = r.center();
            (c.x - cell_center.x).abs() < grid.cell_w * 0.5
                && (c.y - cell_center.y).abs() < grid.cell_h * 0.5
        })
        .copied()
}

/// Convert a routing engine Route to pixel waypoints for drawing.
///
/// The routing engine works in 1-based integer grid coordinates.
/// This function converts those to pixel positions using the GridInfo geometry,
/// and adds face connection points (ramps) at the start and end.
#[allow(clippy::too_many_arguments)]
fn waypoints_to_pixels(
    route: &routing::types::Route,
    grid: &GridInfo,
    from_rect: &egui::Rect,
    to_rect: &egui::Rect,
    node_margin: f32,
    lane_spacing: f32,
    port_offset_start: f32,
    port_offset_end: f32,
) -> Vec<Pos2> {
    if route.waypoints.len() < 2 {
        return Vec::new();
    }

    let mut pixels = Vec::new();

    // Determine exit direction from the first two waypoints
    let first = &route.waypoints[0];
    let second = &route.waypoints[1];
    let exit_dir = coord_direction(first.coord, second.coord);
    let exit_face = direction_to_face(exit_dir);

    // Determine entry direction from the last two waypoints
    let n = route.waypoints.len();
    let penult = &route.waypoints[n - 2];
    let last = &route.waypoints[n - 1];
    let entry_dir = coord_direction(penult.coord, last.coord);
    let entry_face = direction_to_face(entry_dir.opposite());

    // Start: face point and ramp on the source node
    let fp_start = face_point_with_port(from_rect, exit_face, port_offset_start);
    let ramp_start = ramp_from_face(from_rect, exit_face, port_offset_start, node_margin);
    pixels.push(fp_start);
    pixels.push(ramp_start);

    // Intermediate waypoints (skip first = source center, skip last = target center)
    for i in 1..n - 1 {
        let wp = &route.waypoints[i];
        let px = coord_to_pixel_x(wp.coord, grid);
        let py = coord_to_pixel_y(wp.coord, grid);

        // Compute incoming offset (from previous segment's direction and lane)
        let prev_wp = &route.waypoints[i - 1];
        let in_dir = coord_direction(prev_wp.coord, wp.coord);
        let in_lane = prev_wp.lane;
        let (in_ox, in_oy) = lane_offset(in_dir, in_lane, lane_spacing);

        // Compute outgoing offset (from this waypoint's direction and lane to next)
        let (out_ox, out_oy) = if i + 1 < n {
            let next_wp = &route.waypoints[i + 1];
            let out_dir = coord_direction(wp.coord, next_wp.coord);
            lane_offset(out_dir, wp.lane, lane_spacing)
        } else {
            (0.0, 0.0)
        };

        // At a turn (horizontal↔vertical), compute a single combined corner point
        // that keeps both the incoming and outgoing segments straight:
        //   - Horizontal segments offset Y → keep incoming Y at the corner
        //   - Vertical segments offset X → keep outgoing X at the corner
        let is_turn = in_dir.is_horizontal() != {
            if i + 1 < n {
                let next_wp = &route.waypoints[i + 1];
                coord_direction(wp.coord, next_wp.coord).is_horizontal()
            } else {
                in_dir.is_horizontal()
            }
        };

        let pt = if is_turn {
            let (cx, cy) = if in_dir.is_horizontal() {
                // Horizontal → Vertical: keep incoming Y, use outgoing X
                (out_ox, in_oy)
            } else {
                // Vertical → Horizontal: keep incoming X, use outgoing Y
                (in_ox, out_oy)
            };
            Pos2::new(px + cx, py + cy)
        } else {
            // Straight segment: use outgoing offset (matches next segment)
            Pos2::new(px + out_ox, py + out_oy)
        };

        if let Some(prev) = pixels.last() {
            if (*prev - pt).length() < 1.0 {
                continue;
            }
        }
        pixels.push(pt);
    }

    // End: ramp and face point on the target node
    let ramp_end = ramp_from_face(to_rect, entry_face, port_offset_end, node_margin);
    let fp_end = face_point_with_port(to_rect, entry_face, port_offset_end);
    if let Some(prev) = pixels.last() {
        if (*prev - ramp_end).length() >= 1.0 {
            pixels.push(ramp_end);
        }
    } else {
        pixels.push(ramp_end);
    }
    pixels.push(fp_end);

    // Ensure all segments are orthogonal by inserting corner points
    ensure_orthogonal(&mut pixels);

    pixels
}

/// Convert a grid coordinate's column to pixel X.
/// The routing engine uses 1-based integer coords. A coord with col=1 means
/// the center of column 1 → pixel x = (1 - 0.5) * cell_w + origin_x = 0.5 * cell_w + origin_x.
/// Half-integer coords (junction between columns) are handled naturally via col_f64().
fn coord_to_pixel_x(coord: routing::types::GridCoord, grid: &GridInfo) -> f32 {
    // The routing engine uses 1-based coords. Grid layout uses: center_x = (col - 0.5) * cell_w
    // But origin_x is already in the GridInfo, so: px = (col_f64 - 0.5) * cell_w + origin_x
    // However, the routing engine coords are 1-based integers stored as doubled.
    // col_f64() gives the actual column number (1.0, 1.5, 2.0, etc.)
    // The grid layout places col 1 at (1 - 0.5) * cell_w = 0.5 * cell_w from origin.
    (coord.col_f64() as f32 - 0.5) * grid.cell_w + grid.origin_x
}

/// Convert a grid coordinate's row to pixel Y.
fn coord_to_pixel_y(coord: routing::types::GridCoord, grid: &GridInfo) -> f32 {
    (coord.row_f64() as f32 - 0.5) * grid.cell_h + grid.origin_y
}

/// Determine the direction from one grid coord to another.
fn coord_direction(
    from: routing::types::GridCoord,
    to: routing::types::GridCoord,
) -> routing::types::Direction {
    let dc = to.col2 - from.col2;
    let dr = to.row2 - from.row2;
    if dc.abs() >= dr.abs() {
        if dc >= 0 {
            routing::types::Direction::East
        } else {
            routing::types::Direction::West
        }
    } else if dr >= 0 {
        routing::types::Direction::South
    } else {
        routing::types::Direction::North
    }
}

/// Compute pixel offset for a lane perpendicular to the travel direction.
/// Lane 0 = center (no offset). Uses absolute convention:
///   - Horizontal segments: positive lanes offset south (+Y), negative offset north (-Y)
///   - Vertical segments: positive lanes offset east (+X), negative offset west (-X)
///
/// This ensures lane numbers map to the same physical position on a segment
/// regardless of travel direction.
fn lane_offset(dir: routing::types::Direction, lane: i32, lane_spacing: f32) -> (f32, f32) {
    if lane == 0 {
        return (0.0, 0.0);
    }
    let offset = lane as f32 * lane_spacing;
    if dir.is_horizontal() {
        // Horizontal travel: lane offset in Y. Positive lane = south.
        (0.0, offset)
    } else {
        // Vertical travel: lane offset in X. Positive lane = east.
        (offset, 0.0)
    }
}

/// Post-process waypoints to ensure every consecutive pair is axis-aligned.
fn ensure_orthogonal(waypoints: &mut Vec<Pos2>) {
    let mut i = 0;
    while i + 1 < waypoints.len() {
        let a = waypoints[i];
        let b = waypoints[i + 1];
        let dx = (a.x - b.x).abs();
        let dy = (a.y - b.y).abs();
        if dx > 1.0 && dy > 1.0 {
            let was_horizontal = if i > 0 {
                let prev = waypoints[i - 1];
                (prev.y - a.y).abs() < (prev.x - a.x).abs()
            } else {
                dx > dy
            };
            let corner = if was_horizontal {
                Pos2::new(b.x, a.y)
            } else {
                Pos2::new(a.x, b.y)
            };
            waypoints.insert(i + 1, corner);
        }
        i += 1;
    }
}

/// Apply rounded corners to an orthogonal polyline.
/// Returns a new polyline with arcs at each bend.
fn apply_rounded_corners(waypoints: &[Pos2], radius: f32) -> Vec<Pos2> {
    if waypoints.len() < 3 {
        return waypoints.to_vec();
    }

    let mut result = Vec::new();
    result.push(waypoints[0]);

    for i in 1..waypoints.len() - 1 {
        let prev = waypoints[i - 1];
        let curr = waypoints[i];
        let next = waypoints[i + 1];

        // Compute available lengths on incoming and outgoing segments
        let in_len = (curr - prev).length();
        let out_len = (next - curr).length();

        // Clamp radius to half the shorter adjacent segment
        let r = radius.min(in_len / 2.0).min(out_len / 2.0);
        if r < 1.0 {
            result.push(curr);
            continue;
        }

        // Direction vectors
        let in_dir = (curr - prev).normalized();
        let out_dir = (next - curr).normalized();

        // Points where the arc starts and ends
        let arc_start = curr - in_dir * r;
        let arc_end = curr + out_dir * r;

        // Generate arc points (8-point approximation of quarter circle)
        let n_arc_points = 8;
        for j in 0..=n_arc_points {
            let t = j as f32 / n_arc_points as f32;
            let x = arc_start.x * (1.0 - t) * (1.0 - t)
                + curr.x * 2.0 * (1.0 - t) * t
                + arc_end.x * t * t;
            let y = arc_start.y * (1.0 - t) * (1.0 - t)
                + curr.y * 2.0 * (1.0 - t) * t
                + arc_end.y * t * t;
            result.push(Pos2::new(x, y));
        }
    }

    result.push(*waypoints.last().unwrap());
    result
}

/// Compute the total length of a polyline.
fn polyline_length(points: &[Pos2]) -> f32 {
    let mut total = 0.0;
    for i in 0..points.len().saturating_sub(1) {
        total += (points[i + 1] - points[i]).length();
    }
    total
}

/// Find the point at a given distance along a polyline.
fn polyline_point_at_distance(points: &[Pos2], distance: f32) -> Pos2 {
    let mut remaining = distance;
    for i in 0..points.len().saturating_sub(1) {
        let seg_len = (points[i + 1] - points[i]).length();
        if remaining <= seg_len {
            let t = remaining / seg_len.max(0.001);
            return Pos2::new(
                points[i].x + (points[i + 1].x - points[i].x) * t,
                points[i].y + (points[i + 1].y - points[i].y) * t,
            );
        }
        remaining -= seg_len;
    }
    *points.last().unwrap_or(&Pos2::ZERO)
}

/// Draw a dashed polyline across multiple segments with continuity.
fn draw_dashed_polyline(
    painter: &egui::Painter,
    points: &[Pos2],
    width: f32,
    color: Color32,
    scale: f32,
) {
    let dash_len = 8.0 * scale;
    let gap_len = 5.0 * scale;
    let total_len = polyline_length(points);
    let stroke = Stroke::new(width, color);

    let mut d = 0.0;
    let mut drawing = true;
    while d < total_len {
        if drawing {
            let seg_end_d = (d + dash_len).min(total_len);
            let p1 = polyline_point_at_distance(points, d);
            let p2 = polyline_point_at_distance(points, seg_end_d);
            painter.line_segment([p1, p2], stroke);
            d += dash_len;
        } else {
            d += gap_len;
        }
        drawing = !drawing;
    }
}

/// Draw a routed edge with rounded corners, arrowheads, and optional label.
#[allow(clippy::too_many_arguments)]
fn draw_routed_edge(
    painter: &egui::Painter,
    waypoints: &[Pos2],
    arrow: ArrowKind,
    label: &str,
    edge_color: Color32,
    label_bg: Color32,
    label_text_color: Color32,
    line_width: f32,
    arrow_size: f32,
    corner_radius: f32,
    theme: &Theme,
    scale: f32,
    _opacity: f32,
    anim_progress: f32,
) {
    if waypoints.len() < 2 {
        return;
    }

    let is_dashed = matches!(arrow, ArrowKind::DashedLine | ArrowKind::DashedArrow);
    let start = waypoints[0];
    let end = *waypoints.last().unwrap();

    // Apply rounded corners to get smooth polyline
    let smooth_points = apply_rounded_corners(waypoints, corner_radius);

    // Determine arrowhead status
    let has_end_arrow = matches!(
        arrow,
        ArrowKind::Forward | ArrowKind::DashedArrow | ArrowKind::Bidirectional
    );
    let has_start_arrow = matches!(arrow, ArrowKind::Reverse | ArrowKind::Bidirectional);

    // Total length for animation clipping
    let total_len = polyline_length(&smooth_points);

    // Clip the effective drawing length by animation progress
    let effective_len = total_len * anim_progress;

    // Shorten the polyline for arrowheads
    let draw_start_d = if has_start_arrow { arrow_size } else { 0.0 };
    let draw_end_d_full = if has_end_arrow {
        (total_len - arrow_size).max(0.0)
    } else {
        total_len
    };
    // Clip draw_end by animation progress
    let draw_end_d = draw_end_d_full.min(effective_len);

    // Build shortened polyline for line drawing
    if draw_end_d > draw_start_d + 1.0 {
        let line_start = polyline_point_at_distance(&smooth_points, draw_start_d);
        let line_end = polyline_point_at_distance(&smooth_points, draw_end_d);

        // Collect intermediate points
        let mut draw_points = vec![line_start];
        let mut cumulative = 0.0;
        for i in 0..smooth_points.len().saturating_sub(1) {
            let seg_len = (smooth_points[i + 1] - smooth_points[i]).length();
            let next_cumulative = cumulative + seg_len;
            if next_cumulative > draw_start_d && cumulative < draw_end_d {
                if cumulative > draw_start_d {
                    draw_points.push(smooth_points[i]);
                }
                if next_cumulative < draw_end_d {
                    draw_points.push(smooth_points[i + 1]);
                }
            }
            cumulative = next_cumulative;
        }
        draw_points.push(line_end);
        draw_points.dedup_by(|a, b| (*a - *b).length() < 0.5);

        if draw_points.len() >= 2 {
            if is_dashed {
                draw_dashed_polyline(painter, &draw_points, line_width, edge_color, scale);
            } else {
                painter.add(egui::Shape::line(
                    draw_points,
                    Stroke::new(line_width, edge_color),
                ));
            }
        }
    }

    // Draw arrowheads only when animation is complete
    let draw_arrowhead = |tip: Pos2, direction: egui::Vec2| {
        let d = direction.normalized();
        let p = egui::vec2(-d.y, d.x);
        let p1 = tip - d * arrow_size + p * arrow_size * 0.4;
        let p2 = tip - d * arrow_size - p * arrow_size * 0.4;
        painter.add(egui::Shape::convex_polygon(
            vec![tip, p1, p2],
            edge_color,
            Stroke::NONE,
        ));
    };

    if has_end_arrow && anim_progress >= 1.0 {
        let n = waypoints.len();
        let last_seg_len = if n >= 2 {
            (waypoints[n - 1] - waypoints[n - 2]).length()
        } else {
            total_len
        };
        if last_seg_len >= arrow_size * 1.2 {
            let last_dir = if n >= 2 {
                waypoints[n - 1] - waypoints[n - 2]
            } else {
                end - start
            };
            draw_arrowhead(end, last_dir);
        } else if n >= 3 {
            let pre_turn_dir = waypoints[n - 2] - waypoints[n - 3];
            let arrowhead_tip = waypoints[n - 2];
            draw_arrowhead(arrowhead_tip, pre_turn_dir);
        } else {
            let last_dir = end - start;
            draw_arrowhead(end, last_dir);
        }
    }

    if has_start_arrow && anim_progress >= 1.0 {
        let first_seg_len = if waypoints.len() >= 2 {
            (waypoints[1] - waypoints[0]).length()
        } else {
            total_len
        };
        if first_seg_len >= arrow_size * 1.2 {
            let first_dir = if waypoints.len() >= 2 {
                waypoints[0] - waypoints[1]
            } else {
                start - end
            };
            draw_arrowhead(start, first_dir);
        } else if waypoints.len() >= 3 {
            let post_turn_dir = waypoints[1] - waypoints[2];
            let arrowhead_tip = waypoints[1];
            draw_arrowhead(arrowhead_tip, post_turn_dir);
        } else {
            let first_dir = start - end;
            draw_arrowhead(start, first_dir);
        }
    }

    // Edge label only when animation is complete
    if !label.is_empty() && anim_progress >= 1.0 {
        let mid_distance = total_len * 0.20;
        let mid = polyline_point_at_distance(&smooth_points, mid_distance);
        let label_font_size = theme.body_size * 0.45 * scale;
        let label_padding = 5.0 * scale;
        let galley = painter.layout_no_wrap(
            label.to_string(),
            FontId::proportional(label_font_size),
            label_text_color,
        );
        let label_w = galley.rect.width() + label_padding * 2.0;
        let label_h = galley.rect.height() + label_padding * 2.0;
        let label_rect = egui::Rect::from_center_size(mid, egui::vec2(label_w, label_h));
        painter.rect_filled(label_rect, label_h / 2.0, label_bg);
        painter.galley(
            egui::pos2(
                label_rect.left() + label_padding,
                label_rect.top() + label_padding,
            ),
            galley,
            label_text_color,
        );
    }
}
// ─── Diagram parser ──────────────────────────────────────────────────────────

/// Parse parenthetical metadata like `(icon: database, pos: 1,2)`.
/// Returns (icon, grid_pos) and the line content without the metadata.
fn parse_metadata(s: &str) -> (&str, String, Option<(u32, u32)>) {
    let trimmed = s.trim_end();
    if !trimmed.ends_with(')') {
        return (trimmed, String::new(), None);
    }
    let Some(paren_start) = trimmed.rfind('(') else {
        return (trimmed, String::new(), None);
    };
    // Only parse if there's whitespace before the paren
    if paren_start == 0 || trimmed.as_bytes()[paren_start - 1] != b' ' {
        return (trimmed, String::new(), None);
    }

    let before = trimmed[..paren_start].trim_end();
    let meta_str = &trimmed[paren_start + 1..trimmed.len() - 1]; // contents between parens

    let mut icon = String::new();
    let mut grid_pos = None;

    for part in meta_str.split(',') {
        let part = part.trim();
        if let Some(val) = part
            .strip_prefix("icon:")
            .or_else(|| part.strip_prefix("icon :"))
        {
            icon = val.trim().to_string();
        } else if let Some(val) = part
            .strip_prefix("pos:")
            .or_else(|| part.strip_prefix("pos :"))
        {
            let val = val.trim();
            // pos can be "x,y" but we already split on comma, so handle both forms
            if let Some((x_str, y_str)) = val.split_once(',') {
                if let (Ok(x), Ok(y)) = (x_str.trim().parse(), y_str.trim().parse()) {
                    grid_pos = Some((x, y));
                }
            } else if grid_pos.is_none() {
                // Might be split across commas: "pos: 1" then next part is "2"
                // Store x and look for y in next iteration
                if let Ok(x) = val.parse::<u32>() {
                    grid_pos = Some((x, 0)); // placeholder, y filled below
                }
            }
        } else if let Some((x, 0)) = grid_pos {
            // Continuation of pos value split by comma
            if let Ok(y) = part.trim().parse::<u32>() {
                grid_pos = Some((x, y));
            }
        }
    }

    (before, icon, grid_pos)
}

/// Detect arrow type and position in a line. Returns (arrow_pos, arrow_len, ArrowKind).
fn detect_arrow(s: &str) -> Option<(usize, usize, ArrowKind)> {
    // Order matters: check longer patterns first to avoid partial matches
    if let Some(p) = s.find(" <-> ") {
        return Some((p, 5, ArrowKind::Bidirectional));
    }
    if let Some(p) = s.find(" --> ") {
        return Some((p, 5, ArrowKind::DashedArrow));
    }
    if let Some(p) = s.find(" -> ") {
        return Some((p, 4, ArrowKind::Forward));
    }
    if let Some(p) = s.find(" <- ") {
        return Some((p, 4, ArrowKind::Reverse));
    }
    if let Some(p) = s.find(" -- ") {
        return Some((p, 4, ArrowKind::DashedLine));
    }
    None
}

fn parse_diagram(content: &str) -> (Vec<DiagramNode>, Vec<DiagramEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen_nodes: HashMap<String, usize> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comment lines
        if trimmed.starts_with('#') {
            continue;
        }

        // Strip list-style prefixes and record reveal marker
        let (trimmed, reveal) = if let Some(rest) = trimmed.strip_prefix("+ ") {
            (rest, DiagramReveal::NextStep)
        } else if let Some(rest) = trimmed.strip_prefix("* ") {
            (rest, DiagramReveal::WithPrev)
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            (rest, DiagramReveal::Static)
        } else {
            (trimmed, DiagramReveal::Static)
        };

        if trimmed.is_empty() {
            continue;
        }

        // Parse and strip trailing metadata (icon, pos)
        let (trimmed, meta_icon, meta_pos) = parse_metadata(trimmed);

        if let Some((arrow_pos, arrow_len, arrow_kind)) = detect_arrow(trimmed) {
            let from = trimmed[..arrow_pos].trim().to_string();
            let rest = &trimmed[arrow_pos + arrow_len..];
            let (to, label) = if let Some(colon_pos) = rest.find(": ") {
                (
                    rest[..colon_pos].trim().to_string(),
                    rest[colon_pos + 2..].trim().to_string(),
                )
            } else {
                (rest.trim().to_string(), String::new())
            };

            // Auto-create nodes for edges if not already declared
            for node_name in [&from, &to] {
                if !seen_nodes.contains_key(node_name) {
                    seen_nodes.insert(node_name.clone(), nodes.len());
                    nodes.push(DiagramNode {
                        name: node_name.clone(),
                        label: node_name.clone(),
                        icon: String::new(),
                        grid_pos: None,
                        reveal: DiagramReveal::Static,
                    });
                }
            }

            edges.push(DiagramEdge {
                from,
                to,
                label,
                arrow: arrow_kind,
                reveal,
            });
        } else if let Some(colon_pos) = trimmed.find(": ") {
            // Node declaration with label: "Name: Label"
            let name = trimmed[..colon_pos].trim().to_string();
            let label = trimmed[colon_pos + 2..].trim().to_string();

            if let Some(&idx) = seen_nodes.get(&name) {
                nodes[idx].label = label;
                if !meta_icon.is_empty() {
                    nodes[idx].icon = meta_icon.clone();
                }
                if meta_pos.is_some() {
                    nodes[idx].grid_pos = meta_pos;
                }
            } else {
                seen_nodes.insert(name.clone(), nodes.len());
                nodes.push(DiagramNode {
                    name,
                    label,
                    icon: meta_icon.clone(),
                    grid_pos: meta_pos,
                    reveal,
                });
            }
        } else {
            // Plain node name (e.g. "Server" or "Server (icon: server, pos: 1,1)")
            let name = trimmed.trim().to_string();
            if !name.is_empty() {
                if let Some(&idx) = seen_nodes.get(&name) {
                    if !meta_icon.is_empty() {
                        nodes[idx].icon = meta_icon.clone();
                    }
                    if meta_pos.is_some() {
                        nodes[idx].grid_pos = meta_pos;
                    }
                } else {
                    seen_nodes.insert(name.clone(), nodes.len());
                    nodes.push(DiagramNode {
                        name: name.clone(),
                        label: name,
                        icon: meta_icon.clone(),
                        grid_pos: meta_pos,
                        reveal,
                    });
                }
            }
        }
    }

    (nodes, edges)
}

// ─── Diagram layout ──────────────────────────────────────────────────────────

struct NodeLayout {
    center_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
}

fn layout_nodes(
    nodes: &[DiagramNode],
    area_width: f32,
    area_height: f32,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
) -> (Vec<NodeLayout>, GridInfo) {
    let has_grid = nodes.iter().any(|n| n.grid_pos.is_some());

    if has_grid {
        layout_grid(nodes, area_width, area_height, origin_x, origin_y, scale)
    } else {
        layout_auto(nodes, area_width, area_height, origin_x, origin_y, scale)
    }
}

fn layout_grid(
    nodes: &[DiagramNode],
    area_width: f32,
    area_height: f32,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
) -> (Vec<NodeLayout>, GridInfo) {
    // Find grid dimensions from pos values
    let mut max_col: u32 = 1;
    let mut max_row: u32 = 1;
    for node in nodes {
        if let Some((col, row)) = node.grid_pos {
            max_col = max_col.max(col);
            max_row = max_row.max(row);
        }
    }

    let cell_w = area_width / max_col as f32;
    let cell_h = area_height / max_row as f32;

    // Responsive node sizes: fill a fraction of each cell, with min/max bounds
    let node_w = (cell_w * 0.65).clamp(100.0 * scale, 220.0 * scale);
    let node_h = (cell_h * 0.6).clamp(80.0 * scale, 160.0 * scale);

    // Assign unpositioned nodes to first available cells
    let mut occupied: Vec<(u32, u32)> = nodes.iter().filter_map(|n| n.grid_pos).collect();
    let mut next_unplaced = 0u32;

    let layouts = nodes
        .iter()
        .map(|node| {
            let (col, row) = node.grid_pos.unwrap_or_else(|| {
                // Find next unoccupied cell
                loop {
                    let c = next_unplaced % max_col + 1;
                    let r = next_unplaced / max_col + 1;
                    next_unplaced += 1;
                    if !occupied.contains(&(c, r)) {
                        occupied.push((c, r));
                        return (c, r);
                    }
                }
            });

            let cx = (col as f32 - 0.5) * cell_w;
            let cy = (row as f32 - 0.5) * cell_h;

            NodeLayout {
                center_x: cx,
                center_y: cy,
                width: node_w,
                height: node_h,
            }
        })
        .collect();

    // Build occupied set (convert 1-based grid_pos to 0-based)
    let occupied_set: HashSet<(usize, usize)> = occupied
        .iter()
        .map(|&(c, r)| ((c - 1) as usize, (r - 1) as usize))
        .collect();

    let grid_info = GridInfo {
        cols: max_col as usize,
        rows: max_row as usize,
        cell_w,
        cell_h,
        origin_x,
        origin_y,
        occupied: occupied_set,
    };

    (layouts, grid_info)
}

fn layout_auto(
    nodes: &[DiagramNode],
    area_width: f32,
    area_height: f32,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
) -> (Vec<NodeLayout>, GridInfo) {
    let n = nodes.len();
    if n == 0 {
        let grid_info = GridInfo {
            cols: 1,
            rows: 1,
            cell_w: area_width,
            cell_h: area_height,
            origin_x,
            origin_y,
            occupied: HashSet::new(),
        };
        return (Vec::new(), grid_info);
    }

    // For small node counts, use a single row
    if n <= 5 {
        // Responsive: size nodes to fill available space
        let max_node_w = (area_width / n as f32 * 0.6).clamp(100.0 * scale, 240.0 * scale);
        let node_h = (area_height * 0.4).clamp(80.0 * scale, 220.0 * scale);
        let node_w = max_node_w.min(node_h * 1.4); // keep reasonable aspect ratio

        let gap = if n > 1 {
            ((area_width - n as f32 * node_w) / (n - 1) as f32).max(20.0 * scale)
        } else {
            0.0
        };
        let total_w = n as f32 * node_w + (n - 1).max(0) as f32 * gap;
        let start_x = (area_width - total_w) / 2.0 + node_w / 2.0;

        let cell_w = if n > 1 {
            area_width / n as f32
        } else {
            area_width
        };

        let layouts = nodes
            .iter()
            .enumerate()
            .map(|(i, _)| NodeLayout {
                center_x: start_x + i as f32 * (node_w + gap),
                center_y: area_height / 2.0,
                width: node_w,
                height: node_h,
            })
            .collect();

        let grid_info = GridInfo {
            cols: n,
            rows: 1,
            cell_w,
            cell_h: area_height,
            origin_x,
            origin_y,
            occupied: (0..n).map(|i| (i, 0)).collect(),
        };

        return (layouts, grid_info);
    }

    // For larger counts, arrange in a grid pattern
    let cols = ((n as f32).sqrt().ceil() as usize).max(2);
    let rows = n.div_ceil(cols);

    let cell_w = area_width / cols as f32;
    let cell_h = area_height / rows as f32;

    // Responsive node sizes
    let node_w = (cell_w * 0.65).clamp(100.0 * scale, 220.0 * scale);
    let node_h = (cell_h * 0.6).clamp(80.0 * scale, 160.0 * scale);

    let layouts = nodes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let col = i % cols;
            let row = i / cols;

            NodeLayout {
                center_x: (col as f32 + 0.5) * cell_w,
                center_y: (row as f32 + 0.5) * cell_h,
                width: node_w,
                height: node_h,
            }
        })
        .collect();

    // In auto-layout, all cells up to n are occupied; remaining may be empty
    let occupied: HashSet<(usize, usize)> = (0..n).map(|i| (i % cols, i / cols)).collect();

    let grid_info = GridInfo {
        cols,
        rows,
        cell_w,
        cell_h,
        origin_x,
        origin_y,
        occupied,
    };

    (layouts, grid_info)
}

// ─── Geometric icon fallbacks ────────────────────────────────────────────────

fn draw_icon_fallback(
    painter: &egui::Painter,
    icon: &str,
    center: Pos2,
    size: f32,
    color: Color32,
    stroke_width: f32,
) {
    let s = size * 0.4; // icon draws within this radius
    let stroke = Stroke::new(stroke_width, color);

    match icon {
        "user" => {
            // Circle head
            let head_r = s * 0.35;
            let head_center = Pos2::new(center.x, center.y - s * 0.25);
            painter.circle_stroke(head_center, head_r, stroke);
            // Body arc (shoulders)
            let body_top = center.y + s * 0.15;
            let body_w = s * 0.6;
            let pts: Vec<Pos2> = (0..=8)
                .map(|i| {
                    let t = std::f32::consts::PI * i as f32 / 8.0;
                    Pos2::new(
                        center.x - body_w * t.cos(),
                        body_top + body_w * 0.5 * t.sin(),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        "server" => {
            // Stacked rectangles
            let w = s * 0.7;
            let h = s * 0.25;
            for i in 0..3 {
                let y = center.y - s * 0.45 + i as f32 * (h + 2.0);
                let rect = egui::Rect::from_center_size(
                    Pos2::new(center.x, y + h / 2.0),
                    egui::vec2(w * 2.0, h),
                );
                painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Outside);
                // Small indicator dot
                painter.circle_filled(
                    Pos2::new(rect.right() - h * 0.4, rect.center().y),
                    h * 0.15,
                    color,
                );
            }
        }
        "database" => {
            // Cylinder: top ellipse + sides + bottom ellipse
            let w = s * 0.6;
            let h = s * 0.7;
            let ey = s * 0.2; // ellipse vertical radius
            let top_y = center.y - h / 2.0;
            let bot_y = center.y + h / 2.0;

            // Side lines
            painter.line_segment(
                [
                    Pos2::new(center.x - w, top_y),
                    Pos2::new(center.x - w, bot_y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + w, top_y),
                    Pos2::new(center.x + w, bot_y),
                ],
                stroke,
            );

            // Top ellipse (full)
            let top_pts: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let t = 2.0 * std::f32::consts::PI * i as f32 / 20.0;
                    Pos2::new(center.x + w * t.cos(), top_y + ey * t.sin())
                })
                .collect();
            painter.add(egui::Shape::line(top_pts, stroke));

            // Bottom ellipse (half, lower arc only)
            let bot_pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = std::f32::consts::PI * i as f32 / 10.0;
                    Pos2::new(center.x - w * t.cos(), bot_y + ey * t.sin())
                })
                .collect();
            painter.add(egui::Shape::line(bot_pts, stroke));
        }
        "cloud" => {
            // Overlapping circles
            let r = s * 0.28;
            let offsets = [
                (-0.35, 0.1),
                (0.35, 0.1),
                (0.0, -0.2),
                (-0.2, 0.0),
                (0.2, 0.0),
            ];
            for (dx, dy) in offsets {
                painter.circle_stroke(Pos2::new(center.x + s * dx, center.y + s * dy), r, stroke);
            }
        }
        "lock" => {
            // Padlock: rectangle body + arc shackle
            let body_w = s * 0.6;
            let body_h = s * 0.5;
            let body_top = center.y;
            let body_rect = egui::Rect::from_min_size(
                Pos2::new(center.x - body_w, body_top),
                egui::vec2(body_w * 2.0, body_h),
            );
            painter.rect_stroke(body_rect, 3.0, stroke, egui::StrokeKind::Outside);

            // Shackle arc
            let shackle_pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = std::f32::consts::PI * i as f32 / 10.0;
                    Pos2::new(
                        center.x + body_w * 0.6 * t.cos(),
                        body_top - body_w * 0.6 * t.sin(),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(shackle_pts, stroke));
        }
        "api" => {
            // Hexagon
            let r = s * 0.55;
            let pts: Vec<Pos2> = (0..6)
                .map(|i| {
                    let angle =
                        std::f32::consts::PI / 6.0 + std::f32::consts::PI * 2.0 * i as f32 / 6.0;
                    Pos2::new(center.x + r * angle.cos(), center.y + r * angle.sin())
                })
                .collect();
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        "cache" => {
            // Lightning bolt
            let pts = vec![
                Pos2::new(center.x + s * 0.1, center.y - s * 0.5),
                Pos2::new(center.x - s * 0.2, center.y + s * 0.05),
                Pos2::new(center.x + s * 0.05, center.y + s * 0.05),
                Pos2::new(center.x - s * 0.1, center.y + s * 0.5),
            ];
            painter.add(egui::Shape::line(
                pts,
                Stroke::new(stroke_width * 1.5, color),
            ));
        }
        "queue" | "mail" => {
            // Envelope shape
            let w = s * 0.65;
            let h = s * 0.45;
            let rect = egui::Rect::from_center_size(center, egui::vec2(w * 2.0, h * 2.0));
            painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Outside);
            // V flap
            painter.add(egui::Shape::line(
                vec![
                    rect.left_top(),
                    Pos2::new(center.x, center.y + h * 0.3),
                    rect.right_top(),
                ],
                stroke,
            ));
        }
        "monitor" | "browser" => {
            // Monitor/screen
            let w = s * 0.7;
            let h = s * 0.5;
            let screen = egui::Rect::from_center_size(
                Pos2::new(center.x, center.y - s * 0.1),
                egui::vec2(w * 2.0, h * 2.0),
            );
            painter.rect_stroke(screen, 3.0, stroke, egui::StrokeKind::Outside);
            // Stand
            let stand_y = screen.bottom() + 2.0;
            painter.line_segment(
                [
                    Pos2::new(center.x, stand_y),
                    Pos2::new(center.x, stand_y + s * 0.25),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - s * 0.35, stand_y + s * 0.25),
                    Pos2::new(center.x + s * 0.35, stand_y + s * 0.25),
                ],
                stroke,
            );
        }
        "mobile" => {
            // Phone outline
            let w = s * 0.35;
            let h = s * 0.7;
            let rect = egui::Rect::from_center_size(center, egui::vec2(w * 2.0, h * 2.0));
            painter.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Outside);
            // Home button
            painter.circle_stroke(
                Pos2::new(center.x, rect.bottom() - s * 0.15),
                s * 0.08,
                stroke,
            );
        }
        "storage" | "container" => {
            // Nested rectangles
            let outer = egui::Rect::from_center_size(center, egui::vec2(s * 1.2, s * 1.0));
            let inner = egui::Rect::from_center_size(center, egui::vec2(s * 0.7, s * 0.55));
            painter.rect_stroke(outer, 3.0, stroke, egui::StrokeKind::Outside);
            painter.rect_stroke(inner, 2.0, stroke, egui::StrokeKind::Outside);
        }
        "function" => {
            // f(x) — lambda symbol
            let font = FontId::new(s * 1.2, FontFamily::Monospace);
            let galley = painter.layout_no_wrap("λ".to_string(), font, color);
            let text_pos = Pos2::new(
                center.x - galley.rect.width() / 2.0,
                center.y - galley.rect.height() / 2.0,
            );
            painter.galley(text_pos, galley, color);
        }
        "network" => {
            // Three connected dots
            let positions = [
                Pos2::new(center.x, center.y - s * 0.4),
                Pos2::new(center.x - s * 0.4, center.y + s * 0.3),
                Pos2::new(center.x + s * 0.4, center.y + s * 0.3),
            ];
            for &p in &positions {
                painter.circle_filled(p, s * 0.12, color);
            }
            for i in 0..3 {
                painter.line_segment([positions[i], positions[(i + 1) % 3]], stroke);
            }
        }
        "key" => {
            // Key shape: circle + stem
            let head_r = s * 0.25;
            let head_center = Pos2::new(center.x - s * 0.2, center.y);
            painter.circle_stroke(head_center, head_r, stroke);
            let stem_start = Pos2::new(head_center.x + head_r, center.y);
            let stem_end = Pos2::new(center.x + s * 0.5, center.y);
            painter.line_segment([stem_start, stem_end], stroke);
            // Teeth
            painter.line_segment(
                [
                    Pos2::new(stem_end.x - s * 0.1, center.y),
                    Pos2::new(stem_end.x - s * 0.1, center.y + s * 0.15),
                ],
                stroke,
            );
            painter.line_segment(
                [stem_end, Pos2::new(stem_end.x, center.y + s * 0.15)],
                stroke,
            );
        }
        "logs" => {
            // Stacked lines (like a document)
            let w = s * 0.55;
            let rect = egui::Rect::from_center_size(center, egui::vec2(w * 2.0, s * 1.2));
            painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Outside);
            for i in 0..4 {
                let y = rect.top() + s * 0.2 + i as f32 * s * 0.25;
                let line_w = if i == 2 { w * 1.2 } else { w * 1.6 };
                painter.line_segment(
                    [
                        Pos2::new(center.x - line_w / 2.0, y),
                        Pos2::new(center.x + line_w / 2.0, y),
                    ],
                    Stroke::new(stroke_width * 0.7, color),
                );
            }
        }
        _ => {
            // Default: simple rounded rectangle
            let rect = egui::Rect::from_center_size(center, egui::vec2(s * 1.0, s * 0.8));
            painter.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Outside);
        }
    }
}

// ─── Debug info ──────────────────────────────────────────────────────────────

/// Generate a structured text summary of diagram nodes, edges, and routing results.
/// Used by the debug overlay to show routing engine inputs/outputs.
pub fn diagram_debug_info(content: &str) -> String {
    use std::fmt::Write;

    let (nodes, edges) = parse_diagram(content);
    if nodes.is_empty() {
        return "No nodes parsed.".to_string();
    }

    // Determine grid positions using the same logic as layout_nodes:
    // if any node has grid_pos, use explicit placement; otherwise auto-layout.
    let has_grid = nodes.iter().any(|n| n.grid_pos.is_some());

    // Assign (col, row) to each node (1-based, matching routing convention)
    let positions: Vec<(i32, i32)> = if has_grid {
        let mut max_col: u32 = 1;
        let mut max_row: u32 = 1;
        for node in &nodes {
            if let Some((c, r)) = node.grid_pos {
                max_col = max_col.max(c);
                max_row = max_row.max(r);
            }
        }
        let mut occupied: Vec<(u32, u32)> = nodes.iter().filter_map(|n| n.grid_pos).collect();
        let mut next_unplaced = 0u32;
        nodes
            .iter()
            .map(|node| {
                let (c, r) = node.grid_pos.unwrap_or_else(|| {
                    loop {
                        let c = next_unplaced % max_col + 1;
                        let r = next_unplaced / max_col + 1;
                        next_unplaced += 1;
                        if !occupied.contains(&(c, r)) {
                            occupied.push((c, r));
                            return (c, r);
                        }
                    }
                });
                (c as i32, r as i32)
            })
            .collect()
    } else {
        let n = nodes.len();
        if n <= 5 {
            // Single row
            (0..n).map(|i| (i as i32 + 1, 1)).collect()
        } else {
            // Grid
            let cols = ((n as f32).sqrt().ceil() as usize).max(2);
            (0..n)
                .map(|i| ((i % cols) as i32 + 1, (i / cols) as i32 + 1))
                .collect()
        }
    };

    let mut out = String::new();

    // NODES section
    writeln!(out, "NODES ({}):", nodes.len()).unwrap();
    for (i, node) in nodes.iter().enumerate() {
        let (c, r) = positions[i];
        writeln!(out, "  {} @ ({},{})", node.name, c, r).unwrap();
    }

    // EDGES section
    writeln!(out).unwrap();
    writeln!(out, "EDGES ({}):", edges.len()).unwrap();
    for edge in &edges {
        let arrow_str = match edge.arrow {
            ArrowKind::Forward => "->",
            ArrowKind::Reverse => "<-",
            ArrowKind::Bidirectional => "<->",
            ArrowKind::DashedLine => "--",
            ArrowKind::DashedArrow => "-->",
        };
        if edge.label.is_empty() {
            writeln!(out, "  {} {} {}", edge.from, arrow_str, edge.to).unwrap();
        } else {
            writeln!(
                out,
                "  {} {} {} \"{}\"",
                edge.from, arrow_str, edge.to, edge.label
            )
            .unwrap();
        }
    }

    // Build routing types and run the routing engine
    let routing_nodes: Vec<routing::types::DiagramNode> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let (c, r) = positions[i];
            routing::types::DiagramNode {
                name: node.name.clone(),
                col: c,
                row: r,
            }
        })
        .collect();

    let routing_edges: Vec<routing::types::DiagramEdge> = edges
        .iter()
        .map(|e| routing::types::DiagramEdge {
            source: e.from.clone(),
            target: e.to.clone(),
            label: if e.label.is_empty() {
                None
            } else {
                Some(e.label.clone())
            },
        })
        .collect();

    let config = routing::types::RoutingConfig::default();
    writeln!(out).unwrap();
    writeln!(
        out,
        "CONFIG: h_lanes={}, v_lanes={}",
        config.h_lane_capacity, config.v_lane_capacity
    )
    .unwrap();

    let routing_output = routing::route_all_edges(&routing_nodes, &routing_edges, &config);

    writeln!(out).unwrap();
    writeln!(out, "ROUTING RESULTS:").unwrap();
    for (edge, result) in &routing_output.results {
        match result {
            routing::types::RouteResult::Success(route) => {
                let label = edge
                    .label
                    .as_deref()
                    .map_or(String::new(), |l| format!(" \"{l}\""));
                writeln!(
                    out,
                    "  {} -> {}{}: OK len={:.1} turns={} lc={} cx={}",
                    edge.source,
                    edge.target,
                    label,
                    route.complexity.length,
                    route.complexity.turns,
                    route.complexity.lane_changes,
                    route.complexity.crossings,
                )
                .unwrap();
                let mut route_str = String::new();
                for (i, w) in route.waypoints.iter().enumerate() {
                    if i > 0 {
                        // Lane label goes between coordinates (on the segment)
                        let prev = &route.waypoints[i - 1];
                        route_str.push_str(&format!(" L{} → ", prev.lane));
                    }
                    route_str.push_str(&format!("{}", w.coord));
                }
                writeln!(out, "    {route_str}").unwrap();
            }
            routing::types::RouteResult::Failure { warning } => {
                let label = edge
                    .label
                    .as_deref()
                    .map_or(String::new(), |l| format!(" \"{l}\""));
                writeln!(
                    out,
                    "  {} -> {}{}: FAIL: {}",
                    edge.source, edge.target, label, warning
                )
                .unwrap();
            }
        }
    }

    out
}

// ─── Diagram renderer ────────────────────────────────────────────────────────

/// Count the number of reveal steps (`+` markers) in a diagram content string.
pub fn count_diagram_steps(content: &str) -> usize {
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("+ ") {
            count += 1;
        }
    }
    count
}

/// Draw a diagram parsed from `- Node: label` and `- A -> B: label` lines.
#[allow(clippy::too_many_arguments)]
/// Draw a diagram. `max_height` controls the vertical space; pass 0 for a default.
pub fn draw_diagram_sized(
    ui: &egui::Ui,
    content: &str,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) -> f32 {
    let (nodes, edges) = parse_diagram(content);

    // Compute reveal step assignments for each element.
    // Static elements are always visible (step 0). Each `+` increments the step counter.
    // `*` elements share the same step as the previous `+`.
    // Process all elements (nodes then edges) — this matches parse_diagram ordering
    // since it collects all nodes first from declarations, then edges.
    let mut step_counter = 0usize;
    let node_steps: Vec<usize> = nodes
        .iter()
        .map(|n| match n.reveal {
            DiagramReveal::Static => 0,
            DiagramReveal::NextStep => {
                step_counter += 1;
                step_counter
            }
            DiagramReveal::WithPrev => step_counter,
        })
        .collect();
    let edge_steps: Vec<usize> = edges
        .iter()
        .map(|e| match e.reveal {
            DiagramReveal::Static => 0,
            DiagramReveal::NextStep => {
                step_counter += 1;
                step_counter
            }
            DiagramReveal::WithPrev => step_counter,
        })
        .collect();

    if nodes.is_empty() {
        // Fallback for unparseable diagrams
        let color = Theme::with_opacity(theme.foreground, opacity * 0.6);
        let bg = Theme::with_opacity(theme.code_background, opacity);
        let height = 200.0 * scale;
        let rect = egui::Rect::from_min_size(pos, egui::vec2(max_width, height));
        ui.painter().rect_filled(rect, 8.0 * scale, bg);
        let galley = ui.painter().layout(
            "[Diagram]".to_string(),
            FontId::proportional(theme.body_size * 0.8 * scale),
            color,
            max_width,
        );
        let text_pos = Pos2::new(
            pos.x + (max_width - galley.rect.width()) / 2.0,
            pos.y + (height - galley.rect.height()) / 2.0,
        );
        ui.painter().galley(text_pos, galley, color);
        return height;
    }

    // Use the provided max_height, or default to 500px
    let diagram_height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };
    let padding = 30.0 * scale;
    let area_width = max_width - padding * 2.0;
    let area_height = diagram_height - padding * 2.0;

    let abs_origin_x = pos.x + padding;
    let abs_origin_y = pos.y + padding;
    let (layouts, grid) = layout_nodes(
        &nodes,
        area_width,
        area_height,
        abs_origin_x,
        abs_origin_y,
        scale,
    );

    // Build name -> layout index map and compute absolute positions
    let mut node_rects: HashMap<String, egui::Rect> = HashMap::new();
    let painter = ui.painter();

    let accent = theme.accent;
    let node_border_color = Theme::with_opacity(accent, opacity * 0.8);
    let node_fill = Theme::with_opacity(theme.code_background, opacity * 0.95);
    let shadow_color = Theme::with_opacity(Color32::from_rgb(0, 0, 0), opacity * 0.1);
    let label_color = Theme::with_opacity(theme.foreground, opacity);
    let icon_color = Theme::with_opacity(accent, opacity * 0.9);

    // Draw nodes (skip those not yet revealed)
    for (i, node) in nodes.iter().enumerate() {
        // Always compute rect for routing, but skip drawing if not revealed
        let layout = &layouts[i];
        let abs_x = pos.x + padding + layout.center_x;
        let abs_y = pos.y + padding + layout.center_y;

        let node_rect = egui::Rect::from_center_size(
            egui::pos2(abs_x, abs_y),
            egui::vec2(layout.width, layout.height),
        );
        // Always register node rect (needed for edge routing even if not visible)
        node_rects.insert(node.name.clone(), node_rect);

        // Skip drawing if this node hasn't been revealed yet
        let node_step = node_steps.get(i).copied().unwrap_or(0);
        if node_step > reveal_step {
            continue;
        }

        let corner_radius = 8.0 * scale;

        // Drop shadow
        let shadow_rect = node_rect.translate(egui::vec2(3.0 * scale, 3.0 * scale));
        painter.rect_filled(shadow_rect, corner_radius, shadow_color);

        // Node background
        painter.rect_filled(node_rect, corner_radius, node_fill);

        // Node border
        painter.rect_stroke(
            node_rect,
            corner_radius,
            Stroke::new(2.5 * scale, node_border_color),
            egui::StrokeKind::Outside,
        );

        // Icon area (top portion of node)
        let icon_size = layout.height * 0.5;
        let icon_center = Pos2::new(abs_x, abs_y - layout.height * 0.12);

        // Try loading icon image from media/diagram-icons/{icon}.png
        let icon_path = if !node.icon.is_empty() {
            format!("media/diagram-icons/{}.png", node.icon)
        } else {
            String::new()
        };

        let has_image = if !icon_path.is_empty() {
            if let Some(texture) = image_cache.get_or_load(ui, &icon_path) {
                // Draw the icon image
                let img_size = icon_size * 0.85;
                let img_rect =
                    egui::Rect::from_center_size(icon_center, egui::vec2(img_size, img_size));
                let tint = Theme::with_opacity(Color32::WHITE, opacity);
                painter.image(
                    texture.id(),
                    img_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
                true
            } else {
                false
            }
        } else {
            false
        };

        if !has_image {
            // Draw geometric fallback icon
            let icon_name = if node.icon.is_empty() {
                "box"
            } else {
                &node.icon
            };
            draw_icon_fallback(
                painter,
                icon_name,
                icon_center,
                icon_size,
                icon_color,
                2.0 * scale,
            );
        }

        // Label text below icon
        let label_font_size = theme.body_size * 0.55 * scale;
        let galley = painter.layout(
            node.label.clone(),
            FontId::proportional(label_font_size),
            label_color,
            layout.width - 8.0 * scale,
        );
        let text_y = abs_y + layout.height * 0.25;
        let text_pos = egui::pos2(abs_x - galley.rect.width() / 2.0, text_y);
        painter.galley(text_pos, galley, label_color);
    }

    // ── Draw edges (orthogonal routing) ────────────────────────────────────

    let edge_palette = theme.edge_palette();
    let label_bg = Theme::with_opacity(theme.code_background, opacity * 0.9);
    let label_text_color = Theme::with_opacity(theme.foreground, opacity * 0.8);
    let line_width = 4.0 * scale;
    let arrow_size = 20.0 * scale;
    let node_margin = 10.0 * scale;
    let corner_radius = 10.0 * scale;
    let lane_spacing = 20.0 * scale;
    let port_spacing = 22.0 * scale;

    let animation_duration = 0.4; // seconds
    let mut needs_repaint = false;

    // Build routing input from diagram data
    // Filter to only visible edges and collect their grid positions
    let visible_edges: Vec<(usize, &DiagramEdge)> = edges
        .iter()
        .enumerate()
        .filter(|(edge_idx, edge)| {
            let edge_step = edge_steps.get(*edge_idx).copied().unwrap_or(0);
            if edge_step > reveal_step {
                return false;
            }
            // Skip self-loops and edges with missing nodes
            if edge.from == edge.to {
                return false;
            }
            node_rects.contains_key(&edge.from) && node_rects.contains_key(&edge.to)
        })
        .collect();

    // Build routing nodes from the parsed diagram nodes that have grid positions
    let routing_nodes: Vec<routing::types::DiagramNode> = nodes
        .iter()
        .filter_map(|n| {
            let rect = node_rects.get(&n.name)?;
            let center = rect.center();
            let (col, row) = grid.cell_at(center)?;
            Some(routing::types::DiagramNode {
                name: n.name.clone(),
                col: (col + 1) as i32, // convert 0-indexed to 1-based
                row: (row + 1) as i32,
            })
        })
        .collect();

    let routing_edges: Vec<routing::types::DiagramEdge> = visible_edges
        .iter()
        .map(|(_, edge)| routing::types::DiagramEdge {
            source: edge.from.clone(),
            target: edge.to.clone(),
            label: if edge.label.is_empty() {
                None
            } else {
                Some(edge.label.clone())
            },
        })
        .collect();

    let config = routing::types::RoutingConfig {
        h_lane_capacity: compute_h_capacity(&grid, &node_rects, lane_spacing),
        v_lane_capacity: compute_v_capacity(&grid, &node_rects, lane_spacing),
        weights: *ROUTING_WEIGHTS,
    };

    // Use cached routing output — only recompute when inputs change.
    let cache_key = route_cache_key(&routing_nodes, &routing_edges, &config);
    let routing_output = ROUTE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.as_ref() {
            if entry.key == cache_key {
                return entry.output.clone();
            }
        }
        let output = routing::route_all_edges(&routing_nodes, &routing_edges, &config);
        *cache = Some(RouteCacheEntry {
            key: cache_key,
            output: output.clone(),
        });
        output
    });

    // Track port usage per (node, face) to spread connections
    let mut port_counts: HashMap<(String, Face), usize> = HashMap::new();
    let claim_port = |counts: &mut HashMap<(String, Face), usize>,
                      node_name: &str,
                      face: Face,
                      rect: &egui::Rect|
     -> f32 {
        let key = (node_name.to_string(), face);
        let idx = counts.entry(key).or_insert(0);
        let current = *idx;
        *idx += 1;

        if current == 0 {
            return 0.0;
        }

        let face_length = match face {
            Face::Left | Face::Right => rect.height(),
            Face::Top | Face::Bottom => rect.width(),
        };
        let max_offset = face_length * 0.3;
        let level = current.div_ceil(2);
        let sign = if current % 2 == 1 { 1.0 } else { -1.0 };
        let offset = sign * level as f32 * port_spacing;
        offset.clamp(-max_offset, max_offset)
    };

    // Draw each routed edge
    for (result_idx, (_, route_result)) in routing_output.results.iter().enumerate() {
        let (edge_idx, edge) = visible_edges[result_idx];

        let from_rect = &node_rects[&edge.from];
        let to_rect = &node_rects[&edge.to];

        // Each edge gets a distinct color from the palette
        let base_color = edge_palette[edge_idx % edge_palette.len()];
        let is_dashed = matches!(edge.arrow, ArrowKind::DashedLine | ArrowKind::DashedArrow);
        let current_edge_color = if is_dashed {
            Theme::with_opacity(base_color, opacity * 0.55)
        } else {
            Theme::with_opacity(base_color, opacity * 0.85)
        };

        // Compute animation progress for edges appearing on the current step
        let edge_step = edge_steps.get(edge_idx).copied().unwrap_or(0);
        let anim_progress = if edge_step == reveal_step && edge_step > 0 {
            if let Some(ts) = reveal_timestamp {
                let elapsed = ts.elapsed().as_secs_f32();
                let t = (elapsed / animation_duration).min(1.0);
                let eased = if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0_f32 * t + 2.0).powi(2) / 2.0
                };
                if t < 1.0 {
                    needs_repaint = true;
                }
                eased
            } else {
                1.0
            }
        } else {
            1.0
        };

        let pixel_waypoints = match route_result {
            routing::types::RouteResult::Success(route) => {
                // Determine faces from route direction
                let first = &route.waypoints[0];
                let second = &route.waypoints[1];
                let exit_dir = coord_direction(first.coord, second.coord);
                let exit_face = direction_to_face(exit_dir);

                let n = route.waypoints.len();
                let penult = &route.waypoints[n - 2];
                let last = &route.waypoints[n - 1];
                let entry_dir = coord_direction(penult.coord, last.coord);
                let entry_face = direction_to_face(entry_dir.opposite());

                // Derive port offsets from lane so face points align with corridor
                let exit_lane = first.lane;
                let (elx, ely) = lane_offset(exit_dir, exit_lane, lane_spacing);
                let port_start = match exit_face {
                    Face::Left | Face::Right => ely,
                    Face::Top | Face::Bottom => elx,
                };

                let entry_lane = penult.lane;
                let (nlx, nly) = lane_offset(entry_dir, entry_lane, lane_spacing);
                let port_end = match entry_face {
                    Face::Left | Face::Right => nly,
                    Face::Top | Face::Bottom => nlx,
                };

                waypoints_to_pixels(
                    route,
                    &grid,
                    from_rect,
                    to_rect,
                    node_margin,
                    lane_spacing,
                    port_start,
                    port_end,
                )
            }
            routing::types::RouteResult::Failure { warning } => {
                eprintln!("ROUTE WARNING: {warning}");
                // Fallback: direct connection
                let exit_face = choose_exit_face(from_rect, to_rect.center());
                let entry_face = choose_entry_face(to_rect, from_rect.center());
                let port_start = claim_port(&mut port_counts, &edge.from, exit_face, from_rect);
                let port_end = claim_port(&mut port_counts, &edge.to, entry_face, to_rect);
                let fp_start = face_point_with_port(from_rect, exit_face, port_start);
                let fp_end = face_point_with_port(to_rect, entry_face, port_end);
                let ramp_start = ramp_from_face(from_rect, exit_face, port_start, node_margin);
                let ramp_end = ramp_from_face(to_rect, entry_face, port_end, node_margin);
                vec![fp_start, ramp_start, ramp_end, fp_end]
            }
        };

        draw_routed_edge(
            painter,
            &pixel_waypoints,
            edge.arrow,
            &edge.label,
            current_edge_color,
            label_bg,
            label_text_color,
            line_width,
            arrow_size,
            corner_radius,
            theme,
            scale,
            opacity,
            anim_progress,
        );
    }

    // Request repaint while edges are still animating
    if needs_repaint {
        ui.ctx().request_repaint();
    }

    diagram_height
}

// ─── Diagram tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod diagram_tests {
    use super::*;

    // ── Parsing tests ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_simple_chain() {
        let content = "- A -> B: sends\n- B -> C: forwards";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 3);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].from, "A");
        assert_eq!(edges[0].to, "B");
        assert_eq!(edges[0].label, "sends");
        assert!(matches!(edges[0].arrow, ArrowKind::Forward));
    }

    #[test]
    fn test_skip_comments() {
        let content = "# Components\n- A -> B\n# Relationships\n- B -> C";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 3);
        assert_eq!(edges.len(), 2);
        assert!(!nodes.iter().any(|n| n.name.starts_with('#')));
    }

    #[test]
    fn test_parse_metadata() {
        let (before, icon, pos) = parse_metadata("Server (icon: server, pos: 2, 3)");
        assert_eq!(before, "Server");
        assert_eq!(icon, "server");
        assert_eq!(pos, Some((2, 3)));
    }

    #[test]
    fn test_arrow_types() {
        let content = "A -> B\nC <- D\nE <-> F\nG -- H\nI --> J";
        let (_, edges) = parse_diagram(content);
        assert!(matches!(edges[0].arrow, ArrowKind::Forward));
        assert!(matches!(edges[1].arrow, ArrowKind::Reverse));
        assert!(matches!(edges[2].arrow, ArrowKind::Bidirectional));
        assert!(matches!(edges[3].arrow, ArrowKind::DashedLine));
        assert!(matches!(edges[4].arrow, ArrowKind::DashedArrow));
    }

    #[test]
    fn test_node_with_label_and_metadata() {
        let content = "- DB: Database (icon: database, pos: 1, 2)";
        let (nodes, _) = parse_diagram(content);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "DB");
        assert_eq!(nodes[0].label, "Database");
        assert_eq!(nodes[0].icon, "database");
        assert_eq!(nodes[0].grid_pos, Some((1, 2)));
    }

    #[test]
    fn test_detect_arrow_ordering() {
        // <-> must be detected before -> and <-
        assert!(matches!(
            detect_arrow("A <-> B"),
            Some((_, _, ArrowKind::Bidirectional))
        ));
        assert!(matches!(
            detect_arrow("A --> B"),
            Some((_, _, ArrowKind::DashedArrow))
        ));
        assert!(matches!(
            detect_arrow("A -> B"),
            Some((_, _, ArrowKind::Forward))
        ));
        assert!(matches!(
            detect_arrow("A <- B"),
            Some((_, _, ArrowKind::Reverse))
        ));
        assert!(matches!(
            detect_arrow("A -- B"),
            Some((_, _, ArrowKind::DashedLine))
        ));
        assert!(detect_arrow("A B").is_none());
    }

    #[test]
    fn test_empty_diagram() {
        let (nodes, edges) = parse_diagram("");
        assert_eq!(nodes.len(), 0);
        assert_eq!(edges.len(), 0);
    }

    #[test]
    fn test_comments_only() {
        let (nodes, edges) = parse_diagram("# comment\n# another");
        assert_eq!(nodes.len(), 0);
        assert_eq!(edges.len(), 0);
    }

    #[test]
    fn test_reveal_markers_parsed() {
        let content = "- A (pos: 1, 1)\n+ B (pos: 2, 1)\n* C (pos: 3, 1)";
        let (nodes, _) = parse_diagram(content);
        assert_eq!(nodes[0].reveal, DiagramReveal::Static);
        assert_eq!(nodes[1].reveal, DiagramReveal::NextStep);
        assert_eq!(nodes[2].reveal, DiagramReveal::WithPrev);
    }

    #[test]
    fn test_reveal_markers_on_edges() {
        let content = "- A -> B\n+ C -> D\n* E -> F";
        let (_, edges) = parse_diagram(content);
        assert_eq!(edges[0].reveal, DiagramReveal::Static);
        assert_eq!(edges[1].reveal, DiagramReveal::NextStep);
        assert_eq!(edges[2].reveal, DiagramReveal::WithPrev);
    }

    #[test]
    fn test_count_diagram_steps() {
        let content = "- A\n+ B\n+ C\n* D";
        assert_eq!(count_diagram_steps(content), 2);
    }

    #[test]
    fn test_count_diagram_steps_none() {
        let content = "- A\n- B\n- C";
        assert_eq!(count_diagram_steps(content), 0);
    }

    #[test]
    fn test_parse_diagram_whitespace() {
        let content = "  A -> B  ";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 2);
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_parse_diagram_mixed_definitions() {
        let content = "- Server: Web Server\n- Server -> DB: queries\n- DB: Database";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].label, "Web Server");
        assert_eq!(nodes[1].label, "Database");
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_parse_diagram_reverse_arrow() {
        let content = "A <- B";
        let (_, edges) = parse_diagram(content);
        assert!(matches!(edges[0].arrow, ArrowKind::Reverse));
        assert_eq!(edges[0].from, "A");
        assert_eq!(edges[0].to, "B");
    }

    #[test]
    fn test_parse_diagram_bidirectional() {
        let content = "A <-> B";
        let (_, edges) = parse_diagram(content);
        assert!(matches!(edges[0].arrow, ArrowKind::Bidirectional));
    }

    #[test]
    fn test_detect_arrow_none() {
        assert!(detect_arrow("no arrow here").is_none());
        assert!(detect_arrow("A B C").is_none());
    }

    #[test]
    fn test_detect_arrow_with_labels() {
        let result = detect_arrow("Client -> Server: HTTP");
        assert!(result.is_some());
        let (pos, len, kind) = result.unwrap();
        assert!(matches!(kind, ArrowKind::Forward));
        assert_eq!(&"Client -> Server: HTTP"[pos + 1..pos + len - 1], "->");
    }

    // ── Rendering geometry tests ─────────────────────────────────────────────

    #[test]
    fn test_apply_rounded_corners() {
        let pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
        ];
        let result = apply_rounded_corners(&pts, 10.0);
        assert!(result.len() > 3);
        assert_eq!(result[0], Pos2::new(0.0, 0.0));
        assert_eq!(*result.last().unwrap(), Pos2::new(100.0, 100.0));
    }

    #[test]
    fn test_rounded_corners_straight_line_unchanged() {
        let pts = vec![Pos2::new(0.0, 0.0), Pos2::new(100.0, 0.0)];
        let result = apply_rounded_corners(&pts, 10.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_rounded_corners_preserves_endpoints() {
        let pts = vec![
            Pos2::new(10.0, 20.0),
            Pos2::new(100.0, 20.0),
            Pos2::new(100.0, 100.0),
            Pos2::new(200.0, 100.0),
        ];
        let result = apply_rounded_corners(&pts, 8.0);
        assert_eq!(result[0], pts[0]);
        assert_eq!(*result.last().unwrap(), *pts.last().unwrap());
    }

    #[test]
    fn test_rounded_corners_zero_radius() {
        let pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
        ];
        let result = apply_rounded_corners(&pts, 0.0);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_polyline_length() {
        let pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 50.0),
        ];
        assert!((polyline_length(&pts) - 150.0).abs() < 0.1);
    }

    #[test]
    fn test_polyline_length_single_point() {
        assert_eq!(polyline_length(&[Pos2::new(5.0, 5.0)]), 0.0);
    }

    #[test]
    fn test_polyline_length_two_points() {
        let pts = [Pos2::new(0.0, 0.0), Pos2::new(3.0, 4.0)];
        assert!((polyline_length(&pts) - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_polyline_length_multi_segment() {
        let pts = [
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            Pos2::new(10.0, 10.0),
            Pos2::new(0.0, 10.0),
        ];
        assert!((polyline_length(&pts) - 30.0).abs() < 0.01);
    }

    // ── Face/ramp helper tests ───────────────────────────────────────────────

    #[test]
    fn test_face_selection_horizontal() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        assert!(matches!(
            choose_exit_face(&r, Pos2::new(200.0, 40.0)),
            Face::Right
        ));
        assert!(matches!(
            choose_exit_face(&r, Pos2::new(-100.0, 40.0)),
            Face::Left
        ));
    }

    #[test]
    fn test_face_selection_vertical() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        assert!(matches!(
            choose_exit_face(&r, Pos2::new(50.0, 200.0)),
            Face::Bottom
        ));
        assert!(matches!(
            choose_exit_face(&r, Pos2::new(50.0, -100.0)),
            Face::Top
        ));
    }

    #[test]
    fn test_face_selection_diagonal_right_down() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        // When dx and dy are equal, dx.abs() >= dy.abs() is true, so Right
        let face = choose_exit_face(&r, Pos2::new(200.0, 200.0));
        assert!(matches!(face, Face::Right | Face::Bottom));
    }

    #[test]
    fn test_entry_face_matches_direction() {
        let r = egui::Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(200.0, 180.0));
        assert!(matches!(
            choose_entry_face(&r, Pos2::new(0.0, 140.0)),
            Face::Left
        ));
        assert!(matches!(
            choose_entry_face(&r, Pos2::new(300.0, 140.0)),
            Face::Right
        ));
        assert!(matches!(
            choose_entry_face(&r, Pos2::new(150.0, 0.0)),
            Face::Top
        ));
        assert!(matches!(
            choose_entry_face(&r, Pos2::new(150.0, 300.0)),
            Face::Bottom
        ));
    }

    #[test]
    fn test_ramp_from_face_right() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        let ramp = ramp_from_face(&r, Face::Right, 0.0, 10.0);
        assert_eq!(ramp.x, 110.0);
        assert_eq!(ramp.y, 40.0);
    }

    #[test]
    fn test_ramp_from_face_left() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        let ramp = ramp_from_face(&r, Face::Left, 0.0, 10.0);
        assert_eq!(ramp.x, -10.0);
        assert_eq!(ramp.y, 40.0);
    }

    #[test]
    fn test_ramp_from_face_bottom() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        let ramp = ramp_from_face(&r, Face::Bottom, 0.0, 10.0);
        assert_eq!(ramp.x, 50.0);
        assert_eq!(ramp.y, 90.0);
    }

    #[test]
    fn test_ramp_from_face_top() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        let ramp = ramp_from_face(&r, Face::Top, 0.0, 10.0);
        assert_eq!(ramp.x, 50.0);
        assert_eq!(ramp.y, -10.0);
    }

    #[test]
    fn test_face_point_with_port_offset() {
        let r = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        let pt = face_point_with_port(&r, Face::Top, 5.0);
        assert_eq!(pt.x, 55.0); // center.x + 5
        assert_eq!(pt.y, 0.0);
    }

    // ── GridInfo tests ───────────────────────────────────────────────────────

    #[test]
    fn test_cell_at_origin() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 100.0,
            cell_h: 80.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        assert_eq!(grid.cell_at(Pos2::new(10.0, 10.0)), Some((0, 0)));
    }

    #[test]
    fn test_cell_at_center_cell() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 100.0,
            cell_h: 80.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        assert_eq!(grid.cell_at(Pos2::new(150.0, 40.0)), Some((1, 0)));
    }

    #[test]
    fn test_cell_at_out_of_bounds() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 100.0,
            cell_h: 80.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        assert_eq!(grid.cell_at(Pos2::new(-10.0, 10.0)), None);
        assert_eq!(grid.cell_at(Pos2::new(10.0, -10.0)), None);
        assert_eq!(grid.cell_at(Pos2::new(310.0, 10.0)), None);
        assert_eq!(grid.cell_at(Pos2::new(10.0, 170.0)), None);
    }

    #[test]
    fn test_is_cell_empty() {
        let mut occupied = HashSet::new();
        occupied.insert((1, 0));
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 100.0,
            cell_h: 80.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };
        assert!(grid.is_cell_empty(0, 0));
        assert!(!grid.is_cell_empty(1, 0));
        assert!(grid.is_cell_empty(2, 0));
    }

    #[test]
    fn test_corridor_positions() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 100.0,
            cell_h: 80.0,
            origin_x: 10.0,
            origin_y: 20.0,
            occupied: HashSet::new(),
        };
        assert_eq!(grid.h_corridor_y(0), 20.0);
        assert_eq!(grid.h_corridor_y(1), 100.0);
        assert_eq!(grid.h_corridor_y(2), 180.0);
        assert_eq!(grid.v_corridor_x(0), 10.0);
        assert_eq!(grid.v_corridor_x(1), 110.0);
        assert_eq!(grid.v_corridor_x(3), 310.0);
    }

    // ── Edge palette tests ───────────────────────────────────────────────────

    #[test]
    fn test_edge_palette_dark_has_entries() {
        let theme = Theme::dark();
        let palette = theme.edge_palette();
        assert!(!palette.is_empty());
        assert!(palette.len() >= 6);
    }

    #[test]
    fn test_edge_palette_light_has_entries() {
        let theme = Theme::light();
        let palette = theme.edge_palette();
        assert!(!palette.is_empty());
        assert!(palette.len() >= 6);
    }

    #[test]
    fn test_edge_palette_colors_are_distinct() {
        let theme = Theme::dark();
        let palette = theme.edge_palette();
        for i in 0..palette.len() {
            for j in i + 1..palette.len() {
                assert_ne!(
                    palette[i], palette[j],
                    "Colors at {i} and {j} should differ"
                );
            }
        }
    }

    // ── Ensure orthogonal tests ──────────────────────────────────────────────

    #[test]
    fn test_ensure_orthogonal_inserts_corner() {
        let mut pts = vec![Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0)];
        ensure_orthogonal(&mut pts);
        assert!(pts.len() >= 3);
        // All consecutive pairs should be axis-aligned
        for w in pts.windows(2) {
            let dx = (w[0].x - w[1].x).abs();
            let dy = (w[0].y - w[1].y).abs();
            assert!(dx < 1.5 || dy < 1.5, "Non-orthogonal segment: {w:?}");
        }
    }

    #[test]
    fn test_ensure_orthogonal_already_orthogonal() {
        let mut pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 80.0),
        ];
        ensure_orthogonal(&mut pts);
        assert_eq!(pts.len(), 3); // no insertions needed
    }

    // ── Coordinate conversion tests ──────────────────────────────────────────

    #[test]
    fn test_coord_to_pixel_basic() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 10.0,
            origin_y: 20.0,
            occupied: HashSet::new(),
        };
        // Col 1, Row 1 (1-based) → center of first cell
        let coord = routing::types::GridCoord::from_int(1, 1);
        let px = coord_to_pixel_x(coord, &grid);
        let py = coord_to_pixel_y(coord, &grid);
        // (1 - 0.5) * 200 + 10 = 110
        assert!((px - 110.0).abs() < 0.1);
        // (1 - 0.5) * 150 + 20 = 95
        assert!((py - 95.0).abs() < 0.1);
    }

    #[test]
    fn test_coord_to_pixel_junction() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Junction at (1.5, 1.0) — between columns 1 and 2
        let coord = routing::types::GridCoord::from_grid(1.5, 1.0);
        let px = coord_to_pixel_x(coord, &grid);
        let py = coord_to_pixel_y(coord, &grid);
        // (1.5 - 0.5) * 200 + 0 = 200
        assert!((px - 200.0).abs() < 0.1);
        // (1.0 - 0.5) * 150 + 0 = 75
        assert!((py - 75.0).abs() < 0.1);
    }

    #[test]
    fn test_lane_offset_zero() {
        let (ox, oy) = lane_offset(routing::types::Direction::East, 0, 20.0);
        assert_eq!(ox, 0.0);
        assert_eq!(oy, 0.0);
    }

    #[test]
    fn test_lane_offset_positive() {
        // Absolute convention: positive lane = south for horizontal, east for vertical.
        // Lane +1 traveling East → south (+Y)
        let (ox, oy) = lane_offset(routing::types::Direction::East, 1, 20.0);
        assert_eq!(ox, 0.0);
        assert_eq!(oy, 20.0);

        // Lane +1 traveling West → also south (+Y) — absolute, not direction-relative
        let (ox, oy) = lane_offset(routing::types::Direction::West, 1, 20.0);
        assert_eq!(ox, 0.0);
        assert_eq!(oy, 20.0);

        // Lane +1 traveling South → east (+X)
        let (ox, oy) = lane_offset(routing::types::Direction::South, 1, 20.0);
        assert_eq!(ox, 20.0);
        assert_eq!(oy, 0.0);

        // Lane +1 traveling North → also east (+X)
        let (ox, oy) = lane_offset(routing::types::Direction::North, 1, 20.0);
        assert_eq!(ox, 20.0);
        assert_eq!(oy, 0.0);
    }

    #[test]
    fn test_lane_offset_negative() {
        // Lane -1 traveling East → north (-Y)
        let (ox, oy) = lane_offset(routing::types::Direction::East, -1, 20.0);
        assert_eq!(ox, 0.0);
        assert_eq!(oy, -20.0);

        // Lane -1 traveling West → also north (-Y)
        let (ox, oy) = lane_offset(routing::types::Direction::West, -1, 20.0);
        assert_eq!(ox, 0.0);
        assert_eq!(oy, -20.0);

        // Lane -1 traveling North → west (-X)
        let (ox, oy) = lane_offset(routing::types::Direction::North, -1, 20.0);
        assert_eq!(ox, -20.0);
        assert_eq!(oy, 0.0);
    }

    // ── Integration tests: full routing pipeline ─────────────────────────────

    #[test]
    fn test_integration_two_adjacent_nodes() {
        // Two nodes side by side: A(1,1) → B(2,1)
        let nodes = vec![
            routing::types::DiagramNode {
                name: "A".into(),
                col: 1,
                row: 1,
            },
            routing::types::DiagramNode {
                name: "B".into(),
                col: 2,
                row: 1,
            },
        ];
        let edges = vec![routing::types::DiagramEdge {
            source: "A".into(),
            target: "B".into(),
            label: None,
        }];
        let config = routing::types::RoutingConfig {
            h_lane_capacity: 3,
            v_lane_capacity: 3,
            weights: routing::types::CostWeights::default(),
        };
        let output = routing::route_all_edges(&nodes, &edges, &config);
        assert_eq!(output.results.len(), 1);
        match &output.results[0].1 {
            routing::types::RouteResult::Success(route) => {
                assert!(route.waypoints.len() >= 2);
                // Source at (1,1), target at (2,1) — should go east
                let first = route.waypoints[0].coord;
                let last = route.waypoints.last().unwrap().coord;
                assert_eq!(first, routing::types::GridCoord::from_int(1, 1));
                assert_eq!(last, routing::types::GridCoord::from_int(2, 1));
            }
            routing::types::RouteResult::Failure { warning } => {
                panic!("Expected success, got failure: {warning}");
            }
        }
    }

    #[test]
    fn test_integration_l_shaped_route() {
        // A(1,1) → C(2,2): should produce an L-shape with 1 turn
        let nodes = vec![
            routing::types::DiagramNode {
                name: "A".into(),
                col: 1,
                row: 1,
            },
            routing::types::DiagramNode {
                name: "B".into(),
                col: 2,
                row: 1,
            },
            routing::types::DiagramNode {
                name: "C".into(),
                col: 2,
                row: 2,
            },
        ];
        let edges = vec![routing::types::DiagramEdge {
            source: "A".into(),
            target: "C".into(),
            label: None,
        }];
        let config = routing::types::RoutingConfig {
            h_lane_capacity: 3,
            v_lane_capacity: 3,
            weights: routing::types::CostWeights::default(),
        };
        let output = routing::route_all_edges(&nodes, &edges, &config);
        match &output.results[0].1 {
            routing::types::RouteResult::Success(route) => {
                assert!(route.complexity.turns >= 1);
                assert!(route.waypoints.len() >= 3);
            }
            routing::types::RouteResult::Failure { warning } => {
                panic!("Expected success, got failure: {warning}");
            }
        }
    }

    #[test]
    fn test_integration_waypoints_to_pixels() {
        // Test the full pipeline: route → pixels
        let nodes = vec![
            routing::types::DiagramNode {
                name: "A".into(),
                col: 1,
                row: 1,
            },
            routing::types::DiagramNode {
                name: "B".into(),
                col: 2,
                row: 1,
            },
        ];
        let edges = vec![routing::types::DiagramEdge {
            source: "A".into(),
            target: "B".into(),
            label: None,
        }];
        let config = routing::types::RoutingConfig {
            h_lane_capacity: 3,
            v_lane_capacity: 3,
            weights: routing::types::CostWeights::default(),
        };
        let output = routing::route_all_edges(&nodes, &edges, &config);

        let grid = GridInfo {
            cols: 2,
            rows: 1,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: [(0, 0), (1, 0)].iter().cloned().collect(),
        };
        let from_rect =
            egui::Rect::from_center_size(Pos2::new(100.0, 75.0), egui::vec2(130.0, 90.0));
        let to_rect = egui::Rect::from_center_size(Pos2::new(300.0, 75.0), egui::vec2(130.0, 90.0));

        match &output.results[0].1 {
            routing::types::RouteResult::Success(route) => {
                let pixels =
                    waypoints_to_pixels(route, &grid, &from_rect, &to_rect, 10.0, 20.0, 0.0, 0.0);
                assert!(pixels.len() >= 2);
                // First pixel should be near the right face of from_rect
                assert!((pixels[0].x - from_rect.right()).abs() < 1.0);
                // Last pixel should be near the left face of to_rect
                assert!((pixels.last().unwrap().x - to_rect.left()).abs() < 1.0);
                // All segments should be orthogonal
                for w in pixels.windows(2) {
                    let dx = (w[0].x - w[1].x).abs();
                    let dy = (w[0].y - w[1].y).abs();
                    assert!(
                        dx < 1.5 || dy < 1.5,
                        "Non-orthogonal: {:?} → {:?}",
                        w[0],
                        w[1]
                    );
                }
            }
            routing::types::RouteResult::Failure { warning } => {
                panic!("Expected success, got failure: {warning}");
            }
        }
    }

    #[test]
    fn test_integration_route_with_lane_offset() {
        // Two parallel edges between same nodes should get different lanes
        let nodes = vec![
            routing::types::DiagramNode {
                name: "A".into(),
                col: 1,
                row: 1,
            },
            routing::types::DiagramNode {
                name: "B".into(),
                col: 2,
                row: 1,
            },
        ];
        let edges = vec![
            routing::types::DiagramEdge {
                source: "A".into(),
                target: "B".into(),
                label: None,
            },
            routing::types::DiagramEdge {
                source: "A".into(),
                target: "B".into(),
                label: Some("second".into()),
            },
        ];
        let config = routing::types::RoutingConfig {
            h_lane_capacity: 5,
            v_lane_capacity: 5,
            weights: routing::types::CostWeights::default(),
        };
        let output = routing::route_all_edges(&nodes, &edges, &config);
        assert_eq!(output.results.len(), 2);
        // Both should succeed
        for (_, result) in &output.results {
            assert!(matches!(result, routing::types::RouteResult::Success(_)));
        }
    }

    /// Regression test: a turn with a lane change must produce a clean corner,
    /// not an S-curve jog. When a Westbound L-1 segment turns Southbound L0,
    /// the corner point should combine the incoming Y offset with the outgoing
    /// X offset so both segments stay straight.
    #[test]
    fn test_turn_with_lane_change_no_scurve() {
        // Reproduce the Hub-and-Spoke API→Auth route:
        //   (2,2) L-1 → (1.5,2) L-1 → (1,2) L0 → (1,2.5) L0 → (1,3)
        // This goes West at lane -1, turns South at lane 0 at coord (1,2).
        let route = routing::types::Route {
            waypoints: vec![
                routing::types::Waypoint {
                    coord: routing::types::GridCoord::from_int(2, 2),
                    lane: -1,
                },
                routing::types::Waypoint {
                    coord: routing::types::GridCoord { col2: 3, row2: 4 }, // (1.5, 2)
                    lane: -1,
                },
                routing::types::Waypoint {
                    coord: routing::types::GridCoord::from_int(1, 2),
                    lane: 0,
                },
                routing::types::Waypoint {
                    coord: routing::types::GridCoord { col2: 2, row2: 5 }, // (1, 2.5)
                    lane: 0,
                },
                routing::types::Waypoint {
                    coord: routing::types::GridCoord::from_int(1, 3),
                    lane: 0,
                },
            ],
            complexity: routing::types::RouteComplexity {
                length: 2.0,
                turns: 1,
                lane_changes: 0,
                crossings: 0,
            },
        };

        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 300.0,
            cell_h: 200.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: [
                (0, 0),
                (1, 0),
                (2, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (0, 2),
                (1, 2),
                (2, 2),
            ]
            .iter()
            .cloned()
            .collect(),
        };
        let lane_spacing = 20.0;

        // API at (2,2) → center pixel (450, 300)
        let from_rect =
            egui::Rect::from_center_size(Pos2::new(450.0, 300.0), egui::vec2(160.0, 120.0));
        // Auth at (1,3) → center pixel (150, 500)
        let to_rect =
            egui::Rect::from_center_size(Pos2::new(150.0, 500.0), egui::vec2(160.0, 120.0));

        // Derive port offsets the same way the renderer does
        let exit_dir = coord_direction(route.waypoints[0].coord, route.waypoints[1].coord);
        let exit_lane = route.waypoints[0].lane;
        let (elx, ely) = lane_offset(exit_dir, exit_lane, lane_spacing);
        let exit_face = direction_to_face(exit_dir);
        let port_start = match exit_face {
            Face::Left | Face::Right => ely,
            Face::Top | Face::Bottom => elx,
        };

        let n = route.waypoints.len();
        let entry_dir = coord_direction(route.waypoints[n - 2].coord, route.waypoints[n - 1].coord);
        let entry_lane = route.waypoints[n - 2].lane;
        let (nlx, nly) = lane_offset(entry_dir, entry_lane, lane_spacing);
        let entry_face = direction_to_face(entry_dir.opposite());
        let port_end = match entry_face {
            Face::Left | Face::Right => nly,
            Face::Top | Face::Bottom => nlx,
        };

        let pixels = waypoints_to_pixels(
            &route,
            &grid,
            &from_rect,
            &to_rect,
            10.0,
            lane_spacing,
            port_start,
            port_end,
        );

        // All segments must be orthogonal (no diagonal jogs)
        for w in pixels.windows(2) {
            let dx = (w[0].x - w[1].x).abs();
            let dy = (w[0].y - w[1].y).abs();
            assert!(
                dx < 1.5 || dy < 1.5,
                "Non-orthogonal segment: {:?} → {:?} (dx={dx:.1}, dy={dy:.1})",
                w[0],
                w[1]
            );
        }

        // The key check: no S-curve at the turn. Find the vertical segments
        // near the turn column (x ≈ 150, column 1 center). The Y values must
        // be monotonically decreasing (going up) or increasing (going down) —
        // never reversing direction.
        let turn_col_x = 150.0; // center of column 1
        let vertical_near_turn: Vec<&Pos2> = pixels
            .iter()
            .filter(|p| (p.x - turn_col_x).abs() < lane_spacing * 2.0)
            .collect();

        if vertical_near_turn.len() >= 2 {
            // Check Y values don't reverse: once they start going down, they
            // must keep going down (no up-then-down S-curve).
            let mut prev_y = vertical_near_turn[0].y;
            let mut direction: Option<bool> = None; // true = going down
            for pt in &vertical_near_turn[1..] {
                let dy = pt.y - prev_y;
                if dy.abs() > 1.0 {
                    let going_down = dy > 0.0;
                    if let Some(was_down) = direction {
                        assert_eq!(
                            was_down, going_down,
                            "S-curve detected at turn: Y reversed direction at {:?} \
                             (vertical points: {:?})",
                            pt, vertical_near_turn
                        );
                    }
                    direction = Some(going_down);
                }
                prev_y = pt.y;
            }
        }
    }

    // ── Capacity computation tests ───────────────────────────────────────────

    #[test]
    fn test_compute_h_capacity() {
        let grid = GridInfo {
            cols: 2,
            rows: 2,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: [(0, 0), (1, 0), (0, 1), (1, 1)].iter().cloned().collect(),
        };
        let mut rects = HashMap::new();
        rects.insert(
            "A".into(),
            egui::Rect::from_center_size(Pos2::new(100.0, 75.0), egui::vec2(130.0, 90.0)),
        );
        rects.insert(
            "B".into(),
            egui::Rect::from_center_size(Pos2::new(300.0, 75.0), egui::vec2(130.0, 90.0)),
        );
        rects.insert(
            "C".into(),
            egui::Rect::from_center_size(Pos2::new(100.0, 225.0), egui::vec2(130.0, 90.0)),
        );
        rects.insert(
            "D".into(),
            egui::Rect::from_center_size(Pos2::new(300.0, 225.0), egui::vec2(130.0, 90.0)),
        );
        let cap = compute_h_capacity(&grid, &rects, 20.0);
        // cell_h=150, node_h=90, gap=60, 60/20 = 3
        assert_eq!(cap, 3);
    }

    #[test]
    fn test_compute_v_capacity() {
        let grid = GridInfo {
            cols: 2,
            rows: 2,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: [(0, 0), (1, 0)].iter().cloned().collect(),
        };
        let mut rects = HashMap::new();
        rects.insert(
            "A".into(),
            egui::Rect::from_center_size(Pos2::new(100.0, 75.0), egui::vec2(130.0, 90.0)),
        );
        rects.insert(
            "B".into(),
            egui::Rect::from_center_size(Pos2::new(300.0, 75.0), egui::vec2(130.0, 90.0)),
        );
        let cap = compute_v_capacity(&grid, &rects, 20.0);
        // cell_w=200, node_w=130, gap=70, 70/20 = 3
        assert_eq!(cap, 3);
    }

    #[test]
    fn test_diagram_debug_info_basic() {
        let content = "A (pos: 1,1)\nB (pos: 2,1)\nA -> B: link";
        let info = diagram_debug_info(content);
        assert!(info.contains("NODES (2):"));
        assert!(info.contains("A @ (1,1)"));
        assert!(info.contains("B @ (2,1)"));
        assert!(info.contains("EDGES (1):"));
        assert!(info.contains("A -> B \"link\""));
        assert!(info.contains("ROUTING RESULTS:"));
        assert!(info.contains("OK"));
    }

    #[test]
    fn test_diagram_debug_info_empty() {
        let info = diagram_debug_info("");
        assert_eq!(info, "No nodes parsed.");
    }

    #[test]
    fn test_diagram_debug_info_auto_layout() {
        let content = "A\nB\nC\nA -> B\nB -> C";
        let info = diagram_debug_info(content);
        // Auto-layout: 3 nodes → single row at (1,1), (2,1), (3,1)
        assert!(info.contains("NODES (3):"));
        assert!(info.contains("A @ (1,1)"));
        assert!(info.contains("B @ (2,1)"));
        assert!(info.contains("C @ (3,1)"));
        assert!(info.contains("EDGES (2):"));
        assert!(info.contains("ROUTING RESULTS:"));
    }
}
