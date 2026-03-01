use super::types::{GridCoord, Lane, Route, RouteComplexity, Waypoint};

/// Serialize a route to the routing language format.
///
/// Format: `(1,1)-L0-(1.5,1)-L1-(1.5,2)-L0-(2,2)`
///
/// Half-integer coordinates are displayed as decimals (e.g., 1.5),
/// integer coordinates without decimals (e.g., 1).
pub fn route_to_string(route: &Route) -> String {
    let mut parts = Vec::new();
    for (i, wp) in route.waypoints.iter().enumerate() {
        parts.push(format_coord(wp.coord));
        if i + 1 < route.waypoints.len() {
            parts.push(format!("L{}", wp.lane));
        }
    }
    parts.join("-")
}

/// Format a GridCoord for the routing language.
fn format_coord(coord: GridCoord) -> String {
    let col = coord.col_f64();
    let row = coord.row_f64();
    if col.fract() == 0.0 && row.fract() == 0.0 {
        format!("({},{})", col as i32, row as i32)
    } else if col.fract() == 0.0 {
        format!("({},{})", col as i32, row)
    } else if row.fract() == 0.0 {
        format!("({},{})", col, row as i32)
    } else {
        format!("({},{})", col, row)
    }
}

/// Parse a route from the routing language format.
///
/// Format: `(1,1)-L0-(1.5,1)-L1-(1.5,2)-L0-(2,2)`
///
/// Returns `None` if the string is malformed.
pub fn string_to_route(s: &str) -> Option<Route> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Split by `-` but be careful: negative lane numbers like L-1 contain a `-`.
    // Strategy: tokenize by finding `(...)` groups and `L...` groups.
    let tokens = tokenize(s)?;

    if tokens.is_empty() {
        return None;
    }

    // Tokens alternate: Coord, Lane, Coord, Lane, ..., Coord
    // So: odd count of tokens, coords at even indices, lanes at odd indices.
    if tokens.len() % 2 == 0 {
        return None; // Must end with a coord.
    }

    let mut waypoints = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if i % 2 == 0 {
            // Coordinate token.
            let coord = parse_coord(token)?;
            let lane = if i + 1 < tokens.len() {
                parse_lane(&tokens[i + 1])?
            } else {
                0 // Last waypoint, lane is unused.
            };
            waypoints.push(Waypoint { coord, lane });
        }
        // Odd indices are lane tokens, handled above.
    }

    if waypoints.len() < 2 {
        return None;
    }

    let complexity = compute_complexity_from_waypoints(&waypoints);
    Some(Route {
        waypoints,
        complexity,
    })
}

/// Tokenize a routing language string into alternating coord and lane tokens.
fn tokenize(s: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();

    while chars.peek().is_some() {
        // Skip leading dashes (separators).
        while chars.peek() == Some(&'-') {
            chars.next();
        }

        if chars.peek().is_none() {
            break;
        }

        if chars.peek() == Some(&'(') {
            // Parse coordinate: (...)
            let mut token = String::new();
            while let Some(&ch) = chars.peek() {
                token.push(ch);
                chars.next();
                if ch == ')' {
                    break;
                }
            }
            tokens.push(token);
        } else if chars.peek() == Some(&'L') {
            // Parse lane: L followed by optional minus and digits.
            let mut token = String::new();
            token.push('L');
            chars.next();
            // Optional minus sign.
            if chars.peek() == Some(&'-') {
                token.push('-');
                chars.next();
            }
            // Digits.
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    token.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(token);
        } else {
            // Unexpected character.
            return None;
        }
    }

    Some(tokens)
}

/// Parse a coordinate token like `(1,2)` or `(1.5,2)`.
fn parse_coord(s: &str) -> Option<GridCoord> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let col: f64 = parts[0].trim().parse().ok()?;
    let row: f64 = parts[1].trim().parse().ok()?;
    Some(GridCoord::from_grid(col, row))
}

/// Parse a lane token like `L0`, `L1`, `L-1`.
fn parse_lane(s: &str) -> Option<Lane> {
    let s = s.trim();
    if !s.starts_with('L') {
        return None;
    }
    s[1..].parse().ok()
}

/// Compute route complexity from waypoints (for parsed routes).
fn compute_complexity_from_waypoints(waypoints: &[Waypoint]) -> RouteComplexity {
    super::lanes::compute_complexity(waypoints)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_format_coord_integer() {
        let c = GridCoord::from_int(1, 2);
        assert_eq!(format_coord(c), "(1,2)");
    }

    #[test]
    fn test_format_coord_half_col() {
        let c = GridCoord::from_grid(1.5, 2.0);
        assert_eq!(format_coord(c), "(1.5,2)");
    }

    #[test]
    fn test_format_coord_half_both() {
        let c = GridCoord::from_grid(1.5, 2.5);
        assert_eq!(format_coord(c), "(1.5,2.5)");
    }

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("(1,1)-L0-(2,1)").unwrap();
        assert_eq!(tokens, vec!["(1,1)", "L0", "(2,1)"]);
    }

    #[test]
    fn test_tokenize_negative_lane() {
        let tokens = tokenize("(1,1)-L-1-(2,1)").unwrap();
        assert_eq!(tokens, vec!["(1,1)", "L-1", "(2,1)"]);
    }

    #[test]
    fn test_parse_coord_integer() {
        let c = parse_coord("(3,2)").unwrap();
        assert_eq!(c, GridCoord::from_int(3, 2));
    }

    #[test]
    fn test_parse_coord_half() {
        let c = parse_coord("(1.5,2)").unwrap();
        assert_eq!(c, GridCoord::from_grid(1.5, 2.0));
    }

    #[test]
    fn test_parse_lane() {
        assert_eq!(parse_lane("L0"), Some(0));
        assert_eq!(parse_lane("L1"), Some(1));
        assert_eq!(parse_lane("L-2"), Some(-2));
    }
}
