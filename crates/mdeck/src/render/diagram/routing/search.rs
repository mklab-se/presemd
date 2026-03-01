use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use rayon::prelude::*;

use super::graph::RoutingGraph;
use super::lanes::LaneOccupancy;
use super::types::{Direction, GridCoord, Lane, Route, RouteComplexity, SegmentId, Waypoint};

/// State key for the visited set — identifies a unique search state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StateKey {
    coord: GridCoord,
    lane: Lane,
    last_direction: Direction,
}

/// A* search state.
#[derive(Debug, Clone)]
struct SearchState {
    coord: GridCoord,
    lane: Lane,
    last_direction: Direction,
    /// Cost so far: length + turns + lane_changes.
    g_cost: f64,
    /// Heuristic estimate to target.
    h_cost: f64,
    /// Parent state key for path reconstruction.
    parent: Option<StateKey>,
    /// How we got here: the lane of the parent's segment.
    parent_lane: Lane,
    /// Breakdown of cost components for final route complexity.
    length_so_far: f64,
    turns_so_far: u32,
    lane_changes_so_far: u32,
}

impl SearchState {
    fn f_cost(&self) -> f64 {
        self.g_cost + self.h_cost
    }

    fn key(&self) -> StateKey {
        StateKey {
            coord: self.coord,
            lane: self.lane,
            last_direction: self.last_direction,
        }
    }
}

/// Wrapper for the priority queue with deterministic ordering.
/// BinaryHeap is a max-heap, so we reverse the ordering (lowest cost = highest priority).
#[derive(Debug)]
struct PqEntry {
    f_cost: f64,
    g_cost: f64,
    coord: GridCoord,
    lane: Lane,
    direction: Direction,
    state: SearchState,
}

impl PartialEq for PqEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for PqEntry {}

impl Ord for PqEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior in BinaryHeap.
        // Lower f_cost is better → compare other.f_cost to self.f_cost.
        other
            .f_cost
            .partial_cmp(&self.f_cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                // For same f_cost, prefer lower g_cost (explored less).
                other
                    .g_cost
                    .partial_cmp(&self.g_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .then(other.coord.cmp(&self.coord))
            // Prefer lanes closer to center (smaller |lane|).
            .then_with(|| self.lane.abs().cmp(&other.lane.abs()).reverse())
            // Deterministic tie-break for same |lane|: prefer positive lane.
            .then(other.lane.cmp(&self.lane))
            .then(other.direction.cmp(&self.direction))
    }
}

impl PartialOrd for PqEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Heuristic: Manhattan distance in actual grid units (halved doubled-coords).
fn heuristic(from: GridCoord, to: GridCoord) -> f64 {
    from.manhattan_to(to) as f64 / 2.0
}

/// Run A* search from `source` to `target` starting in direction `initial_dir`.
///
/// The search starts at the source cell center, steps one unit in `initial_dir` to the
/// adjacent junction, then explores the routing graph. The search terminates when
/// the target cell center is reached.
///
/// Returns `None` if no route is found.
fn astar_single_direction(
    graph: &RoutingGraph,
    occupancy: &LaneOccupancy,
    source: GridCoord,
    target: GridCoord,
    initial_dir: Direction,
) -> Option<Route> {
    // The first step: source center → adjacent junction in initial_dir.
    let first_junction = source.step(initial_dir);
    if !graph.contains(&first_junction) {
        return None;
    }

    let first_seg = SegmentId::new(source, first_junction);
    let first_capacity = graph.capacity(&first_seg);

    // Get available lanes for the first segment.
    let first_lanes = occupancy.available_lanes(&first_seg, first_capacity);
    if first_lanes.is_empty() {
        return None;
    }

    let mut open = BinaryHeap::new();
    let mut best_g: HashMap<StateKey, f64> = HashMap::new();
    let mut came_from: HashMap<StateKey, (StateKey, Lane)> = HashMap::new();

    // Seed the open set with states at the first junction.
    for &lane in &first_lanes {
        let h = heuristic(first_junction, target);
        let g = 0.5; // Length of one step (half unit in grid coords).
        let state = SearchState {
            coord: first_junction,
            lane,
            last_direction: initial_dir,
            g_cost: g,
            h_cost: h,
            parent: None,
            parent_lane: lane,
            length_so_far: 0.5,
            turns_so_far: 0,
            lane_changes_so_far: 0,
        };
        let key = state.key();
        best_g.insert(key, g);
        open.push(PqEntry {
            f_cost: state.f_cost(),
            g_cost: g,
            coord: first_junction,
            lane,
            direction: initial_dir,
            state,
        });
    }

    while let Some(entry) = open.pop() {
        let current = entry.state;
        let current_key = current.key();

        // Skip if we've found a better path to this state.
        if let Some(&best) = best_g.get(&current_key) {
            if current.g_cost > best {
                continue;
            }
        }

        // Check if we reached the target cell center.
        if current.coord == target {
            // Reconstruct the path.
            return Some(reconstruct_route(
                &came_from,
                current_key,
                source,
                initial_dir,
                &first_lanes,
                &current,
            ));
        }

        // Expand neighbors.
        for &(neighbor, seg, dir) in graph.neighbors(&current.coord) {
            // Don't go backwards.
            if dir == current.last_direction.opposite() {
                continue;
            }

            // Check if the neighbor goes through an occupied cell.
            // A cell center is occupied if it's not source or target.
            if neighbor.is_cell_center()
                && graph.is_occupied(&neighbor)
                && neighbor != source
                && neighbor != target
            {
                continue;
            }

            // Also check if we'd be routing through an occupied cell's internal road.
            // If current is a junction adjacent to an occupied cell, and neighbor is
            // the cell center of that occupied cell, that's only OK if it's source/target.
            // If current is a cell center of an occupied cell, and the neighbor is a junction,
            // that's only OK if current is source or target.
            if current.coord.is_cell_center()
                && graph.is_occupied(&current.coord)
                && current.coord != source
                && current.coord != target
            {
                continue;
            }

            let is_turn = current.last_direction.is_turn(dir);
            let seg_capacity = graph.capacity(&seg);

            // Get available lanes on this segment.
            let available = occupancy.available_lanes(&seg, seg_capacity);
            if available.is_empty() {
                continue;
            }

            // If target is the neighbor (arriving at target center), we need any available lane.
            let step_length = 0.5_f64; // Each step in doubled coords is 0.5 grid units.

            for &next_lane in &available {
                let lane_changed = next_lane != current.lane;
                let lane_change_cost = if lane_changed && !is_turn { 1.0 } else { 0.0 };
                let turn_cost = if is_turn { 1.0 } else { 0.0 };

                let new_g = current.g_cost + step_length + turn_cost + lane_change_cost;
                let new_h = heuristic(neighbor, target);

                let new_key = StateKey {
                    coord: neighbor,
                    lane: next_lane,
                    last_direction: dir,
                };

                // Only expand if this is a better path.
                if let Some(&best) = best_g.get(&new_key) {
                    if new_g >= best {
                        continue;
                    }
                }

                best_g.insert(new_key, new_g);
                came_from.insert(new_key, (current_key, current.lane));

                let new_state = SearchState {
                    coord: neighbor,
                    lane: next_lane,
                    last_direction: dir,
                    g_cost: new_g,
                    h_cost: new_h,
                    parent: Some(current_key),
                    parent_lane: current.lane,
                    length_so_far: current.length_so_far + step_length,
                    turns_so_far: current.turns_so_far + if is_turn { 1 } else { 0 },
                    lane_changes_so_far: current.lane_changes_so_far
                        + if lane_changed && !is_turn { 1 } else { 0 },
                };

                open.push(PqEntry {
                    f_cost: new_state.f_cost(),
                    g_cost: new_g,
                    coord: neighbor,
                    lane: next_lane,
                    direction: dir,
                    state: new_state,
                });
            }
        }
    }

    None
}

/// Reconstruct the route from the A* search results.
fn reconstruct_route(
    came_from: &HashMap<StateKey, (StateKey, Lane)>,
    final_key: StateKey,
    source: GridCoord,
    _initial_dir: Direction,
    first_lanes: &[Lane],
    final_state: &SearchState,
) -> Route {
    // Trace back through came_from to build the path.
    let mut path_keys = vec![final_key];
    let mut current_key = final_key;
    while let Some(&(parent_key, _parent_lane)) = came_from.get(&current_key) {
        path_keys.push(parent_key);
        current_key = parent_key;
    }
    path_keys.reverse();

    // Build waypoints: source center, then each state's coord+lane.
    let mut waypoints = Vec::with_capacity(path_keys.len() + 1);

    // Source center waypoint: use the lane of the first segment.
    let first_lane = if let Some(first) = path_keys.first() {
        // The lane used on the first segment is the lane of the first state.
        first.lane
    } else if let Some(&l) = first_lanes.first() {
        l
    } else {
        0
    };

    waypoints.push(Waypoint {
        coord: source,
        lane: first_lane,
    });

    // Add each waypoint from the path.
    for (i, key) in path_keys.iter().enumerate() {
        let lane = if i + 1 < path_keys.len() {
            // Lane for the next segment: it's the lane of the next state.
            path_keys[i + 1].lane
        } else {
            // Last waypoint (target): lane is unused, use 0.
            0
        };
        waypoints.push(Waypoint {
            coord: key.coord,
            lane,
        });
    }

    let complexity = RouteComplexity {
        length: final_state.length_so_far,
        turns: final_state.turns_so_far,
        lane_changes: final_state.lane_changes_so_far,
    };

    Route {
        waypoints,
        complexity,
    }
}

/// Search for the best route from source to target, trying all 4 initial directions in parallel.
///
/// Returns the route with the lowest complexity, with deterministic tie-breaking.
pub fn find_best_route(
    graph: &RoutingGraph,
    occupancy: &LaneOccupancy,
    source: GridCoord,
    target: GridCoord,
) -> Option<Route> {
    // If source == target, return a trivial route.
    if source == target {
        return Some(Route {
            waypoints: vec![Waypoint {
                coord: source,
                lane: 0,
            }],
            complexity: RouteComplexity {
                length: 0.0,
                turns: 0,
                lane_changes: 0,
            },
        });
    }

    // Launch 4 parallel A* searches, one per initial direction.
    let results: Vec<Option<Route>> = Direction::ALL
        .par_iter()
        .map(|&dir| astar_single_direction(graph, occupancy, source, target, dir))
        .collect();

    // Collect successful results and pick the best.
    let mut best: Option<Route> = None;
    for route in results.into_iter().flatten() {
        best = Some(match best {
            None => route,
            Some(current_best) => {
                if route.complexity < current_best.complexity {
                    route
                } else if route.complexity == current_best.complexity {
                    // Deterministic tie-break: compare waypoint sequences.
                    if route_tiebreak(&route) < route_tiebreak(&current_best) {
                        route
                    } else {
                        current_best
                    }
                } else {
                    current_best
                }
            }
        });
    }

    best
}

/// Generate a deterministic tie-breaking key for a route.
/// Returns a vector of (coord, lane) tuples that can be compared lexicographically.
fn route_tiebreak(route: &Route) -> Vec<(i32, i32, Lane)> {
    route
        .waypoints
        .iter()
        .map(|w| (w.coord.col2, w.coord.row2, w.lane))
        .collect()
}
