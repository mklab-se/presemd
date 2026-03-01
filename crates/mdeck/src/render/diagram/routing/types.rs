use std::cmp::Ordering;
use std::fmt;

/// Doubled-integer grid coordinate.
///
/// All coordinates are stored as `col*2, row*2` internally. The point (1.5, 2) becomes (3, 4).
/// This avoids all floating-point comparison/hashing issues and makes Eq, Hash, Ord exact.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridCoord {
    pub col2: i32,
    pub row2: i32,
}

impl GridCoord {
    /// Create from actual grid coordinates (col, row) which may be half-integers.
    /// E.g., `from_grid(1.5, 2.0)` → `GridCoord { col2: 3, row2: 4 }`.
    pub fn from_grid(col: f64, row: f64) -> Self {
        Self {
            col2: (col * 2.0).round() as i32,
            row2: (row * 2.0).round() as i32,
        }
    }

    /// Create from integer grid coordinates.
    pub fn from_int(col: i32, row: i32) -> Self {
        Self {
            col2: col * 2,
            row2: row * 2,
        }
    }

    /// The actual column as a float.
    pub fn col_f64(self) -> f64 {
        self.col2 as f64 / 2.0
    }

    /// The actual row as a float.
    pub fn row_f64(self) -> f64 {
        self.row2 as f64 / 2.0
    }

    /// Step one unit in the given direction. In doubled coords, one step = +/-1 in col2 or row2.
    pub fn step(self, dir: Direction) -> Self {
        match dir {
            Direction::North => Self {
                col2: self.col2,
                row2: self.row2 - 1,
            },
            Direction::South => Self {
                col2: self.col2,
                row2: self.row2 + 1,
            },
            Direction::East => Self {
                col2: self.col2 + 1,
                row2: self.row2,
            },
            Direction::West => Self {
                col2: self.col2 - 1,
                row2: self.row2,
            },
        }
    }

    /// Manhattan distance to another coordinate in doubled-integer space.
    pub fn manhattan_to(self, other: GridCoord) -> i32 {
        (self.col2 - other.col2).abs() + (self.row2 - other.row2).abs()
    }

    /// Whether this coordinate represents a cell center (both col2 and row2 are even).
    pub fn is_cell_center(self) -> bool {
        self.col2 % 2 == 0 && self.row2 % 2 == 0
    }

    /// Whether this is a street intersection (both col2 and row2 are odd).
    pub fn is_street_intersection(self) -> bool {
        self.col2 % 2 != 0 && self.row2 % 2 != 0
    }

    /// Whether this is a junction (one of col2/row2 is odd, the other is even).
    pub fn is_junction(self) -> bool {
        (self.col2 % 2 != 0) != (self.row2 % 2 != 0)
    }
}

impl Ord for GridCoord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.row2.cmp(&other.row2).then(self.col2.cmp(&other.col2))
    }
}

impl PartialOrd for GridCoord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for GridCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.col_f64(), self.row_f64())
    }
}

impl fmt::Display for GridCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let col = self.col_f64();
        let row = self.row_f64();
        if col.fract() == 0.0 && row.fract() == 0.0 {
            write!(f, "({},{})", col as i32, row as i32)
        } else if col.fract() == 0.0 {
            write!(f, "({},{})", col as i32, row)
        } else if row.fract() == 0.0 {
            write!(f, "({},{})", col, row as i32)
        } else {
            write!(f, "({},{})", col, row)
        }
    }
}

/// Cardinal direction for travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    /// All four directions.
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    /// Whether this direction is horizontal (East/West).
    pub fn is_horizontal(self) -> bool {
        matches!(self, Direction::East | Direction::West)
    }

    /// Whether this direction is vertical (North/South).
    pub fn is_vertical(self) -> bool {
        !self.is_horizontal()
    }

    /// Whether a transition from `self` to `other` constitutes a turn.
    pub fn is_turn(self, other: Direction) -> bool {
        self.is_horizontal() != other.is_horizontal()
    }

    /// The opposite direction.
    pub fn opposite(self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }
}

/// Canonical segment identifier between two adjacent nodes.
/// `from` is always the lesser coordinate to ensure canonical form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentId {
    pub from: GridCoord,
    pub to: GridCoord,
}

impl SegmentId {
    /// Create a canonical segment ID. The lesser coordinate goes first.
    pub fn new(a: GridCoord, b: GridCoord) -> Self {
        if a <= b {
            Self { from: a, to: b }
        } else {
            Self { from: b, to: a }
        }
    }

    /// Whether this segment is horizontal.
    pub fn is_horizontal(&self) -> bool {
        self.from.row2 == self.to.row2
    }

    /// Whether this segment is vertical.
    pub fn is_vertical(&self) -> bool {
        self.from.col2 == self.to.col2
    }
}

/// Lane number. 0 is center, positive goes right/down, negative goes left/up.
pub type Lane = i32;

/// A waypoint in a route: a coordinate and the lane used on the segment *after* this waypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Waypoint {
    pub coord: GridCoord,
    pub lane: Lane,
}

/// Complexity metrics for a route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteComplexity {
    /// Total path length in grid units.
    pub length: f64,
    /// Number of turns (horizontal↔vertical direction changes).
    pub turns: u32,
    /// Number of lane changes (excluding those simultaneous with a turn).
    pub lane_changes: u32,
}

impl RouteComplexity {
    /// Total complexity score: length + turns + lane_changes.
    pub fn total(&self) -> f64 {
        self.length + self.turns as f64 + self.lane_changes as f64
    }
}

impl Eq for RouteComplexity {}

impl Ord for RouteComplexity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.total()
            .partial_cmp(&other.total())
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                self.length
                    .partial_cmp(&other.length)
                    .unwrap_or(Ordering::Equal)
            })
            .then(self.turns.cmp(&other.turns))
            .then(self.lane_changes.cmp(&other.lane_changes))
    }
}

impl PartialOrd for RouteComplexity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A complete route from source to target.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Ordered waypoints from source center to target center.
    /// The lane on each waypoint refers to the lane of the *next* segment (after this waypoint).
    /// The last waypoint's lane is unused (sentinel).
    pub waypoints: Vec<Waypoint>,
    pub complexity: RouteComplexity,
}

/// A node in the diagram with a name and grid position.
#[derive(Debug, Clone)]
pub struct DiagramNode {
    pub name: String,
    pub col: i32,
    pub row: i32,
}

/// An edge in the diagram connecting two nodes by name.
#[derive(Debug, Clone)]
pub struct DiagramEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
}

/// Configuration for the routing engine.
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Number of lanes available on horizontal segments.
    pub h_lane_capacity: i32,
    /// Number of lanes available on vertical segments.
    pub v_lane_capacity: i32,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            h_lane_capacity: 3,
            v_lane_capacity: 3,
        }
    }
}

/// Result for routing a single edge.
#[derive(Debug, Clone)]
pub enum RouteResult {
    /// Successfully found a route.
    Success(Route),
    /// Could not find a route.
    Failure { warning: String },
}

/// Output of routing all edges in a diagram.
#[derive(Debug, Clone)]
pub struct RoutingOutput {
    pub results: Vec<(DiagramEdge, RouteResult)>,
}
