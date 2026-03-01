pub mod graph;
pub mod lanes;
pub mod search;
pub mod serialize;
pub mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use graph::RoutingGraph;
use lanes::LaneOccupancy;
use search::find_best_route;
use types::{DiagramEdge, DiagramNode, GridCoord, RouteResult, RoutingConfig, RoutingOutput};

/// Route all edges in a diagram.
///
/// Edges are processed sequentially in definition order. Earlier edges claim lanes
/// first, so later edges route around them. Each individual edge's route search
/// is parallelized across 4 initial directions via rayon.
///
/// # Arguments
/// * `nodes` — Diagram nodes with names and grid positions.
/// * `edges` — Diagram edges connecting nodes by name.
/// * `config` — Routing configuration (lane capacities).
///
/// # Returns
/// A `RoutingOutput` with a result for each edge.
pub fn route_all_edges(
    nodes: &[DiagramNode],
    edges: &[DiagramEdge],
    config: &RoutingConfig,
) -> RoutingOutput {
    // Build lookup from node name to grid position.
    let name_to_pos: HashMap<&str, (i32, i32)> = nodes
        .iter()
        .map(|n| (n.name.as_str(), (n.col, n.row)))
        .collect();

    // Build the routing graph.
    let positions: Vec<(i32, i32)> = nodes.iter().map(|n| (n.col, n.row)).collect();
    let graph = RoutingGraph::build(&positions, config.h_lane_capacity, config.v_lane_capacity);

    // Lane occupancy tracker.
    let mut occupancy = LaneOccupancy::new();

    // Route each edge sequentially.
    let mut results = Vec::with_capacity(edges.len());

    for edge in edges {
        let source_pos = name_to_pos.get(edge.source.as_str());
        let target_pos = name_to_pos.get(edge.target.as_str());

        let result = match (source_pos, target_pos) {
            (Some(&(sc, sr)), Some(&(tc, tr))) => {
                let source = GridCoord::from_int(sc, sr);
                let target = GridCoord::from_int(tc, tr);

                match find_best_route(&graph, &occupancy, source, target) {
                    Some(route) => {
                        occupancy.claim_route(&route);
                        RouteResult::Success(route)
                    }
                    None => RouteResult::Failure {
                        warning: format!(
                            "Could not find route from '{}' to '{}'",
                            edge.source, edge.target
                        ),
                    },
                }
            }
            (None, _) => RouteResult::Failure {
                warning: format!("Unknown source node '{}'", edge.source),
            },
            (_, None) => RouteResult::Failure {
                warning: format!("Unknown target node '{}'", edge.target),
            },
        };

        results.push((edge.clone(), result));
    }

    RoutingOutput { results }
}
