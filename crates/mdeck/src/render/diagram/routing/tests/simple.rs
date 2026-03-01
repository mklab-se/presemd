use super::*;

#[test]
fn adjacent_horizontal_right() {
    // A(1,1) -> B(2,1): should route right.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(
        route.waypoints.first().unwrap().coord,
        GridCoord::from_int(1, 1)
    );
    assert_eq!(
        route.waypoints.last().unwrap().coord,
        GridCoord::from_int(2, 1)
    );
    assert_eq!(route.complexity.length, 1.0);
}

#[test]
fn adjacent_horizontal_left() {
    // A(2,1) -> B(1,1): should route left.
    let nodes = vec![node("A", 2, 1), node("B", 1, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 1.0);
}

#[test]
fn adjacent_vertical_down() {
    // A(1,1) -> B(1,2): should route down.
    let nodes = vec![node("A", 1, 1), node("B", 1, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 1.0);
}

#[test]
fn adjacent_vertical_up() {
    // A(1,2) -> B(1,1): should route up.
    let nodes = vec![node("A", 1, 2), node("B", 1, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 1.0);
}

#[test]
fn straight_route_two_apart_horizontal() {
    // A(1,1) -> B(3,1): straight horizontal, 2 units.
    let nodes = vec![node("A", 1, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 2.0);
}

#[test]
fn straight_route_two_apart_vertical() {
    // A(1,1) -> B(1,3): straight vertical, 2 units.
    let nodes = vec![node("A", 1, 1), node("B", 1, 3)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 2.0);
}

#[test]
fn l_shaped_route() {
    // A(1,1) -> B(2,2): L-shaped route through empty cells.
    // Optimal: go right through empty cell (2,1) then turn down. Length 2, 1 turn.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 2.0);
    // Only 1 turn needed: go straight then turn once to reach target.
    assert_eq!(route.complexity.turns, 1);
}

#[test]
fn route_complexity_includes_turns() {
    // A(1,1) -> B(2,2): complexity should be length + turns.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert!(route.complexity.total() >= 2.0); // At least 2 for length.
}

#[test]
fn route_through_empty_cell() {
    // A(1,1), B(3,1), empty cell at (2,1). Route should go through the empty cell.
    let nodes = vec![node("A", 1, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Direct path is through empty cell (2,1) center.
    assert_eq!(route.complexity.length, 2.0);
    assert_eq!(route.complexity.turns, 0);
}

#[test]
fn route_around_obstacle() {
    // A(1,1), C(2,1) blocks, B(3,1). Must route around C.
    let nodes = vec![node("A", 1, 1), node("C", 2, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Can't go through C, must detour.
    assert!(route.complexity.length > 2.0);
    assert!(route.complexity.turns >= 2);
}

#[test]
fn route_uses_boundary_streets() {
    // A(1,1), B(3,1), C(2,1) blocks. Route may use boundary streets.
    let nodes = vec![node("A", 1, 1), node("C", 2, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    // Just verify it succeeds and doesn't go through C.
    let route = get_route(&output, 0);
    for wp in &route.waypoints {
        if wp.coord.is_cell_center()
            && wp.coord != GridCoord::from_int(1, 1)
            && wp.coord != GridCoord::from_int(3, 1)
        {
            assert_ne!(
                wp.coord,
                GridCoord::from_int(2, 1),
                "Route should not go through C"
            );
        }
    }
}

#[test]
fn two_edges_same_direction() {
    // A(1,1) -> B(2,1) and A(1,1) -> B(2,1): two edges, same direction.
    // Second should use a different lane.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn bidirectional_edges() {
    // A(1,1) -> B(2,1) and B(2,1) -> A(1,1).
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("B", "A")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn three_nodes_chain() {
    // A(1,1) -> B(2,1) -> C(3,1).
    let nodes = vec![node("A", 1, 1), node("B", 2, 1), node("C", 3, 1)];
    let edges = vec![edge("A", "B"), edge("B", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn waypoints_start_and_end_at_cell_centers() {
    // Every route should start and end at cell centers.
    let nodes = vec![node("A", 1, 1), node("B", 3, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert!(route.waypoints.first().unwrap().coord.is_cell_center());
    assert!(route.waypoints.last().unwrap().coord.is_cell_center());
}

#[test]
fn route_length_matches_waypoints() {
    // Verify the complexity length matches the sum of segment lengths.
    let nodes = vec![node("A", 1, 1), node("B", 3, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);

    let mut computed_length = 0.0;
    for pair in route.waypoints.windows(2) {
        let dcol = (pair[1].coord.col2 - pair[0].coord.col2).abs() as f64 / 2.0;
        let drow = (pair[1].coord.row2 - pair[0].coord.row2).abs() as f64 / 2.0;
        computed_length += dcol + drow;
    }
    assert!(
        (computed_length - route.complexity.length).abs() < 0.001,
        "Length mismatch: computed={}, reported={}",
        computed_length,
        route.complexity.length
    );
}

#[test]
fn adjacent_diagonal() {
    // A(1,1) -> B(2,2): diagonal neighbors.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Manhattan distance is 2, so length should be at least 2.
    assert!(route.complexity.length >= 2.0);
}

#[test]
fn longer_straight_route() {
    // A(1,1) -> B(5,1): 4 units apart.
    let nodes = vec![node("A", 1, 1), node("B", 5, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 4.0);
    assert_eq!(route.complexity.turns, 0);
}

#[test]
fn no_lane_changes_on_simple_route() {
    // A(1,1) -> B(2,1): simple route should have 0 lane changes.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.lane_changes, 0);
}

#[test]
fn center_lane_preferred() {
    // First edge should prefer center lane (0).
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // The first waypoint's lane should be 0 (center).
    assert_eq!(route.waypoints[0].lane, 0);
}

#[test]
fn route_with_label() {
    // Labels don't affect routing.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge_labeled("A", "B", "hello")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn route_three_in_a_row() {
    // A(1,1), B(2,1), C(3,1): route A->C through empty (direct not possible, B blocks).
    let nodes = vec![node("A", 1, 1), node("B", 2, 1), node("C", 3, 1)];
    let edges = vec![edge("A", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert!(route.complexity.length > 2.0, "Must detour around B");
}

#[test]
fn two_separate_routes() {
    // Two independent edges that don't share segments.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 2, 1),
        node("C", 1, 3),
        node("D", 2, 3),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn complexity_total_correct() {
    // Verify total = length + turns + lane_changes.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    let expected_total = route.complexity.length
        + route.complexity.turns as f64
        + route.complexity.lane_changes as f64;
    assert!(
        (route.complexity.total() - expected_total).abs() < 0.001,
        "Total complexity mismatch"
    );
}
