use std::collections::{HashMap, HashSet};

use super::types::{Lane, Route, SegmentId, Waypoint};

/// Tracks which lanes are claimed on each segment.
#[derive(Debug, Clone, Default)]
pub struct LaneOccupancy {
    claimed: HashMap<SegmentId, HashSet<Lane>>,
}

impl LaneOccupancy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a specific lane on a segment is available.
    pub fn is_available(&self, seg: &SegmentId, lane: Lane) -> bool {
        self.claimed
            .get(seg)
            .is_none_or(|lanes| !lanes.contains(&lane))
    }

    /// Claim a specific lane on a segment.
    pub fn claim(&mut self, seg: SegmentId, lane: Lane) {
        self.claimed.entry(seg).or_default().insert(lane);
    }

    /// Claim all lanes used by a route.
    ///
    /// For each consecutive pair of waypoints, claims the lane of the first waypoint
    /// on the segment between them.
    pub fn claim_route(&mut self, route: &Route) {
        for pair in route.waypoints.windows(2) {
            let seg = SegmentId::new(pair[0].coord, pair[1].coord);
            self.claim(seg, pair[0].lane);
        }
    }

    /// Find the first available lane on a segment, spiraling from center.
    /// Returns lane numbers in order: 0, 1, -1, 2, -2, 3, -3, ...
    /// `capacity` limits how many lanes are available total.
    pub fn first_available(&self, seg: &SegmentId, capacity: i32) -> Option<Lane> {
        spiral_lanes(capacity)
            .into_iter()
            .find(|lane| self.is_available(seg, *lane))
    }

    /// Get all available lanes on a segment, ordered by preference (center first, spiral out).
    pub fn available_lanes(&self, seg: &SegmentId, capacity: i32) -> Vec<Lane> {
        spiral_lanes(capacity)
            .into_iter()
            .filter(|lane| self.is_available(seg, *lane))
            .collect()
    }

    /// Get the set of claimed lanes on a segment.
    pub fn claimed_lanes(&self, seg: &SegmentId) -> Option<&HashSet<Lane>> {
        self.claimed.get(seg)
    }

    /// Count how many lanes are claimed on a segment.
    pub fn claimed_count(&self, seg: &SegmentId) -> usize {
        self.claimed.get(seg).map_or(0, |s| s.len())
    }

    /// Build a route from waypoint coordinates and this occupancy's state.
    /// Used internally — callers typically use `claim_route` after finding a route.
    pub fn build_route_from_waypoints(waypoints: Vec<Waypoint>) -> Route {
        let complexity = compute_complexity(&waypoints);
        Route {
            waypoints,
            complexity,
        }
    }
}

/// Generate lane numbers in spiral order: 0, 1, -1, 2, -2, ...
/// Returns exactly `capacity` lanes (0 if capacity <= 0).
fn spiral_lanes(capacity: i32) -> Vec<Lane> {
    if capacity <= 0 {
        return Vec::new();
    }
    let mut lanes = Vec::with_capacity(capacity as usize);
    lanes.push(0);
    let mut offset = 1;
    while lanes.len() < capacity as usize {
        lanes.push(offset);
        if lanes.len() < capacity as usize {
            lanes.push(-offset);
        }
        offset += 1;
    }
    lanes
}

/// Compute the complexity of a route from its waypoints.
pub fn compute_complexity(waypoints: &[Waypoint]) -> super::types::RouteComplexity {
    let mut length = 0.0_f64;
    let mut turns = 0_u32;
    let mut lane_changes = 0_u32;

    for i in 1..waypoints.len() {
        let prev = &waypoints[i - 1];
        let curr = &waypoints[i];

        // Length: sum of absolute coordinate differences (in actual grid units).
        let dcol = (curr.coord.col2 - prev.coord.col2).abs() as f64 / 2.0;
        let drow = (curr.coord.row2 - prev.coord.row2).abs() as f64 / 2.0;
        length += dcol + drow;

        // Determine direction of this segment.
        if i >= 2 {
            let prev_prev = &waypoints[i - 2];
            let prev_dir = segment_direction(prev_prev.coord, prev.coord);
            let curr_dir = segment_direction(prev.coord, curr.coord);
            if let (Some(pd), Some(cd)) = (prev_dir, curr_dir) {
                let is_turn = pd.is_turn(cd);
                if is_turn {
                    turns += 1;
                }
                // Lane change at waypoint[i-1]: compare lane of segment before and after.
                if prev_prev.lane != prev.lane && !is_turn {
                    lane_changes += 1;
                }
            }
        }
    }

    super::types::RouteComplexity {
        length,
        turns,
        lane_changes,
    }
}

/// Determine the direction of travel from `a` to `b`.
fn segment_direction(
    a: super::types::GridCoord,
    b: super::types::GridCoord,
) -> Option<super::types::Direction> {
    use super::types::Direction;
    let dc = b.col2 - a.col2;
    let dr = b.row2 - a.row2;
    if dc > 0 && dr == 0 {
        Some(Direction::East)
    } else if dc < 0 && dr == 0 {
        Some(Direction::West)
    } else if dr > 0 && dc == 0 {
        Some(Direction::South)
    } else if dr < 0 && dc == 0 {
        Some(Direction::North)
    } else {
        None
    }
}
