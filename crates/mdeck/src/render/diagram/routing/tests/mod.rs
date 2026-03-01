mod complex;
mod crowded;
mod determinism;
mod edge_cases;
mod invalid;
mod serialization;
mod simple;

use super::types::{DiagramEdge, DiagramNode, GridCoord, RoutingConfig};
use super::{RoutingOutput, route_all_edges};

/// Helper to create a DiagramNode.
fn node(name: &str, col: i32, row: i32) -> DiagramNode {
    DiagramNode {
        name: name.to_string(),
        col,
        row,
    }
}

/// Helper to create a DiagramEdge.
fn edge(source: &str, target: &str) -> DiagramEdge {
    DiagramEdge {
        source: source.to_string(),
        target: target.to_string(),
        label: None,
    }
}

/// Helper to create a DiagramEdge with a label.
fn edge_labeled(source: &str, target: &str, label: &str) -> DiagramEdge {
    DiagramEdge {
        source: source.to_string(),
        target: target.to_string(),
        label: Some(label.to_string()),
    }
}

/// Helper to create a RoutingConfig.
fn config(h: i32, v: i32) -> RoutingConfig {
    RoutingConfig {
        h_lane_capacity: h,
        v_lane_capacity: v,
    }
}

/// Assert that a routing output has all successes.
fn assert_all_success(output: &RoutingOutput) {
    for (edge, result) in &output.results {
        match result {
            super::types::RouteResult::Success(_) => {}
            super::types::RouteResult::Failure { warning } => {
                panic!(
                    "Edge {} -> {} failed: {}",
                    edge.source, edge.target, warning
                );
            }
        }
    }
}

/// Get the route for a specific edge index from routing output.
fn get_route(output: &RoutingOutput, idx: usize) -> &super::types::Route {
    match &output.results[idx].1 {
        super::types::RouteResult::Success(route) => route,
        super::types::RouteResult::Failure { warning } => {
            panic!("Edge {} failed: {}", idx, warning);
        }
    }
}

/// Assert that a specific edge in the output failed.
fn assert_edge_failed(output: &RoutingOutput, idx: usize) {
    match &output.results[idx].1 {
        super::types::RouteResult::Failure { .. } => {}
        super::types::RouteResult::Success(route) => {
            panic!(
                "Edge {} should have failed but succeeded with complexity {:?}",
                idx, route.complexity
            );
        }
    }
}
