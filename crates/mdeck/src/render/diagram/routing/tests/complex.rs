use super::*;

#[test]
fn multi_turn_route() {
    // A(1,1) -> B(3,3): can go straight then turn once through empty cells.
    let nodes = vec![node("A", 1, 1), node("B", 3, 3)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Optimal: go through empty cells with just 1 turn. Length = Manhattan distance.
    assert_eq!(route.complexity.length, 4.0);
    assert_eq!(route.complexity.turns, 1);
}

#[test]
fn star_topology() {
    // Center node with 4 edges to corners.
    let nodes = vec![
        node("Center", 2, 2),
        node("TL", 1, 1),
        node("TR", 3, 1),
        node("BL", 1, 3),
        node("BR", 3, 3),
    ];
    let edges = vec![
        edge("Center", "TL"),
        edge("Center", "TR"),
        edge("Center", "BL"),
        edge("Center", "BR"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn shared_corridor() {
    // Multiple edges sharing the same street.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 1, 3),
        node("C", 3, 1),
        node("D", 3, 3),
    ];
    // A->D and C->B both need to cross the middle.
    let edges = vec![edge("A", "D"), edge("C", "B")];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn route_around_occupied_center() {
    // 3x3 grid, center is occupied.
    let nodes = vec![node("A", 1, 1), node("Center", 2, 2), node("B", 3, 3)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Should not pass through Center.
    for wp in &route.waypoints {
        if wp.coord.is_cell_center()
            && wp.coord != GridCoord::from_int(1, 1)
            && wp.coord != GridCoord::from_int(3, 3)
        {
            assert_ne!(wp.coord, GridCoord::from_int(2, 2));
        }
    }
}

#[test]
fn five_nodes_in_row() {
    let nodes = vec![
        node("A", 1, 1),
        node("B", 2, 1),
        node("C", 3, 1),
        node("D", 4, 1),
        node("E", 5, 1),
    ];
    let edges = vec![
        edge("A", "B"),
        edge("B", "C"),
        edge("C", "D"),
        edge("D", "E"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn parallel_horizontal_edges() {
    // Two parallel horizontal edges.
    let nodes = vec![
        node("A1", 1, 1),
        node("B1", 3, 1),
        node("A2", 1, 2),
        node("B2", 3, 2),
    ];
    let edges = vec![edge("A1", "B1"), edge("A2", "B2")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn crossing_edges() {
    // Two edges that cross: A(1,1)->D(3,3) and C(3,1)->B(1,3).
    let nodes = vec![
        node("A", 1, 1),
        node("B", 1, 3),
        node("C", 3, 1),
        node("D", 3, 3),
    ];
    let edges = vec![edge("A", "D"), edge("C", "B")];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn large_grid_4x4() {
    // 4x4 grid with edges.
    let mut nodes = Vec::new();
    for r in 1..=4 {
        for c in 1..=4 {
            nodes.push(node(&format!("N{c}_{r}"), c, r));
        }
    }
    let edges = vec![
        edge("N1_1", "N4_4"),
        edge("N4_1", "N1_4"),
        edge("N1_1", "N4_1"),
        edge("N1_4", "N4_4"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn route_order_affects_result() {
    // Earlier edges get priority. Reversing edge order may change routes.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 3, 1),
        node("C", 1, 3),
        node("D", 3, 3),
    ];

    let edges1 = vec![edge("A", "B"), edge("C", "D")];
    let output1 = route_all_edges(&nodes, &edges1, &config(3, 3));
    assert_all_success(&output1);

    let edges2 = vec![edge("C", "D"), edge("A", "B")];
    let output2 = route_all_edges(&nodes, &edges2, &config(3, 3));
    assert_all_success(&output2);
    // Both should succeed regardless of order.
}

#[test]
fn triangle_topology() {
    let nodes = vec![node("A", 1, 1), node("B", 3, 1), node("C", 2, 3)];
    let edges = vec![edge("A", "B"), edge("B", "C"), edge("C", "A")];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn diamond_topology() {
    let nodes = vec![
        node("Top", 2, 1),
        node("Left", 1, 2),
        node("Right", 3, 2),
        node("Bottom", 2, 3),
    ];
    let edges = vec![
        edge("Top", "Left"),
        edge("Top", "Right"),
        edge("Left", "Bottom"),
        edge("Right", "Bottom"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn six_edges_between_four_nodes() {
    // All pairs in a 4-node graph.
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
    let output = route_all_edges(&nodes, &edges, &config(7, 7));
    assert_all_success(&output);
}

#[test]
fn linear_chain_5_nodes() {
    let nodes = vec![
        node("A", 1, 1),
        node("B", 2, 1),
        node("C", 3, 1),
        node("D", 4, 1),
        node("E", 5, 1),
    ];
    let edges = vec![edge("A", "E")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Must route around B, C, D — can't go through occupied cells.
    assert!(route.complexity.length > 4.0);
}

#[test]
fn vertical_chain_with_skip() {
    let nodes = vec![node("A", 1, 1), node("B", 1, 2), node("C", 1, 3)];
    let edges = vec![edge("A", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Must route around B.
    assert!(route.complexity.length > 2.0);
}

#[test]
fn route_prefers_shortest_path() {
    // With no obstacles, the shortest Manhattan path should be chosen.
    let nodes = vec![node("A", 1, 1), node("B", 4, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 3.0);
    assert_eq!(route.complexity.turns, 0);
}

#[test]
fn route_with_wide_gap() {
    // A(1,1) -> B(1,5): nodes far apart vertically, no obstacles between.
    let nodes = vec![node("A", 1, 1), node("B", 1, 5)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    assert_eq!(route.complexity.length, 4.0);
}

#[test]
fn many_edges_same_pair() {
    // 3 edges between same pair of nodes — must use different lanes.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn grid_3x3_with_center_edges() {
    let nodes = vec![
        node("TL", 1, 1),
        node("TC", 2, 1),
        node("TR", 3, 1),
        node("ML", 1, 2),
        node("MC", 2, 2),
        node("MR", 3, 2),
        node("BL", 1, 3),
        node("BC", 2, 3),
        node("BR", 3, 3),
    ];
    let edges = vec![
        edge("MC", "TC"),
        edge("MC", "ML"),
        edge("MC", "MR"),
        edge("MC", "BC"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn l_shaped_with_nodes_at_corner() {
    // Route must navigate around a node at the corner of an L-shape.
    let nodes = vec![node("A", 1, 1), node("Corner", 3, 1), node("B", 3, 3)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn route_from_corner_to_corner() {
    // Route from top-left to bottom-right of a grid with obstacles.
    let nodes = vec![
        node("A", 1, 1),
        node("O1", 2, 2),
        node("O2", 3, 3),
        node("B", 4, 4),
    ];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(5, 5));
    assert_all_success(&output);
}

#[test]
fn five_edges_fan_out() {
    // One source, 5 targets arranged in a column.
    let nodes = vec![
        node("S", 1, 3),
        node("T1", 3, 1),
        node("T2", 3, 2),
        node("T3", 3, 3),
        node("T4", 3, 4),
        node("T5", 3, 5),
    ];
    let edges = vec![
        edge("S", "T1"),
        edge("S", "T2"),
        edge("S", "T3"),
        edge("S", "T4"),
        edge("S", "T5"),
    ];
    let output = route_all_edges(&nodes, &edges, &config(7, 7));
    assert_all_success(&output);
}
