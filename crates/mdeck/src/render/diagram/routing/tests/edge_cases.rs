use super::*;

#[test]
fn single_node_no_edges() {
    let nodes = vec![node("A", 1, 1)];
    let edges: Vec<DiagramEdge> = vec![];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_eq!(output.results.len(), 0);
}

#[test]
fn two_nodes_no_edges() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges: Vec<DiagramEdge> = vec![];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_eq!(output.results.len(), 0);
}

#[test]
fn self_edge() {
    // Edge from a node to itself.
    let nodes = vec![node("A", 1, 1)];
    let edges = vec![edge("A", "A")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 0.0);
}

#[test]
fn single_row_grid() {
    // All nodes in a single row.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 2, 1),
        node("C", 3, 1),
        node("D", 4, 1),
    ];
    let edges = vec![edge("A", "D")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn single_column_grid() {
    // All nodes in a single column.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 1, 2),
        node("C", 1, 3),
        node("D", 1, 4),
    ];
    let edges = vec![edge("A", "D")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn large_coordinates() {
    // Nodes at large coordinates.
    let nodes = vec![node("A", 100, 100), node("B", 101, 100)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 1.0);
}

#[test]
fn negative_coordinates() {
    // Nodes at negative coordinates (if system supports them).
    let nodes = vec![node("A", -1, -1), node("B", 0, -1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 1.0);
}

#[test]
fn sparse_grid() {
    // Nodes far apart with lots of empty space.
    let nodes = vec![node("A", 1, 1), node("B", 10, 10)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 18.0); // Manhattan distance.
}

#[test]
fn two_adjacent_nodes_only() {
    let nodes = vec![node("X", 5, 5), node("Y", 6, 5)];
    let edges = vec![edge("X", "Y")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn nodes_at_same_position() {
    // Two different nodes at the same grid position.
    let nodes = vec![node("A", 1, 1), node("B", 1, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    // Source == target, should return trivial route.
    assert_all_success(&output);
}

#[test]
fn result_count_matches_edge_count() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 1), node("C", 3, 1)];
    let edges = vec![edge("A", "B"), edge("B", "C"), edge("A", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_eq!(output.results.len(), 3);
}

#[test]
fn wide_horizontal_gap() {
    // Nodes 5 apart horizontally, no obstacles.
    let nodes = vec![node("A", 1, 1), node("B", 6, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 5.0);
    assert_eq!(route.complexity.turns, 0);
}

#[test]
fn all_waypoints_are_valid_nodes() {
    // Every waypoint in a route should be a valid routing graph node.
    let nodes = vec![node("A", 1, 1), node("B", 3, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    for wp in &route.waypoints {
        // Each waypoint should have valid doubled-integer coordinates.
        // Cell centers: both even. Junctions: one odd, one even. Intersections: both odd.
        let valid = wp.coord.is_cell_center()
            || wp.coord.is_junction()
            || wp.coord.is_street_intersection();
        assert!(valid, "Invalid waypoint coord: {:?}", wp.coord);
    }
}

#[test]
fn consecutive_waypoints_are_adjacent() {
    // Each pair of consecutive waypoints should be exactly 0.5 grid units apart.
    let nodes = vec![node("A", 1, 1), node("B", 3, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    for pair in route.waypoints.windows(2) {
        let manhattan = pair[0].coord.manhattan_to(pair[1].coord);
        assert_eq!(
            manhattan, 1,
            "Waypoints {:?} and {:?} are not adjacent (manhattan={})",
            pair[0].coord, pair[1].coord, manhattan
        );
    }
}

#[test]
fn route_has_at_least_two_waypoints() {
    // Non-trivial routes should have at least source and target.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert!(route.waypoints.len() >= 2);
}

#[test]
fn grid_2x1() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn grid_1x2() {
    let nodes = vec![node("A", 1, 1), node("B", 1, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn offset_coordinates() {
    // Nodes not starting at (1,1).
    let nodes = vec![node("A", 5, 10), node("B", 6, 10)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 1.0);
}
