use super::*;
use crate::render::diagram::routing::serialize::route_to_string;

/// Run a routing scenario multiple times and assert identical output.
fn assert_deterministic(
    nodes: &[DiagramNode],
    edges: &[DiagramEdge],
    cfg: &RoutingConfig,
    iterations: usize,
) {
    let reference = route_all_edges(nodes, edges, cfg);
    let ref_strings: Vec<String> = reference
        .results
        .iter()
        .map(|(_, r)| match r {
            super::super::types::RouteResult::Success(route) => route_to_string(route),
            super::super::types::RouteResult::Failure { warning } => {
                format!("FAIL:{}", warning)
            }
        })
        .collect();

    for i in 1..iterations {
        let output = route_all_edges(nodes, edges, cfg);
        let strings: Vec<String> = output
            .results
            .iter()
            .map(|(_, r)| match r {
                super::super::types::RouteResult::Success(route) => route_to_string(route),
                super::super::types::RouteResult::Failure { warning } => {
                    format!("FAIL:{}", warning)
                }
            })
            .collect();

        assert_eq!(
            ref_strings, strings,
            "Non-deterministic result on iteration {}",
            i
        );
    }
}

#[test]
fn simple_route_deterministic() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    assert_deterministic(&nodes, &edges, &config(3, 3), 50);
}

#[test]
fn l_route_deterministic() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];
    assert_deterministic(&nodes, &edges, &config(3, 3), 50);
}

#[test]
fn multi_edge_deterministic() {
    let nodes = vec![
        node("A", 1, 1),
        node("B", 3, 1),
        node("C", 1, 3),
        node("D", 3, 3),
    ];
    let edges = vec![
        edge("A", "D"),
        edge("C", "B"),
        edge("A", "B"),
        edge("C", "D"),
    ];
    assert_deterministic(&nodes, &edges, &config(5, 5), 50);
}

#[test]
fn star_deterministic() {
    let nodes = vec![
        node("Center", 2, 2),
        node("N", 2, 1),
        node("S", 2, 3),
        node("E", 3, 2),
        node("W", 1, 2),
    ];
    let edges = vec![
        edge("Center", "N"),
        edge("Center", "S"),
        edge("Center", "E"),
        edge("Center", "W"),
    ];
    assert_deterministic(&nodes, &edges, &config(5, 5), 50);
}

#[test]
fn congested_deterministic() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B"), edge("A", "B")];
    assert_deterministic(&nodes, &edges, &config(3, 3), 50);
}

#[test]
fn diamond_deterministic() {
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
    assert_deterministic(&nodes, &edges, &config(5, 5), 50);
}

#[test]
fn obstacle_route_deterministic() {
    let nodes = vec![node("A", 1, 1), node("O", 2, 1), node("B", 3, 1)];
    let edges = vec![edge("A", "B")];
    assert_deterministic(&nodes, &edges, &config(3, 3), 50);
}

#[test]
fn large_grid_deterministic() {
    let mut nodes = Vec::new();
    for r in 1..=4 {
        for c in 1..=4 {
            nodes.push(node(&format!("N{c}_{r}"), c, r));
        }
    }
    let edges = vec![edge("N1_1", "N4_4"), edge("N4_1", "N1_4")];
    assert_deterministic(&nodes, &edges, &config(5, 5), 20);
}

#[test]
fn repeated_runs_100x() {
    let nodes = vec![node("A", 1, 1), node("B", 3, 2)];
    let edges = vec![edge("A", "B")];
    assert_deterministic(&nodes, &edges, &config(3, 3), 100);
}

#[test]
fn tie_breaking_deterministic() {
    // Scenario with symmetric routes where tie-breaking matters.
    let nodes = vec![node("A", 1, 1), node("B", 3, 3)];
    let edges = vec![edge("A", "B")];
    assert_deterministic(&nodes, &edges, &config(3, 3), 50);
}
