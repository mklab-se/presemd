use super::*;
use crate::render::diagram::routing::graph::RoutingGraph;
use crate::render::diagram::routing::lanes::LaneOccupancy;
use crate::render::diagram::routing::search::find_best_route;
use crate::render::diagram::routing::types::{CostWeights, GridCoord, SegmentId};

// =============================================================================
// Crossing detection tests (LaneOccupancy::count_crossings)
// =============================================================================

#[test]
fn count_crossings_no_prior_routes() {
    // Empty occupancy → 0 crossings for any segment.
    let occ = LaneOccupancy::new();

    // A horizontal segment through a street intersection.
    // Intersection at (1,1) in doubled coords (both odd).
    let seg = SegmentId::new(
        GridCoord { col2: 0, row2: 1 }, // junction
        GridCoord { col2: 1, row2: 1 }, // street intersection
    );
    assert_eq!(occ.count_crossings(&seg, 0, &[]), 0);

    // A vertical segment through the same intersection.
    let seg_v = SegmentId::new(
        GridCoord { col2: 1, row2: 0 },
        GridCoord { col2: 1, row2: 1 },
    );
    assert_eq!(occ.count_crossings(&seg_v, 0, &[]), 0);
}

#[test]
fn count_crossings_perpendicular_one_side_only() {
    // Claim a horizontal segment on ONE side of intersection (1,1) only.
    // This is a route that turns at (1,1), not one that passes through.
    // With pass-through semantics, this should NOT count as a crossing.
    let mut occ = LaneOccupancy::new();

    // Horizontal segment: (0,1) ↔ (1,1)  (junction to street intersection)
    let h_seg = SegmentId::new(
        GridCoord { col2: 0, row2: 1 },
        GridCoord { col2: 1, row2: 1 },
    );
    occ.claim(h_seg, 0);

    // Vertical segment: (1,0) ↔ (1,1)  (junction to street intersection)
    let v_seg = SegmentId::new(
        GridCoord { col2: 1, row2: 0 },
        GridCoord { col2: 1, row2: 1 },
    );

    // Only one side has a horizontal segment → turn, not pass-through → 0 crossings.
    let crossings = occ.count_crossings(&v_seg, 0, &[]);
    assert_eq!(
        crossings, 0,
        "One-sided perpendicular is a turn, not a crossing"
    );
}

#[test]
fn count_crossings_perpendicular_pass_through() {
    // Claim horizontal segments on BOTH sides of intersection (1,1).
    // This represents a route passing straight through → real crossing.
    let mut occ = LaneOccupancy::new();

    // Horizontal segments: (0,1) ↔ (1,1) and (1,1) ↔ (2,1)
    let h_left = SegmentId::new(
        GridCoord { col2: 0, row2: 1 },
        GridCoord { col2: 1, row2: 1 },
    );
    let h_right = SegmentId::new(
        GridCoord { col2: 1, row2: 1 },
        GridCoord { col2: 2, row2: 1 },
    );
    occ.claim(h_left, 0);
    occ.claim(h_right, 0);

    // Vertical segment: (1,0) ↔ (1,1)
    let v_seg = SegmentId::new(
        GridCoord { col2: 1, row2: 0 },
        GridCoord { col2: 1, row2: 1 },
    );

    let crossings = occ.count_crossings(&v_seg, 0, &[]);
    assert_eq!(
        crossings, 1,
        "Horizontal pass-through at (1,1) is a real crossing"
    );
}

#[test]
fn count_crossings_parallel() {
    // Parallel segments at the same intersection → 0 crossings.
    let mut occ = LaneOccupancy::new();

    // Horizontal segment: (0,1) ↔ (1,1)
    let h_seg1 = SegmentId::new(
        GridCoord { col2: 0, row2: 1 },
        GridCoord { col2: 1, row2: 1 },
    );
    occ.claim(h_seg1, 0);

    // Another horizontal segment: (1,1) ↔ (2,1)
    let h_seg2 = SegmentId::new(
        GridCoord { col2: 1, row2: 1 },
        GridCoord { col2: 2, row2: 1 },
    );

    // Both horizontal → parallel → no crossing.
    let crossings = occ.count_crossings(&h_seg2, 0, &[]);
    assert_eq!(crossings, 0, "Parallel segments should not cross");
}

#[test]
fn count_crossings_both_endpoints() {
    // A segment with pass-through crossings at both endpoints.
    let mut occ = LaneOccupancy::new();

    // Set up pass-through at (1,1): horizontal segments on BOTH sides.
    let h_at_11_left = SegmentId::new(
        GridCoord { col2: 0, row2: 1 },
        GridCoord { col2: 1, row2: 1 },
    );
    let h_at_11_right = SegmentId::new(
        GridCoord { col2: 1, row2: 1 },
        GridCoord { col2: 2, row2: 1 },
    );
    occ.claim(h_at_11_left, 0);
    occ.claim(h_at_11_right, 0);

    // Set up pass-through at (1,3): horizontal segments on BOTH sides.
    let h_at_13_left = SegmentId::new(
        GridCoord { col2: 0, row2: 3 },
        GridCoord { col2: 1, row2: 3 },
    );
    let h_at_13_right = SegmentId::new(
        GridCoord { col2: 1, row2: 3 },
        GridCoord { col2: 2, row2: 3 },
    );
    occ.claim(h_at_13_left, 0);
    occ.claim(h_at_13_right, 0);

    // Vertical segment from (1,1) to (1,2): endpoint (1,1) has pass-through
    let v_seg_1 = SegmentId::new(
        GridCoord { col2: 1, row2: 1 },
        GridCoord { col2: 1, row2: 2 },
    );
    assert_eq!(
        occ.count_crossings(&v_seg_1, 0, &[]),
        1,
        "Pass-through crossing at (1,1) only"
    );

    // Vertical segment from (1,2) to (1,3): endpoint (1,3) has pass-through
    let v_seg_2 = SegmentId::new(
        GridCoord { col2: 1, row2: 2 },
        GridCoord { col2: 1, row2: 3 },
    );
    assert_eq!(
        occ.count_crossings(&v_seg_2, 0, &[]),
        1,
        "Pass-through crossing at (1,3) only"
    );
}

#[test]
fn count_crossings_pass_through_counts_as_one() {
    // Both perpendicular segments claimed at a single intersection form
    // a single pass-through crossing (not 2 separate ones).
    let mut occ = LaneOccupancy::new();

    // At intersection (1,1), claim both horizontal perpendiculars:
    // (0,1) ↔ (1,1) and (1,1) ↔ (2,1)
    let h_left = SegmentId::new(
        GridCoord { col2: 0, row2: 1 },
        GridCoord { col2: 1, row2: 1 },
    );
    let h_right = SegmentId::new(
        GridCoord { col2: 1, row2: 1 },
        GridCoord { col2: 2, row2: 1 },
    );
    occ.claim(h_left, 0);
    occ.claim(h_right, 0);

    // Vertical segment through (1,1): (1,0)↔(1,1)
    let v_seg = SegmentId::new(
        GridCoord { col2: 1, row2: 0 },
        GridCoord { col2: 1, row2: 1 },
    );

    assert_eq!(
        occ.count_crossings(&v_seg, 0, &[]),
        1,
        "Pass-through at (1,1) is a single crossing"
    );
}

#[test]
fn count_crossings_at_cell_center() {
    // Regression: crossings at empty cell centers (both col2 and row2 even)
    // must be detected, not only at street intersections (both odd).
    //
    // Route 1 claims horizontal segments through cell center (4,4):
    //   (3,4)→(4,4) and (4,4)→(5,4) on lane 0
    // A new vertical segment (4,3)→(4,4) should detect the crossing.
    let mut occ = LaneOccupancy::new();

    let h_left = SegmentId::new(
        GridCoord { col2: 3, row2: 4 },
        GridCoord { col2: 4, row2: 4 },
    );
    let h_right = SegmentId::new(
        GridCoord { col2: 4, row2: 4 },
        GridCoord { col2: 5, row2: 4 },
    );
    occ.claim(h_left, 0);
    occ.claim(h_right, 0);

    // Vertical segment approaching cell center (4,4)
    let v_seg = SegmentId::new(
        GridCoord { col2: 4, row2: 3 },
        GridCoord { col2: 4, row2: 4 },
    );
    assert!(
        occ.count_crossings(&v_seg, 0, &[]) > 0,
        "Should detect crossing at cell center (both-even endpoint)"
    );
}

#[test]
fn count_crossings_at_junction_one_side() {
    // One-sided perpendicular at a junction (one odd, one even coord)
    // is a turn, not a pass-through → 0 crossings.
    let mut occ = LaneOccupancy::new();

    // Claim a horizontal segment through junction (5,4): (4,4)→(5,4)
    let h_seg = SegmentId::new(
        GridCoord { col2: 4, row2: 4 },
        GridCoord { col2: 5, row2: 4 },
    );
    occ.claim(h_seg, 0);

    // Vertical segment through the same junction: (5,3)→(5,4)
    let v_seg = SegmentId::new(
        GridCoord { col2: 5, row2: 3 },
        GridCoord { col2: 5, row2: 4 },
    );
    assert_eq!(
        occ.count_crossings(&v_seg, 0, &[]),
        0,
        "One-sided at junction is a turn, not a crossing"
    );
}

#[test]
fn count_crossings_at_junction_pass_through() {
    // Pass-through at a junction: horizontal segments on BOTH sides.
    let mut occ = LaneOccupancy::new();

    // Claim horizontal segments through junction (5,4): (4,4)→(5,4) and (5,4)→(6,4)
    let h_left = SegmentId::new(
        GridCoord { col2: 4, row2: 4 },
        GridCoord { col2: 5, row2: 4 },
    );
    let h_right = SegmentId::new(
        GridCoord { col2: 5, row2: 4 },
        GridCoord { col2: 6, row2: 4 },
    );
    occ.claim(h_left, 0);
    occ.claim(h_right, 0);

    // Vertical segment through the same junction: (5,3)→(5,4)
    let v_seg = SegmentId::new(
        GridCoord { col2: 5, row2: 3 },
        GridCoord { col2: 5, row2: 4 },
    );
    assert_eq!(
        occ.count_crossings(&v_seg, 0, &[]),
        1,
        "Pass-through at junction is a real crossing"
    );
}

#[test]
fn count_crossings_no_perpendicular_at_non_intersection() {
    // No perpendicular claims → still 0 crossings regardless of node type.
    let mut occ = LaneOccupancy::new();

    // Claim a horizontal segment at junction (1,0)
    let seg_at_junction = SegmentId::new(
        GridCoord { col2: 0, row2: 0 },
        GridCoord { col2: 1, row2: 0 },
    );
    occ.claim(seg_at_junction, 0);

    // Another horizontal segment sharing endpoint (1,0) — parallel, not perpendicular
    let seg = SegmentId::new(
        GridCoord { col2: 1, row2: 0 },
        GridCoord { col2: 2, row2: 0 },
    );
    assert_eq!(
        occ.count_crossings(&seg, 0, &[]),
        0,
        "Parallel segments at a junction should not count as crossings"
    );
}

#[test]
fn count_crossings_skips_excluded_endpoints() {
    // Crossings at source/target node centers should be skippable.
    let mut occ = LaneOccupancy::new();

    // Claim horizontal pass-through at cell center (4,4):
    // segments on BOTH sides to create a real crossing.
    let h_left = SegmentId::new(
        GridCoord { col2: 3, row2: 4 },
        GridCoord { col2: 4, row2: 4 },
    );
    let h_right = SegmentId::new(
        GridCoord { col2: 4, row2: 4 },
        GridCoord { col2: 5, row2: 4 },
    );
    occ.claim(h_left, 0);
    occ.claim(h_right, 0);

    // Vertical segment through (4,4)
    let v_seg = SegmentId::new(
        GridCoord { col2: 4, row2: 3 },
        GridCoord { col2: 4, row2: 4 },
    );

    // Without exclusion: crossing detected (pass-through).
    assert_eq!(occ.count_crossings(&v_seg, 0, &[]), 1);

    // With (4,4) excluded (it's a node center): no crossing.
    let hub = GridCoord { col2: 4, row2: 4 };
    assert_eq!(
        occ.count_crossings(&v_seg, 0, &[hub]),
        0,
        "Excluded node center should not count crossings"
    );
}

// =============================================================================
// Turn-conflict tests (lane-aware crossing detection)
// =============================================================================

#[test]
fn turn_conflict_lane_on_same_side_as_turn() {
    // At a junction, an existing route turns from above (north).
    // A new horizontal segment on lane -1 (north side, absolute) should trigger
    // a turn conflict. Lane +1 (south side) should not.
    let mut occ = LaneOccupancy::new();

    // Claim vertical segment from above into junction (3,4):
    // (3,3) → (3,4) going south, lane 0
    let v_from_above = SegmentId::new(
        GridCoord { col2: 3, row2: 3 },
        GridCoord { col2: 3, row2: 4 },
    );
    occ.claim(v_from_above, 0);

    // Horizontal segment through the same junction: (2,4) ↔ (3,4)
    let h_seg = SegmentId::new(
        GridCoord { col2: 2, row2: 4 },
        GridCoord { col2: 3, row2: 4 },
    );

    // Lane 0: no turn conflict (center lane never triggers).
    assert_eq!(
        occ.count_crossings(&h_seg, 0, &[]),
        0,
        "Lane 0 should not trigger turn conflict"
    );

    // Lane -1 (north, absolute): same side as the turn from above → conflict.
    assert_eq!(
        occ.count_crossings(&h_seg, -1, &[]),
        1,
        "Lane -1 (north) should conflict with turn from above"
    );

    // Lane +1 (south, absolute): opposite side → no conflict.
    assert_eq!(
        occ.count_crossings(&h_seg, 1, &[]),
        0,
        "Lane +1 (south) should NOT conflict with turn from above"
    );
}

#[test]
fn turn_conflict_lane_on_same_side_as_turn_from_below() {
    // Mirror: existing route turns from below (south).
    // Lane +1 (south, absolute) should trigger conflict.
    let mut occ = LaneOccupancy::new();

    // Claim vertical segment from below into junction (3,4):
    // (3,4) → (3,5) going south, lane 0
    let v_from_below = SegmentId::new(
        GridCoord { col2: 3, row2: 4 },
        GridCoord { col2: 3, row2: 5 },
    );
    occ.claim(v_from_below, 0);

    // Horizontal segment through the same junction: (2,4) ↔ (3,4)
    let h_seg = SegmentId::new(
        GridCoord { col2: 2, row2: 4 },
        GridCoord { col2: 3, row2: 4 },
    );

    // Lane +1 (south): same side as turn from below → conflict.
    assert_eq!(
        occ.count_crossings(&h_seg, 1, &[]),
        1,
        "Lane +1 (south) should conflict with turn from below"
    );

    // Lane -1 (north): opposite side → no conflict.
    assert_eq!(
        occ.count_crossings(&h_seg, -1, &[]),
        0,
        "Lane -1 (north) should NOT conflict with turn from below"
    );
}

#[test]
fn turn_conflict_vertical_segment_from_left() {
    // Vertical segment case: existing route turns from the left (west).
    // Lane -1 (west, absolute) should trigger conflict.
    let mut occ = LaneOccupancy::new();

    // Claim horizontal segment from the left into junction (4,3):
    // (3,3) → (4,3)
    let h_from_left = SegmentId::new(
        GridCoord { col2: 3, row2: 3 },
        GridCoord { col2: 4, row2: 3 },
    );
    occ.claim(h_from_left, 0);

    // Vertical segment through the same junction: (4,2) ↔ (4,3)
    let v_seg = SegmentId::new(
        GridCoord { col2: 4, row2: 2 },
        GridCoord { col2: 4, row2: 3 },
    );

    // Lane -1 (west, absolute): same side as turn from left → conflict.
    assert_eq!(
        occ.count_crossings(&v_seg, -1, &[]),
        1,
        "Lane -1 (west) should conflict with turn from left"
    );

    // Lane +1 (east, absolute): opposite side → no conflict.
    assert_eq!(
        occ.count_crossings(&v_seg, 1, &[]),
        0,
        "Lane +1 (east) should NOT conflict with turn from left"
    );
}

#[test]
fn turn_conflict_pass_through_always_detects() {
    // When both sides are claimed (pass-through), crossing is detected
    // regardless of lane — even lane 0.
    let mut occ = LaneOccupancy::new();

    let v_above = SegmentId::new(
        GridCoord { col2: 3, row2: 3 },
        GridCoord { col2: 3, row2: 4 },
    );
    let v_below = SegmentId::new(
        GridCoord { col2: 3, row2: 4 },
        GridCoord { col2: 3, row2: 5 },
    );
    occ.claim(v_above, 0);
    occ.claim(v_below, 0);

    let h_seg = SegmentId::new(
        GridCoord { col2: 2, row2: 4 },
        GridCoord { col2: 3, row2: 4 },
    );

    // Pass-through is detected for all lanes.
    assert_eq!(occ.count_crossings(&h_seg, 0, &[]), 1);
    assert_eq!(occ.count_crossings(&h_seg, 1, &[]), 1);
    assert_eq!(occ.count_crossings(&h_seg, -1, &[]), 1);
}

#[test]
fn hub_node_crossings_not_counted_in_routing() {
    // Routes converging at a hub node should not count as crossings.
    // A(1,2) enters API(2,2) from the west, B(2,1) enters API from the north.
    // A third route C(2,3) exits API going south — this should NOT count
    // crossings at the API center where the first two routes also pass through.
    let nodes = vec![
        node("A", 1, 2),
        node("API", 2, 2),
        node("B", 2, 1),
        node("C", 2, 3),
    ];
    let edges = vec![edge("A", "API"), edge("B", "API"), edge("API", "C")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route3 = get_route(&output, 2);
    assert_eq!(
        route3.complexity.crossings, 0,
        "Route from hub should not count convergence at hub as crossings"
    );
}

// =============================================================================
// Crossing avoidance routing tests
// =============================================================================

#[test]
fn crossing_detected_at_empty_cell_center() {
    // Regression test for the hub-and-spoke bug:
    // Two routes crossing at an empty cell center were not detected.
    //
    // A(1,2) → B(3,2): horizontal route straight through empty cell (2,2)
    // C(2,1) → D(2,3): vertical route straight through same cell (2,2)
    // The second route must report crossings > 0.
    let nodes = vec![
        node("A", 1, 2),
        node("B", 3, 2),
        node("C", 2, 1),
        node("D", 2, 3),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route2 = get_route(&output, 1);
    assert!(
        route2.complexity.crossings > 0,
        "Vertical route through empty cell center should cross the horizontal route, \
         got crossings={}",
        route2.complexity.crossings,
    );
}

#[test]
fn crossing_avoidance_simple() {
    // 2x2 grid: A(1,1) B(2,1) C(1,2) D(2,2)
    // First edge: A -> B (horizontal)
    // Second edge: C -> D (horizontal, same row below)
    // These shouldn't cross since they're on different rows.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 2, 1),
        node("C", 1, 2),
        node("D", 2, 2),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route1 = get_route(&output, 0);
    let route2 = get_route(&output, 1);
    assert_eq!(route1.complexity.crossings, 0);
    assert_eq!(route2.complexity.crossings, 0);
}

#[test]
fn crossing_avoidance_perpendicular_edges() {
    // A(1,1) B(3,1) on top row, C(2,1) in between.
    // D(2,0) above, E(2,2) below.
    // Wait — this test needs nodes positioned so that a horizontal edge and vertical edge
    // would naturally cross at an intersection.
    //
    // Grid layout (3x3):
    //   A(1,1) -- B(3,1) horizontal edge across row 1
    //   C(2,0) -- D(2,2) vertical edge across column 2
    // These should cross at the intersection between cells (2,1) area.
    //
    // But (2,1) is occupied by... nothing. Actually with A at (1,1) and B at (3,1),
    // the horizontal route goes through (2,1) area. With C at (2,0) and D at (2,2),
    // the vertical route goes through (2,1) area. They'd cross.
    //
    // However, the cell (2,1) is not occupied (no node there), so routes can go through it.
    // The horizontal route A->B naturally passes through the street intersection at
    // the center of cell (2,1). The vertical route C->D also passes through the same area.

    let nodes = vec![
        node("A", 1, 1),
        node("B", 3, 1),
        node("C", 2, 0),
        node("D", 2, 2),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];

    // With default crossing weight (1.0), the router should try to avoid the crossing.
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
}

#[test]
fn crossing_weight_zero_ignores_crossings() {
    // With crossing weight 0, the router doesn't care about crossings.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 3, 1),
        node("C", 2, 0),
        node("D", 2, 2),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];

    let weights = CostWeights {
        crossing: 0.0,
        ..CostWeights::default()
    };
    let output = route_all_edges(&nodes, &edges, &config_weighted(3, 3, weights));
    assert_all_success(&output);
}

#[test]
fn crossing_weight_high_avoids_crossing() {
    // With very high crossing weight, the router should prefer detours over crossings.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 3, 1),
        node("C", 2, 0),
        node("D", 2, 2),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];

    let weights = CostWeights {
        crossing: 100.0,
        ..CostWeights::default()
    };
    let output = route_all_edges(&nodes, &edges, &config_weighted(5, 5, weights));
    assert_all_success(&output);
    // The second route should have 0 crossings if a detour is available.
    let route2 = get_route(&output, 1);
    assert_eq!(
        route2.complexity.crossings, 0,
        "With high crossing weight, router should find a crossing-free route"
    );
}

#[test]
fn crossing_avoidance_prefers_detour() {
    // Set up a scenario where the direct vertical route crosses a horizontal route,
    // and verify that with high crossing weight, the router picks a longer but
    // crossing-free detour.
    //
    // Layout:
    //   A(1,1) ---- B(4,1)   horizontal edge
    //   C(2,0) and D(2,2)    vertical edge would cross the horizontal one
    //
    // With crossing weight = 0, the vertical route may cross.
    // With crossing weight = 100, the vertical route should detour.
    let nodes = vec![
        node("A", 1, 1),
        node("B", 4, 1),
        node("C", 2, 0),
        node("D", 2, 2),
    ];
    let edges = vec![edge("A", "B"), edge("C", "D")];

    // Low crossing weight: may cross.
    let low_weights = CostWeights {
        crossing: 0.0,
        ..CostWeights::default()
    };
    let output_low = route_all_edges(&nodes, &edges, &config_weighted(5, 5, low_weights));
    assert_all_success(&output_low);

    // High crossing weight: should avoid crossing.
    let high_weights = CostWeights {
        crossing: 100.0,
        ..CostWeights::default()
    };
    let output_high = route_all_edges(&nodes, &edges, &config_weighted(5, 5, high_weights));
    assert_all_success(&output_high);
    let route2_high = get_route(&output_high, 1);
    assert_eq!(
        route2_high.complexity.crossings, 0,
        "High crossing weight should force a detour avoiding crossings"
    );
}

// =============================================================================
// Lane preference tests
// =============================================================================

#[test]
fn positive_lane_preferred_over_negative() {
    // When lane 0 is taken, the router should prefer lane +1 over lane -1.
    // A→B claims lane 0, then a second A→B edge should use lane +1.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B")];
    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);
    let route2 = get_route(&output, 1);
    // The second route should use lane +1 (positive preferred tie-break).
    assert_eq!(
        route2.waypoints[0].lane, 1,
        "When lane 0 is taken, positive lane +1 should be preferred over -1"
    );
}

// =============================================================================
// Multiplier / weight tests
// =============================================================================

#[test]
fn default_weights_all_one() {
    let w = CostWeights::default();
    assert_eq!(w.length, 1.0);
    assert_eq!(w.turn, 1.0);
    assert_eq!(w.lane_change, 1.0);
    assert_eq!(w.crossing, 1.0);
}

#[test]
fn weighted_complexity_total() {
    use crate::render::diagram::routing::types::RouteComplexity;

    let c = RouteComplexity {
        length: 3.0,
        turns: 2,
        lane_changes: 1,
        crossings: 4,
    };

    // Default weights: 3 + 2 + 1 + 4 = 10
    assert!((c.total(&CostWeights::default()) - 10.0).abs() < f64::EPSILON);

    // Custom weights: 2*3 + 3*2 + 0.5*1 + 10*4 = 6 + 6 + 0.5 + 40 = 52.5
    let w = CostWeights {
        length: 2.0,
        turn: 3.0,
        lane_change: 0.5,
        crossing: 10.0,
    };
    assert!((c.total(&w) - 52.5).abs() < f64::EPSILON);
}

#[test]
fn weighted_complexity_zero_weights() {
    use crate::render::diagram::routing::types::RouteComplexity;

    let c = RouteComplexity {
        length: 5.0,
        turns: 3,
        lane_changes: 2,
        crossings: 1,
    };

    let zero_weights = CostWeights {
        length: 0.0,
        turn: 0.0,
        lane_change: 0.0,
        crossing: 0.0,
    };
    assert!((c.total(&zero_weights)).abs() < f64::EPSILON);
}

#[test]
fn high_turn_weight_prefers_fewer_turns() {
    // A(1,1) -> B(2,2): L-shaped route needs 1 turn.
    // With very high turn weight, the router should still find a route
    // but prefer the one with fewest turns.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];

    // Normal weights.
    let output_normal = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output_normal);
    let route_normal = get_route(&output_normal, 0);

    // Very high turn weight.
    let high_turn = CostWeights {
        turn: 100.0,
        ..CostWeights::default()
    };
    let output_high = route_all_edges(&nodes, &edges, &config_weighted(3, 3, high_turn));
    assert_all_success(&output_high);
    let route_high = get_route(&output_high, 0);

    // Both routes need at least 1 turn for an L-shaped path.
    // With high turn weight, the router should minimize turns.
    assert!(route_high.complexity.turns <= route_normal.complexity.turns);
}

#[test]
fn high_lane_change_weight() {
    // With high lane_change weight, router should avoid lane switches.
    let nodes = vec![node("A", 1, 1), node("B", 2, 1)];
    let edges = vec![edge("A", "B"), edge("A", "B")]; // Two parallel edges

    let high_lc = CostWeights {
        lane_change: 100.0,
        ..CostWeights::default()
    };
    let output = route_all_edges(&nodes, &edges, &config_weighted(3, 3, high_lc));
    assert_all_success(&output);

    // Both routes should have 0 lane changes.
    let route1 = get_route(&output, 0);
    let route2 = get_route(&output, 1);
    assert_eq!(route1.complexity.lane_changes, 0);
    assert_eq!(route2.complexity.lane_changes, 0);
}

#[test]
fn high_length_weight_prefers_shorter() {
    // With very high length weight, the router should prefer shorter routes
    // even if they have more turns.
    let nodes = vec![node("A", 1, 1), node("B", 2, 2)];
    let edges = vec![edge("A", "B")];

    let high_length = CostWeights {
        length: 100.0,
        turn: 0.1,
        ..CostWeights::default()
    };
    let output = route_all_edges(&nodes, &edges, &config_weighted(3, 3, high_length));
    assert_all_success(&output);
    let route = get_route(&output, 0);
    // Manhattan distance is 2 for (1,1)->(2,2), so optimal length is 2.0.
    assert_eq!(route.complexity.length, 2.0);
}

// =============================================================================
// Integration tests: crossing detection in find_best_route
// =============================================================================

#[test]
fn find_best_route_with_crossings_counted() {
    // Build a simple graph, claim a horizontal route, then find a vertical route.
    // Verify crossings are counted in the result.
    let positions = vec![(1, 1), (2, 1), (1, 2), (2, 2)];
    let graph = RoutingGraph::build(&positions, 3, 3);
    let mut occ = LaneOccupancy::new();
    let weights = CostWeights::default();

    // Route A(1,1) -> B(2,1): horizontal
    let source = GridCoord::from_int(1, 1);
    let target = GridCoord::from_int(2, 1);
    let route1 = find_best_route(&graph, &occ, source, target, &weights).unwrap();
    occ.claim_route(&route1);
    assert_eq!(
        route1.complexity.crossings, 0,
        "First route has no crossings"
    );
}

#[test]
fn find_best_route_respects_crossing_weight() {
    // With crossing weight = 0, verify the router still works.
    let positions = vec![(1, 1), (3, 1)];
    let graph = RoutingGraph::build(&positions, 3, 3);
    let occ = LaneOccupancy::new();

    let weights = CostWeights {
        crossing: 0.0,
        ..CostWeights::default()
    };
    let source = GridCoord::from_int(1, 1);
    let target = GridCoord::from_int(3, 1);
    let route = find_best_route(&graph, &occ, source, target, &weights).unwrap();
    assert_eq!(route.complexity.length, 2.0);
}

// =============================================================================
// Hub-and-spoke diagram integration test
// =============================================================================

/// Comprehensive test of the hub-and-spoke diagram topology:
///
/// ```text
///          Logs(2,1)
///            |
///  Web(1,1)  |  App(3,1)
///     \      |      /
///      \     |     /
///       --- API(2,2) ---
///      /              \
///     /                \
///  Auth(1,3)         DB(3,3)
///            |
///          Mail(2,3)
/// ```
///
/// Edges (processed in order):
///   0: Web(1,1)  -> API(2,2) "requests"
///   1: App(3,1)  -> API(2,2) "requests"
///   2: API(2,2)  -> Auth(1,3) "validates"  ← must exit west on lane +1 (south)
///   3: API(2,2)  -> DB(3,3)   "queries"    ← must exit east on lane +1 (south)
///   4: API(2,2)  -> Logs(2,1) "writes"
///   5: API(2,2)  -> Mail(2,3) "sends"
///
/// Lane convention (absolute):
///   Horizontal: lane +1 = south of center, lane -1 = north of center
///   Vertical:   lane +1 = east of center,  lane -1 = west of center
///
/// Expected routes:
///   "validates" (API→Auth): exit west on lane +1 (south), turn south at (1,2).
///     Avoids crossing Web's "requests" which arrives from the north.
///     Length 2.0, 1 turn, 0 lane changes, 0 crossings.
///
///   "queries" (API→DB): exit east on lane +1 (south), turn south at (3,2).
///     Avoids crossing App's "requests" which arrives from the north.
///     Length 2.0, 1 turn, 0 lane changes, 0 crossings.
#[test]
fn hub_and_spoke_diagram_routes() {
    let nodes = vec![
        node("Web", 1, 1),
        node("App", 3, 1),
        node("API", 2, 2),
        node("Auth", 1, 3),
        node("DB", 3, 3),
        node("Logs", 2, 1),
        node("Mail", 2, 3),
    ];
    let edges = vec![
        edge_labeled("Web", "API", "requests"),
        edge_labeled("App", "API", "requests"),
        edge_labeled("API", "Auth", "validates"),
        edge_labeled("API", "DB", "queries"),
        edge_labeled("API", "Logs", "writes"),
        edge_labeled("API", "Mail", "sends"),
    ];

    let output = route_all_edges(&nodes, &edges, &config(3, 3));
    assert_all_success(&output);

    // --- "validates" route (index 2: API→Auth) ---
    let validates_route = get_route(&output, 2);

    assert_eq!(
        validates_route.complexity.length, 2.0,
        "validates route should have length 2.0 (Manhattan distance)"
    );
    assert_eq!(
        validates_route.complexity.turns, 1,
        "validates route should have exactly 1 turn (west then south)"
    );
    assert_eq!(
        validates_route.complexity.lane_changes, 0,
        "validates route should have 0 lane changes"
    );
    assert_eq!(
        validates_route.complexity.crossings, 0,
        "validates route should have 0 crossings"
    );

    // Validates must exit API on lane +1 (south of center, absolute convention).
    // This avoids crossing Web's "requests" which arrives from the north.
    assert_eq!(
        validates_route.waypoints[0].lane, 1,
        "validates should exit API on lane +1 (south of center)"
    );

    // Verify path goes through (1,2) — west from API, then south to Auth.
    let validates_coords: Vec<(f64, f64)> = validates_route
        .waypoints
        .iter()
        .map(|w| (w.coord.col_f64(), w.coord.row_f64()))
        .collect();
    assert_eq!(validates_coords.first(), Some(&(2.0, 2.0)), "starts at API");
    assert_eq!(validates_coords.last(), Some(&(1.0, 3.0)), "ends at Auth");
    assert!(
        validates_coords.contains(&(1.0, 2.0)),
        "validates should pass through (1,2) — the turn point. Got: {:?}",
        validates_coords
    );

    // --- "queries" route (index 3: API→DB) ---
    let queries_route = get_route(&output, 3);

    assert_eq!(
        queries_route.complexity.length, 2.0,
        "queries route should have length 2.0 (Manhattan distance)"
    );
    assert_eq!(
        queries_route.complexity.turns, 1,
        "queries route should have exactly 1 turn (east then south)"
    );
    assert_eq!(
        queries_route.complexity.lane_changes, 0,
        "queries route should have 0 lane changes"
    );
    assert_eq!(
        queries_route.complexity.crossings, 0,
        "queries route should have 0 crossings"
    );

    // Queries must exit API on lane +1 (south of center, absolute convention).
    assert_eq!(
        queries_route.waypoints[0].lane, 1,
        "queries should exit API on lane +1 (south of center)"
    );

    // Verify path goes through (3,2) — east from API, then south to DB.
    let queries_coords: Vec<(f64, f64)> = queries_route
        .waypoints
        .iter()
        .map(|w| (w.coord.col_f64(), w.coord.row_f64()))
        .collect();
    assert_eq!(queries_coords.first(), Some(&(2.0, 2.0)), "starts at API");
    assert_eq!(queries_coords.last(), Some(&(3.0, 3.0)), "ends at DB");
    assert!(
        queries_coords.contains(&(3.0, 2.0)),
        "queries should pass through (3,2) — the turn point. Got: {:?}",
        queries_coords
    );

    // --- All routes should have 0 crossings (no false positives) ---
    for (i, (edge_def, _)) in output.results.iter().enumerate() {
        let route = get_route(&output, i);
        let fallback = format!("{}→{}", edge_def.source, edge_def.target);
        let label = edge_def.label.as_deref().unwrap_or(&fallback);
        assert_eq!(
            route.complexity.crossings, 0,
            "Edge '{}' should have 0 crossings, got {}",
            label, route.complexity.crossings,
        );
    }
}
