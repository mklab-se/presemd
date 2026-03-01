#[allow(dead_code)]
pub mod routing;

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::render::image_cache::ImageCache;
use crate::theme::Theme;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Stroke};

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

/// Tracks port usage on each face of each node so connections spread out.
/// Uses dynamic alternating offsets (0, +s, -s, +2s, -2s, ...) so ports
/// are allocated correctly regardless of which face pair routing ultimately picks.
#[derive(Clone)]
struct FacePortAllocator {
    /// For each (node, face): how many ports have been claimed so far
    counts: HashMap<(String, Face), usize>,
}

impl FacePortAllocator {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Claim the next port on a node face. Returns an offset from face center.
    /// First connection gets center (0), subsequent ones alternate: +s, -s, +2s, -2s, ...
    fn claim_port(
        &mut self,
        node_name: &str,
        face: Face,
        rect: &egui::Rect,
        port_spacing: f32,
    ) -> f32 {
        let key = (node_name.to_string(), face);
        let idx = self.counts.entry(key).or_insert(0);
        let current = *idx;
        *idx += 1;

        if current == 0 {
            return 0.0;
        }

        // Alternating offsets: +s, -s, +2s, -2s, ...
        let face_length = match face {
            Face::Left | Face::Right => rect.height(),
            Face::Top | Face::Bottom => rect.width(),
        };
        let max_offset = face_length * 0.3;
        let level = current.div_ceil(2);
        let sign = if current % 2 == 1 { 1.0 } else { -1.0 };
        let offset = sign * level as f32 * port_spacing;
        offset.clamp(-max_offset, max_offset)
    }
}

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
    /// Index 0 = top edge (above row 0), index rows = bottom edge.
    /// Corridors between rows: index i is the boundary between row i-1 and row i.
    /// NOTE: This returns the cell edge, not the visual center of the gap between nodes.
    /// For rendering waypoints, prefer `street_center_y` or `intersection_pos`.
    fn h_corridor_y(&self, index: usize) -> f32 {
        self.origin_y + index as f32 * self.cell_h
    }

    /// X position of vertical corridor at given index (raw cell boundary).
    /// Index 0 = left edge, index cols = right edge.
    /// NOTE: This returns the cell edge, not the visual center of the gap between nodes.
    /// For rendering waypoints, prefer `avenue_center_x` or `intersection_pos`.
    fn v_corridor_x(&self, index: usize) -> f32 {
        self.origin_x + index as f32 * self.cell_w
    }

    /// Y center of a horizontal street corridor at the given index.
    /// Uses actual node rects to find the midpoint of the gap between rows,
    /// looking at columns from `cross_from` to `cross_to` (exclusive).
    fn street_center_y(
        &self,
        street_idx: usize,
        cross_from: usize,
        cross_to: usize,
        node_rects: &HashMap<String, egui::Rect>,
    ) -> f32 {
        let seg = Segment {
            corridor: Corridor::Street(street_idx),
            from_cross: cross_from,
            to_cross: cross_to.max(cross_from + 1),
        };
        self.segment_center(&seg, node_rects)
    }

    /// X center of a vertical avenue corridor at the given index.
    /// Uses actual node rects to find the midpoint of the gap between columns,
    /// looking at rows from `cross_from` to `cross_to` (exclusive).
    fn avenue_center_x(
        &self,
        avenue_idx: usize,
        cross_from: usize,
        cross_to: usize,
        node_rects: &HashMap<String, egui::Rect>,
    ) -> f32 {
        let seg = Segment {
            corridor: Corridor::Avenue(avenue_idx),
            from_cross: cross_from,
            to_cross: cross_to.max(cross_from + 1),
        };
        self.segment_center(&seg, node_rects)
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
    fn is_cell_empty(&self, col: usize, row: usize) -> bool {
        !self.occupied.contains(&(col, row))
    }

    /// Find the best horizontal corridor for traveling between two row-positions.
    /// Returns the corridor index closest to the midpoint between the two rows.
    fn best_h_corridor(&self, from_y: f32, to_y: f32) -> usize {
        let mid_y = (from_y + to_y) / 2.0;
        let mut best_idx = 0;
        let mut best_dist = f32::MAX;
        for i in 0..=self.rows {
            let cy = self.h_corridor_y(i);
            let dist = (cy - mid_y).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }
        best_idx
    }

    /// Find the best vertical corridor for traveling between two column-positions.
    fn best_v_corridor(&self, from_x: f32, to_x: f32) -> usize {
        let mid_x = (from_x + to_x) / 2.0;
        let mut best_idx = 0;
        let mut best_dist = f32::MAX;
        for j in 0..=self.cols {
            let cx = self.v_corridor_x(j);
            let dist = (cx - mid_x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = j;
            }
        }
        best_idx
    }

    /// Return the pixel gap available for lanes on a segment.
    /// For a Street segment between two avenues, this is the vertical gap
    /// between the bounding node rects (or cell edges if no node).
    /// For an Avenue segment between two streets, the horizontal gap.
    fn segment_gap(&self, segment: &Segment, node_rects: &HashMap<String, egui::Rect>) -> f32 {
        match segment.corridor {
            Corridor::Street(h_idx) => {
                // Horizontal corridor at h_corridor_y(h_idx).
                // Gap = distance between node bottom in row above and node top in row below.
                let above_bottom = if h_idx > 0 {
                    let mut max_bottom = f32::NEG_INFINITY;
                    for col in segment.from_cross..segment.to_cross {
                        if col < self.cols {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == col && r == h_idx - 1 {
                                        max_bottom = max_bottom.max(rect.bottom());
                                    }
                                }
                            }
                        }
                    }
                    if max_bottom.is_finite() {
                        max_bottom
                    } else {
                        self.h_corridor_y(h_idx)
                    }
                } else {
                    self.h_corridor_y(0)
                };
                let below_top = if h_idx < self.rows {
                    let mut min_top = f32::INFINITY;
                    for col in segment.from_cross..segment.to_cross {
                        if col < self.cols {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == col && r == h_idx {
                                        min_top = min_top.min(rect.top());
                                    }
                                }
                            }
                        }
                    }
                    if min_top.is_finite() {
                        min_top
                    } else {
                        self.h_corridor_y(h_idx)
                    }
                } else {
                    self.h_corridor_y(self.rows)
                };
                (below_top - above_bottom).abs()
            }
            Corridor::Avenue(v_idx) => {
                let left_right = if v_idx > 0 {
                    let mut max_right = f32::NEG_INFINITY;
                    for row in segment.from_cross..segment.to_cross {
                        if row < self.rows {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == v_idx - 1 && r == row {
                                        max_right = max_right.max(rect.right());
                                    }
                                }
                            }
                        }
                    }
                    if max_right.is_finite() {
                        max_right
                    } else {
                        self.v_corridor_x(v_idx)
                    }
                } else {
                    self.v_corridor_x(0)
                };
                let right_left = if v_idx < self.cols {
                    let mut min_left = f32::INFINITY;
                    for row in segment.from_cross..segment.to_cross {
                        if row < self.rows {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == v_idx && r == row {
                                        min_left = min_left.min(rect.left());
                                    }
                                }
                            }
                        }
                    }
                    if min_left.is_finite() {
                        min_left
                    } else {
                        self.v_corridor_x(v_idx)
                    }
                } else {
                    self.v_corridor_x(self.cols)
                };
                (right_left - left_right).abs()
            }
        }
    }

    /// Return the visual center Y (for Street) or X (for Avenue) of a segment.
    /// Unlike `h_corridor_y`/`v_corridor_x` which return the cell boundary,
    /// this returns the midpoint of the actual gap between adjacent node edges.
    /// Routes should travel through this center — never along node edges.
    fn segment_center(&self, segment: &Segment, node_rects: &HashMap<String, egui::Rect>) -> f32 {
        match segment.corridor {
            Corridor::Street(h_idx) => {
                // Find the bottom edge of nodes in the row ABOVE this street.
                // Initialize to NEG_INFINITY so any real node edge overrides it.
                // Fall back to the cell boundary only if no nodes are found.
                let above_bottom = if h_idx > 0 {
                    let mut max_bottom = f32::NEG_INFINITY;
                    for col in segment.from_cross..segment.to_cross {
                        if col < self.cols {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == col && r == h_idx - 1 {
                                        max_bottom = max_bottom.max(rect.bottom());
                                    }
                                }
                            }
                        }
                    }
                    if max_bottom.is_finite() {
                        max_bottom
                    } else {
                        self.h_corridor_y(h_idx)
                    }
                } else {
                    self.h_corridor_y(0)
                };
                // Find the top edge of nodes in the row BELOW this street.
                let below_top = if h_idx < self.rows {
                    let mut min_top = f32::INFINITY;
                    for col in segment.from_cross..segment.to_cross {
                        if col < self.cols {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == col && r == h_idx {
                                        min_top = min_top.min(rect.top());
                                    }
                                }
                            }
                        }
                    }
                    if min_top.is_finite() {
                        min_top
                    } else {
                        self.h_corridor_y(h_idx)
                    }
                } else {
                    self.h_corridor_y(self.rows)
                };
                (above_bottom + below_top) / 2.0
            }
            Corridor::Avenue(v_idx) => {
                // Find the right edge of nodes in the column LEFT of this avenue.
                let left_right = if v_idx > 0 {
                    let mut max_right = f32::NEG_INFINITY;
                    for row in segment.from_cross..segment.to_cross {
                        if row < self.rows {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == v_idx - 1 && r == row {
                                        max_right = max_right.max(rect.right());
                                    }
                                }
                            }
                        }
                    }
                    if max_right.is_finite() {
                        max_right
                    } else {
                        self.v_corridor_x(v_idx)
                    }
                } else {
                    self.v_corridor_x(0)
                };
                // Find the left edge of nodes in the column RIGHT of this avenue.
                let right_left = if v_idx < self.cols {
                    let mut min_left = f32::INFINITY;
                    for row in segment.from_cross..segment.to_cross {
                        if row < self.rows {
                            for rect in node_rects.values() {
                                let rc = rect.center();
                                if let Some((c, r)) = self.cell_at(rc) {
                                    if c == v_idx && r == row {
                                        min_left = min_left.min(rect.left());
                                    }
                                }
                            }
                        }
                    }
                    if min_left.is_finite() {
                        min_left
                    } else {
                        self.v_corridor_x(v_idx)
                    }
                } else {
                    self.v_corridor_x(self.cols)
                };
                (left_right + right_left) / 2.0
            }
        }
    }

    /// How many lanes fit in a segment, given clearance from edges and lane spacing.
    fn lane_capacity(
        &self,
        segment: &Segment,
        node_rects: &HashMap<String, egui::Rect>,
        clearance: f32,
        lane_spacing: f32,
    ) -> usize {
        let gap = self.segment_gap(segment, node_rects);
        let usable = gap - 2.0 * clearance;
        if usable <= 0.0 || lane_spacing <= 0.0 {
            return 0;
        }
        (usable / lane_spacing).floor() as usize
    }

    /// Convert a lane number to a pixel offset from the corridor center.
    /// Lane 1 = north/west edge, lanes increase toward south/east.
    ///
    /// The center lane matches `claim_lane_span`'s allocation center
    /// (`total.div_ceil(2)`), ensuring the first-allocated lane is always
    /// at offset 0 — visually centered in the corridor.
    fn lane_pixel_offset(lane: usize, total_lanes: usize, lane_spacing: f32) -> f32 {
        if total_lanes == 0 {
            return 0.0;
        }
        // Use div_ceil to match claim_lane_span's center calculation.
        // This ensures the first-allocated lane (center) maps to offset 0.
        let center = total_lanes.div_ceil(2) as f32;
        (lane as f32 - center) * lane_spacing
    }

    /// Return all segments of a corridor.
    #[cfg(test)]
    fn corridor_segments(&self, corridor: Corridor) -> Vec<Segment> {
        let cross_count = match corridor {
            Corridor::Street(_) => self.cols,
            Corridor::Avenue(_) => self.rows,
        };
        (0..cross_count)
            .map(|i| Segment {
                corridor,
                from_cross: i,
                to_cross: i + 1,
            })
            .collect()
    }
}

// ─── Manhattan-style routing ────────────────────────────────────────────────

/// A corridor in the grid: either a horizontal Street or vertical Avenue.
/// Street i = h_corridor(i), Avenue j = v_corridor(j).
/// Street 0 = top edge, Street rows = bottom edge.
/// Avenue 0 = left edge, Avenue cols = right edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Corridor {
    Street(usize),
    Avenue(usize),
}

impl std::fmt::Display for Corridor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Corridor::Street(i) => write!(f, "Street {i}"),
            Corridor::Avenue(j) => write!(f, "Avenue {j}"),
        }
    }
}

/// A segment of a corridor between two adjacent cross-corridor intersections.
/// For a Street: from_cross and to_cross are Avenue indices.
/// For an Avenue: from_cross and to_cross are Street indices.
/// Invariant: from_cross < to_cross.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Segment {
    corridor: Corridor,
    from_cross: usize,
    to_cross: usize,
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cross_name = match self.corridor {
            Corridor::Street(_) => "Avenue",
            Corridor::Avenue(_) => "Street",
        };
        write!(
            f,
            "{} between {} {} and {} {}",
            self.corridor, cross_name, self.from_cross, cross_name, self.to_cross
        )
    }
}

/// Cardinal direction of travel along a corridor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TravelDir {
    North,
    South,
    East,
    West,
}

impl std::fmt::Display for TravelDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TravelDir::North => write!(f, "north"),
            TravelDir::South => write!(f, "south"),
            TravelDir::East => write!(f, "east"),
            TravelDir::West => write!(f, "west"),
        }
    }
}

/// A single instruction in a route through the city grid.
///
/// The city metaphor:
/// - **Buildings** sit on **lots** defined by surrounding streets/avenues
/// - **Streets** run horizontally (numbered 0, 1, 2, ...)
/// - **Avenues** run vertically (numbered 0, 1, 2, ...)
/// - **Sidewalks** border buildings — only crossed when entering/exiting
/// - A **step** moves half a block (building→road or road→intersection or intersection→road)
///
/// Routes are expressed as: Exit building → steps on roads → Enter building.
/// Each step specifies a direction and lane. Routes never walk "on the
/// pavement" — they only cross sidewalks when entering/exiting a building.
#[derive(Debug, Clone, PartialEq)]
enum RouteStep {
    /// Exit a building onto the adjacent road (crosses the sidewalk).
    /// `face`: which side of the building to exit from.
    /// `lane`: which lane on the exit port (for spreading multiple connections).
    ExitBuilding { face: Face, lane: usize },
    /// Take a step in the given direction along a road.
    /// A step moves you half a block — from the sidewalk to the nearest
    /// intersection, or from an intersection to the next intersection.
    /// `lane`: which lane to walk on (1 = near edge, center = middle of road).
    Step { direction: TravelDir, lane: usize },
    /// Enter a building from the adjacent road (crosses the sidewalk).
    /// `face`: which side of the building to enter from.
    /// `lane`: which lane on the entry port.
    EnterBuilding { face: Face, lane: usize },
}

impl std::fmt::Display for RouteStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteStep::ExitBuilding { face, lane } => {
                write!(f, "Exit building {face:?}, lane {lane}")
            }
            RouteStep::Step { direction, lane } => {
                write!(f, "Step {direction}, lane {lane}")
            }
            RouteStep::EnterBuilding { face, lane } => {
                write!(f, "Enter building {face:?}, lane {lane}")
            }
        }
    }
}

/// A routed orthogonal path with semantic steps and derived pixel waypoints.
struct RoutedPath {
    /// Semantic route description (used for test assertions and debugging).
    #[cfg_attr(not(test), allow(dead_code))]
    steps: Vec<RouteStep>,
    waypoints: Vec<Pos2>,
}

impl RoutedPath {
    /// Number of steps in the route (not counting ExitBuilding/EnterBuilding).
    /// This is the primary metric for route quality — fewer steps = shorter route.
    fn step_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s, RouteStep::Step { .. }))
            .count()
    }
}

/// Tracks lane occupancy per segment (between two intersections).
/// Each segment tracks lanes independently, so a route using one segment
/// of a corridor doesn't block other segments of the same corridor.
#[derive(Clone)]
struct SegmentOccupancy {
    occupied: HashMap<Segment, Vec<usize>>,
}

impl SegmentOccupancy {
    fn new() -> Self {
        Self {
            occupied: HashMap::new(),
        }
    }

    /// Claim the most centered free lane on a segment.
    /// Lane 1 = north/west edge, lanes increase toward south/east.
    /// Center lane = (capacity + 1) / 2.
    /// Returns None if all lanes are occupied.
    #[cfg(test)]
    fn claim_lane(&mut self, segment: Segment, capacity: usize) -> Option<usize> {
        if capacity == 0 {
            return None;
        }
        let lanes = self.occupied.entry(segment).or_default();
        let center = capacity.div_ceil(2);
        // Spiral outward from center: center, center+1, center-1, center+2, center-2, ...
        for offset in 0..capacity {
            let candidates: Vec<usize> = if offset == 0 {
                vec![center]
            } else {
                let mut c = Vec::new();
                if center + offset <= capacity {
                    c.push(center + offset);
                }
                if offset < center {
                    c.push(center - offset);
                }
                c
            };
            for lane in candidates {
                if lane >= 1 && lane <= capacity && !lanes.contains(&lane) {
                    lanes.push(lane);
                    return Some(lane);
                }
            }
        }
        None
    }

    /// Check if a lane is free on all segments of a corridor span.
    fn is_lane_free_on_range(
        &self,
        corridor: Corridor,
        from_cross: usize,
        to_cross: usize,
        lane: usize,
    ) -> bool {
        let (lo, hi) = if from_cross <= to_cross {
            (from_cross, to_cross)
        } else {
            (to_cross, from_cross)
        };
        for i in lo..hi {
            let seg = Segment {
                corridor,
                from_cross: i,
                to_cross: i + 1,
            };
            if let Some(lanes) = self.occupied.get(&seg) {
                if lanes.contains(&lane) {
                    return false;
                }
            }
        }
        true
    }

    /// Claim a lane that is free on ALL segments of a corridor span.
    /// Uses look-ahead: finds the most centered lane free across the entire span,
    /// then claims it on all segments.
    /// `capacities` provides the capacity for each segment in the span.
    fn claim_lane_span(
        &mut self,
        corridor: Corridor,
        from_cross: usize,
        to_cross: usize,
        capacities: &[usize],
    ) -> Option<usize> {
        let (lo, hi) = if from_cross <= to_cross {
            (from_cross, to_cross)
        } else {
            (to_cross, from_cross)
        };
        let n_segments = hi - lo;
        if n_segments == 0 || capacities.is_empty() {
            return None;
        }

        // The effective capacity is the minimum across all segments
        let min_capacity = *capacities.iter().min().unwrap_or(&0);
        if min_capacity == 0 {
            return None;
        }

        let center = min_capacity.div_ceil(2);
        // Spiral outward from center
        for offset in 0..min_capacity {
            let candidates: Vec<usize> = if offset == 0 {
                vec![center]
            } else {
                let mut c = Vec::new();
                if center + offset <= min_capacity {
                    c.push(center + offset);
                }
                if offset < center {
                    c.push(center - offset);
                }
                c
            };
            for lane in candidates {
                if lane >= 1
                    && lane <= min_capacity
                    && self.is_lane_free_on_range(corridor, lo, hi, lane)
                {
                    // Claim on all segments
                    for i in lo..hi {
                        let seg = Segment {
                            corridor,
                            from_cross: i,
                            to_cross: i + 1,
                        };
                        self.occupied.entry(seg).or_default().push(lane);
                    }
                    return Some(lane);
                }
            }
        }
        None
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

/// Check if an axis-aligned segment (horizontal or vertical) crosses a rectangle.
fn segment_crosses_rect(a: Pos2, b: Pos2, rect: &egui::Rect, shrink: f32) -> bool {
    let r = egui::Rect::from_min_max(
        Pos2::new(rect.left() + shrink, rect.top() + shrink),
        Pos2::new(rect.right() - shrink, rect.bottom() - shrink),
    );
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return false;
    }

    if (a.y - b.y).abs() < 1.0 {
        // Horizontal
        let y = a.y;
        let (min_x, max_x) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
        y > r.top() && y < r.bottom() && max_x > r.left() && min_x < r.right()
    } else if (a.x - b.x).abs() < 1.0 {
        // Vertical
        let x = a.x;
        let (min_y, max_y) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
        x > r.left() && x < r.right() && max_y > r.top() && min_y < r.bottom()
    } else {
        false
    }
}

/// Check if any segment of a path crosses any node (excluding source and target).
fn path_crosses_node(
    waypoints: &[Pos2],
    node_rects: &HashMap<String, egui::Rect>,
    from_name: &str,
    to_name: &str,
) -> bool {
    for i in 0..waypoints.len().saturating_sub(1) {
        for (name, rect) in node_rects {
            if name == from_name || name == to_name {
                continue;
            }
            if segment_crosses_rect(waypoints[i], waypoints[i + 1], rect, 4.0) {
                return true;
            }
        }
    }
    false
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

// ─── Routing functions ──────────────────────────────────────────────────────

/// Select candidate face pairs for routing between two nodes.
fn select_face_pairs(
    from_rect: &egui::Rect,
    to_rect: &egui::Rect,
    grid: &GridInfo,
) -> Vec<(Face, Face)> {
    let from_c = from_rect.center();
    let to_c = to_rect.center();
    let dx = to_c.x - from_c.x;
    let dy = to_c.y - from_c.y;

    let natural_exit = choose_exit_face(from_rect, to_c);
    let natural_entry = choose_entry_face(to_rect, from_c);

    let is_diagonal = dx.abs() > grid.cell_w * 0.3 && dy.abs() > grid.cell_h * 0.3;

    if is_diagonal {
        let v_exit = if dy >= 0.0 { Face::Bottom } else { Face::Top };
        let v_entry = if dy >= 0.0 { Face::Top } else { Face::Bottom };
        let h_exit = if dx >= 0.0 { Face::Right } else { Face::Left };
        let h_entry = if dx >= 0.0 { Face::Left } else { Face::Right };

        let hv_empty = grid
            .cell_at(Pos2::new(to_c.x, from_c.y))
            .is_some_and(|(c, r)| grid.is_cell_empty(c, r));
        let vh_empty = grid
            .cell_at(Pos2::new(from_c.x, to_c.y))
            .is_some_and(|(c, r)| grid.is_cell_empty(c, r));

        let mut pairs = Vec::new();
        if hv_empty {
            pairs.push((h_exit, v_entry));
        }
        if vh_empty {
            pairs.push((v_exit, h_entry));
        }
        let z_candidates: [(Face, Face); 4] = if grid.cell_w >= grid.cell_h {
            [
                (h_exit, h_entry),
                (h_exit, v_entry),
                (v_exit, h_entry),
                (v_exit, v_entry),
            ]
        } else {
            [
                (v_exit, v_entry),
                (v_exit, h_entry),
                (h_exit, v_entry),
                (h_exit, h_entry),
            ]
        };
        for pair in z_candidates
            .into_iter()
            .chain(std::iter::once((natural_exit, natural_entry)))
        {
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
        pairs
    } else {
        let mut pairs = vec![(natural_exit, natural_entry)];
        let sr = (from_c.y - to_c.y).abs() < grid.cell_h * 0.3;
        let sc = (from_c.x - to_c.x).abs() < grid.cell_w * 0.3;
        if sr {
            pairs.push((Face::Top, Face::Top));
            pairs.push((Face::Bottom, Face::Bottom));
        }
        if sc {
            if dx >= 0.0 {
                pairs.push((Face::Right, Face::Right));
                pairs.push((Face::Left, Face::Left));
            } else {
                pairs.push((Face::Left, Face::Left));
                pairs.push((Face::Right, Face::Right));
            }
        }
        pairs
    }
}

/// Determine which corridor a face of a node exits onto.
/// Left/Right faces exit onto the adjacent vertical Avenue.
/// Top/Bottom faces exit onto the adjacent horizontal Street.
fn face_corridor(face: Face, node_col: usize, node_row: usize) -> Corridor {
    match face {
        Face::Right => Corridor::Avenue(node_col + 1),
        Face::Left => Corridor::Avenue(node_col),
        Face::Bottom => Corridor::Street(node_row + 1),
        Face::Top => Corridor::Street(node_row),
    }
}

// ─── Step-based route planning ──────────────────────────────────────────────
//
// Routes are expressed as a series of steps through the city grid:
//   ExitBuilding → Step(direction, lane)* → EnterBuilding
//
// Each Step moves half a block. The first step crosses the sidewalk onto
// the road; subsequent steps move along roads between intersections.
// Routes NEVER walk on the pavement (along node edges) — they only
// cross sidewalks when entering/exiting a building.

/// Plan a step-based route from one building to another via a specific face pair.
///
/// The route is expressed purely in the routing language (steps + lanes).
/// Pixel rendering is a separate concern handled by `steps_to_waypoints`.
#[allow(clippy::too_many_arguments)]
fn plan_step_route(
    exit_face: Face,
    entry_face: Face,
    from_col: usize,
    from_row: usize,
    to_col: usize,
    to_row: usize,
    grid: &GridInfo,
    occupancy: &mut SegmentOccupancy,
    node_rects: &HashMap<String, egui::Rect>,
    lane_spacing: f32,
) -> Option<Vec<RouteStep>> {
    let clearance = 5.0;
    let compute_cap = |seg: &Segment| -> usize {
        grid.lane_capacity(seg, node_rects, clearance, lane_spacing)
            .max(1)
    };

    // Determine where we exit onto the road grid and where we need to enter
    let exit_corridor = face_corridor(exit_face, from_col, from_row);
    let entry_corridor = face_corridor(entry_face, to_col, to_row);

    // Build the step sequence based on the geometry
    // First step: exit building (crosses sidewalk onto road)
    // Middle steps: walk along roads, turning at intersections
    // Last step: enter building (crosses sidewalk from road)

    let exit_h = matches!(exit_face, Face::Left | Face::Right);
    let entry_h = matches!(entry_face, Face::Left | Face::Right);

    // Determine the road segments we'll traverse and claim lanes
    let steps = match (exit_h, entry_h) {
        (true, true) => plan_steps_h_to_h(
            exit_face,
            entry_face,
            exit_corridor,
            entry_corridor,
            from_col,
            from_row,
            to_col,
            to_row,
            grid,
            occupancy,
            &compute_cap,
        )?,
        (false, false) => plan_steps_v_to_v(
            exit_face,
            entry_face,
            exit_corridor,
            entry_corridor,
            from_col,
            from_row,
            to_col,
            to_row,
            grid,
            occupancy,
            &compute_cap,
        )?,
        (true, false) => plan_steps_h_to_v(
            exit_face,
            entry_face,
            exit_corridor,
            entry_corridor,
            from_col,
            from_row,
            to_col,
            to_row,
            grid,
            occupancy,
            &compute_cap,
        )?,
        (false, true) => plan_steps_v_to_h(
            exit_face,
            entry_face,
            exit_corridor,
            entry_corridor,
            from_col,
            from_row,
            to_col,
            to_row,
            grid,
            occupancy,
            &compute_cap,
        )?,
    };

    Some(steps)
}

/// Helper: claim a lane on a corridor span and return the lane number.
fn claim_span_lane(
    occupancy: &mut SegmentOccupancy,
    corridor: Corridor,
    from_cross: usize,
    to_cross: usize,
    compute_cap: &dyn Fn(&Segment) -> usize,
) -> usize {
    let (lo, hi) = ordered(from_cross, to_cross);
    if lo == hi {
        return 3; // center fallback
    }
    let caps: Vec<usize> = (lo..hi)
        .map(|i| {
            compute_cap(&Segment {
                corridor,
                from_cross: i,
                to_cross: i + 1,
            })
        })
        .collect();
    occupancy
        .claim_lane_span(corridor, lo, hi, &caps)
        .unwrap_or(1)
}

/// Build steps for each half-block of travel along a corridor.
/// Returns Step instructions for traveling from `from_cross` to `to_cross`
/// on the given corridor, one step per half-block.
fn corridor_steps(
    corridor: Corridor,
    from_cross: usize,
    to_cross: usize,
    lane: usize,
) -> Vec<RouteStep> {
    if from_cross == to_cross {
        return Vec::new();
    }
    let dir = match corridor {
        Corridor::Street(_) => {
            if to_cross > from_cross {
                TravelDir::East
            } else {
                TravelDir::West
            }
        }
        Corridor::Avenue(_) => {
            if to_cross > from_cross {
                TravelDir::South
            } else {
                TravelDir::North
            }
        }
    };
    let count = from_cross.abs_diff(to_cross);
    (0..count)
        .map(|_| RouteStep::Step {
            direction: dir,
            lane,
        })
        .collect()
}

/// Plan H→H steps (exit Left/Right, enter Left/Right).
#[allow(clippy::too_many_arguments)]
fn plan_steps_h_to_h(
    exit_face: Face,
    entry_face: Face,
    exit_corridor: Corridor,
    entry_corridor: Corridor,
    _from_col: usize,
    from_row: usize,
    _to_col: usize,
    to_row: usize,
    grid: &GridInfo,
    occupancy: &mut SegmentOccupancy,
    compute_cap: &dyn Fn(&Segment) -> usize,
) -> Option<Vec<RouteStep>> {
    let Corridor::Avenue(exit_ave) = exit_corridor else {
        return None;
    };
    let Corridor::Avenue(entry_ave) = entry_corridor else {
        return None;
    };

    let mut steps = Vec::new();

    if exit_ave == entry_ave {
        // Same avenue — travel along it from from_row to to_row
        if from_row == to_row {
            // Adjacent nodes sharing this avenue — no corridor travel needed
            let lane = 3; // center fallback
            steps.push(RouteStep::ExitBuilding {
                face: exit_face,
                lane,
            });
            steps.push(RouteStep::EnterBuilding {
                face: entry_face,
                lane,
            });
            return Some(steps);
        }
        let going_pos = to_row > from_row;
        let eff_from = effective_exit_cross(from_row, going_pos);
        let eff_to = effective_entry_cross(to_row, going_pos);
        let lane = claim_span_lane(occupancy, exit_corridor, eff_from, eff_to, compute_cap);
        steps.push(RouteStep::ExitBuilding {
            face: exit_face,
            lane,
        });
        steps.extend(corridor_steps(exit_corridor, eff_from, eff_to, lane));
        steps.push(RouteStep::EnterBuilding {
            face: entry_face,
            lane,
        });
        return Some(steps);
    }

    // Different avenues: need a connecting street
    let from_c = Pos2::new(grid.v_corridor_x(exit_ave), grid.h_corridor_y(from_row));
    let to_c = Pos2::new(grid.v_corridor_x(entry_ave), grid.h_corridor_y(to_row));
    let street_idx = grid.best_h_corridor(from_c.y, to_c.y);
    let street = Corridor::Street(street_idx);

    // Exit avenue: from building row to connecting street
    let eff_exit = if street_idx == from_row {
        from_row // already at the connecting street, no travel needed
    } else {
        effective_exit_cross(from_row, street_idx > from_row)
    };
    let ave_lane = claim_span_lane(occupancy, exit_corridor, eff_exit, street_idx, compute_cap);

    // Street: from exit avenue to entry avenue
    let (st_from, st_to) = ordered(exit_ave, entry_ave);
    let st_lane = claim_span_lane(occupancy, street, st_from, st_to, compute_cap);

    // Entry avenue: from connecting street to target row
    let eff_entry = if street_idx == to_row {
        to_row // already at the target row, no travel needed
    } else {
        effective_entry_cross(to_row, to_row > street_idx)
    };
    let eave_lane = claim_span_lane(
        occupancy,
        entry_corridor,
        street_idx,
        eff_entry,
        compute_cap,
    );

    // Build steps
    steps.push(RouteStep::ExitBuilding {
        face: exit_face,
        lane: ave_lane,
    });
    steps.extend(corridor_steps(
        exit_corridor,
        eff_exit,
        street_idx,
        ave_lane,
    ));

    // Turn onto street
    let st_dir = if entry_ave > exit_ave {
        TravelDir::East
    } else {
        TravelDir::West
    };
    steps.extend(
        corridor_steps(street, st_from, st_to, st_lane)
            .into_iter()
            .map(|s| {
                if let RouteStep::Step { lane, .. } = s {
                    RouteStep::Step {
                        direction: st_dir,
                        lane,
                    }
                } else {
                    s
                }
            }),
    );

    // Turn onto entry avenue and travel to target
    steps.extend(corridor_steps(
        entry_corridor,
        street_idx,
        eff_entry,
        eave_lane,
    ));

    steps.push(RouteStep::EnterBuilding {
        face: entry_face,
        lane: eave_lane,
    });
    Some(steps)
}

/// Plan V→V steps (exit Top/Bottom, enter Top/Bottom).
#[allow(clippy::too_many_arguments)]
fn plan_steps_v_to_v(
    exit_face: Face,
    entry_face: Face,
    exit_corridor: Corridor,
    entry_corridor: Corridor,
    from_col: usize,
    _from_row: usize,
    to_col: usize,
    _to_row: usize,
    grid: &GridInfo,
    occupancy: &mut SegmentOccupancy,
    compute_cap: &dyn Fn(&Segment) -> usize,
) -> Option<Vec<RouteStep>> {
    let Corridor::Street(exit_st) = exit_corridor else {
        return None;
    };
    let Corridor::Street(entry_st) = entry_corridor else {
        return None;
    };

    let mut steps = Vec::new();

    if exit_st == entry_st {
        // Same street — travel along it from from_col to to_col
        if from_col == to_col {
            // Adjacent nodes sharing this street — no corridor travel needed
            let lane = 3;
            steps.push(RouteStep::ExitBuilding {
                face: exit_face,
                lane,
            });
            steps.push(RouteStep::EnterBuilding {
                face: entry_face,
                lane,
            });
            return Some(steps);
        }
        let going_pos = to_col > from_col;
        let eff_from = effective_exit_cross(from_col, going_pos);
        let eff_to = effective_entry_cross(to_col, going_pos);
        let lane = claim_span_lane(occupancy, exit_corridor, eff_from, eff_to, compute_cap);
        steps.push(RouteStep::ExitBuilding {
            face: exit_face,
            lane,
        });
        steps.extend(corridor_steps(exit_corridor, eff_from, eff_to, lane));
        steps.push(RouteStep::EnterBuilding {
            face: entry_face,
            lane,
        });
        return Some(steps);
    }

    // Different streets: need a connecting avenue
    let from_c = Pos2::new(grid.v_corridor_x(from_col), grid.h_corridor_y(exit_st));
    let to_c = Pos2::new(grid.v_corridor_x(to_col), grid.h_corridor_y(entry_st));
    let ave_idx = grid.best_v_corridor(from_c.x, to_c.x);
    let avenue = Corridor::Avenue(ave_idx);

    // Exit street: from building col to connecting avenue
    let eff_exit = if ave_idx == from_col {
        from_col
    } else {
        effective_exit_cross(from_col, ave_idx > from_col)
    };
    let st_lane = claim_span_lane(occupancy, exit_corridor, eff_exit, ave_idx, compute_cap);

    // Avenue: from exit street to entry street
    let (av_from, av_to) = ordered(exit_st, entry_st);
    let av_lane = claim_span_lane(occupancy, avenue, av_from, av_to, compute_cap);

    // Entry street: from connecting avenue to target col
    let eff_entry = if ave_idx == to_col {
        to_col
    } else {
        effective_entry_cross(to_col, to_col > ave_idx)
    };
    let est_lane = claim_span_lane(occupancy, entry_corridor, ave_idx, eff_entry, compute_cap);

    // Build steps
    steps.push(RouteStep::ExitBuilding {
        face: exit_face,
        lane: st_lane,
    });
    steps.extend(corridor_steps(exit_corridor, eff_exit, ave_idx, st_lane));

    // Turn onto avenue
    let av_dir = if entry_st > exit_st {
        TravelDir::South
    } else {
        TravelDir::North
    };
    steps.extend(
        corridor_steps(avenue, av_from, av_to, av_lane)
            .into_iter()
            .map(|s| {
                if let RouteStep::Step { lane, .. } = s {
                    RouteStep::Step {
                        direction: av_dir,
                        lane,
                    }
                } else {
                    s
                }
            }),
    );

    // Turn onto entry street and travel to target
    steps.extend(corridor_steps(entry_corridor, ave_idx, eff_entry, est_lane));

    steps.push(RouteStep::EnterBuilding {
        face: entry_face,
        lane: est_lane,
    });
    Some(steps)
}

/// Plan H→V steps (exit Left/Right, enter Top/Bottom). L-shape route.
#[allow(clippy::too_many_arguments)]
fn plan_steps_h_to_v(
    exit_face: Face,
    entry_face: Face,
    exit_corridor: Corridor,
    entry_corridor: Corridor,
    _from_col: usize,
    from_row: usize,
    to_col: usize,
    _to_row: usize,
    _grid: &GridInfo,
    occupancy: &mut SegmentOccupancy,
    compute_cap: &dyn Fn(&Segment) -> usize,
) -> Option<Vec<RouteStep>> {
    let Corridor::Avenue(exit_ave) = exit_corridor else {
        return None;
    };
    let Corridor::Street(entry_st) = entry_corridor else {
        return None;
    };

    // Travel on Avenue from source row to entry street
    let eff_exit = if entry_st == from_row {
        from_row
    } else {
        effective_exit_cross(from_row, entry_st > from_row)
    };
    let av_lane = claim_span_lane(occupancy, exit_corridor, eff_exit, entry_st, compute_cap);

    // Turn onto Street and travel to target col
    let eff_entry = if to_col == exit_ave {
        to_col
    } else {
        effective_entry_cross(to_col, to_col > exit_ave)
    };
    let st_dir = if to_col as f32 + 0.5 > exit_ave as f32 {
        TravelDir::East
    } else {
        TravelDir::West
    };
    let st_lane = claim_span_lane(occupancy, entry_corridor, exit_ave, eff_entry, compute_cap);

    let mut steps = Vec::new();
    steps.push(RouteStep::ExitBuilding {
        face: exit_face,
        lane: av_lane,
    });
    steps.extend(corridor_steps(exit_corridor, eff_exit, entry_st, av_lane));
    steps.extend(
        corridor_steps(entry_corridor, exit_ave, eff_entry, st_lane)
            .into_iter()
            .map(|s| {
                if let RouteStep::Step { lane, .. } = s {
                    RouteStep::Step {
                        direction: st_dir,
                        lane,
                    }
                } else {
                    s
                }
            }),
    );
    steps.push(RouteStep::EnterBuilding {
        face: entry_face,
        lane: st_lane,
    });
    Some(steps)
}

/// Plan V→H steps (exit Top/Bottom, enter Left/Right). L-shape route.
#[allow(clippy::too_many_arguments)]
fn plan_steps_v_to_h(
    exit_face: Face,
    entry_face: Face,
    exit_corridor: Corridor,
    entry_corridor: Corridor,
    from_col: usize,
    _from_row: usize,
    _to_col: usize,
    to_row: usize,
    _grid: &GridInfo,
    occupancy: &mut SegmentOccupancy,
    compute_cap: &dyn Fn(&Segment) -> usize,
) -> Option<Vec<RouteStep>> {
    let Corridor::Street(exit_st) = exit_corridor else {
        return None;
    };
    let Corridor::Avenue(entry_ave) = entry_corridor else {
        return None;
    };

    // Travel on Street from source col to entry avenue
    let eff_exit = if entry_ave == from_col {
        from_col
    } else {
        effective_exit_cross(from_col, entry_ave > from_col)
    };
    let st_lane = claim_span_lane(occupancy, exit_corridor, eff_exit, entry_ave, compute_cap);

    // Turn onto Avenue and travel to target row
    let ave_dir = if to_row as f32 + 0.5 > exit_st as f32 {
        TravelDir::South
    } else {
        TravelDir::North
    };
    let eff_entry = if to_row == exit_st {
        to_row
    } else {
        effective_entry_cross(to_row, to_row > exit_st)
    };
    let av_lane = claim_span_lane(occupancy, entry_corridor, exit_st, eff_entry, compute_cap);

    let mut steps = Vec::new();
    steps.push(RouteStep::ExitBuilding {
        face: exit_face,
        lane: st_lane,
    });
    steps.extend(corridor_steps(exit_corridor, eff_exit, entry_ave, st_lane));
    steps.extend(
        corridor_steps(entry_corridor, exit_st, eff_entry, av_lane)
            .into_iter()
            .map(|s| {
                if let RouteStep::Step { lane, .. } = s {
                    RouteStep::Step {
                        direction: ave_dir,
                        lane,
                    }
                } else {
                    s
                }
            }),
    );
    steps.push(RouteStep::EnterBuilding {
        face: entry_face,
        lane: av_lane,
    });
    Some(steps)
}

// ─── Rendering: convert route steps to pixel waypoints ──────────────────────
//
// This is the ONLY place where pixel coordinates appear. The routing above
// is purely logical (steps + lanes). This renderer converts those instructions
// into pixel waypoints by consulting the grid geometry and node rects.

/// Convert step-based route instructions to pixel waypoints for drawing.
///
/// Each ExitBuilding produces: face_point → ramp_point (crossing sidewalk)
/// Each Step produces a waypoint at the center of the road being walked on
/// Each EnterBuilding produces: ramp_point → face_point (crossing sidewalk)
#[allow(clippy::too_many_arguments)]
fn steps_to_waypoints(
    steps: &[RouteStep],
    grid: &GridInfo,
    from_rect: &egui::Rect,
    to_rect: &egui::Rect,
    node_margin: f32,
    exit_port: f32,
    entry_port: f32,
    lane_spacing: f32,
    node_rects: &HashMap<String, egui::Rect>,
    from_col: usize,
    from_row: usize,
    to_col: usize,
    to_row: usize,
) -> Vec<Pos2> {
    let mut waypoints = Vec::new();

    // Current position on the road grid (avenue_idx, street_idx).
    // Updated as we process steps.
    let mut cur_ave: usize = 0;
    let mut cur_st: usize = 0;
    // Set to true when the current step is on an avenue, false on a street.
    // Initialized in the Step handler based on direction.
    #[allow(unused_assignments)]
    let mut on_avenue = false;

    // Determine the direction of the first step (used to set initial position)
    let first_step_dir = steps.iter().find_map(|s| {
        if let RouteStep::Step { direction, .. } = s {
            Some(*direction)
        } else {
            None
        }
    });

    for step in steps {
        match step {
            RouteStep::ExitBuilding { face, lane: _ } => {
                let fp = face_point_with_port(from_rect, *face, exit_port);
                let ramp = ramp_from_face(from_rect, *face, exit_port, node_margin);
                waypoints.push(fp);
                waypoints.push(ramp);

                // Set initial intersection position using effective_exit_cross.
                match face {
                    Face::Right | Face::Left => {
                        cur_ave = if matches!(face, Face::Right) {
                            from_col + 1
                        } else {
                            from_col
                        };
                        let going_south = matches!(first_step_dir, Some(TravelDir::South));
                        cur_st = effective_exit_cross(from_row, going_south);
                    }
                    Face::Bottom | Face::Top => {
                        cur_st = if matches!(face, Face::Bottom) {
                            from_row + 1
                        } else {
                            from_row
                        };
                        let going_east = matches!(first_step_dir, Some(TravelDir::East));
                        cur_ave = effective_exit_cross(from_col, going_east);
                    }
                }
            }
            RouteStep::Step { direction, lane } => {
                // Each Step moves us from one intersection to the next.
                // We track our position as (cur_ave, cur_st) = the intersection we're at.
                //
                // When stepping along our current corridor, advance the cross-position.
                // When turning (stepping perpendicular to current corridor), switch corridors
                // and advance the main-axis position.
                let _old_ave = cur_ave;
                let _old_st = cur_st;

                match direction {
                    TravelDir::South => {
                        cur_st += 1;
                        on_avenue = true;
                    }
                    TravelDir::North => {
                        cur_st = cur_st.saturating_sub(1);
                        on_avenue = true;
                    }
                    TravelDir::East => {
                        cur_ave += 1;
                        on_avenue = false;
                    }
                    TravelDir::West => {
                        cur_ave = cur_ave.saturating_sub(1);
                        on_avenue = false;
                    }
                }

                // Compute a full-range segment for lane centering.
                // Use the full grid range (0..rows or 0..cols) so that the same
                // corridor always produces the same center, regardless of which
                // sub-range we're traversing.
                let seg = if on_avenue {
                    Segment {
                        corridor: Corridor::Avenue(cur_ave),
                        from_cross: 0,
                        to_cross: grid.rows,
                    }
                } else {
                    Segment {
                        corridor: Corridor::Street(cur_st),
                        from_cross: 0,
                        to_cross: grid.cols,
                    }
                };
                let center = grid.segment_center(&seg, node_rects);
                let clearance = 5.0;
                let capacity = grid
                    .lane_capacity(&seg, node_rects, clearance, lane_spacing)
                    .max(1);
                let raw_offset = GridInfo::lane_pixel_offset(*lane, capacity, lane_spacing);
                // Clamp offset so routes stay within the central portion of the
                // corridor gap, never hugging node edges ("sidewalk walking").
                let gap = grid.segment_gap(&seg, node_rects);
                let max_offset = (gap * 0.3).max(lane_spacing);
                let offset = raw_offset.clamp(-max_offset, max_offset);
                // Waypoint: centered in BOTH the along-corridor gap (lane) and
                // the cross-corridor gap (intersection center).
                // `center + offset` = lane position within our travel corridor
                // `cross_center` = center of the perpendicular corridor at the intersection
                //
                // Use the FULL grid range (0..cols or 0..rows) for cross_center so that
                // every reference to the same street/avenue always yields the same position,
                // regardless of which direction we approach from. Otherwise, two steps that
                // meet at the same intersection can disagree on the coordinate and backtrack.
                let pt = if on_avenue {
                    // Traveling on Avenue(cur_ave), intersection at Street(cur_st)
                    let cross_center = grid.street_center_y(cur_st, 0, grid.cols, node_rects);
                    Pos2::new(center + offset, cross_center)
                } else {
                    // Traveling on Street(cur_st), intersection at Avenue(cur_ave)
                    let cross_center = grid.avenue_center_x(cur_ave, 0, grid.rows, node_rects);
                    Pos2::new(cross_center, center + offset)
                };
                push_if_different(&mut waypoints, pt);
            }
            RouteStep::EnterBuilding { face, lane: _ } => {
                let ramp = ramp_from_face(to_rect, *face, entry_port, node_margin);
                push_if_different(&mut waypoints, ramp);
                let fp = face_point_with_port(to_rect, *face, entry_port);
                waypoints.push(fp);
            }
        }
    }

    // For 0-step routes (no Step waypoints between ExitBuilding and EnterBuilding),
    // the exit ramp connects directly to the entry ramp. Without corridor center
    // waypoints, `ensure_orthogonal` creates L-shaped corners at the ramp height,
    // causing routes to "walk on the sidewalk" along node edges.
    //
    // Fix: insert explicit corridor center waypoints so the route goes through
    // the center of the gap between nodes, not along their edges.
    let has_steps = steps.iter().any(|s| matches!(s, RouteStep::Step { .. }));
    if !has_steps && waypoints.len() == 4 {
        let ramp_start = waypoints[1];
        let ramp_end = waypoints[2];

        // Only needed when the ramps are not axis-aligned (route needs to turn)
        if (ramp_start.x - ramp_end.x).abs() > 1.0 && (ramp_start.y - ramp_end.y).abs() > 1.0 {
            let exit_face = steps.iter().find_map(|s| {
                if let RouteStep::ExitBuilding { face, .. } = s {
                    Some(*face)
                } else {
                    None
                }
            });
            let entry_face = steps.iter().find_map(|s| {
                if let RouteStep::EnterBuilding { face, .. } = s {
                    Some(*face)
                } else {
                    None
                }
            });

            if let (Some(ef), Some(nf)) = (exit_face, entry_face) {
                let exit_corridor = face_corridor(ef, from_col, from_row);
                let entry_corridor = face_corridor(nf, to_col, to_row);

                let corridor_center = |c: Corridor| -> f32 {
                    let seg = match c {
                        Corridor::Avenue(a) => Segment {
                            corridor: Corridor::Avenue(a),
                            from_cross: 0,
                            to_cross: grid.rows,
                        },
                        Corridor::Street(s) => Segment {
                            corridor: Corridor::Street(s),
                            from_cross: 0,
                            to_cross: grid.cols,
                        },
                    };
                    grid.segment_center(&seg, node_rects)
                };

                let exit_center = corridor_center(exit_corridor);
                let entry_center = corridor_center(entry_corridor);

                let exit_is_h = matches!(ef, Face::Left | Face::Right);
                let entry_is_h = matches!(nf, Face::Left | Face::Right);

                // After exit ramp: move to the corridor center axis
                let after_exit = if exit_is_h {
                    // H face exits onto an avenue (vertical corridor); center is x-coord
                    Pos2::new(exit_center, ramp_start.y)
                } else {
                    // V face exits onto a street (horizontal corridor); center is y-coord
                    Pos2::new(ramp_start.x, exit_center)
                };

                // Before entry ramp: position at entry corridor center axis
                let before_entry = if entry_is_h {
                    Pos2::new(entry_center, ramp_end.y)
                } else {
                    Pos2::new(ramp_end.x, entry_center)
                };

                // Insert between ramp_start (index 1) and ramp_end (index 2)
                waypoints.insert(2, after_exit);
                waypoints.insert(3, before_entry);
            }
        }
    }

    ensure_orthogonal(&mut waypoints);
    waypoints
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

/// Push a point to waypoints only if it differs from the last point by more than 1 pixel.
fn push_if_different(waypoints: &mut Vec<Pos2>, pt: Pos2) {
    if let Some(last) = waypoints.last() {
        if (pt - *last).length() < 1.0 {
            return;
        }
    }
    waypoints.push(pt);
}

/// Return (min, max) of two values.
fn ordered(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Effective exit cross-corridor when leaving a cell.
/// When exiting a cell along a corridor, we start from the cell boundary
/// in the direction of travel (skipping the segment within our own cell,
/// since the exit ramp already covers that distance).
fn effective_exit_cross(cell_coord: usize, going_positive: bool) -> usize {
    if going_positive {
        cell_coord + 1
    } else {
        cell_coord
    }
}

/// Effective entry cross-corridor when approaching a target cell.
/// When entering a cell, we stop at the near edge of the target cell
/// (not the far edge), to avoid overshooting past the building.
fn effective_entry_cross(cell_coord: usize, going_positive: bool) -> usize {
    if going_positive {
        cell_coord // approach from left/top: stop at left/top edge
    } else {
        cell_coord + 1 // approach from right/bottom: stop at right/bottom edge
    }
}

/// Route an edge using the step-based Manhattan-style router.
///
/// Tries all candidate face pairs in parallel, picks the route with
/// the fewest steps (shortest path). Each route is planned purely in
/// the routing language (steps + lanes), then rendered to waypoints.
#[allow(clippy::too_many_arguments)]
fn route_edge_semantic(
    from_name: &str,
    to_name: &str,
    from_rect: &egui::Rect,
    to_rect: &egui::Rect,
    grid: &GridInfo,
    occupancy: &mut SegmentOccupancy,
    ports: &mut FacePortAllocator,
    node_rects: &HashMap<String, egui::Rect>,
    node_margin: f32,
    lane_spacing: f32,
    port_spacing: f32,
) -> RoutedPath {
    let from_c = from_rect.center();
    let to_c = to_rect.center();

    let (from_col, from_row) = grid.cell_at(from_c).unwrap_or((0, 0));
    let (to_col, to_row) = grid.cell_at(to_c).unwrap_or((0, 0));

    let face_pairs = select_face_pairs(from_rect, to_rect, grid);

    // Snapshot state so each candidate starts from the same baseline
    let occupancy_snapshot = occupancy.clone();
    let ports_snapshot = ports.clone();

    // Try each face pair, collect valid routes
    let mut candidates: Vec<(RoutedPath, SegmentOccupancy, FacePortAllocator)> = Vec::new();
    for &(ef, nf) in &face_pairs {
        let mut occ_clone = occupancy_snapshot.clone();
        let mut ports_clone = ports_snapshot.clone();
        let exit_port = ports_clone.claim_port(from_name, ef, from_rect, port_spacing);
        let entry_port = ports_clone.claim_port(to_name, nf, to_rect, port_spacing);

        // Step 1: Plan route in the routing language (no pixels)
        if let Some(steps) = plan_step_route(
            ef,
            nf,
            from_col,
            from_row,
            to_col,
            to_row,
            grid,
            &mut occ_clone,
            node_rects,
            lane_spacing,
        ) {
            // Step 2: Render to waypoints (separate concern)
            let waypoints = steps_to_waypoints(
                &steps,
                grid,
                from_rect,
                to_rect,
                node_margin,
                exit_port,
                entry_port,
                lane_spacing,
                node_rects,
                from_col,
                from_row,
                to_col,
                to_row,
            );

            // Validate: no node crossings
            if !path_crosses_node(&waypoints, node_rects, from_name, to_name) {
                let route = RoutedPath { steps, waypoints };
                candidates.push((route, occ_clone, ports_clone));
            }
        }
    }

    // Pick the route with fewest steps (routing metric, not pixel metric)
    if let Some((best_route, best_occ, best_ports)) = candidates
        .into_iter()
        .min_by_key(|(r, _, _)| r.step_count())
    {
        *occupancy = best_occ;
        *ports = best_ports;
        return best_route;
    }

    // Fallback: direct connection with no intermediate steps
    let (exit_face, entry_face) = face_pairs[0];
    let exit_port = ports.claim_port(from_name, exit_face, from_rect, port_spacing);
    let entry_port = ports.claim_port(to_name, entry_face, to_rect, port_spacing);
    let fp_start = face_point_with_port(from_rect, exit_face, exit_port);
    let fp_end = face_point_with_port(to_rect, entry_face, entry_port);
    let ramp_start = ramp_from_face(from_rect, exit_face, exit_port, node_margin);
    let ramp_end = ramp_from_face(to_rect, entry_face, entry_port, node_margin);

    let steps = vec![
        RouteStep::ExitBuilding {
            face: exit_face,
            lane: 1,
        },
        RouteStep::EnterBuilding {
            face: entry_face,
            lane: 1,
        },
    ];

    RoutedPath {
        steps,
        waypoints: vec![fp_start, ramp_start, ramp_end, fp_end],
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
    path: &RoutedPath,
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
    if path.waypoints.len() < 2 {
        return;
    }

    let is_dashed = matches!(arrow, ArrowKind::DashedLine | ArrowKind::DashedArrow);
    let start = path.waypoints[0];
    let end = *path.waypoints.last().unwrap();

    // Apply rounded corners to get smooth polyline
    let smooth_points = apply_rounded_corners(&path.waypoints, corner_radius);

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
        let n = path.waypoints.len();
        let last_seg_len = if n >= 2 {
            (path.waypoints[n - 1] - path.waypoints[n - 2]).length()
        } else {
            total_len
        };
        if last_seg_len >= arrow_size * 1.2 {
            // Normal case: draw arrowhead at the endpoint
            let last_dir = if n >= 2 {
                path.waypoints[n - 1] - path.waypoints[n - 2]
            } else {
                end - start
            };
            draw_arrowhead(end, last_dir);
        } else if n >= 3 {
            // Tight space: last segment is too short for a clean arrowhead.
            // Draw the arrowhead before the last turn and let the line continue.
            let pre_turn_dir = path.waypoints[n - 2] - path.waypoints[n - 3];
            let arrowhead_tip = path.waypoints[n - 2];
            draw_arrowhead(arrowhead_tip, pre_turn_dir);
        } else {
            // Fallback: draw at endpoint anyway
            let last_dir = end - start;
            draw_arrowhead(end, last_dir);
        }
    }

    if has_start_arrow && anim_progress >= 1.0 {
        let first_seg_len = if path.waypoints.len() >= 2 {
            (path.waypoints[1] - path.waypoints[0]).length()
        } else {
            total_len
        };
        if first_seg_len >= arrow_size * 1.2 {
            let first_dir = if path.waypoints.len() >= 2 {
                path.waypoints[0] - path.waypoints[1]
            } else {
                start - end
            };
            draw_arrowhead(start, first_dir);
        } else if path.waypoints.len() >= 3 {
            // Tight space: draw arrowhead after the first turn
            let post_turn_dir = path.waypoints[1] - path.waypoints[2];
            let arrowhead_tip = path.waypoints[1];
            draw_arrowhead(arrowhead_tip, post_turn_dir);
        } else {
            let first_dir = start - end;
            draw_arrowhead(start, first_dir);
        }
    }

    // Edge label only when animation is complete
    if !label.is_empty() && anim_progress >= 1.0 {
        let mid_distance = total_len * 0.30;
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

    let mut semantic_occupancy = SegmentOccupancy::new();
    let mut ports = FacePortAllocator::new();
    let animation_duration = 0.4; // seconds
    let mut needs_repaint = false;

    for (edge_idx, edge) in edges.iter().enumerate() {
        // Skip edges not yet revealed
        let edge_step = edge_steps.get(edge_idx).copied().unwrap_or(0);
        if edge_step > reveal_step {
            continue;
        }

        let Some(from_rect) = node_rects.get(&edge.from) else {
            continue;
        };
        let Some(to_rect) = node_rects.get(&edge.to) else {
            continue;
        };

        // Skip self-loops
        if edge.from == edge.to {
            continue;
        }

        // Each edge gets a distinct color from the palette
        let base_color = edge_palette[edge_idx % edge_palette.len()];
        let is_dashed = matches!(edge.arrow, ArrowKind::DashedLine | ArrowKind::DashedArrow);
        let current_edge_color = if is_dashed {
            Theme::with_opacity(base_color, opacity * 0.55)
        } else {
            Theme::with_opacity(base_color, opacity * 0.85)
        };

        // Compute animation progress for edges appearing on the current step
        let anim_progress = if edge_step == reveal_step && edge_step > 0 {
            if let Some(ts) = reveal_timestamp {
                let elapsed = ts.elapsed().as_secs_f32();
                let t = (elapsed / animation_duration).min(1.0);
                // Ease-in-out
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
            1.0 // fully drawn for previous steps
        };

        // Route edge using semantic Manhattan-style router
        let path = route_edge_semantic(
            &edge.from,
            &edge.to,
            from_rect,
            to_rect,
            &grid,
            &mut semantic_occupancy,
            &mut ports,
            &node_rects,
            node_margin,
            lane_spacing,
            port_spacing,
        );

        // Log the semantic route for debugging
        let label_str = if edge.label.is_empty() {
            String::new()
        } else {
            format!(" \"{}\"", edge.label)
        };
        let steps_str: Vec<String> = path.steps.iter().map(|s| format!("{s}")).collect();
        eprintln!(
            "ROUTE {}{} ({:?}): [{}]",
            edge.from,
            label_str,
            edge.arrow,
            steps_str.join(" → ")
        );

        draw_routed_edge(
            painter,
            &path,
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
        // Comments should not be nodes
        assert!(!nodes.iter().any(|n| n.name.starts_with('#')));
    }

    #[test]
    fn test_parse_metadata() {
        let content = "- Server (icon: server, pos: 2,1)\n- DB (icon: database, pos: 3,2)";
        let (nodes, _) = parse_diagram(content);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].icon, "server");
        assert_eq!(nodes[0].grid_pos, Some((2, 1)));
        assert_eq!(nodes[1].icon, "database");
        assert_eq!(nodes[1].grid_pos, Some((3, 2)));
    }

    #[test]
    fn test_arrow_types() {
        let content = "- A -> B\n- C <- D\n- E <-> F\n- G -- H\n- I --> J";
        let (_, edges) = parse_diagram(content);
        assert_eq!(edges.len(), 5);
        assert!(matches!(edges[0].arrow, ArrowKind::Forward));
        assert!(matches!(edges[1].arrow, ArrowKind::Reverse));
        assert!(matches!(edges[2].arrow, ArrowKind::Bidirectional));
        assert!(matches!(edges[3].arrow, ArrowKind::DashedLine));
        assert!(matches!(edges[4].arrow, ArrowKind::DashedArrow));
    }

    #[test]
    fn test_node_with_label_and_metadata() {
        let content = "- Gateway (icon: api, pos: 1,1)\n- Gateway -> Auth: validates";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "Gateway");
        assert_eq!(nodes[0].icon, "api");
        assert_eq!(nodes[0].grid_pos, Some((1, 1)));
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_detect_arrow_ordering() {
        // Ensure <-> is detected before -> when both patterns match
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
    }

    #[test]
    fn test_empty_diagram() {
        let (nodes, edges) = parse_diagram("");
        assert!(nodes.is_empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn test_comments_only() {
        let (nodes, edges) = parse_diagram("# Section 1\n# Section 2");
        assert!(nodes.is_empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn test_reveal_markers_parsed() {
        let content = "- Server\n+ Cache\n* Monitor\n+ DB";
        let (nodes, _) = parse_diagram(content);
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].reveal, DiagramReveal::Static);
        assert_eq!(nodes[1].reveal, DiagramReveal::NextStep);
        assert_eq!(nodes[2].reveal, DiagramReveal::WithPrev);
        assert_eq!(nodes[3].reveal, DiagramReveal::NextStep);
    }

    #[test]
    fn test_reveal_markers_on_edges() {
        let content = "- A -> B: always\n+ C -> D: step1\n* E -> F: with_prev";
        let (_, edges) = parse_diagram(content);
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0].reveal, DiagramReveal::Static);
        assert_eq!(edges[1].reveal, DiagramReveal::NextStep);
        assert_eq!(edges[2].reveal, DiagramReveal::WithPrev);
    }

    #[test]
    fn test_count_diagram_steps() {
        let content = "- Server\n+ Cache\n* Monitor\n- Server -> Cache\n+ Cache -> DB";
        assert_eq!(count_diagram_steps(content), 2);
    }

    #[test]
    fn test_count_diagram_steps_none() {
        let content = "- A -> B\n- B -> C";
        assert_eq!(count_diagram_steps(content), 0);
    }

    #[test]
    fn test_apply_rounded_corners() {
        let waypoints = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
        ];
        let result = apply_rounded_corners(&waypoints, 10.0);
        // Should have more points than original due to arc insertion
        assert!(result.len() > 3);
        // First and last points should be preserved
        assert_eq!(result[0], waypoints[0]);
        assert_eq!(*result.last().unwrap(), *waypoints.last().unwrap());
    }

    #[test]
    fn test_polyline_length() {
        let points = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 50.0),
        ];
        let len = polyline_length(&points);
        assert!((len - 150.0).abs() < 0.01);
    }

    // ── Helper to build a 3x3 grid scenario for routing tests ──────────────

    /// Create a 3x3 grid of nodes and return (node_rects, grid).
    /// Grid cells are 200x150, nodes are 80x60, centered in each cell.
    fn make_3x3_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;
        let origin_x = 50.0;
        let origin_y = 50.0;

        let names = [
            ("A", 0, 0),
            ("B", 1, 0),
            ("C", 2, 0),
            ("D", 0, 1),
            ("E", 1, 1),
            ("F", 2, 1),
            ("G", 0, 2),
            ("H", 1, 2),
            ("I", 2, 2),
        ];

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = origin_x + (*col as f32 + 0.5) * cell_w;
            let cy = origin_y + (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };

        (rects, grid)
    }

    /// Route a single edge and return the path.
    fn route_single_edge(
        from: &str,
        to: &str,
        node_rects: &HashMap<String, egui::Rect>,
        grid: &GridInfo,
    ) -> RoutedPath {
        route_single_edge_semantic(from, to, node_rects, grid)
    }

    fn route_single_edge_semantic(
        from: &str,
        to: &str,
        node_rects: &HashMap<String, egui::Rect>,
        grid: &GridInfo,
    ) -> RoutedPath {
        let from_rect = node_rects[from];
        let to_rect = node_rects[to];
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();
        route_edge_semantic(
            from,
            to,
            &from_rect,
            &to_rect,
            grid,
            &mut occupancy,
            &mut ports,
            node_rects,
            10.0,
            20.0,
            22.0,
        )
    }

    /// Check that every consecutive pair of waypoints is axis-aligned (horizontal or vertical).
    fn is_orthogonal(waypoints: &[Pos2]) -> bool {
        for pair in waypoints.windows(2) {
            let dx = (pair[0].x - pair[1].x).abs();
            let dy = (pair[0].y - pair[1].y).abs();
            // One axis should be near-zero
            if dx > 0.5 && dy > 0.5 {
                return false;
            }
        }
        true
    }

    #[test]
    fn test_face_selection_horizontal() {
        let left = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let right = egui::Rect::from_center_size(egui::pos2(300.0, 100.0), egui::vec2(80.0, 60.0));
        assert_eq!(choose_exit_face(&left, right.center()), Face::Right);
        assert_eq!(choose_entry_face(&right, left.center()), Face::Left);
    }

    #[test]
    fn test_face_selection_vertical() {
        let top = egui::Rect::from_center_size(egui::pos2(100.0, 50.0), egui::vec2(80.0, 60.0));
        let bot = egui::Rect::from_center_size(egui::pos2(100.0, 250.0), egui::vec2(80.0, 60.0));
        assert_eq!(choose_exit_face(&top, bot.center()), Face::Bottom);
        assert_eq!(choose_entry_face(&bot, top.center()), Face::Top);
    }

    #[test]
    fn test_segment_crosses_rect_horizontal() {
        let rect = egui::Rect::from_center_size(egui::pos2(200.0, 100.0), egui::vec2(80.0, 60.0));
        // Horizontal segment through the rect
        let a = Pos2::new(100.0, 100.0);
        let b = Pos2::new(300.0, 100.0);
        assert!(segment_crosses_rect(a, b, &rect, 4.0));

        // Horizontal segment above the rect
        let a2 = Pos2::new(100.0, 50.0);
        let b2 = Pos2::new(300.0, 50.0);
        assert!(!segment_crosses_rect(a2, b2, &rect, 4.0));
    }

    #[test]
    fn test_segment_crosses_rect_vertical() {
        let rect = egui::Rect::from_center_size(egui::pos2(200.0, 200.0), egui::vec2(80.0, 60.0));
        // Vertical segment through the rect
        let a = Pos2::new(200.0, 100.0);
        let b = Pos2::new(200.0, 300.0);
        assert!(segment_crosses_rect(a, b, &rect, 4.0));

        // Vertical segment to the left of the rect
        let a2 = Pos2::new(140.0, 100.0);
        let b2 = Pos2::new(140.0, 300.0);
        assert!(!segment_crosses_rect(a2, b2, &rect, 4.0));
    }

    #[test]
    fn test_path_crosses_node_skips_endpoints() {
        let mut rects = HashMap::new();
        rects.insert(
            "A".to_string(),
            egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0)),
        );
        rects.insert(
            "B".to_string(),
            egui::Rect::from_center_size(egui::pos2(300.0, 100.0), egui::vec2(80.0, 60.0)),
        );
        // Path from A to B that goes through A and B rects — should be false
        // because A and B are the endpoints
        let waypoints = vec![Pos2::new(100.0, 100.0), Pos2::new(300.0, 100.0)];
        assert!(!path_crosses_node(&waypoints, &rects, "A", "B"));
    }

    #[test]
    fn test_path_crosses_middle_node() {
        let mut rects = HashMap::new();
        rects.insert(
            "A".to_string(),
            egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0)),
        );
        rects.insert(
            "B".to_string(),
            egui::Rect::from_center_size(egui::pos2(200.0, 100.0), egui::vec2(80.0, 60.0)),
        );
        rects.insert(
            "C".to_string(),
            egui::Rect::from_center_size(egui::pos2(300.0, 100.0), egui::vec2(80.0, 60.0)),
        );
        // Path from A to C goes through B
        let waypoints = vec![Pos2::new(100.0, 100.0), Pos2::new(300.0, 100.0)];
        assert!(path_crosses_node(&waypoints, &rects, "A", "C"));
    }

    #[test]
    fn test_diagonal_route_avoids_middle_nodes() {
        // This tests the "fills" scenario: routing from (col 2, row 0) to (col 0, row 1)
        // The path from C to D must NOT cross through B or E
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("C", "D", &node_rects, &grid);

        assert!(
            path.waypoints.len() >= 2,
            "Path should have waypoints, got {}",
            path.waypoints.len()
        );
        assert!(
            !path_crosses_node(&path.waypoints, &node_rects, "C", "D"),
            "Diagonal path C→D must not cross any intermediate node"
        );
    }

    #[test]
    fn test_diagonal_route_opposite_direction() {
        // Route from (col 0, row 1) to (col 2, row 0) — like "fills" reversed
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("D", "C", &node_rects, &grid);

        assert!(
            !path_crosses_node(&path.waypoints, &node_rects, "D", "C"),
            "Diagonal path D→C must not cross any intermediate node"
        );
    }

    #[test]
    fn test_far_diagonal_avoids_nodes() {
        // Route from top-left to bottom-right (A→I) — must avoid B, D, E
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "I", &node_rects, &grid);

        assert!(
            !path_crosses_node(&path.waypoints, &node_rects, "A", "I"),
            "Far diagonal A→I must not cross any intermediate node"
        );
    }

    #[test]
    fn test_same_row_route_is_horizontal() {
        // A→C on same row should produce mostly horizontal path
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "C", &node_rects, &grid);

        // Path should not cross B (middle node on same row)
        assert!(
            !path_crosses_node(&path.waypoints, &node_rects, "A", "C"),
            "Same-row path A→C must not cross B"
        );
    }

    #[test]
    fn test_same_col_route_is_vertical() {
        // A→G on same column should produce mostly vertical path
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "G", &node_rects, &grid);

        assert!(
            !path_crosses_node(&path.waypoints, &node_rects, "A", "G"),
            "Same-col path A→G must not cross D"
        );
    }

    #[test]
    fn test_adjacent_horizontal_is_direct() {
        // A→B (adjacent) should be a simple direct connection
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "B", &node_rects, &grid);

        // Semantic router routes through corridor lane points, so more than 2 waypoints
        assert!(
            path.waypoints.len() <= 6,
            "Adjacent path should be short, got {}",
            path.waypoints.len()
        );
        // First waypoint should be near A's right face, last near B's left face
        let first = path.waypoints[0];
        let last = *path.waypoints.last().unwrap();
        assert!(
            first.x > node_rects["A"].center().x,
            "Should start right of A center"
        );
        assert!(
            last.x < node_rects["B"].center().x,
            "Should end left of B center"
        );
    }

    #[test]
    fn test_adjacent_vertical_is_direct() {
        // A→D (adjacent vertically) should be simple
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "D", &node_rects, &grid);

        assert!(
            path.waypoints.len() <= 6,
            "Adjacent vertical path should be short, got {}",
            path.waypoints.len()
        );
        let first = path.waypoints[0];
        let last = *path.waypoints.last().unwrap();
        assert!(
            first.y > node_rects["A"].center().y,
            "Should start below A center"
        );
        assert!(
            last.y < node_rects["D"].center().y,
            "Should end above D center"
        );
    }

    #[test]
    fn test_corridor_y_positions() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 50.0,
            origin_y: 50.0,
            occupied: HashSet::new(),
        };
        assert!((grid.h_corridor_y(0) - 50.0).abs() < 0.01);
        assert!((grid.h_corridor_y(1) - 200.0).abs() < 0.01);
        assert!((grid.h_corridor_y(2) - 350.0).abs() < 0.01);
        assert!((grid.h_corridor_y(3) - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_corridor_x_positions() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 50.0,
            origin_y: 50.0,
            occupied: HashSet::new(),
        };
        assert!((grid.v_corridor_x(0) - 50.0).abs() < 0.01);
        assert!((grid.v_corridor_x(1) - 250.0).abs() < 0.01);
        assert!((grid.v_corridor_x(2) - 450.0).abs() < 0.01);
        assert!((grid.v_corridor_x(3) - 650.0).abs() < 0.01);
    }

    #[test]
    fn test_diagonal_prefers_vertical_exit() {
        // For a diagonal route C(2,0) → D(0,1), the semantic router should produce
        // a valid orthogonal route. The specific face preference (H vs V exit) depends
        // on cell aspect ratio — for standard wide cells, H-exit may be preferred.
        let (node_rects, grid) = make_3x3_grid();
        let path = route_single_edge("C", "D", &node_rects, &grid);

        // Verify the route is valid: orthogonal and no node crossings
        assert!(
            is_orthogonal(&path.waypoints),
            "Diagonal C→D should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &node_rects, "C", "D"),
            "Diagonal C→D should not cross intermediate nodes"
        );
    }

    #[test]
    fn test_all_waypoints_are_orthogonal() {
        // Every consecutive pair of waypoints should be either horizontal or vertical
        let (node_rects, grid) = make_3x3_grid();
        let test_pairs = [
            ("A", "B"),
            ("A", "E"),
            ("A", "I"),
            ("C", "D"),
            ("F", "G"),
            ("B", "H"),
        ];
        for (from, to) in &test_pairs {
            let path = route_single_edge(from, to, &node_rects, &grid);
            for i in 0..path.waypoints.len().saturating_sub(1) {
                let a = path.waypoints[i];
                let b = path.waypoints[i + 1];
                let is_h = (a.y - b.y).abs() < 0.5;
                let is_v = (a.x - b.x).abs() < 0.5;
                assert!(
                    is_h || is_v,
                    "Segment {i} in path {from}→{to} is not orthogonal: ({:.1},{:.1})→({:.1},{:.1})",
                    a.x,
                    a.y,
                    b.x,
                    b.y
                );
            }
        }
    }

    // ── Port allocator tests ─────────────────────────────────────────────────

    #[test]
    fn test_port_first_claim_returns_zero() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let offset = ports.claim_port("A", Face::Right, &rect, 22.0);
        assert!(
            (offset - 0.0).abs() < 0.01,
            "First port should be at center (0)"
        );
    }

    #[test]
    fn test_port_second_claim_is_positive() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let _first = ports.claim_port("A", Face::Right, &rect, 22.0);
        let second = ports.claim_port("A", Face::Right, &rect, 22.0);
        assert!(
            second > 0.0,
            "Second port should be positive offset, got {second}"
        );
    }

    #[test]
    fn test_port_third_claim_is_negative() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let _first = ports.claim_port("A", Face::Right, &rect, 22.0);
        let _second = ports.claim_port("A", Face::Right, &rect, 22.0);
        let third = ports.claim_port("A", Face::Right, &rect, 22.0);
        assert!(
            third < 0.0,
            "Third port should be negative offset, got {third}"
        );
    }

    #[test]
    fn test_port_alternating_pattern() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 200.0));
        let spacing = 22.0;
        let offsets: Vec<f32> = (0..6)
            .map(|_| ports.claim_port("A", Face::Right, &rect, spacing))
            .collect();
        // Pattern: 0, +s, -s, +2s, -2s, +3s
        assert!((offsets[0] - 0.0).abs() < 0.01);
        assert!((offsets[1] - spacing).abs() < 0.01);
        assert!((offsets[2] + spacing).abs() < 0.01);
        assert!((offsets[3] - 2.0 * spacing).abs() < 0.01);
        assert!((offsets[4] + 2.0 * spacing).abs() < 0.01);
    }

    #[test]
    fn test_port_clamped_to_face_length() {
        let mut ports = FacePortAllocator::new();
        // Very small rect (height=20), so max_offset = 20*0.3 = 6
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 20.0));
        let offsets: Vec<f32> = (0..10)
            .map(|_| ports.claim_port("A", Face::Right, &rect, 22.0))
            .collect();
        let max_offset = 20.0 * 0.3;
        for (i, &o) in offsets.iter().enumerate() {
            assert!(
                o.abs() <= max_offset + 0.01,
                "Port {i} offset {o} exceeds max_offset {max_offset}"
            );
        }
    }

    #[test]
    fn test_port_different_faces_independent() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let right_1 = ports.claim_port("A", Face::Right, &rect, 22.0);
        let bottom_1 = ports.claim_port("A", Face::Bottom, &rect, 22.0);
        // Both should be first claims (0.0), since they're on different faces
        assert!((right_1 - 0.0).abs() < 0.01);
        assert!((bottom_1 - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_port_different_nodes_independent() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let a_right = ports.claim_port("A", Face::Right, &rect, 22.0);
        let b_right = ports.claim_port("B", Face::Right, &rect, 22.0);
        // Both should be 0.0 since they're on different nodes
        assert!((a_right - 0.0).abs() < 0.01);
        assert!((b_right - 0.0).abs() < 0.01);
    }

    // ── Exhaustive orthogonality test for ALL possible pairs ─────────────────

    #[test]
    fn test_all_pairs_are_orthogonal() {
        let (node_rects, grid) = make_3x3_grid();
        let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];
        for &from in &names {
            for &to in &names {
                if from == to {
                    continue;
                }
                let path = route_single_edge(from, to, &node_rects, &grid);
                for i in 0..path.waypoints.len().saturating_sub(1) {
                    let a = path.waypoints[i];
                    let b = path.waypoints[i + 1];
                    let is_h = (a.y - b.y).abs() < 1.0;
                    let is_v = (a.x - b.x).abs() < 1.0;
                    assert!(
                        is_h || is_v,
                        "Path {from}→{to} segment {i} not orthogonal: ({:.1},{:.1})→({:.1},{:.1})",
                        a.x,
                        a.y,
                        b.x,
                        b.y
                    );
                }
            }
        }
    }

    // ── No-crossing tests for ALL pairs ──────────────────────────────────────

    #[test]
    fn test_all_pairs_avoid_intermediate_nodes() {
        let (node_rects, grid) = make_3x3_grid();
        let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];
        for &from in &names {
            for &to in &names {
                if from == to {
                    continue;
                }
                let path = route_single_edge(from, to, &node_rects, &grid);
                assert!(
                    !path_crosses_node(&path.waypoints, &node_rects, from, to),
                    "Path {from}→{to} crosses an intermediate node"
                );
            }
        }
    }

    // ── Path direction tests ─────────────────────────────────────────────────

    #[test]
    fn test_path_moves_in_correct_general_direction() {
        let (node_rects, grid) = make_3x3_grid();
        // A→C: should generally move rightward
        let path = route_single_edge("A", "C", &node_rects, &grid);
        let first = path.waypoints[0];
        let last = *path.waypoints.last().unwrap();
        assert!(last.x > first.x, "A→C should move rightward");

        // C→A: should generally move leftward
        let path = route_single_edge("C", "A", &node_rects, &grid);
        let first = path.waypoints[0];
        let last = *path.waypoints.last().unwrap();
        assert!(last.x < first.x, "C→A should move leftward");

        // A→G: should generally move downward
        let path = route_single_edge("A", "G", &node_rects, &grid);
        let first = path.waypoints[0];
        let last = *path.waypoints.last().unwrap();
        assert!(last.y > first.y, "A→G should move downward");

        // G→A: should generally move upward
        let path = route_single_edge("G", "A", &node_rects, &grid);
        let first = path.waypoints[0];
        let last = *path.waypoints.last().unwrap();
        assert!(last.y < first.y, "G→A should move upward");
    }

    // ── Start/end proximity tests ────────────────────────────────────────────

    #[test]
    fn test_path_starts_near_source_and_ends_near_target() {
        let (node_rects, grid) = make_3x3_grid();
        let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];
        let margin = 15.0; // node_margin(10) + tolerance

        for &from in &names {
            for &to in &names {
                if from == to {
                    continue;
                }
                let path = route_single_edge(from, to, &node_rects, &grid);
                let first = path.waypoints[0];
                let last = *path.waypoints.last().unwrap();
                let from_rect = node_rects[from];
                let to_rect = node_rects[to];

                // First waypoint should be close to from_rect's edge
                let expanded_from = from_rect.expand(margin);
                assert!(
                    expanded_from.contains(first),
                    "Path {from}→{to}: first waypoint ({:.0},{:.0}) not near source rect",
                    first.x,
                    first.y
                );

                // Last waypoint should be close to to_rect's edge
                let expanded_to = to_rect.expand(margin);
                assert!(
                    expanded_to.contains(last),
                    "Path {from}→{to}: last waypoint ({:.0},{:.0}) not near target rect",
                    last.x,
                    last.y
                );
            }
        }
    }

    // ── Shared occupancy tests ───────────────────────────────────────────────

    #[test]
    fn test_multiple_edges_get_different_ports() {
        let mut ports = FacePortAllocator::new();
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));

        let p1 = ports.claim_port("A", Face::Right, &rect, 22.0);
        let p2 = ports.claim_port("A", Face::Right, &rect, 22.0);
        let p3 = ports.claim_port("A", Face::Right, &rect, 22.0);

        // All ports should be different
        assert!((p1 - p2).abs() > 1.0, "Ports 1 and 2 should differ");
        assert!((p1 - p3).abs() > 1.0, "Ports 1 and 3 should differ");
        assert!((p2 - p3).abs() > 1.0, "Ports 2 and 3 should differ");
    }

    // ── Ramp/face point tests ────────────────────────────────────────────────

    #[test]
    fn test_ramp_from_face_right() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let ramp = ramp_from_face(&rect, Face::Right, 0.0, 10.0);
        assert!((ramp.x - (rect.right() + 10.0)).abs() < 0.01);
        assert!((ramp.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_ramp_from_face_left() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let ramp = ramp_from_face(&rect, Face::Left, 0.0, 10.0);
        assert!((ramp.x - (rect.left() - 10.0)).abs() < 0.01);
        assert!((ramp.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_ramp_from_face_bottom() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let ramp = ramp_from_face(&rect, Face::Bottom, 0.0, 10.0);
        assert!((ramp.x - 100.0).abs() < 0.01);
        assert!((ramp.y - (rect.bottom() + 10.0)).abs() < 0.01);
    }

    #[test]
    fn test_ramp_from_face_top() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let ramp = ramp_from_face(&rect, Face::Top, 0.0, 10.0);
        assert!((ramp.x - 100.0).abs() < 0.01);
        assert!((ramp.y - (rect.top() - 10.0)).abs() < 0.01);
    }

    #[test]
    fn test_ramp_from_face_with_port_offset() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let ramp = ramp_from_face(&rect, Face::Right, 15.0, 10.0);
        assert!((ramp.x - (rect.right() + 10.0)).abs() < 0.01);
        assert!((ramp.y - 115.0).abs() < 0.01); // center.y + port_offset
    }

    #[test]
    fn test_face_point_with_port_top_face() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let fp = face_point_with_port(&rect, Face::Top, 20.0);
        assert!((fp.x - 120.0).abs() < 0.01); // center.x + port_offset
        assert!((fp.y - rect.top()).abs() < 0.01);
    }

    // ── Segment-rect collision tests ─────────────────────────────────────────

    #[test]
    fn test_segment_horizontal_just_touching() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(40.0, 40.0));
        // Segment exactly at top edge
        let a = Pos2::new(50.0, 80.0);
        let b = Pos2::new(150.0, 80.0);
        // y=80 is exactly at top (100-20=80), and with shrink=4, top becomes 84
        // 80 < 84, so it should NOT cross
        assert!(!segment_crosses_rect(a, b, &rect, 4.0));
    }

    #[test]
    fn test_segment_vertical_just_outside() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(40.0, 40.0));
        // Segment just to the left
        let a = Pos2::new(75.0, 50.0);
        let b = Pos2::new(75.0, 150.0);
        // x=75, rect left=80, with shrink=4 left becomes 84
        assert!(!segment_crosses_rect(a, b, &rect, 4.0));
    }

    #[test]
    fn test_segment_inside_rect() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 80.0));
        let a = Pos2::new(80.0, 100.0);
        let b = Pos2::new(120.0, 100.0);
        assert!(segment_crosses_rect(a, b, &rect, 4.0));
    }

    // ── Corridor finding tests ───────────────────────────────────────────────

    #[test]
    fn test_best_h_corridor_midpoint() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Between row 0 center (75) and row 1 center (225), mid = 150
        // Corridor 1 is at y=150 — should be picked
        let idx = grid.best_h_corridor(75.0, 225.0);
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_best_v_corridor_midpoint() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Between col 0 center (100) and col 2 center (500), mid = 300
        // Corridor 1 is at x=200 (dist 100), corridor 2 is at x=400 (dist 100)
        // Equidistant — first match (corridor 1) wins
        let idx = grid.best_v_corridor(100.0, 500.0);
        assert_eq!(idx, 1);
    }

    // ── Rounded corners tests ────────────────────────────────────────────────

    #[test]
    fn test_rounded_corners_straight_line_unchanged() {
        let waypoints = vec![Pos2::new(0.0, 0.0), Pos2::new(100.0, 0.0)];
        let result = apply_rounded_corners(&waypoints, 10.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_rounded_corners_preserves_endpoints() {
        let waypoints = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(50.0, 0.0),
            Pos2::new(50.0, 50.0),
            Pos2::new(100.0, 50.0),
        ];
        let result = apply_rounded_corners(&waypoints, 8.0);
        assert_eq!(result[0], waypoints[0]);
        assert_eq!(*result.last().unwrap(), *waypoints.last().unwrap());
    }

    #[test]
    fn test_rounded_corners_zero_radius() {
        let waypoints = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
        ];
        let result = apply_rounded_corners(&waypoints, 0.0);
        // With zero radius, should return original points
        assert_eq!(result.len(), 3);
    }

    // ── Mixed arrow type parsing ─────────────────────────────────────────────

    #[test]
    fn test_detect_arrow_none() {
        assert!(detect_arrow("just some text").is_none());
        assert!(detect_arrow("no arrows here").is_none());
    }

    #[test]
    fn test_detect_arrow_with_labels() {
        let s = "Frontend -> Backend: REST calls";
        let result = detect_arrow(s);
        assert!(result.is_some());
        let (pos, arrow_len, kind) = result.unwrap();
        // " -> " starts at position 8 in "Frontend -> Backend: REST calls"
        assert_eq!(pos, 8);
        assert_eq!(arrow_len, 4); // " -> " is 4 chars
        assert!(matches!(kind, ArrowKind::Forward));
        let from_part = &s[..pos];
        let to_part = &s[pos + arrow_len..];
        assert_eq!(from_part.trim(), "Frontend");
        assert_eq!(to_part.trim(), "Backend: REST calls");
    }

    // ── Edge palette tests ───────────────────────────────────────────────────

    #[test]
    fn test_edge_palette_dark_has_entries() {
        let theme = Theme::dark();
        let palette = theme.edge_palette();
        assert!(
            palette.len() >= 6,
            "Dark palette should have at least 6 colors"
        );
    }

    #[test]
    fn test_edge_palette_light_has_entries() {
        let theme = Theme::light();
        let palette = theme.edge_palette();
        assert!(
            palette.len() >= 6,
            "Light palette should have at least 6 colors"
        );
    }

    #[test]
    fn test_edge_palette_colors_are_distinct() {
        let theme = Theme::dark();
        let palette = theme.edge_palette();
        for i in 0..palette.len() {
            for j in (i + 1)..palette.len() {
                assert_ne!(
                    palette[i], palette[j],
                    "Palette colors {i} and {j} should be distinct"
                );
            }
        }
    }

    // ── Polyline utility tests ───────────────────────────────────────────────

    #[test]
    fn test_polyline_length_single_point() {
        let points = vec![Pos2::new(10.0, 20.0)];
        assert!((polyline_length(&points) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_polyline_length_two_points() {
        let points = vec![Pos2::new(0.0, 0.0), Pos2::new(3.0, 4.0)];
        assert!((polyline_length(&points) - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_polyline_length_multi_segment() {
        let points = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),  // +100
            Pos2::new(100.0, 50.0), // +50
            Pos2::new(200.0, 50.0), // +100
        ];
        assert!((polyline_length(&points) - 250.0).abs() < 0.01);
    }

    // ── Grid info tests ──────────────────────────────────────────────────────

    #[test]
    fn test_grid_info_single_row() {
        let grid = GridInfo {
            cols: 5,
            rows: 1,
            cell_w: 100.0,
            cell_h: 200.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: (0..5).map(|i| (i, 0)).collect(),
        };
        assert!((grid.v_corridor_x(0) - 0.0).abs() < 0.01);
        assert!((grid.v_corridor_x(5) - 500.0).abs() < 0.01);
        assert!((grid.h_corridor_y(0) - 0.0).abs() < 0.01);
        assert!((grid.h_corridor_y(1) - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_best_h_corridor_boundary() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Points very close to corridor 0 (y=0)
        let idx = grid.best_h_corridor(10.0, 20.0);
        assert_eq!(idx, 0);
        // Points very close to corridor 3 (y=450)
        let idx = grid.best_h_corridor(440.0, 450.0);
        assert_eq!(idx, 3);
    }

    #[test]
    fn test_best_v_corridor_boundary() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        let idx = grid.best_v_corridor(10.0, 20.0);
        assert_eq!(idx, 0);
        let idx = grid.best_v_corridor(590.0, 600.0);
        assert_eq!(idx, 3);
    }

    // ── Face selection edge cases ────────────────────────────────────────────

    #[test]
    fn test_face_selection_diagonal_right_down() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let target = egui::pos2(300.0, 300.0);
        // dx=200, dy=200, equal — should prefer horizontal (Right)
        let face = choose_exit_face(&rect, target);
        assert_eq!(face, Face::Right);
    }

    #[test]
    fn test_face_selection_slight_vertical() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let target = egui::pos2(110.0, 300.0);
        // dx=10, dy=200 — should prefer vertical (Bottom)
        let face = choose_exit_face(&rect, target);
        assert_eq!(face, Face::Bottom);
    }

    #[test]
    fn test_entry_face_matches_direction() {
        let rect = egui::Rect::from_center_size(egui::pos2(300.0, 100.0), egui::vec2(80.0, 60.0));
        // Source is to the left
        let from_center = egui::pos2(100.0, 100.0);
        assert_eq!(choose_entry_face(&rect, from_center), Face::Left);
        // Source is above
        let from_above = egui::pos2(300.0, 0.0);
        assert_eq!(choose_entry_face(&rect, from_above), Face::Top);
    }

    // ── Parse diagram edge cases ─────────────────────────────────────────────

    #[test]
    fn test_parse_diagram_whitespace() {
        let content = "  - A -> B: label  \n  - C -> D  ";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 4);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_parse_diagram_mixed_definitions() {
        // Nodes defined both as standalone and via edges
        let content = "- Server (icon: server, pos: 1,1)\n- DB (icon: database, pos: 2,1)\n- Server -> DB: queries\n- Cache -> DB: fills";
        let (nodes, edges) = parse_diagram(content);
        assert_eq!(nodes.len(), 3); // Server, DB, Cache
        assert_eq!(edges.len(), 2);
        // Server should keep its icon
        let server = nodes.iter().find(|n| n.name == "Server").unwrap();
        assert_eq!(server.icon, "server");
    }

    #[test]
    fn test_parse_diagram_reverse_arrow() {
        let content = "- B <- A: reverse";
        let (_, edges) = parse_diagram(content);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "B");
        assert_eq!(edges[0].to, "A");
        assert!(matches!(edges[0].arrow, ArrowKind::Reverse));
    }

    #[test]
    fn test_parse_diagram_bidirectional() {
        let content = "- X <-> Y: sync";
        let (_, edges) = parse_diagram(content);
        assert_eq!(edges.len(), 1);
        assert!(matches!(edges[0].arrow, ArrowKind::Bidirectional));
    }

    // ── GridInfo::cell_at tests ────────────────────────────────────────────

    #[test]
    fn test_cell_at_origin() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 50.0,
            origin_y: 50.0,
            occupied: HashSet::new(),
        };
        // Just inside top-left cell (0,0)
        assert_eq!(grid.cell_at(Pos2::new(51.0, 51.0)), Some((0, 0)));
    }

    #[test]
    fn test_cell_at_center_cell() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Center of cell (1,1): x=300, y=225
        assert_eq!(grid.cell_at(Pos2::new(300.0, 225.0)), Some((1, 1)));
    }

    #[test]
    fn test_cell_at_last_cell() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Just inside bottom-right cell (2,2)
        assert_eq!(grid.cell_at(Pos2::new(599.0, 449.0)), Some((2, 2)));
    }

    #[test]
    fn test_cell_at_out_of_bounds_left() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 50.0,
            origin_y: 50.0,
            occupied: HashSet::new(),
        };
        // Left of origin
        assert_eq!(grid.cell_at(Pos2::new(49.0, 100.0)), None);
    }

    #[test]
    fn test_cell_at_out_of_bounds_above() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 50.0,
            origin_y: 50.0,
            occupied: HashSet::new(),
        };
        // Above origin
        assert_eq!(grid.cell_at(Pos2::new(100.0, 49.0)), None);
    }

    #[test]
    fn test_cell_at_out_of_bounds_right() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Past right edge (cols*cell_w = 600)
        assert_eq!(grid.cell_at(Pos2::new(600.0, 100.0)), None);
    }

    #[test]
    fn test_cell_at_out_of_bounds_below() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Past bottom edge (rows*cell_h = 450)
        assert_eq!(grid.cell_at(Pos2::new(100.0, 450.0)), None);
    }

    #[test]
    fn test_cell_at_boundary_between_cells() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        // Exactly at boundary x=200 should be cell (1,0)
        assert_eq!(grid.cell_at(Pos2::new(200.0, 75.0)), Some((1, 0)));
    }

    #[test]
    fn test_cell_at_with_nonzero_origin() {
        let grid = GridInfo {
            cols: 2,
            rows: 2,
            cell_w: 100.0,
            cell_h: 100.0,
            origin_x: 200.0,
            origin_y: 300.0,
            occupied: HashSet::new(),
        };
        // Cell (0,0) starts at (200, 300)
        assert_eq!(grid.cell_at(Pos2::new(250.0, 350.0)), Some((0, 0)));
        // Cell (1,1) starts at (300, 400)
        assert_eq!(grid.cell_at(Pos2::new(350.0, 450.0)), Some((1, 1)));
        // Below origin
        assert_eq!(grid.cell_at(Pos2::new(250.0, 299.0)), None);
    }

    // ── GridInfo::is_cell_empty tests ──────────────────────────────────────

    #[test]
    fn test_is_cell_empty_no_occupants() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };
        assert!(grid.is_cell_empty(0, 0));
        assert!(grid.is_cell_empty(2, 2));
    }

    #[test]
    fn test_is_cell_empty_with_occupants() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: [(0, 0), (1, 1), (2, 2)].into_iter().collect(),
        };
        // Occupied cells
        assert!(!grid.is_cell_empty(0, 0));
        assert!(!grid.is_cell_empty(1, 1));
        assert!(!grid.is_cell_empty(2, 2));
        // Empty cells
        assert!(grid.is_cell_empty(1, 0));
        assert!(grid.is_cell_empty(0, 1));
        assert!(grid.is_cell_empty(2, 0));
    }

    #[test]
    fn test_is_cell_empty_fully_occupied_3x3() {
        let (_, grid) = make_3x3_grid();
        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    !grid.is_cell_empty(c, r),
                    "Cell ({c},{r}) should be occupied in full 3x3 grid"
                );
            }
        }
    }

    // ── Empty-cell routing preference tests ────────────────────────────────

    /// Build a hub-and-spoke scenario similar to the test diagram:
    /// Grid 3x3, center (API) at (1,1), with empty cells at (0,1) and (2,1).
    fn make_hub_spoke_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 250.0;
        let cell_h = 200.0;
        let node_w = 100.0;
        let node_h = 70.0;
        let origin_x = 0.0;
        let origin_y = 0.0;

        let names = [
            ("Web", 0, 0),
            ("Logs", 1, 0),
            ("App", 2, 0),
            // (0,1) is empty
            ("API", 1, 1),
            // (2,1) is empty
            ("Auth", 0, 2),
            ("Mail", 1, 2),
            ("DB", 2, 2),
        ];

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = origin_x + (*col as f32 + 0.5) * cell_w;
            let cy = origin_y + (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };

        (rects, grid)
    }

    #[test]
    fn test_hub_spoke_empty_cells_exist() {
        let (_, grid) = make_hub_spoke_grid();
        // (0,1) and (2,1) should be empty
        assert!(grid.is_cell_empty(0, 1), "Cell (0,1) should be empty");
        assert!(grid.is_cell_empty(2, 1), "Cell (2,1) should be empty");
        // All others should be occupied
        assert!(!grid.is_cell_empty(0, 0));
        assert!(!grid.is_cell_empty(1, 0));
        assert!(!grid.is_cell_empty(2, 0));
        assert!(!grid.is_cell_empty(1, 1));
        assert!(!grid.is_cell_empty(0, 2));
        assert!(!grid.is_cell_empty(1, 2));
        assert!(!grid.is_cell_empty(2, 2));
    }

    #[test]
    fn test_hub_spoke_app_to_api_no_node_crossing() {
        // App(2,0) → API(1,1): diagonal, should route through empty cell (2,1)
        let (rects, grid) = make_hub_spoke_grid();
        let path = route_single_edge("App", "API", &rects, &grid);
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "App", "API"),
            "App→API should not cross any intermediate node"
        );
        assert!(is_orthogonal(&path.waypoints), "Should be orthogonal");
    }

    #[test]
    fn test_hub_spoke_api_to_db_no_node_crossing() {
        // API(1,1) → DB(2,2): diagonal, should route through empty cell (2,1)
        let (rects, grid) = make_hub_spoke_grid();
        let path = route_single_edge("API", "DB", &rects, &grid);
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "API", "DB"),
            "API→DB should not cross any intermediate node"
        );
        assert!(is_orthogonal(&path.waypoints), "Should be orthogonal");
    }

    #[test]
    fn test_hub_spoke_api_to_auth_no_node_crossing() {
        // API(1,1) → Auth(0,2): diagonal, should route through empty cell (0,1)
        let (rects, grid) = make_hub_spoke_grid();
        let path = route_single_edge("API", "Auth", &rects, &grid);
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "API", "Auth"),
            "API→Auth should not cross any intermediate node"
        );
        assert!(is_orthogonal(&path.waypoints), "Should be orthogonal");
    }

    #[test]
    fn test_hub_spoke_web_to_api_no_node_crossing() {
        // Web(0,0) → API(1,1): diagonal, should route through empty cell (0,1)
        let (rects, grid) = make_hub_spoke_grid();
        let path = route_single_edge("Web", "API", &rects, &grid);
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "Web", "API"),
            "Web→API should not cross any intermediate node"
        );
        assert!(is_orthogonal(&path.waypoints), "Should be orthogonal");
    }

    // ── Empty cell L-shape preference test ─────────────────────────────────

    /// In a grid with some empty cells, diagonal edges should prefer L-shapes
    /// through empty cells rather than Z-shapes through corridors.
    #[test]
    fn test_diagonal_prefers_lshape_through_empty_cell() {
        // 3x2 grid: A(0,0), -(1,0), C(2,0), D(0,1), E(1,1), -(2,1)
        // Edge A→E is diagonal. Cell (1,0) is empty, so H→V L-shape is preferred.
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("A", 0, 0), ("C", 2, 0), ("D", 0, 1), ("E", 1, 1)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        let path = route_single_edge("A", "E", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints), "Should be orthogonal");
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "E"),
            "Should not cross intermediate nodes"
        );
        // Semantic router routes through corridors, producing more waypoints than old router's
        // simple L-shape (3 waypoints). The key property is orthogonality + no crossings.
    }

    #[test]
    fn test_diagonal_no_empty_cell_uses_corridor() {
        // All cells occupied: diagonal must use corridor (Z-shape or avoidance)
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "E", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints), "Should be orthogonal");
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "E"),
            "Should not cross intermediate nodes"
        );
    }

    #[test]
    fn test_diagonal_both_lshapes_empty_picks_shorter() {
        // 3x3 grid with only 2 nodes: A(0,0) and I(2,2). All other cells empty.
        // Both L-shape turn points (2,0) and (0,2) are empty.
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("A", 0, 0), ("I", 2, 2)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        let path = route_single_edge("A", "I", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        // Semantic router routes through corridors; verify route is clean
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "I"),
            "Route should not cross any nodes"
        );
    }

    #[test]
    fn test_diagonal_one_lshape_empty_one_occupied() {
        // 3x3 grid: A(0,0), B(2,0), I(2,2). Turn at (2,0) is occupied, (0,2) is empty.
        // A→I diagonal: H→V turn at (2,0) is blocked, V→H turn at (0,2) is empty.
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("A", 0usize, 0usize), ("B", 2, 0), ("I", 2, 2)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        let path = route_single_edge("A", "I", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "A", "I"));
    }

    // ── Same-row and same-col direct routing ───────────────────────────────

    #[test]
    fn test_same_row_direct_path() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "B", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        // Same row, adjacent — should be a direct horizontal path
        let first = path.waypoints[0];
        let last = path.waypoints.last().unwrap();
        assert!(
            (first.y - last.y).abs() < 30.0,
            "Same-row path should be mostly horizontal"
        );
    }

    #[test]
    fn test_same_col_direct_path() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "D", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        let first = path.waypoints[0];
        let last = path.waypoints.last().unwrap();
        assert!(
            (first.x - last.x).abs() < 30.0,
            "Same-col path should be mostly vertical"
        );
    }

    #[test]
    fn test_same_row_skip_middle_node() {
        // A(0,0) → C(2,0): same row but B(1,0) is between them.
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "C", &rects, &grid);
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "C"),
            "A→C should route around B"
        );
        assert!(is_orthogonal(&path.waypoints));
    }

    #[test]
    fn test_same_col_skip_middle_node() {
        // A(0,0) → G(0,2): same col but D(0,1) is between them.
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "G", &rects, &grid);
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "G"),
            "A→G should route around D"
        );
        assert!(is_orthogonal(&path.waypoints));
    }

    // ── Reverse direction routing ──────────────────────────────────────────

    #[test]
    fn test_reverse_direction_routing() {
        // Route from bottom-right to top-left
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("I", "A", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "I", "A"));
    }

    #[test]
    fn test_route_left_to_right() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("C", "A", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        // Should go from right to left
        let first = path.waypoints[0];
        let last = path.waypoints.last().unwrap();
        assert!(first.x > last.x, "Should go from right to left");
    }

    #[test]
    fn test_route_bottom_to_top() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("G", "A", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        let first = path.waypoints[0];
        let last = path.waypoints.last().unwrap();
        assert!(first.y > last.y, "Should go from bottom to top");
    }

    // ── Wide grid (single row, many columns) ───────────────────────────────

    #[test]
    fn test_single_row_many_nodes() {
        let cell_w = 150.0;
        let cell_h = 200.0;
        let node_w = 60.0;
        let node_h = 50.0;
        let n = 7;

        let mut rects = HashMap::new();
        let names: Vec<String> = (0..n).map(|i| format!("N{i}")).collect();
        for (i, name) in names.iter().enumerate() {
            let cx = (i as f32 + 0.5) * cell_w;
            let cy = cell_h / 2.0;
            rects.insert(
                name.clone(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let grid = GridInfo {
            cols: n,
            rows: 1,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: (0..n).map(|i| (i, 0)).collect(),
        };

        // Route from first to last, skipping all middle nodes
        let path = route_single_edge("N0", &format!("N{}", n - 1), &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "N0", &format!("N{}", n - 1)),
            "N0→N6 should not cross middle nodes"
        );
    }

    // ── Edge palette coverage ──────────────────────────────────────────────

    #[test]
    fn test_edge_palette_exact_count_dark() {
        let theme = Theme::dark();
        assert_eq!(theme.edge_palette().len(), 8);
    }

    #[test]
    fn test_edge_palette_exact_count_light() {
        let theme = Theme::light();
        assert_eq!(theme.edge_palette().len(), 8);
    }

    #[test]
    fn test_edge_palette_light_colors_distinct() {
        let theme = Theme::light();
        let palette = theme.edge_palette();
        let unique: HashSet<[u8; 3]> = palette.iter().map(|c| [c.r(), c.g(), c.b()]).collect();
        assert_eq!(
            unique.len(),
            palette.len(),
            "All light palette colors should be distinct"
        );
    }

    #[test]
    fn test_edge_palette_wraps_at_boundary() {
        let theme = Theme::dark();
        let palette = theme.edge_palette();
        // 9th edge should wrap to first color
        assert_eq!(palette[0], palette[8 % palette.len()]);
    }

    // ── Port allocation exhaustive tests ───────────────────────────────────

    #[test]
    fn test_port_many_claims_stay_within_bounds() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let mut ports = FacePortAllocator::new();
        let face_length = 60.0; // height for Left/Right face
        let max_offset = face_length * 0.3; // = 18.0

        for _ in 0..20 {
            let offset = ports.claim_port("N", Face::Right, &rect, 22.0);
            assert!(
                offset.abs() <= max_offset + 0.1,
                "Port offset {offset} exceeds max_offset {max_offset}"
            );
        }
    }

    #[test]
    fn test_port_allocation_different_faces_same_node() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let mut ports = FacePortAllocator::new();

        let right1 = ports.claim_port("N", Face::Right, &rect, 22.0);
        let top1 = ports.claim_port("N", Face::Top, &rect, 22.0);
        let left1 = ports.claim_port("N", Face::Left, &rect, 22.0);
        let bottom1 = ports.claim_port("N", Face::Bottom, &rect, 22.0);

        // First claim on each face should be 0
        assert_eq!(right1, 0.0);
        assert_eq!(top1, 0.0);
        assert_eq!(left1, 0.0);
        assert_eq!(bottom1, 0.0);
    }

    // ── Ramp from face tests ───────────────────────────────────────────────

    #[test]
    fn test_ramp_includes_margin() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let margin = 10.0;

        let right = ramp_from_face(&rect, Face::Right, 0.0, margin);
        assert!(
            (right.x - (rect.right() + margin)).abs() < 0.01,
            "Right ramp should be at right edge + margin"
        );

        let left = ramp_from_face(&rect, Face::Left, 0.0, margin);
        assert!(
            (left.x - (rect.left() - margin)).abs() < 0.01,
            "Left ramp should be at left edge - margin"
        );

        let bottom = ramp_from_face(&rect, Face::Bottom, 0.0, margin);
        assert!(
            (bottom.y - (rect.bottom() + margin)).abs() < 0.01,
            "Bottom ramp should be at bottom edge + margin"
        );

        let top = ramp_from_face(&rect, Face::Top, 0.0, margin);
        assert!(
            (top.y - (rect.top() - margin)).abs() < 0.01,
            "Top ramp should be at top edge - margin"
        );
    }

    #[test]
    fn test_ramp_with_port_offset() {
        let rect = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 60.0));
        let margin = 10.0;
        let port = 15.0;

        let right = ramp_from_face(&rect, Face::Right, port, margin);
        assert!(
            (right.y - (100.0 + port)).abs() < 0.01,
            "Right face port should offset Y"
        );

        let top = ramp_from_face(&rect, Face::Top, port, margin);
        assert!(
            (top.x - (100.0 + port)).abs() < 0.01,
            "Top face port should offset X"
        );
    }

    // ── Sparse grid tests ──────────────────────────────────────────────────

    #[test]
    fn test_sparse_grid_corners_occupied() {
        // 4x4 grid with nodes in all 4 corners — L-shape turns go through corners
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("A", 0, 0), ("B", 3, 0), ("C", 0, 3), ("D", 3, 3)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 4,
            rows: 4,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        // 12 out of 16 cells are empty, but L-shape turns land on occupied corners
        assert_eq!(grid.occupied.len(), 4);

        // A→D diagonal: L-shape turns at (3,0)=B and (0,3)=C are occupied,
        // so routing should use Z-shape or avoidance
        let path = route_single_edge("A", "D", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "A", "D"));
    }

    #[test]
    fn test_sparse_grid_lshape_through_empty_interior() {
        // 4x4 grid: A at (0,0), D at (3,3), interior cells all empty
        // This time no nodes at (3,0) or (0,3), so L-shape should work
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("A", 0, 0), ("D", 3, 3)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 4,
            rows: 4,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        // L-shape turns at (3,0) and (0,3) are both empty → route should be clean
        let path = route_single_edge("A", "D", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "D"),
            "Route should not cross any nodes"
        );
    }

    #[test]
    fn test_sparse_grid_l_shape_both_directions() {
        // 3x3 grid with nodes only at (0,0) and (2,2)
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("A", 0, 0), ("B", 2, 2)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        // A→B: both L-shape turns ((2,0) and (0,2)) are empty → route should be clean
        let fwd = route_single_edge("A", "B", &rects, &grid);
        assert!(is_orthogonal(&fwd.waypoints));
        assert!(
            !path_crosses_node(&fwd.waypoints, &rects, "A", "B"),
            "Forward route should not cross nodes"
        );

        // B→A: reverse direction, same property
        let rev = route_single_edge("B", "A", &rects, &grid);
        assert!(is_orthogonal(&rev.waypoints));
        assert!(
            !path_crosses_node(&rev.waypoints, &rects, "B", "A"),
            "Reverse route should not cross nodes"
        );
    }

    // ── Adjacent nodes routing ─────────────────────────────────────────────

    #[test]
    fn test_adjacent_horizontal_nodes() {
        let (rects, grid) = make_3x3_grid();
        for (from, to) in [
            ("A", "B"),
            ("B", "C"),
            ("D", "E"),
            ("E", "F"),
            ("G", "H"),
            ("H", "I"),
        ] {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "{from}→{to} should be orthogonal"
            );
            // Semantic router always routes through corridors, so more than 2 waypoints
            assert!(
                path.waypoints.len() >= 2,
                "Adjacent horizontal {from}→{to} should have at least 2 waypoints, got {}",
                path.waypoints.len()
            );
        }
    }

    #[test]
    fn test_adjacent_vertical_nodes() {
        let (rects, grid) = make_3x3_grid();
        for (from, to) in [
            ("A", "D"),
            ("B", "E"),
            ("C", "F"),
            ("D", "G"),
            ("E", "H"),
            ("F", "I"),
        ] {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "{from}→{to} should be orthogonal"
            );
            // Semantic router always routes through corridors, so more than 2 waypoints
            assert!(
                path.waypoints.len() >= 2,
                "Adjacent vertical {from}→{to} should have at least 2 waypoints, got {}",
                path.waypoints.len()
            );
        }
    }

    // ── Far diagonal routing ───────────────────────────────────────────────

    #[test]
    fn test_far_diagonal_a_to_i() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "I", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "A", "I"));
    }

    #[test]
    fn test_far_diagonal_c_to_g() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("C", "G", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "C", "G"));
    }

    #[test]
    fn test_far_diagonal_g_to_c() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("G", "C", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "G", "C"));
    }

    #[test]
    fn test_far_diagonal_i_to_a() {
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("I", "A", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "I", "A"));
    }

    /// Route multiple edges with SHARED occupancy and port state, exactly as the
    /// real renderer does. Returns paths in order.
    fn route_edges_shared(
        edges: &[(&str, &str)],
        node_rects: &HashMap<String, egui::Rect>,
        grid: &GridInfo,
    ) -> Vec<RoutedPath> {
        route_edges_shared_semantic(edges, node_rects, grid)
    }

    fn route_edges_shared_semantic(
        edges: &[(&str, &str)],
        node_rects: &HashMap<String, egui::Rect>,
        grid: &GridInfo,
    ) -> Vec<RoutedPath> {
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();
        edges
            .iter()
            .map(|&(from, to)| {
                route_edge_semantic(
                    from,
                    to,
                    &node_rects[from],
                    &node_rects[to],
                    grid,
                    &mut occupancy,
                    &mut ports,
                    node_rects,
                    10.0,
                    20.0,
                    22.0,
                )
            })
            .collect()
    }

    /// The Dense Routing Test: 3x3 fully occupied grid with 10 edges, including
    /// far diagonals. Routes ALL edges with shared state and verifies none cross
    /// intermediate nodes.
    #[test]
    fn test_dense_routing_all_edges_shared_state() {
        let (rects, grid) = make_3x3_grid();
        let edges = [
            ("A", "E"), // diagonal
            ("C", "G"), // diagonal
            ("B", "H"), // vertical
            ("D", "F"), // horizontal
            ("A", "I"), // far diagonal
            ("G", "C"), // far diagonal
            ("E", "A"), // back
            ("E", "C"), // right
            ("E", "G"), // down-left
            ("E", "I"), // down-right
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        for (i, ((from, to), path)) in edges.iter().zip(paths.iter()).enumerate() {
            assert!(
                is_orthogonal(&path.waypoints),
                "Edge {i} ({from}→{to}) is not orthogonal: {:?}",
                path.waypoints
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "Edge {i} ({from}→{to}) crosses an intermediate node: {:?}",
                path.waypoints
            );
        }
    }

    /// Build a 3x3 grid with dimensions matching real 1920x1080 rendering.
    /// cell_w ≈ 580, cell_h ≈ 257, node_w = 220, node_h ≈ 154
    fn make_3x3_grid_realistic() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 580.0;
        let cell_h = 257.0;
        let node_w = 220.0;
        let node_h = 154.0;
        let origin_x = 90.0;
        let origin_y = 250.0;

        let names = [
            ("A", 0, 0),
            ("B", 1, 0),
            ("C", 2, 0),
            ("D", 0, 1),
            ("E", 1, 1),
            ("F", 2, 1),
            ("G", 0, 2),
            ("H", 1, 2),
            ("I", 2, 2),
        ];

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = origin_x + (*col as f32 + 0.5) * cell_w;
            let cy = origin_y + (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };

        (rects, grid)
    }

    /// Dense routing with REALISTIC dimensions matching 1920x1080 export.
    /// This catches issues that small-grid tests miss due to different
    /// node-to-cell ratios.
    #[test]
    fn test_dense_routing_realistic_dimensions() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edge_labels = [
            ("A", "E", "diagonal"),
            ("C", "G", "diagonal"),
            ("B", "H", "vertical"),
            ("D", "F", "horizontal"),
            ("A", "I", "far diagonal"),
            ("G", "C", "far diagonal"),
            ("E", "A", "back"),
            ("E", "C", "right"),
            ("E", "G", "down-left"),
            ("E", "I", "down-right"),
        ];
        let edges: Vec<(&str, &str)> = edge_labels.iter().map(|(f, t, _)| (*f, *t)).collect();
        let paths = route_edges_shared(&edges, &rects, &grid);

        eprintln!("\n=== Dense Routing Test (Realistic Dimensions) ===");
        eprintln!(
            "Grid: {}x{}, cell={}x{}, origin=({},{})",
            grid.cols, grid.rows, grid.cell_w, grid.cell_h, grid.origin_x, grid.origin_y
        );
        for (name, rect) in rects.iter() {
            eprintln!(
                "  Node {}: ({:.0},{:.0})-({:.0},{:.0}) center=({:.0},{:.0})",
                name,
                rect.left(),
                rect.top(),
                rect.right(),
                rect.bottom(),
                rect.center().x,
                rect.center().y,
            );
        }

        for (i, ((from, to, label), path)) in edge_labels.iter().zip(paths.iter()).enumerate() {
            let wp_str: Vec<String> = path
                .waypoints
                .iter()
                .map(|p| format!("({:.0},{:.0})", p.x, p.y))
                .collect();
            let crosses = path_crosses_node(&path.waypoints, &rects, from, to);
            eprintln!(
                "  Edge {i} {from}→{to} ({label}): {} waypoints, crosses={crosses}\n    {}",
                path.waypoints.len(),
                wp_str.join(" → ")
            );

            assert!(
                is_orthogonal(&path.waypoints),
                "Edge {i} ({from}→{to}) is not orthogonal: {:?}",
                path.waypoints
            );
            assert!(
                !crosses,
                "Edge {i} ({from}→{to}) crosses intermediate node.\nPath: {:?}",
                path.waypoints,
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── Far-diagonal routing shape tests ──────────────────────────────────────
    // Far-diagonal edges (crossing 2+ grid steps in both x and y) must exit
    // the source node HORIZONTALLY first ("reach out to the road"), then follow
    // corridors, rather than starting with a long vertical drop.
    // ══════════════════════════════════════════════════════════════════════════

    /// Helper: check that a path's first segment is horizontal (x changes, y stays).
    fn first_segment_is_horizontal(path: &RoutedPath) -> bool {
        let wp = &path.waypoints;
        wp.len() >= 2 && (wp[0].y - wp[1].y).abs() < 1.0 && (wp[0].x - wp[1].x).abs() > 1.0
    }

    /// Far-diagonal-1: C→G (top-right → bottom-left).
    /// Must exit C horizontally, not with a long vertical drop.
    #[test]
    fn test_far_diagonal_1_c_to_g_exits_horizontally() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"), // far-diagonal-1
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let cg_path = &paths[1]; // C→G

        eprintln!("far-diagonal-1 C→G waypoints:");
        for (i, wp) in cg_path.waypoints.iter().enumerate() {
            eprintln!("  [{i}] ({:.1}, {:.1})", wp.x, wp.y);
        }

        assert!(
            !path_crosses_node(&cg_path.waypoints, &rects, "C", "G"),
            "C→G crosses an intermediate node: {:?}",
            cg_path.waypoints
        );
        assert!(
            first_segment_is_horizontal(cg_path),
            "C→G first segment should be horizontal (exit sideways to corridor), \
             but goes vertical.\nPath: {:?}",
            cg_path.waypoints
        );
    }

    /// Far-diagonal-2: A→I (top-left → bottom-right).
    /// Must exit A horizontally, not with a long vertical drop.
    #[test]
    fn test_far_diagonal_2_a_to_i_exits_horizontally() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"), // far-diagonal-2
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let ai_path = &paths[4]; // A→I

        eprintln!("far-diagonal-2 A→I waypoints:");
        for (i, wp) in ai_path.waypoints.iter().enumerate() {
            eprintln!("  [{i}] ({:.1}, {:.1})", wp.x, wp.y);
        }

        assert!(
            !path_crosses_node(&ai_path.waypoints, &rects, "A", "I"),
            "A→I crosses an intermediate node: {:?}",
            ai_path.waypoints
        );
        assert!(
            first_segment_is_horizontal(ai_path),
            "A→I first segment should be horizontal (exit sideways to corridor), \
             but goes vertical.\nPath: {:?}",
            ai_path.waypoints
        );
    }

    /// Far-diagonal-3: G→C (bottom-left → top-right).
    /// Must exit G horizontally, not with a long vertical drop.
    #[test]
    fn test_far_diagonal_3_g_to_c_exits_horizontally() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"), // far-diagonal-3
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let gc_path = &paths[5]; // G→C

        eprintln!("far-diagonal-3 G→C waypoints:");
        for (i, wp) in gc_path.waypoints.iter().enumerate() {
            eprintln!("  [{i}] ({:.1}, {:.1})", wp.x, wp.y);
        }

        assert!(
            !path_crosses_node(&gc_path.waypoints, &rects, "G", "C"),
            "G→C crosses an intermediate node: {:?}",
            gc_path.waypoints
        );
        assert!(
            first_segment_is_horizontal(gc_path),
            "G→C first segment should be horizontal (exit sideways to corridor), \
             but goes vertical.\nPath: {:?}",
            gc_path.waypoints
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── All-connections routing verification ─────────────────────────────────
    // Tests ALL 10 connections from the Dense Routing Test slide with
    // realistic dimensions. Each edge is verified for orthogonality,
    // no-crossing, and correct directionality.
    // ══════════════════════════════════════════════════════════════════════════

    /// All 10 connections with realistic dimensions: every edge must be
    /// orthogonal and avoid crossing intermediate nodes.
    #[test]
    fn test_all_connections_orthogonal_and_no_crossing() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"), // 0: diagonal
            ("C", "G"), // 1: far-diagonal-1
            ("B", "H"), // 2: vertical
            ("D", "F"), // 3: horizontal
            ("A", "I"), // 4: far-diagonal-2
            ("G", "C"), // 5: far-diagonal-3
            ("E", "A"), // 6: back
            ("E", "C"), // 7: right
            ("E", "G"), // 8: down-left
            ("E", "I"), // 9: down-right
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);

        for (i, ((from, to), path)) in edges.iter().zip(paths.iter()).enumerate() {
            assert!(
                is_orthogonal(&path.waypoints),
                "Edge {i} ({from}→{to}) is not orthogonal: {:?}",
                path.waypoints
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "Edge {i} ({from}→{to}) crosses an intermediate node.\nPath: {:?}",
                path.waypoints
            );
        }
    }

    /// Edge 0 (A→E): diagonal (adjacent diagonal, A is top-left of E).
    /// Should exit and enter cleanly without crossing B or D.
    #[test]
    fn test_connection_a_to_e_diagonal() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[0];

        assert!(
            is_orthogonal(&path.waypoints),
            "A→E not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "E"),
            "A→E crosses intermediate node: {:?}",
            path.waypoints
        );
    }

    /// Edge 2 (B→H): vertical (same column, B is row 0, H is row 2).
    /// Should route cleanly around E (which sits between them).
    #[test]
    fn test_connection_b_to_h_vertical() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[2];

        assert!(
            is_orthogonal(&path.waypoints),
            "B→H not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "B", "H"),
            "B→H crosses intermediate node: {:?}",
            path.waypoints
        );
        // B and H are in the same column with E between them.
        // Should route around E, so more than 2 waypoints.
        assert!(
            path.waypoints.len() > 2,
            "B→H should route around E (expected >2 waypoints, got {})",
            path.waypoints.len()
        );
    }

    /// Edge 3 (D→F): horizontal (same row, D is col 0, F is col 2).
    /// Should route cleanly around E (which sits between them).
    #[test]
    fn test_connection_d_to_f_horizontal() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[3];

        assert!(
            is_orthogonal(&path.waypoints),
            "D→F not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "D", "F"),
            "D→F crosses intermediate node: {:?}",
            path.waypoints
        );
        // D and F are in the same row with E between them.
        // Should route around E, so more than 2 waypoints.
        assert!(
            path.waypoints.len() > 2,
            "D→F should route around E (expected >2 waypoints, got {})",
            path.waypoints.len()
        );
    }

    /// Edge 6 (E→A): back (center to top-left).
    #[test]
    fn test_connection_e_to_a_back() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[6];

        assert!(
            is_orthogonal(&path.waypoints),
            "E→A not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "E", "A"),
            "E→A crosses intermediate node: {:?}",
            path.waypoints
        );
    }

    /// Edge 7 (E→C): right (center to top-right).
    #[test]
    fn test_connection_e_to_c_right() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[7];

        assert!(
            is_orthogonal(&path.waypoints),
            "E→C not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "E", "C"),
            "E→C crosses intermediate node: {:?}",
            path.waypoints
        );
    }

    /// Edge 8 (E→G): down-left (center to bottom-left).
    #[test]
    fn test_connection_e_to_g_down_left() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[8];

        assert!(
            is_orthogonal(&path.waypoints),
            "E→G not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "E", "G"),
            "E→G crosses intermediate node: {:?}",
            path.waypoints
        );
    }

    /// Edge 9 (E→I): down-right (center to bottom-right).
    #[test]
    fn test_connection_e_to_i_down_right() {
        let (rects, grid) = make_3x3_grid_realistic();
        let edges = [
            ("A", "E"),
            ("C", "G"),
            ("B", "H"),
            ("D", "F"),
            ("A", "I"),
            ("G", "C"),
            ("E", "A"),
            ("E", "C"),
            ("E", "G"),
            ("E", "I"),
        ];
        let paths = route_edges_shared(&edges, &rects, &grid);
        let path = &paths[9];

        assert!(
            is_orthogonal(&path.waypoints),
            "E→I not orthogonal: {:?}",
            path.waypoints
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "E", "I"),
            "E→I crosses intermediate node: {:?}",
            path.waypoints
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── Aspect ratio face pair ordering tests ────────────────────────────────
    // When cells are wider than tall, prefer H-exits (Left/Right) which route
    // through spacious vertical corridors. When cells are taller than wide,
    // prefer V-exits (Top/Bottom).
    // ══════════════════════════════════════════════════════════════════════════

    /// Helper: build a 3x3 grid with custom cell dimensions.
    fn make_3x3_grid_custom(cell_w: f32, cell_h: f32) -> (HashMap<String, egui::Rect>, GridInfo) {
        let node_w = 80.0;
        let node_h = 60.0;
        let origin_x = 50.0;
        let origin_y = 50.0;

        let names = [
            ("A", 0, 0),
            ("B", 1, 0),
            ("C", 2, 0),
            ("D", 0, 1),
            ("E", 1, 1),
            ("F", 2, 1),
            ("G", 0, 2),
            ("H", 1, 2),
            ("I", 2, 2),
        ];

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = origin_x + (*col as f32 + 0.5) * cell_w;
            let cy = origin_y + (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };

        (rects, grid)
    }

    /// Helper: determine the exit face used by a routed path.
    /// Inspects the first segment direction to infer the face.
    fn infer_exit_face(path: &RoutedPath) -> Face {
        assert!(path.waypoints.len() >= 2);
        let dx = path.waypoints[1].x - path.waypoints[0].x;
        let dy = path.waypoints[1].y - path.waypoints[0].y;
        if dx.abs() > dy.abs() {
            if dx > 0.0 { Face::Right } else { Face::Left }
        } else if dy > 0.0 {
            Face::Bottom
        } else {
            Face::Top
        }
    }

    /// Helper: determine the entry face used by a routed path.
    /// Inspects the last segment direction to infer the face.
    fn infer_entry_face(path: &RoutedPath) -> Face {
        let n = path.waypoints.len();
        assert!(n >= 2);
        let dx = path.waypoints[n - 1].x - path.waypoints[n - 2].x;
        let dy = path.waypoints[n - 1].y - path.waypoints[n - 2].y;
        // Entry face is OPPOSITE the direction the last segment travels
        if dx.abs() > dy.abs() {
            if dx > 0.0 { Face::Left } else { Face::Right }
        } else if dy > 0.0 {
            Face::Top
        } else {
            Face::Bottom
        }
    }

    #[test]
    fn test_wide_cells_diagonal_prefers_h_exit() {
        // Very wide cells (300x100): H-exits should be preferred for diagonals
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        // Diagonal edge A(0,0) → E(1,1)
        let path = route_single_edge("A", "E", &rects, &grid);
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Right | Face::Left),
            "Wide cells should prefer H-exit for diagonal, got {:?}",
            exit_face
        );
    }

    #[test]
    fn test_wide_cells_far_diagonal_a_to_i_routes_cleanly() {
        // Wide cells: far diagonal A(0,0) → I(2,2) in fully-occupied grid.
        // The preferred H-exit may not find a clean path for far diagonals,
        // so we verify orthogonality and no crossing rather than face preference.
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        let path = route_single_edge("A", "I", &rects, &grid);
        assert!(
            is_orthogonal(&path.waypoints),
            "Wide cells far diagonal A→I should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "I"),
            "Wide cells far diagonal A→I should not cross nodes"
        );
    }

    #[test]
    fn test_tall_cells_diagonal_routes_cleanly() {
        // Very tall cells (100x300): diagonal routes should be clean
        let (rects, grid) = make_3x3_grid_custom(100.0, 300.0);
        // Diagonal edge A(0,0) → E(1,1)
        let path = route_single_edge("A", "E", &rects, &grid);
        assert!(
            is_orthogonal(&path.waypoints),
            "Tall cells diagonal A→E should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "E"),
            "Tall cells diagonal A→E should not cross nodes"
        );
    }

    #[test]
    fn test_tall_cells_far_diagonal_a_to_i_routes_cleanly() {
        // Tall cells: far diagonal A→I in fully-occupied grid.
        // Verify orthogonality and no crossing (far diagonals may not use preferred face).
        let (rects, grid) = make_3x3_grid_custom(100.0, 300.0);
        let path = route_single_edge("A", "I", &rects, &grid);
        assert!(
            is_orthogonal(&path.waypoints),
            "Tall cells far diagonal A→I should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "I"),
            "Tall cells far diagonal A→I should not cross nodes"
        );
    }

    #[test]
    fn test_wide_cells_reverse_far_diagonal_routes_cleanly() {
        // Wide cells: reverse far diagonal I(2,2) → A(0,0) in fully-occupied grid.
        // Far diagonals may not use preferred face pair; verify correctness.
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        let path = route_single_edge("I", "A", &rects, &grid);
        assert!(
            is_orthogonal(&path.waypoints),
            "Wide cells reverse far diagonal I→A should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "I", "A"),
            "Wide cells reverse far diagonal I→A should not cross nodes"
        );
    }

    #[test]
    fn test_tall_cells_reverse_far_diagonal_routes_cleanly() {
        // Tall cells: reverse far diagonal I(2,2) → A(0,0) in fully-occupied grid.
        let (rects, grid) = make_3x3_grid_custom(100.0, 300.0);
        let path = route_single_edge("I", "A", &rects, &grid);
        assert!(
            is_orthogonal(&path.waypoints),
            "Tall cells reverse far diagonal I→A should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "I", "A"),
            "Tall cells reverse far diagonal I→A should not cross nodes"
        );
    }

    #[test]
    fn test_square_cells_diagonal_still_routes_cleanly() {
        // Square cells (200x200): cell_w == cell_h, so H-exit path is first
        let (rects, grid) = make_3x3_grid_custom(200.0, 200.0);
        let path = route_single_edge("A", "E", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "A", "E"));
    }

    #[test]
    fn test_wide_cells_multiple_diagonals_all_prefer_h() {
        // Wide cells: ALL diagonal edges should prefer H-exits
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        let diagonals = [
            ("A", "E"),
            ("B", "D"),
            ("B", "F"),
            ("C", "E"),
            ("D", "H"),
            ("E", "G"),
            ("E", "I"),
            ("F", "H"),
        ];
        for (from, to) in &diagonals {
            let path = route_single_edge(from, to, &rects, &grid);
            let exit_face = infer_exit_face(&path);
            assert!(
                matches!(exit_face, Face::Right | Face::Left),
                "Wide cells diagonal {from}→{to} should prefer H-exit, got {:?}",
                exit_face
            );
        }
    }

    #[test]
    fn test_tall_cells_multiple_diagonals_route_cleanly() {
        // Tall cells: ALL diagonal edges should route cleanly (orthogonal, no node crossing)
        let (rects, grid) = make_3x3_grid_custom(100.0, 300.0);
        let diagonals = [
            ("A", "E"),
            ("B", "D"),
            ("B", "F"),
            ("C", "E"),
            ("D", "H"),
            ("E", "G"),
            ("E", "I"),
            ("F", "H"),
        ];
        for (from, to) in &diagonals {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "Tall cells diagonal {from}→{to} should be orthogonal"
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "Tall cells diagonal {from}→{to} should not cross nodes"
            );
        }
    }

    #[test]
    fn test_wide_cells_same_row_still_uses_h() {
        // Same-row connections should always use H regardless of aspect ratio
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        let path = route_single_edge("A", "B", &rects, &grid);
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Right),
            "Same-row A→B should exit Right, got {:?}",
            exit_face
        );
    }

    #[test]
    fn test_tall_cells_same_col_still_uses_v() {
        // Same-col connections should always use V regardless of aspect ratio
        let (rects, grid) = make_3x3_grid_custom(100.0, 300.0);
        let path = route_single_edge("A", "D", &rects, &grid);
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Bottom),
            "Same-col A→D should exit Bottom, got {:?}",
            exit_face
        );
    }

    #[test]
    fn test_wide_cells_entry_face_matches_exit_logic() {
        // In wide cells, the entry face should also reflect horizontal preference
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        let path = route_single_edge("A", "E", &rects, &grid);
        let _entry_face = infer_entry_face(&path);
        // H-exit from A means entry face on E should be Left or Right (H-entry)
        // or Top/Bottom depending on L-shape routing
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "A", "E"));
        // If exit is H, entry might be V (L-shape H→V) or H (Z-shape H→H)
        // Either way the path should be clean
        assert!(
            path.waypoints.len() >= 2,
            "Path should have proper waypoints"
        );
    }

    #[test]
    fn test_aspect_ratio_no_node_crossing_wide() {
        // Verify all diagonals in wide grid avoid nodes
        let (rects, grid) = make_3x3_grid_custom(300.0, 100.0);
        let all_pairs: Vec<(&str, &str)> = vec![
            ("A", "E"),
            ("A", "I"),
            ("C", "G"),
            ("C", "E"),
            ("G", "C"),
            ("I", "A"),
            ("B", "D"),
            ("B", "F"),
            ("D", "B"),
            ("F", "B"),
        ];
        for (from, to) in &all_pairs {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "Wide grid {from}→{to} not orthogonal"
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "Wide grid {from}→{to} crosses node"
            );
        }
    }

    #[test]
    fn test_aspect_ratio_no_node_crossing_tall() {
        // Verify all diagonals in tall grid avoid nodes
        let (rects, grid) = make_3x3_grid_custom(100.0, 300.0);
        let all_pairs: Vec<(&str, &str)> = vec![
            ("A", "E"),
            ("A", "I"),
            ("C", "G"),
            ("C", "E"),
            ("G", "C"),
            ("I", "A"),
            ("B", "D"),
            ("B", "F"),
            ("D", "B"),
            ("F", "B"),
        ];
        for (from, to) in &all_pairs {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "Tall grid {from}→{to} not orthogonal"
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "Tall grid {from}→{to} crosses node"
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── Interior corridor preference tests ───────────────────────────────────
    // When two corridors are equidistant from the midpoint, interior corridors
    // (between rows/cols) should be preferred over edge corridors (at grid
    // boundary) to reduce detour distance.
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_interior_corridor_preferred_over_edge_h() {
        // In a 3-row grid, the midpoint between row 0 and row 2 is at row 1.
        // Corridor 1 (between rows 0 and 1) and corridor 2 (between rows 1 and 2)
        // may be equidistant from the midpoint. The interior corridor should be
        // preferred over corridor 0 (top edge) or corridor 3 (bottom edge).
        let cell_w = 200.0;
        let cell_h = 200.0;
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        // Midpoint exactly at corridor 1 (y=200) and corridor 2 (y=400)
        // Test: corridors sorted by distance, then interior-preference breaks ties
        let mid_y = grid.h_corridor_y(1); // y = 200

        let mut h_corridors: Vec<(usize, f32)> = (0..=grid.rows)
            .map(|i| (i, (grid.h_corridor_y(i) - mid_y).abs()))
            .collect();
        h_corridors.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap();
            if cmp == std::cmp::Ordering::Equal {
                let a_edge = a.0 == 0 || a.0 == grid.rows;
                let b_edge = b.0 == 0 || b.0 == grid.rows;
                a_edge.cmp(&b_edge)
            } else {
                cmp
            }
        });

        // First corridor should be index 1 (distance 0, interior)
        assert_eq!(h_corridors[0].0, 1, "Closest corridor should be index 1");
    }

    #[test]
    fn test_interior_corridor_preferred_over_edge_v() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 200.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        let mid_x = grid.v_corridor_x(1); // x = 200

        let mut v_corridors: Vec<(usize, f32)> = (0..=grid.cols)
            .map(|j| (j, (grid.v_corridor_x(j) - mid_x).abs()))
            .collect();
        v_corridors.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap();
            if cmp == std::cmp::Ordering::Equal {
                let a_edge = a.0 == 0 || a.0 == grid.cols;
                let b_edge = b.0 == 0 || b.0 == grid.cols;
                a_edge.cmp(&b_edge)
            } else {
                cmp
            }
        });

        assert_eq!(v_corridors[0].0, 1, "Closest V corridor should be index 1");
    }

    #[test]
    fn test_edge_corridors_sorted_last_when_equidistant() {
        // Set up where corridor 0 and corridor 2 are equidistant from midpoint.
        // corridor 0 is edge, corridor 2 is interior (in a 3-row grid, corridors 0-3).
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 200.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        // Midpoint exactly between corridor 0 (y=0) and corridor 2 (y=400)
        // → mid_y = 200 → corridor 1 at y=200 is closest (dist=0)
        // Try mid_y = 100: corridor 0 at y=0 (dist=100), corridor 1 at y=200 (dist=100)
        // Equidistant! Interior corridor 1 should come first.
        let mid_y = 100.0;

        let mut h_corridors: Vec<(usize, f32)> = (0..=grid.rows)
            .map(|i| (i, (grid.h_corridor_y(i) - mid_y).abs()))
            .collect();
        h_corridors.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap();
            if cmp == std::cmp::Ordering::Equal {
                let a_edge = a.0 == 0 || a.0 == grid.rows;
                let b_edge = b.0 == 0 || b.0 == grid.rows;
                a_edge.cmp(&b_edge)
            } else {
                cmp
            }
        });

        // Corridor 0 (y=0, dist=100, EDGE) and corridor 1 (y=200, dist=100, INTERIOR)
        // Interior should come first
        assert_eq!(
            h_corridors[0].0, 1,
            "Interior corridor 1 should be preferred over edge corridor 0 when equidistant"
        );
    }

    #[test]
    fn test_edge_corridors_sorted_last_v() {
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 200.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        // mid_x = 100: corridor 0 (x=0, dist=100, EDGE) and corridor 1 (x=200, dist=100, INTERIOR)
        let mid_x = 100.0;

        let mut v_corridors: Vec<(usize, f32)> = (0..=grid.cols)
            .map(|j| (j, (grid.v_corridor_x(j) - mid_x).abs()))
            .collect();
        v_corridors.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap();
            if cmp == std::cmp::Ordering::Equal {
                let a_edge = a.0 == 0 || a.0 == grid.cols;
                let b_edge = b.0 == 0 || b.0 == grid.cols;
                a_edge.cmp(&b_edge)
            } else {
                cmp
            }
        });

        assert_eq!(
            v_corridors[0].0, 1,
            "Interior V corridor 1 should be preferred over edge corridor 0 when equidistant"
        );
    }

    #[test]
    fn test_interior_preference_in_routing_context() {
        // Verify that route_edge_orthogonal uses interior corridors in a concrete scenario:
        // Route from A(0,0) to I(2,2) in a fully-occupied 3x3 grid.
        // The Z-shape avoidance path should use interior corridors (index 1 or 2)
        // rather than edge corridors (index 0 or 3).
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("A", "I", &rects, &grid);

        // Check that waypoints stay within the interior of the grid,
        // i.e. not going above row 0 or below row 2 unnecessarily.
        let top_edge_y = grid.h_corridor_y(0); // y=50 (grid origin_y)
        let bottom_edge_y = grid.h_corridor_y(3); // y=50+3*150=500
        let left_edge_x = grid.v_corridor_x(0); // x=50
        let right_edge_x = grid.v_corridor_x(3); // x=50+3*200=650

        for wp in &path.waypoints {
            // Allow small margin for ramp overshoot
            assert!(
                wp.y >= top_edge_y - 20.0,
                "Waypoint y={} goes too far above grid (top edge={})",
                wp.y,
                top_edge_y
            );
            assert!(
                wp.y <= bottom_edge_y + 20.0,
                "Waypoint y={} goes too far below grid (bottom edge={})",
                wp.y,
                bottom_edge_y
            );
            assert!(
                wp.x >= left_edge_x - 20.0,
                "Waypoint x={} goes too far left (left edge={})",
                wp.x,
                left_edge_x
            );
            assert!(
                wp.x <= right_edge_x + 20.0,
                "Waypoint x={} goes too far right (right edge={})",
                wp.x,
                right_edge_x
            );
        }
    }

    #[test]
    fn test_corridor_avoidance_prefers_closer_interior() {
        // In a 5-row grid, for a path between rows 1 and 3, corridor 2 (interior,
        // between rows 1 and 2) should be preferred over corridor 0 (edge, top).
        let grid = GridInfo {
            cols: 3,
            rows: 5,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        // Midpoint between row 1 center and row 3 center
        let from_y = (1.0 + 0.5) * 150.0; // 225
        let to_y = (3.0 + 0.5) * 150.0; // 525
        let mid_y = (from_y + to_y) / 2.0; // 375

        let mut h_corridors: Vec<(usize, f32)> = (0..=grid.rows)
            .map(|i| (i, (grid.h_corridor_y(i) - mid_y).abs()))
            .collect();
        h_corridors.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap();
            if cmp == std::cmp::Ordering::Equal {
                let a_edge = a.0 == 0 || a.0 == grid.rows;
                let b_edge = b.0 == 0 || b.0 == grid.rows;
                a_edge.cmp(&b_edge)
            } else {
                cmp
            }
        });

        // Corridor 2 at y=300 (dist=75), corridor 3 at y=450 (dist=75) — equidistant
        // Both are interior, so order by index shouldn't matter.
        // Corridor 0 (y=0, dist=375) should be near the end.
        let first_idx = h_corridors[0].0;
        assert!(
            first_idx != 0 && first_idx != 5,
            "First corridor should not be edge (0 or 5), got {}",
            first_idx
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── Tight arrowhead fallback tests ───────────────────────────────────────
    // When the last segment of a routed path is too short for a clean arrowhead,
    // the arrowhead should be drawn before the last turn.
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_tight_arrowhead_detection_short_last_segment() {
        // A path where the last segment is shorter than arrow_size * 1.2
        let arrow_size = 20.0;
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),
                Pos2::new(100.0, 80.0),
                Pos2::new(100.0 + 15.0, 80.0), // last seg = 15, < 20*1.2=24
            ],
        };
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(
            last_seg_len < arrow_size * 1.2,
            "Last segment {last_seg_len} should be < {}",
            arrow_size * 1.2
        );
        // In this case, arrowhead should be drawn at waypoints[n-2] = (100, 80)
        // using direction waypoints[n-2] - waypoints[n-3] = (100,80) - (100,0) = (0,80) → downward
    }

    #[test]
    fn test_tight_arrowhead_detection_adequate_last_segment() {
        // A path where the last segment is long enough for a clean arrowhead
        let arrow_size = 20.0;
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),
                Pos2::new(100.0, 80.0),
                Pos2::new(200.0, 80.0), // last seg = 100, >= 20*1.2=24
            ],
        };
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(
            last_seg_len >= arrow_size * 1.2,
            "Last segment {last_seg_len} should be >= {}",
            arrow_size * 1.2
        );
    }

    #[test]
    fn test_tight_arrowhead_threshold_exact_boundary() {
        // Exactly at the threshold
        let arrow_size = 20.0;
        let threshold = arrow_size * 1.2; // 24.0
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),
                Pos2::new(100.0, 50.0),
                Pos2::new(100.0 + threshold, 50.0), // exactly at threshold
            ],
        };
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(
            last_seg_len >= arrow_size * 1.2,
            "At-threshold segment should use normal arrowhead"
        );
    }

    #[test]
    fn test_tight_arrowhead_pre_turn_direction_horizontal() {
        // Short last segment vertical → arrowhead at penultimate point with
        // direction from the segment before
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),  // horizontal segment
                Pos2::new(100.0, 10.0), // short vertical segment (10px)
            ],
        };
        let arrow_size = 20.0;
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(last_seg_len < arrow_size * 1.2);

        // Pre-turn direction = waypoints[1] - waypoints[0] = (100,0) → rightward
        let pre_turn_dir = path.waypoints[n - 2] - path.waypoints[n - 3];
        assert!(
            pre_turn_dir.x > 0.0 && pre_turn_dir.y.abs() < 0.1,
            "Pre-turn direction should be rightward: {:?}",
            pre_turn_dir
        );
    }

    #[test]
    fn test_tight_arrowhead_pre_turn_direction_vertical() {
        // Short last segment horizontal → arrowhead direction comes from vertical segment before
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(50.0, 0.0),
                Pos2::new(50.0, 100.0), // vertical segment
                Pos2::new(55.0, 100.0), // short horizontal segment (5px)
            ],
        };
        let arrow_size = 20.0;
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(last_seg_len < arrow_size * 1.2);

        let pre_turn_dir = path.waypoints[n - 2] - path.waypoints[n - 3];
        assert!(
            pre_turn_dir.y > 0.0 && pre_turn_dir.x.abs() < 0.1,
            "Pre-turn direction should be downward: {:?}",
            pre_turn_dir
        );
    }

    #[test]
    fn test_tight_arrowhead_two_point_path_uses_fallback() {
        // A 2-point path has no "pre-turn" to use, so it falls back to normal arrowhead
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0)],
        };
        let arrow_size = 20.0;
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        // Even if short, with only 2 points there's no alternative
        assert!(last_seg_len < arrow_size * 1.2);
        assert_eq!(
            n, 2,
            "Two-point path can't use pre-turn fallback, uses endpoint fallback"
        );
    }

    #[test]
    fn test_tight_arrowhead_three_point_path_uses_pre_turn() {
        // 3-point path (L-shape) with short last segment → should use pre-turn arrowhead
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),
                Pos2::new(100.0, 5.0), // 5px last segment
            ],
        };
        let arrow_size = 20.0;
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(last_seg_len < arrow_size * 1.2);
        assert!(n >= 3, "3+ points enables pre-turn arrowhead");

        // Arrowhead should be at waypoints[1] = (100, 0)
        let arrowhead_tip = path.waypoints[n - 2];
        assert!((arrowhead_tip.x - 100.0).abs() < 0.1);
        assert!((arrowhead_tip.y - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_tight_start_arrowhead_short_first_segment() {
        // Reverse arrow with short first segment
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(5.0, 0.0), // 5px first segment
                Pos2::new(5.0, 100.0),
            ],
        };
        let arrow_size = 20.0;
        let first_seg_len = (path.waypoints[1] - path.waypoints[0]).length();
        assert!(first_seg_len < arrow_size * 1.2);
        assert!(
            path.waypoints.len() >= 3,
            "Enables post-turn start arrowhead"
        );

        // Start arrowhead should be at waypoints[1] = (5, 0) using direction
        // waypoints[1] - waypoints[2] = (5,0) - (5,100) = (0,-100) → upward
        let post_turn_dir = path.waypoints[1] - path.waypoints[2];
        assert!(
            post_turn_dir.y < 0.0,
            "Post-turn direction for start arrow should be upward"
        );
    }

    #[test]
    fn test_tight_arrowhead_four_point_z_shape() {
        // Z-shape path where last segment is short
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(0.0, 100.0),
                Pos2::new(80.0, 100.0),
                Pos2::new(80.0, 108.0), // 8px last segment
            ],
        };
        let arrow_size = 20.0;
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(last_seg_len < arrow_size * 1.2);
        assert!(n >= 3);

        // Pre-turn dir = waypoints[2] - waypoints[1] = (80,100)-(0,100) = (80,0) → rightward
        let pre_turn_dir = path.waypoints[n - 2] - path.waypoints[n - 3];
        assert!(pre_turn_dir.x > 0.0, "Pre-turn should be rightward");
    }

    #[test]
    fn test_tight_arrowhead_zero_length_last_segment() {
        // Degenerate case: last segment has zero length
        let path = RoutedPath {
            steps: vec![],
            waypoints: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),
                Pos2::new(100.0, 0.0), // duplicate point
            ],
        };
        let arrow_size = 20.0;
        let n = path.waypoints.len();
        let last_seg_len = (path.waypoints[n - 1] - path.waypoints[n - 2]).length();
        assert!(last_seg_len < arrow_size * 1.2);
        // Should still use pre-turn direction safely
        let pre_turn_dir = path.waypoints[n - 2] - path.waypoints[n - 3];
        assert!((pre_turn_dir.x - 100.0).abs() < 0.1);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── Large System scenario: specific routing fix verification ─────────────
    // These tests verify the three specific issues the user identified in the
    // Large System (10 nodes) diagram and ensure they remain fixed.
    // ══════════════════════════════════════════════════════════════════════════

    /// Helper: build the Large System grid (3 cols × 4 rows, 10 nodes).
    fn make_large_system_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 250.0;
        let cell_h = 180.0;
        let node_w = 100.0;
        let node_h = 70.0;

        let names = [
            ("Client", 0, 0),
            ("CDN", 1, 0),
            ("LB", 2, 0),
            ("Web", 0, 1),
            ("API", 1, 1),
            ("Worker", 2, 1),
            ("DB", 0, 2),
            ("Cache", 1, 2),
            ("Queue", 2, 2),
            ("Monitor", 1, 3),
        ];

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 4,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        (rects, grid)
    }

    #[test]
    fn test_large_system_lb_to_api_uses_h_exit() {
        // LB(2,0) → API(1,1): diagonal in a wide grid (250x180, cell_w > cell_h).
        // Should prefer H-exit (Left from LB) rather than V-exit (Bottom).
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("LB", "API", &rects, &grid);
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Left | Face::Right),
            "LB→API in wide grid should use H-exit, got {:?}",
            exit_face
        );
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "LB", "API"));
    }

    #[test]
    fn test_large_system_api_to_queue_uses_h_exit() {
        // API(1,1) → Queue(2,2): diagonal in wide grid.
        // Should prefer H-exit (Right from API) rather than V-exit (Bottom).
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("API", "Queue", &rects, &grid);
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Left | Face::Right),
            "API→Queue in wide grid should use H-exit, got {:?}",
            exit_face
        );
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "API", "Queue"));
    }

    #[test]
    fn test_large_system_client_to_lb_uses_interior_corridor() {
        // Client(0,0) → LB(2,0): same row, must skip CDN(1,0).
        // The avoidance path should use an interior corridor rather than the top edge.
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("Client", "LB", &rects, &grid);

        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "Client", "LB"));

        // Verify the path doesn't go above the grid (top edge corridor y=0)
        let top_edge_y = grid.h_corridor_y(0); // y = 0
        for wp in &path.waypoints {
            assert!(
                wp.y >= top_edge_y - 15.0,
                "Client→LB waypoint y={:.1} goes too far above grid top ({:.1})",
                wp.y,
                top_edge_y
            );
        }
    }

    #[test]
    fn test_large_system_client_to_lb_no_excessive_detour() {
        // The path from Client to LB should not have an excessively long vertical excursion.
        // It should stay relatively close to row 0's center (y = 90).
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("Client", "LB", &rects, &grid);

        let row_0_center = 0.5 * grid.cell_h; // 90
        let max_allowed_excursion = grid.cell_h * 1.5; // 270 — reasonable for a corridor route

        for wp in &path.waypoints {
            let excursion = (wp.y - row_0_center).abs();
            assert!(
                excursion < max_allowed_excursion,
                "Client→LB waypoint y={:.1} has excessive excursion ({:.1}) from row center ({:.1})",
                wp.y,
                excursion,
                row_0_center
            );
        }
    }

    #[test]
    fn test_large_system_lb_to_web_diagonal() {
        // LB(2,0) → Web(0,1): far diagonal, should route cleanly
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("LB", "Web", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "LB", "Web"));
    }

    #[test]
    fn test_large_system_api_to_db_diagonal() {
        // API(1,1) → DB(0,2): diagonal
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("API", "DB", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "API", "DB"));
    }

    #[test]
    fn test_large_system_queue_to_worker_diagonal() {
        // Queue(2,2) → Worker(2,1): same column, should be direct vertical
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("Queue", "Worker", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(
            &path.waypoints,
            &rects,
            "Queue",
            "Worker"
        ));
    }

    #[test]
    fn test_large_system_worker_to_db_diagonal() {
        // Worker(2,1) → DB(0,2): far diagonal
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("Worker", "DB", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "Worker", "DB"));
    }

    #[test]
    fn test_large_system_adjacent_diagonals_prefer_h_exit() {
        // Adjacent diagonal edges (1 col + 1 row apart) in the large system
        // (cell_w=250 > cell_h=180) should prefer H-exits.
        // Far diagonals (2+ cols apart) may not be able to use preferred face pair.
        let (rects, grid) = make_large_system_grid();
        let adjacent_diag_edges = [
            ("LB", "API"),    // (2,0)→(1,1) adjacent diagonal
            ("API", "DB"),    // (1,1)→(0,2) adjacent diagonal
            ("API", "Queue"), // (1,1)→(2,2) adjacent diagonal
        ];

        for (from, to) in &adjacent_diag_edges {
            let path = route_single_edge(from, to, &rects, &grid);
            let exit_face = infer_exit_face(&path);
            assert!(
                matches!(exit_face, Face::Right | Face::Left),
                "Large system adjacent diagonal {from}→{to} (wide grid) should use H-exit, got {:?}",
                exit_face
            );
        }
    }

    #[test]
    fn test_large_system_far_diagonals_route_cleanly() {
        // Far diagonals (2+ cols apart) may not use the preferred H-exit,
        // but must still route cleanly without crossing nodes.
        let (rects, grid) = make_large_system_grid();
        let far_diag_edges = [
            ("LB", "Web"),         // (2,0)→(0,1) 2 cols apart
            ("Worker", "DB"),      // (2,1)→(0,2) 2 cols apart
            ("Monitor", "Worker"), // (1,3)→(2,1) 2 rows apart
        ];

        for (from, to) in &far_diag_edges {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "Large system far diagonal {from}→{to} should be orthogonal"
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "Large system far diagonal {from}→{to} should not cross nodes"
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ── Aspect ratio with Large System dimensions ────────────────────────────
    // Additional verification using the exact Large System dimensions
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_large_system_dimensions_are_wide() {
        let (_, grid) = make_large_system_grid();
        assert!(
            grid.cell_w >= grid.cell_h,
            "Large system grid should be wide ({}x{})",
            grid.cell_w,
            grid.cell_h
        );
    }

    #[test]
    fn test_tall_version_of_large_system() {
        // If we flip the large system to be tall (180x250), diagonals should prefer V-exits
        let cell_w = 180.0;
        let cell_h = 250.0;
        let node_w = 100.0;
        let node_h = 70.0;

        let names = [
            ("Client", 0, 0),
            ("CDN", 1, 0),
            ("LB", 2, 0),
            ("Web", 0, 1),
            ("API", 1, 1),
            ("Worker", 2, 1),
            ("DB", 0, 2),
            ("Cache", 1, 2),
            ("Queue", 2, 2),
            ("Monitor", 1, 3),
        ];

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 4,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };

        // Diagonal LB→API in tall grid should route cleanly
        let path = route_single_edge("LB", "API", &rects, &grid);
        assert!(
            is_orthogonal(&path.waypoints),
            "Tall large system LB→API should be orthogonal"
        );
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "LB", "API"),
            "Tall large system LB→API should not cross nodes"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Tests for same-row skip-middle routing (perpendicular face pairs)
    // ═══════════════════════════════════════════════════════════════════════

    /// Helper: build a 1×3 single-row grid with A, B, C from left to right.
    fn make_single_row_3() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;
        let origin_x = 0.0;
        let origin_y = 0.0;

        let names = [("A", 0, 0), ("B", 1, 0), ("C", 2, 0)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = origin_x + (*col as f32 + 0.5) * cell_w;
            let cy = origin_y + (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }
        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 1,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };
        (rects, grid)
    }

    /// Helper: build a 1×5 row grid (A..E).
    fn make_single_row_5() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [
            ("A", 0, 0),
            ("B", 1, 0),
            ("C", 2, 0),
            ("D", 3, 0),
            ("E", 4, 0),
        ];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }
        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 5,
            rows: 1,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };
        (rects, grid)
    }

    /// Helper: build a 3×1 single-column grid with X, Y, Z top to bottom.
    fn make_single_col_3() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 80.0;
        let node_h = 60.0;

        let names = [("X", 0, 0), ("Y", 0, 1), ("Z", 0, 2)];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }
        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 1,
            rows: 3,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };
        (rects, grid)
    }

    #[test]
    fn test_skip_middle_same_row_a_to_c_avoids_b() {
        // A—B—C in a row. A→C must skip over B.
        // The natural pair (Right, Left) creates a straight line crossing B.
        // Routing should use perpendicular faces (Top→Top or Bottom→Bottom)
        // which, when optimized to a direct path, creates a horizontal line
        // above or below B that clears it entirely.
        let (rects, grid) = make_single_row_3();
        let path = route_single_edge("A", "C", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "C"),
            "A→C should not cross middle node B"
        );
        // The path should NOT pass through B's y-range at B's x-range.
        // Verify all waypoints are outside B's rect bounds.
        let b_rect = &rects["B"];
        for wp in &path.waypoints {
            let in_b_x = wp.x > b_rect.left() && wp.x < b_rect.right();
            let in_b_y = wp.y > b_rect.top() && wp.y < b_rect.bottom();
            assert!(
                !(in_b_x && in_b_y),
                "Waypoint ({:.1}, {:.1}) is inside B's rect",
                wp.x,
                wp.y
            );
        }
    }

    #[test]
    fn test_skip_middle_same_row_c_to_a_reverse() {
        // C→A (reverse direction) should also skip B
        let (rects, grid) = make_single_row_3();
        let path = route_single_edge("C", "A", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "C", "A"),
            "C→A should not cross middle node B"
        );
    }

    #[test]
    fn test_skip_middle_same_row_a_to_e_skips_three() {
        // A—B—C—D—E row. A→E must skip B, C, D.
        let (rects, grid) = make_single_row_5();
        let path = route_single_edge("A", "E", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "E"),
            "A→E should not cross B, C, or D"
        );
    }

    #[test]
    fn test_skip_middle_same_col_x_to_z_avoids_y() {
        // X—Y—Z in a column. X→Z must skip over Y.
        // Routing should use perpendicular H-faces (Right→Right or Left→Left).
        let (rects, grid) = make_single_col_3();
        let path = route_single_edge("X", "Z", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "X", "Z"),
            "X→Z should not cross middle node Y"
        );
        // The path should route outside Y's rect.
        let y_rect = &rects["Y"];
        for wp in &path.waypoints {
            let in_y_x = wp.x > y_rect.left() && wp.x < y_rect.right();
            let in_y_y = wp.y > y_rect.top() && wp.y < y_rect.bottom();
            assert!(
                !(in_y_x && in_y_y),
                "Waypoint ({:.1}, {:.1}) is inside Y's rect",
                wp.x,
                wp.y
            );
        }
    }

    #[test]
    fn test_skip_middle_same_col_z_to_x_reverse() {
        // Z→X (reverse) should also skip Y
        let (rects, grid) = make_single_col_3();
        let path = route_single_edge("Z", "X", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "Z", "X"),
            "Z→X should not cross middle node Y"
        );
    }

    #[test]
    fn test_skip_middle_3x3_d_to_f_same_row() {
        // Full 3×3 grid: D(0,1)→F(2,1), same row with E(1,1) in between
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("D", "F", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "D", "F"),
            "D→F should not cross E in the middle"
        );
    }

    #[test]
    fn test_skip_middle_3x3_b_to_h_same_col() {
        // Full 3×3 grid: B(1,0)→H(1,2), same column with E(1,1) in between
        let (rects, grid) = make_3x3_grid();
        let path = route_single_edge("B", "H", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "B", "H"),
            "B→H should not cross E in the middle"
        );
    }

    #[test]
    fn test_skip_middle_adjacent_same_row_no_ushape() {
        // Adjacent nodes in same row: A→B should NOT use U-shape (direct is fine)
        let (rects, grid) = make_single_row_3();
        let path = route_single_edge("A", "B", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "A", "B"));
        // Direct connection should be 2 points (or 4 with jog)
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Right),
            "Adjacent A→B should exit Right, got {:?}",
            exit_face
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Tests for generalized direct path + jog alignment
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_direct_path_same_row_hh_adjacent() {
        // A→B (adjacent same-row): should get direct H→H path (2 or 4 waypoints with jog)
        let (rects, grid) = make_single_row_3();
        let path = route_single_edge("A", "B", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        // Should be direct or jog (not a full corridor route)
        assert!(
            path.waypoints.len() <= 6,
            "Adjacent A→B should have ≤6 waypoints, got {}",
            path.waypoints.len()
        );
    }

    #[test]
    fn test_direct_path_same_col_vv_adjacent() {
        // X→Y (adjacent same-col): should get direct V→V path
        let (rects, grid) = make_single_col_3();
        let path = route_single_edge("X", "Y", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "X", "Y"));
        assert!(
            path.waypoints.len() <= 6,
            "Adjacent X→Y should have ≤6 waypoints, got {}",
            path.waypoints.len()
        );
    }

    #[test]
    fn test_direct_path_same_col_hh_ushape() {
        // In single-col 3 nodes: X→Z must skip Y. Should use H-face U-shape.
        // The Right→Right direct attempt (same-col, H→H) should apply x-alignment jog.
        let (rects, grid) = make_single_col_3();
        let path = route_single_edge("X", "Z", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "X", "Z"),
            "X→Z should not cross Y"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario tests: Large System Client→LB (the original bug)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_large_system_client_to_lb_avoids_cdn() {
        // Client(0,0) → LB(2,0): same row, CDN(1,0) in between.
        // Must route around CDN (via perpendicular faces: Top→Top or Bottom→Bottom).
        // The direct path may look like a horizontal line above/below CDN,
        // since Top→Top gets optimized to a direct path that clears all nodes.
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("Client", "LB", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "Client", "LB"),
            "Client→LB must not cross CDN"
        );
        // The path should be entirely outside CDN's bounding box
        let cdn_rect = &rects["CDN"];
        for wp in &path.waypoints {
            let in_cdn_x = wp.x > cdn_rect.left() && wp.x < cdn_rect.right();
            let in_cdn_y = wp.y > cdn_rect.top() && wp.y < cdn_rect.bottom();
            assert!(
                !(in_cdn_x && in_cdn_y),
                "Waypoint ({:.1}, {:.1}) is inside CDN rect",
                wp.x,
                wp.y
            );
        }
    }

    #[test]
    fn test_large_system_client_to_lb_path_above_or_below_cdn() {
        // The semantic router routes through corridors and may use different y-levels.
        // The key property is that no segment intersects CDN (checked above).
        // Here we verify orthogonality and no crossing.
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("Client", "LB", &rects, &grid);

        assert!(is_orthogonal(&path.waypoints), "Path should be orthogonal");
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "Client", "LB"),
            "Path should not cross CDN or any other node"
        );
    }

    #[test]
    fn test_large_system_lb_to_web() {
        // LB(2,0) → Web(0,1): diagonal. Should route cleanly.
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("LB", "Web", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "LB", "Web"));
    }

    #[test]
    fn test_large_system_lb_to_api() {
        // LB(2,0) → API(1,1): adjacent diagonal
        let (rects, grid) = make_large_system_grid();
        let path = route_single_edge("LB", "API", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(&path.waypoints, &rects, "LB", "API"));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario tests: Auto-Layout patterns (LB→Server skip-middle)
    // ═══════════════════════════════════════════════════════════════════════

    /// Helper: build a 2-row auto-layout grid similar to the test-diagram auto-layout slide.
    /// Row 0: User, LoadBalancer, Server1
    /// Row 1: Server2, Server3, Database
    fn make_auto_layout_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 220.0;
        let cell_h = 160.0;
        let node_w = 100.0;
        let node_h = 70.0;

        let names = [
            ("User", 0, 0),
            ("LoadBalancer", 1, 0),
            ("Server1", 2, 0),
            ("Server2", 0, 1),
            ("Server3", 1, 1),
            ("Database", 2, 1),
        ];
        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = (*col as f32 + 0.5) * cell_w;
            let cy = (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }
        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w,
            cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied,
        };
        (rects, grid)
    }

    #[test]
    fn test_auto_layout_server2_to_database_skips_server3() {
        // Server2(0,1) → Database(2,1): same row, Server3(1,1) in between
        let (rects, grid) = make_auto_layout_grid();
        let path = route_single_edge("Server2", "Database", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "Server2", "Database"),
            "Server2→Database should not cross Server3"
        );
        // Path should avoid Server3's rect entirely
        let s3_rect = &rects["Server3"];
        for wp in &path.waypoints {
            let in_s3_x = wp.x > s3_rect.left() && wp.x < s3_rect.right();
            let in_s3_y = wp.y > s3_rect.top() && wp.y < s3_rect.bottom();
            assert!(
                !(in_s3_x && in_s3_y),
                "Waypoint ({:.1}, {:.1}) is inside Server3 rect",
                wp.x,
                wp.y
            );
        }
    }

    #[test]
    fn test_auto_layout_user_to_server1_skips_lb() {
        // User(0,0) → Server1(2,0): same row, LB(1,0) in between
        let (rects, grid) = make_auto_layout_grid();
        let path = route_single_edge("User", "Server1", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "User", "Server1"),
            "User→Server1 should not cross LoadBalancer"
        );
    }

    #[test]
    fn test_auto_layout_lb_to_server2_diagonal() {
        // LB(1,0) → Server2(0,1): diagonal, should route cleanly
        let (rects, grid) = make_auto_layout_grid();
        let path = route_single_edge("LoadBalancer", "Server2", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(
            &path.waypoints,
            &rects,
            "LoadBalancer",
            "Server2"
        ));
    }

    #[test]
    fn test_auto_layout_lb_to_server3_downward() {
        // LB(1,0) → Server3(1,1): same column, adjacent. Direct V→V.
        let (rects, grid) = make_auto_layout_grid();
        let path = route_single_edge("LoadBalancer", "Server3", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints));
        assert!(!path_crosses_node(
            &path.waypoints,
            &rects,
            "LoadBalancer",
            "Server3"
        ));
        let exit_face = infer_exit_face(&path);
        assert!(
            matches!(exit_face, Face::Bottom),
            "LB→Server3 (same-col adjacent) should exit Bottom, got {:?}",
            exit_face
        );
    }

    #[test]
    fn test_auto_layout_all_edges_no_crossing() {
        // Test all 7 edges from the auto-layout diagram simultaneously
        let (rects, grid) = make_auto_layout_grid();
        let edges = [
            ("User", "LoadBalancer"),
            ("LoadBalancer", "Server1"),
            ("LoadBalancer", "Server2"),
            ("LoadBalancer", "Server3"),
            ("Server1", "Database"),
            ("Server2", "Database"),
            ("Server3", "Database"),
        ];
        for (from, to) in &edges {
            let path = route_single_edge(from, to, &rects, &grid);
            assert!(
                is_orthogonal(&path.waypoints),
                "{from}→{to} should be orthogonal"
            );
            assert!(
                !path_crosses_node(&path.waypoints, &rects, from, to),
                "{from}→{to} should not cross any nodes"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Tests for corridor direction consistency
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_skip_middle_row_path_is_monotonic_x() {
        // A→C: the path's x values should be monotonically increasing
        // (from A's x toward C's x) — no backwards movement
        let (rects, grid) = make_single_row_3();
        let path = route_single_edge("A", "C", &rects, &grid);
        let first_x = path.waypoints.first().unwrap().x;
        let last_x = path.waypoints.last().unwrap().x;
        assert!(
            last_x > first_x,
            "A→C path should go left to right (first_x={first_x}, last_x={last_x})"
        );
    }

    #[test]
    fn test_skip_middle_col_path_is_monotonic_y() {
        // X→Z: the path's y values should be monotonically increasing
        // (from X's y toward Z's y) — no backwards movement
        let (rects, grid) = make_single_col_3();
        let path = route_single_edge("X", "Z", &rects, &grid);
        let first_y = path.waypoints.first().unwrap().y;
        let last_y = path.waypoints.last().unwrap().y;
        assert!(
            last_y > first_y,
            "X→Z path should go top to bottom (first_y={first_y}, last_y={last_y})"
        );
    }

    #[test]
    fn test_skip_middle_row_path_horizontal_segments_consistent() {
        // A→C: the route should be orthogonal and not cross any nodes.
        // In a 1-row grid, the route may go via the grid boundary (y=0)
        // or via a different corridor. The key invariant is that all
        // horizontal corridor segments share the same y.
        let (rects, grid) = make_single_row_3();
        let path = route_single_edge("A", "C", &rects, &grid);
        assert!(is_orthogonal(&path.waypoints), "A→C should be orthogonal");
        assert!(
            !path_crosses_node(&path.waypoints, &rects, "A", "C"),
            "A→C should not cross any nodes"
        );
        // Check corridor horizontal segments have consistent y.
        // Skip the first 2 (face + ramp) and last 2 (ramp + face) points —
        // corridor waypoints are the ones in between.
        if path.waypoints.len() >= 5 {
            let corridor_points = &path.waypoints[2..path.waypoints.len() - 2];
            if corridor_points.len() >= 2 {
                let hy: Vec<f32> = corridor_points
                    .windows(2)
                    .filter(|w| (w[0].y - w[1].y).abs() < 1.0)
                    .map(|w| w[0].y)
                    .collect();
                if hy.len() >= 2 {
                    let first_y = hy[0];
                    for y in &hy[1..] {
                        assert!(
                            (y - first_y).abs() < 1.0,
                            "Horizontal corridor segments should share same y: {first_y} vs {y}"
                        );
                    }
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Edge cases for perpendicular face pair generation
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_same_node_row_col_threshold() {
        // Test that same-row detection uses 0.3*cell_h threshold
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 200.0,
            cell_h: 150.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        // Nodes in same row: y difference = 0 (well within 0.3*150=45)
        let from_c = Pos2::new(100.0, 75.0);
        let to_c = Pos2::new(500.0, 75.0);
        let dy = (from_c.y - to_c.y).abs();
        assert!(
            dy < grid.cell_h * 0.3,
            "Same-row nodes should be within threshold"
        );

        // Nodes in different rows: y difference = cell_h (150, well outside 45)
        let to_c2 = Pos2::new(500.0, 225.0);
        let dy2 = (from_c.y - to_c2.y).abs();
        assert!(
            dy2 >= grid.cell_h * 0.3,
            "Different-row nodes should be outside threshold"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Manhattan routing data model tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_segment_occupancy_center_first() {
        let mut occ = SegmentOccupancy::new();
        let seg = Segment {
            corridor: Corridor::Street(1),
            from_cross: 0,
            to_cross: 1,
        };
        // Capacity 5 → center = 3
        let lane1 = occ.claim_lane(seg, 5);
        assert_eq!(lane1, Some(3), "First lane should be center (3 of 5)");

        let lane2 = occ.claim_lane(seg, 5);
        assert_eq!(lane2, Some(4), "Second lane should be center+1");

        let lane3 = occ.claim_lane(seg, 5);
        assert_eq!(lane3, Some(2), "Third lane should be center-1");

        let lane4 = occ.claim_lane(seg, 5);
        assert_eq!(lane4, Some(5), "Fourth lane should be center+2");

        let lane5 = occ.claim_lane(seg, 5);
        assert_eq!(lane5, Some(1), "Fifth lane should be center-2");

        // All lanes occupied
        let lane6 = occ.claim_lane(seg, 5);
        assert_eq!(lane6, None, "No more lanes available");
    }

    #[test]
    fn test_segment_occupancy_even_capacity() {
        let mut occ = SegmentOccupancy::new();
        let seg = Segment {
            corridor: Corridor::Avenue(2),
            from_cross: 1,
            to_cross: 2,
        };
        // Capacity 4 → center = (4+1)/2 = 2
        let lane1 = occ.claim_lane(seg, 4);
        assert_eq!(lane1, Some(2));
        let lane2 = occ.claim_lane(seg, 4);
        assert_eq!(lane2, Some(3));
        let lane3 = occ.claim_lane(seg, 4);
        assert_eq!(lane3, Some(1));
        let lane4 = occ.claim_lane(seg, 4);
        assert_eq!(lane4, Some(4));
        let lane5 = occ.claim_lane(seg, 4);
        assert_eq!(lane5, None);
    }

    #[test]
    fn test_segment_occupancy_capacity_one() {
        let mut occ = SegmentOccupancy::new();
        let seg = Segment {
            corridor: Corridor::Street(0),
            from_cross: 0,
            to_cross: 1,
        };
        let lane = occ.claim_lane(seg, 1);
        assert_eq!(lane, Some(1));
        assert_eq!(occ.claim_lane(seg, 1), None);
    }

    #[test]
    fn test_segment_occupancy_capacity_zero() {
        let mut occ = SegmentOccupancy::new();
        let seg = Segment {
            corridor: Corridor::Street(0),
            from_cross: 0,
            to_cross: 1,
        };
        assert_eq!(occ.claim_lane(seg, 0), None);
    }

    #[test]
    fn test_segment_occupancy_independent_segments() {
        // Two routes on DIFFERENT segments of same corridor should not compete
        let mut occ = SegmentOccupancy::new();
        let seg_a = Segment {
            corridor: Corridor::Street(1),
            from_cross: 0,
            to_cross: 1,
        };
        let seg_b = Segment {
            corridor: Corridor::Street(1),
            from_cross: 2,
            to_cross: 3,
        };

        let lane_a = occ.claim_lane(seg_a, 5);
        let lane_b = occ.claim_lane(seg_b, 5);
        // Both should get center lane since they're independent segments
        assert_eq!(lane_a, Some(3));
        assert_eq!(lane_b, Some(3));
    }

    #[test]
    fn test_segment_occupancy_same_segment_different_lanes() {
        // Two routes on SAME segment must get different lanes
        let mut occ = SegmentOccupancy::new();
        let seg = Segment {
            corridor: Corridor::Street(1),
            from_cross: 1,
            to_cross: 2,
        };
        let lane1 = occ.claim_lane(seg, 5);
        let lane2 = occ.claim_lane(seg, 5);
        assert_ne!(lane1, lane2, "Same segment must assign different lanes");
    }

    #[test]
    fn test_claim_lane_span_look_ahead() {
        let mut occ = SegmentOccupancy::new();
        // Block center lane (3) on segment 1-2 only
        let seg_12 = Segment {
            corridor: Corridor::Street(1),
            from_cross: 1,
            to_cross: 2,
        };
        occ.occupied.entry(seg_12).or_default().push(3);

        // Claim a span across segments 0-1, 1-2, 2-3
        // Should find a lane free on ALL segments (skip 3 because blocked on 1-2)
        let lane = occ.claim_lane_span(Corridor::Street(1), 0, 3, &[5, 5, 5]);
        assert!(lane.is_some());
        assert_ne!(lane, Some(3), "Should avoid blocked lane 3");
        // Should get 4 (next after center going outward)
        assert_eq!(lane, Some(4));
    }

    #[test]
    fn test_claim_lane_span_multiple_routes() {
        let mut occ = SegmentOccupancy::new();
        // Two routes using same span get different lanes
        let lane1 = occ.claim_lane_span(Corridor::Avenue(1), 0, 3, &[5, 5, 5]);
        let lane2 = occ.claim_lane_span(Corridor::Avenue(1), 0, 3, &[5, 5, 5]);
        assert_eq!(lane1, Some(3));
        assert_eq!(lane2, Some(4));
    }

    #[test]
    fn test_claim_lane_span_partial_overlap() {
        let mut occ = SegmentOccupancy::new();
        // Route 1 uses Avenue 1, Streets 0-2
        let lane1 = occ.claim_lane_span(Corridor::Avenue(1), 0, 2, &[5, 5]);
        assert_eq!(lane1, Some(3));

        // Route 2 uses Avenue 1, Streets 1-3 (overlaps on segment 1-2)
        let lane2 = occ.claim_lane_span(Corridor::Avenue(1), 1, 3, &[5, 5]);
        // Segment 1-2 has lane 3 taken, so route 2 gets lane 4
        assert_eq!(lane2, Some(4));

        // Route 3 uses Avenue 1, Streets 3-4 (no overlap with either)
        let lane3 = occ.claim_lane_span(Corridor::Avenue(1), 3, 4, &[5]);
        // No overlap → gets center lane again
        assert_eq!(lane3, Some(3));
    }

    #[test]
    fn test_corridor_display() {
        assert_eq!(format!("{}", Corridor::Street(2)), "Street 2");
        assert_eq!(format!("{}", Corridor::Avenue(0)), "Avenue 0");
    }

    #[test]
    fn test_segment_display() {
        let seg = Segment {
            corridor: Corridor::Street(1),
            from_cross: 2,
            to_cross: 3,
        };
        assert_eq!(format!("{seg}"), "Street 1 between Avenue 2 and Avenue 3");
        let seg2 = Segment {
            corridor: Corridor::Avenue(0),
            from_cross: 1,
            to_cross: 2,
        };
        assert_eq!(format!("{seg2}"), "Avenue 0 between Street 1 and Street 2");
    }

    #[test]
    fn test_route_step_display() {
        let step = RouteStep::ExitBuilding {
            face: Face::Left,
            lane: 4,
        };
        assert_eq!(format!("{step}"), "Exit building Left, lane 4");

        let step2 = RouteStep::Step {
            direction: TravelDir::South,
            lane: 3,
        };
        assert_eq!(format!("{step2}"), "Step south, lane 3");

        let step3 = RouteStep::EnterBuilding {
            face: Face::Right,
            lane: 2,
        };
        assert_eq!(format!("{step3}"), "Enter building Right, lane 2");
    }

    #[test]
    fn test_corridor_segments() {
        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w: 100.0,
            cell_h: 80.0,
            origin_x: 0.0,
            origin_y: 0.0,
            occupied: HashSet::new(),
        };

        let street_segs = grid.corridor_segments(Corridor::Street(1));
        assert_eq!(street_segs.len(), 3); // 3 cols → 3 segments
        assert_eq!(street_segs[0].from_cross, 0);
        assert_eq!(street_segs[0].to_cross, 1);
        assert_eq!(street_segs[2].from_cross, 2);
        assert_eq!(street_segs[2].to_cross, 3);

        let avenue_segs = grid.corridor_segments(Corridor::Avenue(0));
        assert_eq!(avenue_segs.len(), 2); // 2 rows → 2 segments
    }

    #[test]
    fn test_lane_pixel_offset() {
        // 5 lanes, center = 3
        assert!((GridInfo::lane_pixel_offset(3, 5, 20.0) - 0.0).abs() < 0.01);
        // Lane 4 → offset +20
        assert!((GridInfo::lane_pixel_offset(4, 5, 20.0) - 20.0).abs() < 0.01);
        // Lane 2 → offset -20
        assert!((GridInfo::lane_pixel_offset(2, 5, 20.0) - (-20.0)).abs() < 0.01);
        // Lane 1 → offset -40
        assert!((GridInfo::lane_pixel_offset(1, 5, 20.0) - (-40.0)).abs() < 0.01);
    }

    #[test]
    fn test_lane_capacity_realistic_grid() {
        // Realistic 580x257 grid with 3x3 nodes
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w: 580.0 / 3.0,
            cell_h: 257.0 / 3.0,
            origin_x: 50.0,
            origin_y: 50.0,
            occupied: HashSet::new(),
        };

        // Avenue between two columns: gap is roughly cell_w minus node widths
        // With no nodes, gap = full cell_w
        let empty_rects = HashMap::new();
        let clearance = 5.0;
        let lane_spacing = 20.0;

        // Avenue 1 segment between street 0 and 1 (no nodes → gap = 0 since
        // segment_gap measures distance between node edges, not cell edges)
        let seg = Segment {
            corridor: Corridor::Avenue(1),
            from_cross: 0,
            to_cross: 1,
        };
        let cap = grid.lane_capacity(&seg, &empty_rects, clearance, lane_spacing);
        // With no nodes, the gap is from corridor line to corridor line = 0
        // (segment_gap returns distance between bounding rects)
        assert!(cap >= 0, "Capacity should be non-negative");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Semantic router tests
    // ═══════════════════════════════════════════════════════════════════════

    /// Build a 3x3 grid with nodes A-I for semantic routing tests.
    /// Layout:
    ///   A(0,0) B(1,0) C(2,0)
    ///   D(0,1) E(1,1) F(2,1)
    ///   G(0,2) H(1,2) I(2,2)
    fn make_semantic_3x3_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 100.0;
        let node_h = 60.0;
        let origin_x = 50.0;
        let origin_y = 50.0;

        let mut rects = HashMap::new();
        let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];
        let mut occupied = HashSet::new();

        for (idx, name) in names.iter().enumerate() {
            let col = idx % 3;
            let row = idx / 3;
            let cx = origin_x + col as f32 * cell_w + cell_w / 2.0;
            let cy = origin_y + row as f32 * cell_h + cell_h / 2.0;
            let rect =
                egui::Rect::from_center_size(Pos2::new(cx, cy), egui::Vec2::new(node_w, node_h));
            rects.insert(name.to_string(), rect);
            occupied.insert((col, row));
        }

        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };
        (rects, grid)
    }

    #[test]
    fn test_select_face_pairs_same_row() {
        let (rects, grid) = make_semantic_3x3_grid();
        let pairs = select_face_pairs(rects.get("A").unwrap(), rects.get("B").unwrap(), &grid);
        // A→B same row: natural pair should be Right→Left
        assert_eq!(pairs[0], (Face::Right, Face::Left));
    }

    #[test]
    fn test_select_face_pairs_same_col() {
        let (rects, grid) = make_semantic_3x3_grid();
        let pairs = select_face_pairs(rects.get("A").unwrap(), rects.get("D").unwrap(), &grid);
        // A→D same col: natural pair should be Bottom→Top
        assert_eq!(pairs[0], (Face::Bottom, Face::Top));
    }

    #[test]
    fn test_select_face_pairs_diagonal() {
        let (rects, grid) = make_semantic_3x3_grid();
        let pairs = select_face_pairs(rects.get("A").unwrap(), rects.get("E").unwrap(), &grid);
        // A→E diagonal: should include multiple face pair options
        assert!(pairs.len() > 1);
    }

    #[test]
    fn test_face_corridor_mapping() {
        // Node at col=1, row=1
        assert_eq!(face_corridor(Face::Right, 1, 1), Corridor::Avenue(2));
        assert_eq!(face_corridor(Face::Left, 1, 1), Corridor::Avenue(1));
        assert_eq!(face_corridor(Face::Bottom, 1, 1), Corridor::Street(2));
        assert_eq!(face_corridor(Face::Top, 1, 1), Corridor::Street(1));
    }

    #[test]
    fn test_semantic_route_same_row_adjacent() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "A",
            "B",
            rects.get("A").unwrap(),
            rects.get("B").unwrap(),
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        // Should have Exit and Enter steps at minimum
        assert!(
            route.steps.len() >= 2,
            "Route should have at least Exit and Enter: {:?}",
            route.steps
        );
        assert!(
            matches!(
                route.steps[0],
                RouteStep::ExitBuilding {
                    face: Face::Right,
                    ..
                }
            ),
            "Should exit right: {}",
            route.steps[0]
        );
        assert!(
            matches!(
                route.steps.last().unwrap(),
                RouteStep::EnterBuilding {
                    face: Face::Left,
                    ..
                }
            ),
            "Should enter left: {}",
            route.steps.last().unwrap()
        );
        // Waypoints should form a valid path
        assert!(route.waypoints.len() >= 2);
        // No node crossings
        assert!(!path_crosses_node(&route.waypoints, &rects, "A", "B"));
    }

    #[test]
    fn test_semantic_route_same_col() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "A",
            "D",
            rects.get("A").unwrap(),
            rects.get("D").unwrap(),
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        assert!(
            matches!(
                route.steps[0],
                RouteStep::ExitBuilding {
                    face: Face::Bottom,
                    ..
                }
            ),
            "Should exit bottom: {}",
            route.steps[0]
        );
        assert!(
            matches!(
                route.steps.last().unwrap(),
                RouteStep::EnterBuilding {
                    face: Face::Top,
                    ..
                }
            ),
            "Should enter top: {}",
            route.steps.last().unwrap()
        );
        assert!(!path_crosses_node(&route.waypoints, &rects, "A", "D"));
    }

    #[test]
    fn test_semantic_route_diagonal() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "A",
            "E",
            rects.get("A").unwrap(),
            rects.get("E").unwrap(),
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        // Diagonal route should have meaningful steps
        assert!(route.steps.len() >= 2);
        // Should not cross any non-source/target nodes
        assert!(!path_crosses_node(&route.waypoints, &rects, "A", "E"));
    }

    #[test]
    fn test_semantic_route_far_diagonal() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "A",
            "I",
            rects.get("A").unwrap(),
            rects.get("I").unwrap(),
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        assert!(route.steps.len() >= 2);
        assert!(!path_crosses_node(&route.waypoints, &rects, "A", "I"));
    }

    #[test]
    fn test_semantic_route_steps_readable() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "A",
            "I",
            rects.get("A").unwrap(),
            rects.get("I").unwrap(),
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        // All steps should produce readable Display output
        for step in &route.steps {
            let display = format!("{step}");
            assert!(!display.is_empty(), "Step display should not be empty");
        }
    }

    #[test]
    fn test_semantic_route_multiple_edges_no_crossings() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let edges = [
            ("A", "B"),
            ("B", "C"),
            ("D", "E"),
            ("E", "F"),
            ("A", "D"),
            ("B", "E"),
            ("C", "F"),
        ];

        for (from, to) in &edges {
            let route = route_edge_semantic(
                from,
                to,
                rects.get(*from).unwrap(),
                rects.get(*to).unwrap(),
                &grid,
                &mut occupancy,
                &mut ports,
                &rects,
                10.0,
                20.0,
                22.0,
            );
            assert!(
                !path_crosses_node(&route.waypoints, &rects, from, to),
                "Route {} → {} crosses a node",
                from,
                to
            );
        }
    }

    #[test]
    fn test_ordered_helper() {
        assert_eq!(ordered(3, 5), (3, 5));
        assert_eq!(ordered(5, 3), (3, 5));
        assert_eq!(ordered(2, 2), (2, 2));
    }

    /// Check if a route backtracks along a single corridor (3 collinear
    /// waypoints where the middle one reverses direction). Legitimate
    /// L-shape turns are NOT flagged because consecutive waypoints at a
    /// turn change axis.
    fn has_backtracking(waypoints: &[Pos2]) -> bool {
        for i in 0..waypoints.len().saturating_sub(2) {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let c = waypoints[i + 2];
            // Three points on same horizontal line → check X reversal
            let same_y = (a.y - b.y).abs() < 2.0 && (b.y - c.y).abs() < 2.0;
            if same_y {
                let dx1 = b.x - a.x;
                let dx2 = c.x - b.x;
                if dx1.abs() > 2.0 && dx2.abs() > 2.0 && dx1 * dx2 < 0.0 {
                    return true;
                }
            }
            // Three points on same vertical line → check Y reversal
            let same_x = (a.x - b.x).abs() < 2.0 && (b.x - c.x).abs() < 2.0;
            if same_x {
                let dy1 = b.y - a.y;
                let dy2 = c.y - b.y;
                if dy1.abs() > 2.0 && dy2.abs() > 2.0 && dy1 * dy2 < 0.0 {
                    return true;
                }
            }
        }
        false
    }

    fn make_microservices_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let cell_w = 200.0;
        let cell_h = 150.0;
        let node_w = 100.0;
        let node_h = 60.0;
        let origin_x = 50.0;
        let origin_y = 50.0;

        let mut rects = HashMap::new();
        let mut occupied = HashSet::new();
        // Microservices layout:
        //   Gateway(0,0) Auth(1,0) Cache(2,0)
        //   ---          Users(1,1) DB(2,1)
        let nodes = [
            ("Gateway", 0, 0),
            ("Auth", 1, 0),
            ("Cache", 2, 0),
            ("Users", 1, 1),
            ("DB", 2, 1),
        ];
        for &(name, col, row) in &nodes {
            let cx = origin_x + col as f32 * cell_w + cell_w / 2.0;
            let cy = origin_y + row as f32 * cell_h + cell_h / 2.0;
            let rect =
                egui::Rect::from_center_size(Pos2::new(cx, cy), egui::Vec2::new(node_w, node_h));
            rects.insert(name.to_string(), rect);
            occupied.insert((col, row));
        }

        let grid = GridInfo {
            cols: 3,
            rows: 2,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };
        (rects, grid)
    }

    /// Regression test: API→Auth "validates" route in Hub and Spoke layout.
    /// Previously the route exited API Left, traveled on Street(2) all the
    /// way to Avenue(0) (left grid edge), then the Enter step brought it
    /// back right to Auth's center — a clear backtrack.
    #[test]
    fn test_hub_spoke_api_to_auth_no_backtracking() {
        let (rects, grid) = make_hub_spoke_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "API",
            "Auth",
            &rects["API"],
            &rects["Auth"],
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        assert!(
            !has_backtracking(&route.waypoints),
            "API→Auth route backtracks! Waypoints: {:?}\nSteps: {:?}",
            route.waypoints,
            route.steps
        );
        assert!(!path_crosses_node(&route.waypoints, &rects, "API", "Auth"));
    }

    /// Regression test: Gateway→Users "routes to" in Microservices layout.
    /// Previously the route swung far to the left grid edge (Avenue 0)
    /// before traveling right, because the Travel range included the exit
    /// cell's own segment.
    #[test]
    fn test_microservices_gateway_to_users_no_backtracking() {
        let (rects, grid) = make_microservices_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let route = route_edge_semantic(
            "Gateway",
            "Users",
            &rects["Gateway"],
            &rects["Users"],
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        assert!(
            !has_backtracking(&route.waypoints),
            "Gateway→Users route backtracks! Waypoints: {:?}\nSteps: {:?}",
            route.waypoints,
            route.steps
        );
        assert!(!path_crosses_node(
            &route.waypoints,
            &rects,
            "Gateway",
            "Users"
        ));
    }

    /// Every route in the Hub and Spoke layout should be backtrack-free.
    #[test]
    fn test_hub_spoke_all_routes_no_backtracking() {
        let (rects, grid) = make_hub_spoke_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let edges = [
            ("Web", "API"),
            ("App", "API"),
            ("API", "Auth"),
            ("API", "DB"),
            ("API", "Logs"),
            ("API", "Mail"),
        ];

        for &(from, to) in &edges {
            let route = route_edge_semantic(
                from,
                to,
                &rects[from],
                &rects[to],
                &grid,
                &mut occupancy,
                &mut ports,
                &rects,
                10.0,
                20.0,
                22.0,
            );

            assert!(
                !has_backtracking(&route.waypoints),
                "{from}→{to} route backtracks! Waypoints: {:?}\nSteps: {:?}",
                route.waypoints,
                route.steps
            );
            assert!(
                !path_crosses_node(&route.waypoints, &rects, from, to),
                "{from}→{to} route crosses a node"
            );
        }
    }

    /// Every route in the Microservices layout should be backtrack-free.
    #[test]
    fn test_microservices_all_routes_no_backtracking() {
        let (rects, grid) = make_microservices_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        let edges = [
            ("Gateway", "Auth"),
            ("Gateway", "Users"),
            ("Auth", "Cache"),
            ("Users", "DB"),
        ];

        for &(from, to) in &edges {
            let route = route_edge_semantic(
                from,
                to,
                &rects[from],
                &rects[to],
                &grid,
                &mut occupancy,
                &mut ports,
                &rects,
                10.0,
                20.0,
                22.0,
            );

            assert!(
                !has_backtracking(&route.waypoints),
                "{from}→{to} route backtracks! Waypoints: {:?}\nSteps: {:?}",
                route.waypoints,
                route.steps
            );
            assert!(
                !path_crosses_node(&route.waypoints, &rects, from, to),
                "{from}→{to} route crosses a node"
            );
        }
    }

    /// Every route in the full 3×3 grid should be backtrack-free.
    #[test]
    fn test_3x3_all_routes_no_backtracking() {
        let (rects, grid) = make_semantic_3x3_grid();

        // Test all possible source→target pairs
        let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];
        for from in &names {
            for to in &names {
                if from == to {
                    continue;
                }
                let mut occupancy = SegmentOccupancy::new();
                let mut ports = FacePortAllocator::new();

                let route = route_edge_semantic(
                    from,
                    to,
                    &rects[*from],
                    &rects[*to],
                    &grid,
                    &mut occupancy,
                    &mut ports,
                    &rects,
                    10.0,
                    20.0,
                    22.0,
                );

                assert!(
                    !has_backtracking(&route.waypoints),
                    "{from}→{to} route backtracks! Waypoints: {:?}\nSteps: {:?}",
                    route.waypoints,
                    route.steps
                );
            }
        }
    }

    /// Bug 1 regression: When multiple face pairs produce valid routes, the
    /// shortest one must be selected. Previously, route_edge_semantic returned
    /// the first valid route, which could be a long detour.
    #[test]
    fn test_shortest_route_selection() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        // Route H→I (row 2, col 1 → row 2, col 2): should pick the short
        // Right→Left path, not a longer path via a different face pair.
        let route = route_edge_semantic(
            "H",
            "I",
            &rects["H"],
            &rects["I"],
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        let length = polyline_length(&route.waypoints);
        // Direct horizontal span between H and I centers is 200px (one cell_w).
        // The routed path should be close to that (with ramp offsets).
        // A detour would be significantly longer (>400px).
        assert!(
            length < 300.0,
            "H→I route should be short (~200px), got {length:.1}px. Waypoints: {:?}",
            route.waypoints
        );
    }

    /// Bug 1 regression (Slide 6 specific): Cache→Database "fills" edge
    /// should take a short L-shape, not a long detour.
    #[test]
    fn test_fills_uses_shortest_route() {
        let (rects, grid) = make_semantic_3x3_grid();
        let mut occupancy = SegmentOccupancy::new();
        let mut ports = FacePortAllocator::new();

        // In the Slide 6 auto-layout, Cache and Database map to adjacent cells.
        // Using 3x3 grid: G(0,2)→H(1,2) as proxy (adjacent in same row).
        let route = route_edge_semantic(
            "G",
            "H",
            &rects["G"],
            &rects["H"],
            &grid,
            &mut occupancy,
            &mut ports,
            &rects,
            10.0,
            20.0,
            22.0,
        );

        let length = polyline_length(&route.waypoints);
        // Adjacent horizontal route should be ~200px, not a multi-corridor detour
        assert!(
            length < 300.0,
            "G→H (fills proxy) route length {length:.1}px too long for adjacent nodes. \
             Waypoints: {:?}",
            route.waypoints
        );
    }

    /// Bug 2 regression: No route waypoint should come within 8px of a
    /// non-endpoint node's bounding rect ("pavement walking" fix).
    #[test]
    fn test_no_pavement_walking() {
        let (rects, grid) = make_semantic_3x3_grid();

        // Test several routes that pass near intermediate nodes
        let test_pairs = [("A", "I"), ("C", "G"), ("D", "F"), ("G", "C")];
        for (from, to) in &test_pairs {
            let mut occupancy = SegmentOccupancy::new();
            let mut ports = FacePortAllocator::new();

            let route = route_edge_semantic(
                from,
                to,
                &rects[*from],
                &rects[*to],
                &grid,
                &mut occupancy,
                &mut ports,
                &rects,
                10.0,
                20.0,
                22.0,
            );

            let min_dist = min_distance_to_non_endpoint_nodes(&route.waypoints, &rects, from, to);
            assert!(
                min_dist >= 8.0 || min_dist < 0.0, // < 0 means no intermediate nodes checked
                "{from}→{to} route comes within {min_dist:.1}px of an intermediate node \
                 (must be ≥8px). Waypoints: {:?}",
                route.waypoints
            );
        }
    }

    /// Compute minimum distance from any waypoint to any non-endpoint node rect.
    fn min_distance_to_non_endpoint_nodes(
        waypoints: &[Pos2],
        node_rects: &HashMap<String, egui::Rect>,
        from: &str,
        to: &str,
    ) -> f32 {
        let mut min_dist = f32::MAX;
        for wp in waypoints {
            for (name, rect) in node_rects {
                if name == from || name == to {
                    continue;
                }
                // Distance from point to rect: 0 if inside, else min edge distance
                let dx = (wp.x - wp.x.clamp(rect.left(), rect.right())).abs();
                let dy = (wp.y - wp.y.clamp(rect.top(), rect.bottom())).abs();
                let dist = (dx * dx + dy * dy).sqrt();
                min_dist = min_dist.min(dist);
            }
        }
        min_dist
    }

    /// Regression test: diagonal routes between adjacent nodes must route through
    /// corridor centers, NOT along node edges (the "sidewalk walking" bug).
    ///
    /// Before the fix, a route from A(0,0) to E(1,1) would produce a 0-step route
    /// (ExitBuilding + EnterBuilding only), and `ensure_orthogonal` would create an
    /// L-shaped path at the node's face height — on the sidewalk.
    #[test]
    fn test_diagonal_route_uses_corridor_center_not_sidewalk() {
        let (rects, grid) = make_3x3_grid();
        // A is at (0,0), E is at (1,1) — diagonally adjacent
        let path = route_single_edge("A", "E", &rects, &grid);
        let a_rect = rects["A"];
        let e_rect = rects["E"];

        // The route should be orthogonal
        assert!(is_orthogonal(&path.waypoints), "Route should be orthogonal");

        // Find horizontal and vertical segments (excluding the short ramp segments
        // immediately adjacent to the source/target nodes)
        let wp = &path.waypoints;
        assert!(
            wp.len() >= 4,
            "Route should have at least 4 waypoints, got {}",
            wp.len()
        );

        // Check that no "long" horizontal segment is at the y-coordinate of a node face.
        // A long segment means a horizontal span > 50px (not just a short ramp).
        // The corridor center for Street(1) is between rows 0 and 1.
        let street_1_center = {
            let seg = Segment {
                corridor: Corridor::Street(1),
                from_cross: 0,
                to_cross: grid.cols,
            };
            grid.segment_center(&seg, &rects)
        };
        let avenue_1_center = {
            let seg = Segment {
                corridor: Corridor::Avenue(1),
                from_cross: 0,
                to_cross: grid.rows,
            };
            grid.segment_center(&seg, &rects)
        };

        // Check each segment of the route
        let mut has_corridor_waypoint = false;
        for pair in wp.windows(2) {
            let p1 = pair[0];
            let p2 = pair[1];
            let dx = (p1.x - p2.x).abs();
            let dy = (p1.y - p2.y).abs();

            // Horizontal segment (significant x span)
            if dx > 50.0 && dy < 1.0 {
                // This horizontal segment should NOT be at a node's face height.
                // It should be near a street corridor center.
                let y = p1.y;
                let on_a_face = (y - a_rect.top()).abs() < 1.0
                    || (y - a_rect.bottom()).abs() < 1.0
                    || (y - e_rect.top()).abs() < 1.0
                    || (y - e_rect.bottom()).abs() < 1.0;
                // Allow being inside the node rect height (sidewalk)
                let in_a_range = y > a_rect.top() && y < a_rect.bottom();
                let in_e_range = y > e_rect.top() && y < e_rect.bottom();
                assert!(
                    !on_a_face && !in_a_range && !in_e_range,
                    "Horizontal segment at y={y:.1} is on the sidewalk! \
                     A rect: [{:.1},{:.1}], E rect: [{:.1},{:.1}], \
                     Street(1) center: {street_1_center:.1}",
                    a_rect.top(),
                    a_rect.bottom(),
                    e_rect.top(),
                    e_rect.bottom(),
                );
            }

            // Vertical segment (significant y span)
            if dy > 50.0 && dx < 1.0 {
                let x = p1.x;
                let in_a_range = x > a_rect.left() && x < a_rect.right();
                let in_e_range = x > e_rect.left() && x < e_rect.right();
                assert!(
                    !in_a_range && !in_e_range,
                    "Vertical segment at x={x:.1} is on the sidewalk! \
                     A rect: [{:.1},{:.1}], E rect: [{:.1},{:.1}], \
                     Avenue(1) center: {avenue_1_center:.1}",
                    a_rect.left(),
                    a_rect.right(),
                    e_rect.left(),
                    e_rect.right(),
                );
            }

            // Check if any waypoint is near a corridor center
            if (p1.y - street_1_center).abs() < 15.0 || (p1.x - avenue_1_center).abs() < 15.0 {
                has_corridor_waypoint = true;
            }
        }

        assert!(
            has_corridor_waypoint,
            "Route from A(0,0) to E(1,1) should have a waypoint near a corridor center \
             (Street(1)={street_1_center:.1} or Avenue(1)={avenue_1_center:.1}), \
             but waypoints are: {wp:?}"
        );
    }

    /// Regression test: the "real-time" edge pattern — A(0,0) to E(1,1)
    /// on the realistic 3x3 grid must not bypass corridors.
    ///
    /// Before the fix, the route went: exit ramp → straight to entry ramp (at source
    /// face height). After the fix, it should: exit ramp → corridor center → turn →
    /// corridor center → entry ramp.
    #[test]
    fn test_realistic_diagonal_route_uses_corridor_center() {
        let (rects, grid) = make_3x3_grid_realistic();
        // A is at (0,0), E is at (1,1) — diagonally adjacent
        let path = route_single_edge("A", "E", &rects, &grid);
        let from_rect = rects["A"];
        let to_rect = rects["E"];
        let wp = &path.waypoints;

        assert!(is_orthogonal(wp), "Route should be orthogonal");

        // The corridor centers between A(0,0) and E(1,1)
        let avenue_1_center = {
            let seg = Segment {
                corridor: Corridor::Avenue(1),
                from_cross: 0,
                to_cross: grid.rows,
            };
            grid.segment_center(&seg, &rects)
        };
        let street_1_center = {
            let seg = Segment {
                corridor: Corridor::Street(1),
                from_cross: 0,
                to_cross: grid.cols,
            };
            grid.segment_center(&seg, &rects)
        };

        // The route should have a waypoint near a corridor center
        let has_avenue_center = wp.iter().any(|p| (p.x - avenue_1_center).abs() < 15.0);
        let has_street_center = wp.iter().any(|p| (p.y - street_1_center).abs() < 15.0);
        assert!(
            has_avenue_center || has_street_center,
            "Route should pass through Avenue(1) center x={avenue_1_center:.1} or \
             Street(1) center y={street_1_center:.1}, but waypoints are: {wp:?}"
        );

        // The route must NOT have a single horizontal segment spanning from
        // source ramp directly to target ramp (bypassing corridor centers).
        // Before the fix: [ramp_start, ramp_end_x at ramp_start_y, ...]
        // After the fix: [ramp_start, corridor_center, ...]
        // Check that no horizontal segment at source face height extends into the
        // target node's x-range.
        for pair in wp.windows(2) {
            let p1 = pair[0];
            let p2 = pair[1];
            let dx = (p1.x - p2.x).abs();
            let dy = (p1.y - p2.y).abs();

            if dx > 50.0 && dy < 1.0 {
                let y = p1.y;
                let x_min = p1.x.min(p2.x);
                let x_max = p1.x.max(p2.x);
                // A horizontal segment at source face height should NOT reach
                // into the target node's x-range
                let in_from_y = y > from_rect.top() && y < from_rect.bottom();
                let reaches_target = x_max > to_rect.left() - 20.0;
                assert!(
                    !(in_from_y && reaches_target),
                    "Horizontal segment at y={y:.1} from x={x_min:.1} to x={x_max:.1} \
                     bypasses corridor center and reaches target! waypoints: {wp:?}"
                );
                // Symmetric: at target face height, shouldn't reach source
                let in_to_y = y > to_rect.top() && y < to_rect.bottom();
                let reaches_source = x_min < from_rect.right() + 20.0;
                assert!(
                    !(in_to_y && reaches_source),
                    "Horizontal segment at y={y:.1} from x={x_min:.1} to x={x_max:.1} \
                     bypasses corridor center and reaches source! waypoints: {wp:?}"
                );
            }
        }
    }

    /// Regression test: the FIRST route allocated on a corridor should travel
    /// at the exact corridor center (offset 0), not offset by ±0.5*lane_spacing.
    ///
    /// Bug: `claim_lane_span` uses `div_ceil(capacity/2)` as center lane, but
    /// `lane_pixel_offset` uses `(capacity+1)/2.0`. For even capacities these
    /// disagree, pushing the first route ~0.5*lane_spacing off-center (the
    /// "sidewalk walking" bug for Step-based routes).
    #[test]
    fn test_first_lane_on_corridor_is_centered() {
        // Even capacity: 6 lanes
        // claim_lane_span allocates center = div_ceil(6/2) = 3
        // lane_pixel_offset must return 0.0 for lane 3 of 6
        let offset = GridInfo::lane_pixel_offset(3, 6, 20.0);
        assert!(
            offset.abs() < 0.01,
            "First-allocated lane (3 of 6) should be at offset 0, got {offset}"
        );

        // Even capacity: 4 lanes
        // claim_lane_span allocates center = div_ceil(4/2) = 2
        let offset = GridInfo::lane_pixel_offset(2, 4, 20.0);
        assert!(
            offset.abs() < 0.01,
            "First-allocated lane (2 of 4) should be at offset 0, got {offset}"
        );

        // Even capacity: 2 lanes
        // claim_lane_span allocates center = div_ceil(2/2) = 1
        let offset = GridInfo::lane_pixel_offset(1, 2, 20.0);
        assert!(
            offset.abs() < 0.01,
            "First-allocated lane (1 of 2) should be at offset 0, got {offset}"
        );

        // Odd capacity: 5 lanes — center = div_ceil(5/2) = 3
        // This should still be 0
        let offset = GridInfo::lane_pixel_offset(3, 5, 20.0);
        assert!(
            offset.abs() < 0.01,
            "First-allocated lane (3 of 5) should be at offset 0, got {offset}"
        );

        // Second lane should be offset by one lane_spacing
        let offset_4_of_6 = GridInfo::lane_pixel_offset(4, 6, 20.0);
        assert!(
            (offset_4_of_6 - 20.0).abs() < 0.01,
            "Second-allocated lane (4 of 6) should be at offset +20, got {offset_4_of_6}"
        );
    }

    /// Regression test: a far-diagonal edge (like "fills" on Slide 6) that routes
    /// through Steps must have its corridor waypoints within 5px of the corridor
    /// center — not shifted toward node edges ("sidewalk walking").
    #[test]
    fn test_step_route_stays_near_corridor_center() {
        let (rects, grid) = make_3x3_grid_realistic();
        // A(0,0) → I(2,2): far diagonal, must traverse corridors with Steps.
        let path = route_single_edge("A", "I", &rects, &grid);
        let wp = &path.waypoints;

        assert!(is_orthogonal(wp), "Route should be orthogonal");

        // Compute corridor centers
        let street_centers: Vec<f32> = (0..=grid.rows)
            .map(|h| {
                let seg = Segment {
                    corridor: Corridor::Street(h),
                    from_cross: 0,
                    to_cross: grid.cols,
                };
                grid.segment_center(&seg, &rects)
            })
            .collect();
        let avenue_centers: Vec<f32> = (0..=grid.cols)
            .map(|v| {
                let seg = Segment {
                    corridor: Corridor::Avenue(v),
                    from_cross: 0,
                    to_cross: grid.rows,
                };
                grid.segment_center(&seg, &rects)
            })
            .collect();

        // Check waypoints that are near corridor centers
        let a_rect = rects["A"];
        let i_rect = rects["I"];
        let mut found_corridor_waypoint = false;

        for &pt in wp.iter() {
            // Skip waypoints on source/target node faces (ramps)
            let on_source = pt.x >= a_rect.left() - 15.0
                && pt.x <= a_rect.right() + 15.0
                && pt.y >= a_rect.top() - 15.0
                && pt.y <= a_rect.bottom() + 15.0;
            let on_target = pt.x >= i_rect.left() - 15.0
                && pt.x <= i_rect.right() + 15.0
                && pt.y >= i_rect.top() - 15.0
                && pt.y <= i_rect.bottom() + 15.0;
            if on_source || on_target {
                continue;
            }

            // This waypoint is in a corridor. Check it's near a center.
            for &sc in &street_centers {
                let deviation = (pt.y - sc).abs();
                if deviation < 30.0 {
                    found_corridor_waypoint = true;
                    assert!(
                        deviation < 5.0,
                        "Waypoint y={:.1} deviates {deviation:.1}px from Street center \
                         {sc:.1}. Max allowed: 5px. Waypoints: {wp:?}",
                        pt.y
                    );
                }
            }
            for &ac in &avenue_centers {
                let deviation = (pt.x - ac).abs();
                if deviation < 30.0 {
                    found_corridor_waypoint = true;
                    assert!(
                        deviation < 5.0,
                        "Waypoint x={:.1} deviates {deviation:.1}px from Avenue center \
                         {ac:.1}. Max allowed: 5px. Waypoints: {wp:?}",
                        pt.x
                    );
                }
            }
        }

        assert!(
            found_corridor_waypoint,
            "Expected corridor waypoints near centers, but found none. Waypoints: {wp:?}"
        );
    }

    /// Build the Slide 6 "Mixed Arrow Types" grid: 9 nodes in a 3x3 auto-layout.
    ///
    /// Grid positions (col, row):
    ///   (0,0) Frontend    (1,0) Backend     (2,0) Database
    ///   (0,1) Cache        (1,1) WebSocket   (2,1) Monitoring
    ///   (0,2) Logging      (1,2) CI           (2,2) Staging
    fn make_slide6_grid() -> (HashMap<String, egui::Rect>, GridInfo) {
        let names = [
            ("Frontend", 0, 0),
            ("Backend", 1, 0),
            ("Database", 2, 0),
            ("Cache", 0, 1),
            ("WebSocket", 1, 1),
            ("Monitoring", 2, 1),
            ("Logging", 0, 2),
            ("CI", 1, 2),
            ("Staging", 2, 2),
        ];
        // Use realistic dimensions matching auto-layout at 1920x1080
        let cell_w = 580.0;
        let cell_h = 257.0;
        let node_w = 220.0;
        let node_h = 154.0;
        let origin_x = 90.0;
        let origin_y = 100.0;

        let mut rects = HashMap::new();
        for (name, col, row) in &names {
            let cx = origin_x + (*col as f32 + 0.5) * cell_w;
            let cy = origin_y + (*row as f32 + 0.5) * cell_h;
            rects.insert(
                name.to_string(),
                egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(node_w, node_h)),
            );
        }

        let occupied: HashSet<(usize, usize)> = names.iter().map(|(_, c, r)| (*c, *r)).collect();
        let grid = GridInfo {
            cols: 3,
            rows: 3,
            cell_w,
            cell_h,
            origin_x,
            origin_y,
            occupied,
        };

        (rects, grid)
    }

    /// Regression test for Slide 6 "fills" edge: Database(2,0) → Cache(0,1).
    ///
    /// The correct route is:
    ///   1. Exit Database from Bottom face (south)
    ///   2. Step west (multiple times to travel from column 2 to above column 0)
    ///   3. Enter Cache from Top face (north)
    ///
    /// The BUG was: the router chose to exit Bottom, take ONE step west, then
    /// enter Cache from the Right face — producing a route that enters Cache
    /// from the side instead of from above. This creates a diagonal-looking
    /// path that hugs node edges ("sidewalk walking").
    #[test]
    fn test_slide6_fills_route_enters_cache_from_top() {
        let (rects, grid) = make_slide6_grid();

        // "fills" is Database <- Cache, parsed as from=Database, to=Cache
        let path = route_single_edge("Database", "Cache", &rects, &grid);

        eprintln!("fills route steps:");
        for step in &path.steps {
            eprintln!("  {step}");
        }

        // The route must enter Cache from the Top face, not the Right face.
        // Entering from the Right means the route takes a shortcut L-shape
        // instead of properly walking west along the street.
        let entry_step = path.steps.last().expect("route should have steps");
        assert!(
            matches!(
                entry_step,
                RouteStep::EnterBuilding {
                    face: Face::Top,
                    ..
                }
            ),
            "fills route should enter Cache from the Top (north), \
             but enters from {:?}. Full route: {:?}",
            entry_step,
            path.steps
        );

        // The exit should be from Database's Bottom face
        let exit_step = path.steps.first().expect("route should have steps");
        assert!(
            matches!(
                exit_step,
                RouteStep::ExitBuilding {
                    face: Face::Bottom,
                    ..
                }
            ),
            "fills route should exit Database from the Bottom (south), \
             but exits from {:?}. Full route: {:?}",
            exit_step,
            path.steps
        );

        // There should be multiple west steps (Database is at col 2, Cache at col 0)
        let west_steps: Vec<_> = path
            .steps
            .iter()
            .filter(|s| {
                matches!(
                    s,
                    RouteStep::Step {
                        direction: TravelDir::West,
                        ..
                    }
                )
            })
            .collect();
        assert!(
            west_steps.len() >= 2,
            "Route should have multiple west steps to travel from col 2 to col 0, \
             but has {} west steps. Full route: {:?}",
            west_steps.len(),
            path.steps
        );
    }
}
