use super::*;

#[test]
fn lane_capacity_1_two_edges() {
    // With capacity 1, second edge on same segment must detour.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(1, 1));
    // First edge should succeed.
    match &output.results[0].1 {
        super::super::types::RouteResult::Success(_) => {}
        super::super::types::RouteResult::Failure { warning } => {
            panic!("First edge failed: {}", warning);
        }
    }
    // Second edge may succeed (via detour) or fail.
}

#[test]
fn progressive_lane_filling() {
    // With capacity 3, route 3 edges on same segment — each uses a different lane.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn lane_exhaustion_forces_detour() {
    // Fill all lanes, 4th edge must detour.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![
        edge("A", "B"),
        edge("A", "B"),
        edge("A", "B"),
        edge("A", "B"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    // First 3 should be direct routes.
    for i in 0..3 {
        match &output.results[i].1 {
            super::super::types::RouteResult::Success(route) => {
                // Should be a short direct route.
                assert!(
                    route.complexity.length <= 1.5,
                    "Edge {} should be direct",
                    i
                );
            }
            super::super::types::RouteResult::Failure { warning } => {
                panic!("Edge {} failed: {}", i, warning);
            }
        }
    }
    // 4th may detour or fail (lanes are exhausted on direct path).
}

#[test]
fn parallel_corridors_fill_independently() {
    // Two separate corridors, each with their own lanes.
    let nodes = vec![
        node("A1", 1, 1),
        node("B1", 3, 1),
        node("A2", 1, 3),
        node("B2", 3, 3),
    ];
    let edges = vec![
        edge("A1", "B1"),
        edge("A2", "B2"),
        edge("A1", "B1"),
        edge("A2", "B2"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn capacity_5_handles_many_edges() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![
        edge("A", "B"),
        edge("A", "B"),
        edge("A", "B"),
        edge("A", "B"),
        edge("A", "B"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn congested_intersection() {
    // Many edges converging at a single node.
    let nodes = vec![
        node("Center", 2, 2),
        node("N", 2, 1),
        node("S", 2, 3),
        node("E", 3, 2),
        node("W", 1, 2),
    ];
    let edges = vec![
        edge("N", "Center"),
        edge("S", "Center"),
        edge("E", "Center"),
        edge("W", "Center"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn first_edge_gets_optimal_route() {
    // First edge should always get the shortest possible route.
    let nodes = vec![node("A", 1, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let first_route = get_route(&output, 0);
    // First edge gets the optimal direct route.
    assert_eq!(first_route.complexity.length, 2.0);
    assert_eq!(first_route.complexity.turns, 0);
}

#[test]
fn heavily_congested_grid() {
    // 2x2 grid, all edges, limited capacity.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 2, 1),
        node("C", 1, 2),
        node("D", 2, 2),
    ];
    let edges = vec![
        edge("A", "B"),
        edge("A", "C"),
        edge("A", "D"),
        edge("B", "C"),
        edge("B", "D"),
        edge("C", "D"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    // With capacity 5, all should succeed.
    assert_all_success(&output);
}

#[test]
fn single_lane_forces_all_different_paths() {
    // Capacity 1: each edge must find a unique path.
    let nodes = vec![node("A", 1, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(1, 1));
    // First succeeds, second may need a long detour.
    match &output.results[0].1 {
        super::super::types::RouteResult::Success(_) => {}
        super::super::types::RouteResult::Failure { warning } => {
            panic!("First edge should succeed: {}", warning);
        }
    }
}

#[test]
fn many_parallel_edges_vertical() {
    // Multiple edges between vertically separated nodes.
    let nodes = vec![node("A", 1, 1), node("B", 1, 2)];
    let edges = vec![edge("A", "B"), edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn priority_ordering_matters() {
    // First-defined edge gets priority for the optimal route.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1), node("C", 1, 2)];
    // Edge A->B goes first, claims the horizontal corridor.
    let edges = vec![edge("A", "B"), edge("A", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn capacity_1_three_edges_partial_failure() {
    // With capacity 1 on a small grid, third edge is likely to fail.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(1, 1));
    // First should succeed.
    match &output.results[0].1 {
        super::super::types::RouteResult::Success(_) => {}
        super::super::types::RouteResult::Failure { warning } => {
            panic!("First edge should succeed: {}", warning);
        }
    }
}

#[test]
fn high_capacity_many_edges() {
    // Very high capacity allows many parallel edges.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let mut edges = Vec::new();
    for _ in 0..7 {
        edges.push(edge("A", "B"));
    }
    let output = route_all_edges(&nodes, &edges, &config(7, 7));
    assert_all_success(&output);
}

#[test]
fn forced_boundary_detour() {
    // Interior fully occupied, must use boundary streets.
    let nodes = vec![node("A", 1, 1), node("O", 2, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(1, 1));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert!(route.complexity.length > 2.0, "Must detour around obstacle");
}

#[test]
fn lane_filling_order_center_first() {
    // With multiple edges, center lane should be claimed first.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // First edge should use center lane (0) on all segments.
    for wp in &route.waypoints[..route.waypoints.len() - 1] {
        assert_eq!(wp.lane, 0, "First edge should use center lane");
    }
}

#[test]
fn two_edges_crossing_corridor() {
    // Two edges that must share a common corridor.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 3, 3),
        node("C", 3, 1),
        node("D", 1, 3),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn asymmetric_capacity() {
    // Different horizontal and vertical capacities.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(5, 1));
    assert_all_success(&output);
}
