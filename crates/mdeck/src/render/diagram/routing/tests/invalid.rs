use super::*;

#[test]
fn missing_source_node() {
    let nodes = vec![node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_edge_failed(&output, 0);
}

#[test]
fn missing_target_node() {
    let nodes = vec![node("A", 1, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_edge_failed(&output, 0);
}

#[test]
fn both_nodes_missing() {
    let nodes = vec![node("C", 1, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_edge_failed(&output, 0);
}

#[test]
fn empty_nodes_with_edges() {
    let nodes: Vec<DiagramNode> = vec![];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_edge_failed(&output, 0);
}

#[test]
fn empty_nodes_empty_edges() {
    let nodes: Vec<DiagramNode> = vec![];
    let edges: Vec<DiagramEdge> = vec![];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_eq!(output.results.len(), 0);
}

#[test]
fn failure_warning_contains_node_names() {
    let nodes = vec![node("Alpha", 1, 1)];
    let edges = vec![edge("Alpha", "Beta")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    match &output.results[0].1 {
        super::super::types::RouteResult::Failure { warning } => {
            assert!(
                warning.contains("Beta"),
                "Warning should mention target name"
            );
        }
        _ => panic!("Expected failure"),
    }
}

#[test]
fn failure_warning_for_missing_source() {
    let nodes = vec![node("B", 1, 1)];
    let edges = vec![edge("MissingSource", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    match &output.results[0].1 {
        super::super::types::RouteResult::Failure { warning } => {
            assert!(
                warning.contains("MissingSource"),
                "Warning should mention source name: {}",
                warning
            );
        }
        _ => panic!("Expected failure"),
    }
}

#[test]
fn mixed_success_and_failure() {
    // One valid edge and one with missing target.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_eq!(output.results.len(), 2);
    match &output.results[0].1 {
        super::super::types::RouteResult::Success(_) => {}
        _ => panic!("First edge should succeed"),
    }
    assert_edge_failed(&output, 1);
}

#[test]
fn duplicate_node_names() {
    // Two nodes with same name — last one wins in the HashMap.
    let nodes = vec![node("A", 1, 1), node("A", 2, 2), node("B", 3, 3)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn zero_h_capacity() {
    // Zero horizontal lane capacity — edges can still use vertical lanes.
    let nodes = vec![node("A", 1, 1), node("B", 1, 2)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(0, 3));
    // This should still work for vertical routes.
    assert_all_success(&output);
}

#[test]
fn zero_v_capacity() {
    // Zero vertical lane capacity — edges can still use horizontal lanes.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 0));
    assert_all_success(&output);
}

#[test]
fn zero_both_capacities() {
    // Zero capacity in both directions — no route possible (except self-edge).
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(0, 0));
    assert_edge_failed(&output, 0);
}

#[test]
fn edge_preserves_label_in_output() {
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge_labeled("A", "B", "test label")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_eq!(output.results[0].0.label.as_deref(), Some("test label"));
}

#[test]
fn failure_does_not_block_subsequent_edges() {
    // A failing edge should not prevent later edges from succeeding.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("X", "Y"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_edge_failed(&output, 0);
    match &output.results[1].1 {
        super::super::types::RouteResult::Success(_) => {}
        _ => panic!("Second edge should succeed"),
    }
}
