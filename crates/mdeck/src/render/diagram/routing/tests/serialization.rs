use crate::render::diagram::routing::serialize::{route_to_string, string_to_route};
use crate::render::diagram::routing::types::{GridCoord, Route, RouteComplexity, Waypoint};

fn make_route(waypoints: Vec<(f64, f64, i32)>) -> Route {
    let wps: Vec<Waypoint> = waypoints
        .into_iter()
        .map(|(col, row, lane)| Waypoint {
            coord: GridCoord::from_grid(col, row),
            lane,
        })
        .collect();
    let complexity = crate::render::diagram::routing::lanes::compute_complexity(&wps);
    Route {
        waypoints: wps,
        complexity,
    }
}

#[test]
fn serialize_simple_horizontal() {
    let route = make_route(vec![(1.0, 1.0, 0), (1.5, 1.0, 0), (2.0, 1.0, 0)]);
    let s = route_to_string(&route);
    assert_eq!(s, "(1,1)-L0-(1.5,1)-L0-(2,1)");
}

#[test]
fn serialize_spec_example_1() {
    // (1,1)-L0-(1.5, 1)-L1-(1.5, 2)-L0-(2,2)
    let route = make_route(vec![
        (1.0, 1.0, 0),
        (1.5, 1.0, 1),
        (1.5, 2.0, 0),
        (2.0, 2.0, 0),
    ]);
    let s = route_to_string(&route);
    assert_eq!(s, "(1,1)-L0-(1.5,1)-L1-(1.5,2)-L0-(2,2)");
}

#[test]
fn serialize_spec_example_2() {
    // (1,1)-L0-(1, 1.5)-L0-(2, 1.5)-L1-(3, 1.5)-L0-(3, 1)
    let route = make_route(vec![
        (1.0, 1.0, 0),
        (1.0, 1.5, 0),
        (2.0, 1.5, 1),
        (3.0, 1.5, 0),
        (3.0, 1.0, 0),
    ]);
    let s = route_to_string(&route);
    assert_eq!(s, "(1,1)-L0-(1,1.5)-L0-(2,1.5)-L1-(3,1.5)-L0-(3,1)");
}

#[test]
fn serialize_negative_lane() {
    let route = make_route(vec![(1.0, 1.0, -1), (1.5, 1.0, 0), (2.0, 1.0, 0)]);
    let s = route_to_string(&route);
    assert_eq!(s, "(1,1)-L-1-(1.5,1)-L0-(2,1)");
}

#[test]
fn parse_simple_horizontal() {
    let route = string_to_route("(1,1)-L0-(1.5,1)-L0-(2,1)").unwrap();
    assert_eq!(route.waypoints.len(), 3);
    assert_eq!(route.waypoints[0].coord, GridCoord::from_int(1, 1));
    assert_eq!(route.waypoints[1].coord, GridCoord::from_grid(1.5, 1.0));
    assert_eq!(route.waypoints[2].coord, GridCoord::from_int(2, 1));
    assert_eq!(route.waypoints[0].lane, 0);
    assert_eq!(route.waypoints[1].lane, 0);
}

#[test]
fn parse_spec_example_1() {
    let route = string_to_route("(1,1)-L0-(1.5,1)-L1-(1.5,2)-L0-(2,2)").unwrap();
    assert_eq!(route.waypoints.len(), 4);
    assert_eq!(route.waypoints[0].lane, 0);
    assert_eq!(route.waypoints[1].lane, 1);
    assert_eq!(route.waypoints[2].lane, 0);
}

#[test]
fn parse_negative_lane() {
    let route = string_to_route("(1,1)-L-1-(1.5,1)-L0-(2,1)").unwrap();
    assert_eq!(route.waypoints[0].lane, -1);
    assert_eq!(route.waypoints[1].lane, 0);
}

#[test]
fn roundtrip_simple() {
    let original = make_route(vec![(1.0, 1.0, 0), (1.5, 1.0, 0), (2.0, 1.0, 0)]);
    let s = route_to_string(&original);
    let parsed = string_to_route(&s).unwrap();
    assert_eq!(original.waypoints.len(), parsed.waypoints.len());
    for (a, b) in original.waypoints.iter().zip(parsed.waypoints.iter()) {
        assert_eq!(a.coord, b.coord);
        assert_eq!(a.lane, b.lane);
    }
}

#[test]
fn roundtrip_complex() {
    let original = make_route(vec![
        (1.0, 1.0, 0),
        (1.0, 1.5, -1),
        (2.0, 1.5, 2),
        (3.0, 1.5, 0),
        (3.0, 1.0, 0),
    ]);
    let s = route_to_string(&original);
    let parsed = string_to_route(&s).unwrap();
    for (a, b) in original.waypoints.iter().zip(parsed.waypoints.iter()) {
        assert_eq!(a.coord, b.coord);
        assert_eq!(a.lane, b.lane);
    }
}

#[test]
fn parse_empty_string() {
    assert!(string_to_route("").is_none());
}

#[test]
fn parse_whitespace_only() {
    assert!(string_to_route("   ").is_none());
}

#[test]
fn parse_invalid_no_coords() {
    assert!(string_to_route("L0-L1").is_none());
}

#[test]
fn parse_single_coord() {
    // A single coordinate is not a valid route (needs at least 2 waypoints).
    assert!(string_to_route("(1,1)").is_none());
}

#[test]
fn parse_missing_lane() {
    // Two coords without a lane between them.
    assert!(string_to_route("(1,1)-(2,1)").is_none());
}

#[test]
fn serialize_large_lane_numbers() {
    let route = make_route(vec![(1.0, 1.0, 5), (1.5, 1.0, -3), (2.0, 1.0, 0)]);
    let s = route_to_string(&route);
    assert_eq!(s, "(1,1)-L5-(1.5,1)-L-3-(2,1)");
    // Roundtrip.
    let parsed = string_to_route(&s).unwrap();
    assert_eq!(parsed.waypoints[0].lane, 5);
    assert_eq!(parsed.waypoints[1].lane, -3);
}

#[test]
fn complexity_computed_on_parse() {
    let route = string_to_route("(1,1)-L0-(1.5,1)-L1-(1.5,2)-L0-(2,2)").unwrap();
    assert_eq!(route.complexity.length, 2.0);
    assert_eq!(route.complexity.turns, 2);
    // Lane change from L0 to L1 is simultaneous with turn, so free.
    assert_eq!(route.complexity.lane_changes, 0);
}
