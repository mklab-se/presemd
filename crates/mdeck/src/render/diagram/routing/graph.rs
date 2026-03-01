use std::collections::{HashMap, HashSet};

use super::types::{Direction, GridCoord, SegmentId};

/// The routing graph built from grid bounds and occupied cells.
///
/// Nodes are cell centers, junctions, and street intersections.
/// Edges are road segments between adjacent nodes.
pub struct RoutingGraph {
    /// Adjacency list: for each node, the list of (neighbor, segment_id, travel_direction).
    pub adjacency: HashMap<GridCoord, Vec<(GridCoord, SegmentId, Direction)>>,
    /// Set of cell centers that are occupied by diagram nodes.
    pub occupied_cells: HashSet<GridCoord>,
    /// Lane capacity for each segment (h_lane_capacity for horizontal, v_lane_capacity for vertical).
    pub segment_capacities: HashMap<SegmentId, i32>,
}

impl RoutingGraph {
    /// Build a routing graph from node positions and lane capacities.
    ///
    /// Grid bounds are derived from the node positions (min/max col/row).
    /// Boundary streets are included (half-unit outside the grid bounds).
    pub fn build(nodes: &[(i32, i32)], h_lane_capacity: i32, v_lane_capacity: i32) -> Self {
        if nodes.is_empty() {
            return Self {
                adjacency: HashMap::new(),
                occupied_cells: HashSet::new(),
                segment_capacities: HashMap::new(),
            };
        }

        let min_col = nodes.iter().map(|n| n.0).min().unwrap();
        let max_col = nodes.iter().map(|n| n.0).max().unwrap();
        let min_row = nodes.iter().map(|n| n.1).min().unwrap();
        let max_row = nodes.iter().map(|n| n.1).max().unwrap();

        let occupied_cells: HashSet<GridCoord> = nodes
            .iter()
            .map(|&(c, r)| GridCoord::from_int(c, r))
            .collect();

        // In doubled coordinates:
        // Cell centers: (col*2, row*2) — both even
        // Junctions: one odd, one even — e.g., (col*2+1, row*2) or (col*2, row*2+1)
        // Street intersections: both odd — e.g., (col*2+1, row*2+1)
        //
        // The grid spans from (min_col, min_row) to (max_col, max_row) in cell centers.
        // Boundary streets are at col = min_col - 0.5 and max_col + 0.5,
        //                        row = min_row - 0.5 and max_row + 0.5.
        // In doubled coords: col2 = 2*min_col - 1 to 2*max_col + 1 (odd values for streets)
        //                    row2 = 2*min_row - 1 to 2*max_row + 1

        // Range of doubled coordinates to consider (inclusive).
        let c2_min = 2 * min_col - 1;
        let c2_max = 2 * max_col + 1;
        let r2_min = 2 * min_row - 1;
        let r2_max = 2 * max_row + 1;

        // Generate all valid nodes in the routing graph.
        let mut all_nodes = Vec::new();
        for c2 in c2_min..=c2_max {
            for r2 in r2_min..=r2_max {
                let coord = GridCoord { col2: c2, row2: r2 };
                // All nodes in the grid are valid routing nodes:
                // - Cell centers (both even)
                // - Junctions (one odd, one even)
                // - Street intersections (both odd)
                // We include everything in the doubled-coord range.
                // But we only include nodes that can actually exist:
                // A node must be at a point where roads cross.
                //
                // Cell centers: every (even, even) in range
                // Junctions: (odd, even) or (even, odd) in range
                // Street intersections: (odd, odd) in range
                all_nodes.push(coord);
            }
        }

        let node_set: HashSet<GridCoord> = all_nodes.iter().copied().collect();

        // Build adjacency: each node connects to its 4 neighbors (if they exist in the graph).
        let mut adjacency: HashMap<GridCoord, Vec<(GridCoord, SegmentId, Direction)>> =
            HashMap::new();
        let mut segment_capacities: HashMap<SegmentId, i32> = HashMap::new();

        for &coord in &all_nodes {
            let mut neighbors = Vec::new();
            for dir in Direction::ALL {
                let neighbor = coord.step(dir);
                if node_set.contains(&neighbor) {
                    let seg = SegmentId::new(coord, neighbor);
                    neighbors.push((neighbor, seg, dir));

                    // Set capacity based on segment orientation.
                    // Horizontal segments use h_lane_capacity,
                    // vertical segments use v_lane_capacity.
                    segment_capacities.entry(seg).or_insert_with(|| {
                        if seg.is_horizontal() {
                            h_lane_capacity
                        } else {
                            v_lane_capacity
                        }
                    });
                }
            }
            adjacency.insert(coord, neighbors);
        }

        Self {
            adjacency,
            occupied_cells,
            segment_capacities,
        }
    }

    /// Check if a coordinate is within the graph.
    pub fn contains(&self, coord: &GridCoord) -> bool {
        self.adjacency.contains_key(coord)
    }

    /// Get neighbors of a coordinate.
    pub fn neighbors(&self, coord: &GridCoord) -> &[(GridCoord, SegmentId, Direction)] {
        static EMPTY: &[(GridCoord, SegmentId, Direction)] = &[];
        self.adjacency.get(coord).map_or(EMPTY, |v| v.as_slice())
    }

    /// Get the lane capacity for a segment.
    pub fn capacity(&self, seg: &SegmentId) -> i32 {
        self.segment_capacities.get(seg).copied().unwrap_or(0)
    }

    /// Check if a cell center is occupied.
    pub fn is_occupied(&self, coord: &GridCoord) -> bool {
        self.occupied_cells.contains(coord)
    }
}
