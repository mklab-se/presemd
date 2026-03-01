# Architecture Diagrams

This page explains both the syntax and semantics of Architecture Diagrams in MDeck, and provides some examples to get you started. Architecture Diagrams are a powerful way to visualize the structure and relationships of your system or organization, and can help you communicate your design decisions to others.

The documentation also serves as an internal reference for how the routing and layout algorithms work, so that you can understand how to control the layout of your diagrams.

## Diagram Syntax

Architecture diagrams are written inside a fenced code block with the `@diagram` language tag:

````markdown
```@diagram
- User -> Server: Sends request
- Server -> Database: Queries data
```
````

### Block structure

A diagram block contains two kinds of lines: **component declarations** and **relationship lines**. Lines starting with `#` are comments/section headers and are ignored by the parser.

### Components

```
- Name (key: value, key: value)
```

| Key     | Values                          | Default       | Description           |
|---------|---------------------------------|---------------|-----------------------|
| `icon`  | icon name from theme icon set   | `box`         | Visual icon           |
| `pos`   | `col,row` (integer grid coords) | auto-layout   | Position on the grid  |
| `label` | string                          | component name| Display label         |
| `style` | `primary`, `secondary`, `muted` | `primary`     | Visual emphasis       |

If no components are explicitly declared, they are inferred from relationship lines. Each unique name becomes a component with default icon and auto-positioned layout.

Example with explicit positions and icons:

````markdown
```@diagram
- Gateway  (icon: api,      pos: 1,1)
- Auth     (icon: lock,     pos: 2,1)
- Users    (icon: user,     pos: 2,2)
- Cache    (icon: cache,    pos: 3,1)
- DB       (icon: database, pos: 3,2)
```
````

### Relationships

```
- Source -> Target: Label
```

The text after `:` is the edge label. If no `:` is present, the relationship has no label.

Arrow types:

| Arrow   | Meaning                    |
|---------|----------------------------|
| `->`    | Solid arrow (directed)     |
| `<-`    | Reverse solid arrow        |
| `<->`   | Bidirectional solid arrow  |
| `--`    | Dashed line (undirected)   |
| `-->`   | Dashed arrow (directed)    |

### Reveal markers

Each component or relationship line starts with a marker that controls when it appears during presentation:

| Marker | Meaning                                          |
|--------|--------------------------------------------------|
| `-`    | Visible from the start                           |
| `+`    | Appears on the next forward press (new step)     |
| `*`    | Appears together with the preceding `+` (grouped)|

Example — an incremental build reveal:

````markdown
```@diagram
# Base infrastructure always visible
- Server (icon: server, pos: 1,1)
- DB     (icon: database, pos: 2,1)
- Server -> DB: queries

# Step 1: Add cache layer
+ Cache (icon: cache, pos: 1,2)
+ Server -> Cache: reads
+ Cache -> DB: fills

# Step 2: Add monitoring
+ Monitor (icon: monitor, pos: 2,2)
* Monitor -- Server: observes
* Monitor -- DB: observes
```
````

### Auto-layout

When no `pos` is specified on any component, the system infers positions automatically: linear chains are laid out left-to-right, and hierarchical structures use a tree layout.

````markdown
```@diagram
- User -> Load Balancer: request
- Load Balancer -> Server 1: route
- Load Balancer -> Server 2: route
- Load Balancer -> Server 3: route
- Server 1 -> Database: query
- Server 2 -> Database: query
- Server 3 -> Database: query
```
````

### Built-in icons

The built-in themes provide these icon names:

`user`, `server`, `database`, `cloud`, `browser`, `mobile`, `api`, `queue`, `cache`, `storage`, `function`, `container`, `network`, `lock`, `key`, `mail`, `logs`, `monitor`, `box`

An unrecognized icon name falls back to `box`.

## Diagram Grid

The diagram grid is a conceptual grid that helps in positioning elements within the diagram. Each element can be placed at specific grid coordinates, which allows for precise control over the layout and alignment of the diagram components. The grid is by  columns and rows (represented in that order, think X, Y), starting from (1,1) at the top-left corner. For example, an element positioned at (3,2) would be located in the third column and second row of the grid. This system enables you to create organized and visually appealing diagrams by strategically placing elements based on their relationships and importance.

Some details and examples about the grid:

- A single element will be centered when rendered.
- If A is located at (1,1) and B is located at (2,1), then A will be to the left of B.
- If A is located at (1,1) and B is located at (1,2), then A will be above B.
- If A is located at (1,1), B is located at (2,1), C is located at (1,2), and D is located at (2,2), then A will be to the left of B, above C, and diagonally above-left of D.
- A diagram where A is located at (1,1) and B is located at (3,3) will have a lot of empty space in the middle, but A will be to the left and above B.
- Shifting all elements to the right by 1 column and down by 1 row will not change their relative positions and will not change how the diagram looks, as the system will automatically adjust the grid to fit all elements. So A at (1,1) and B at (2,1) will look the same as A at (2,2) and B at (3,2).

## Grid Internals

The system thinks of the grid as a city where there are streets running horizontally and vertically. Those streets run between the grid coordinates, so the intersections of the streets are at (1.5, 1.5), (2.5, 1.5), (1.5, 2.5), etc.

Every cell — whether occupied by an element or empty — has internal roads running through it: one horizontal (from the street on the left to the street on the right) and one vertical (from the street above to the street below). These internal roads cross at the center of the cell.

Streets and internal roads together form a **routing graph**. The nodes of this graph are every point where two perpendicular road segments meet. There are three types of nodes:

| Node type              | Coordinates          | Example    | Description |
|------------------------|----------------------|------------|-------------|
| **Cell center**        | (c, r)               | (1, 1)     | Where a cell's horizontal and vertical internal roads cross |
| **Junction**           | (c±0.5, r) or (c, r±0.5) | (1.5, 1) | Where a cell's internal road meets a street |
| **Street intersection**| (c+0.5, r+0.5)      | (1.5, 1.5) | Where a horizontal street meets a vertical street |

The road segments between adjacent nodes are the edges of the routing graph. Each segment has lanes (see below).

### Boundary streets

Streets exist on the outer edges of the grid. For a grid spanning columns 1–3 and rows 1–2, the vertical streets are at x = 0.5, 1.5, 2.5, and 3.5, and the horizontal streets are at y = 0.5, 1.5, and 2.5. This means edges can route around the outside of the grid. This often results in a longer route than going through the interior, but it is available as an option and can be useful when interior streets are congested.

### Lanes

Streets and internal roads have lanes. Only one edge can occupy a lane at a time on a given segment, so if multiple edges need to use the same segment, they will be routed in parallel lanes. The number of lanes on a segment is not fixed — it is determined by the physical space available. For example, a 3x3 grid stretched to a 16:9 aspect ratio will have more horizontal space than vertical space, so the vertical streets (which run in the wider dimension) will have more lanes than the horizontal streets. (The aspect ratio is an input parameter handled automatically by the system.)

Lanes are numbered from the center outwards: lane 0 is the center, lane 1 is the first lane to the right/down, lane -1 is the first lane to the left/up, lane 2 is the second to the right/down, lane -2 is the second to the left/up, etc. So if 3 edges need to use the same segment, they will be routed in lanes -1, 0, and 1.

### Turns and lane changes

A **turn** is any change of direction from horizontal to vertical or vice versa. Turns and lane changes can only occur at nodes in the routing graph — that is, at cell centers, junctions, or street intersections. An edge traveling along a straight road segment between two nodes cannot turn or change lanes mid-segment.

### Occupied vs. empty cells

If a cell is **empty**, its internal roads function just like streets: edges can route through them freely, and the cell center acts as a node where edges can turn or change lanes.

If a cell is **occupied** by an element, no edges may pass through it — only edges that originate from or terminate at that element may use its internal roads.

There are no diagonal shortcuts. All routing is strictly along horizontal and vertical streets and internal roads.

### Entering and exiting elements

Entering or exiting an element uses the same internal-road model. An edge exits the source element's center along one of the four cardinal directions, traveling through the element's internal road to the adjacent junction, and from there onto a street. Similarly, an edge arrives at the target element by traveling from a street through a junction and along the element's internal road to its center.

If A is at (1,1) and B is at (2,1), the edge from A to B can exit A to the right and enter B from the left, or it can exit A downwards and enter B from the top. The system will choose the direction that results in the lowest-complexity path. Entry and exit also use lanes, so if there are multiple edges entering or exiting the same element from the same direction, they will be routed in parallel lanes.

## Routing

When routing edges between elements, the system will try to find the path with the lowest complexity, where complexity is defined as **length + turns + lane changes** (a lane change that occurs simultaneously with a turn is free and not counted).

**Length** is the mathematical distance along the route, computed as the sum of the absolute coordinate differences between consecutive waypoints. For example, a segment from (1.5, 1) to (1.5, 2) has length |2 - 1| = 1. Length is measured in grid units and does not account for any physical stretching due to aspect ratio — it is purely based on the grid coordinates.

An edge starts at the center of the source element, and ends at the center of the target element. The system will use an efficient graph search algorithm (e.g., A* or Dijkstra with complexity as the cost metric), parallelized across threads or other concurrency mechanisms available in Rust, to find the lowest-complexity route. If there are multiple routes with the same complexity, the system may choose any of them.

An edge always starts at the center of the source element and steps out along an internal road in one of the four cardinal directions to reach an adjacent junction. The edge then follows streets and (where allowed) internal roads of empty cells, and can only turn or change lanes at nodes in the routing graph (cell centers, junctions, or street intersections). When the edge turns, it may simultaneously change to a different lane — this combined turn-and-lane-change is free. A lane change without a turn costs 1. The edge continues until it reaches the target element, where it steps from a junction through the target's internal road to its center.

### Route failure

If the system cannot find any valid route for an edge (all lanes on required streets are occupied, or the path is completely blocked), the route fails. The diagram will still render, but a warning message will be displayed indicating which edge could not be routed (showing the source and target names). The unroutable edge will not be drawn.

## Routing Language

The system uses an internal language to describe the routing of edges, which is not exposed to the user, but can be useful for understanding how the system works. The language uses the following syntax:

### Example 1

(1,1)-L0-(1.5, 1)-L1-(1.5, 2)-L0-(2,2)

This describes an edge that starts at cell center (1,1), steps out to the right using lane 0 to junction (1.5, 1), then turns downwards using lane 1 to junction (1.5, 2), then arrives at cell center (2,2) using lane 0. The turn at junction (1.5, 1) changes direction from horizontal to vertical, and the turn at junction (1.5, 2) changes from vertical to horizontal. The lane change from L0 to L1 happens simultaneously with the first turn, so it is free. The edge has a length of 2 and 2 turns. Total complexity: 2 + 2 = 4.

### Example 2

(1,1)-L0-(1, 1.5)-L0-(2, 1.5)-L1-(3, 1.5)-L0-(3, 1)

This describes an edge that starts at (1,1), steps out downwards using lane 0 to junction (1, 1.5), then turns right using lane 0 to junction (2, 1.5), then changes lane (without turning) and continues to the right using lane 1 to junction (3, 1.5), then arrives by going upwards into the node at (3, 1) using the center lane, lane 0. The lane change at (2, 1.5) occurs at a junction node (where the internal road of cell (2,1) or (2,2) meets the horizontal street), which is a valid place to change lanes. The edge has a length of 3, has 2 turns and 1 lane change. The total complexity of the edge is 3+2+1 = 6.

There will always be more than one possible route between two elements, and the system will choose the one with the lowest complexity, where complexity is defined as the length of the edge plus the number of turns plus the number of lane changes. As you saw above, a lane change that happens while also turning, isn't counted. So if there is a route with a length of 2, 2 turns and 0 lane changes (complexity 4), and another route with a length of 3, 2 turns and 1 lane change (complexity 6), the system will choose the first route.

The route search for a single edge is parallelized across threads, and the least complex result is selected. However, edges are processed sequentially in the order they are defined in the diagram. Earlier edges have higher priority: once an edge claims lanes on certain streets, later edges must route around them. This means changing the order of edges in the diagram can change the routing, and the first-defined edge will always get the optimal route.

The routing language is used internally and for debugging purposes, but it's not exposed to the user, as it can be quite complex and is not meant to be used directly by the user. However, understanding how the routing works can help you understand how to control the layout of your diagrams, and can also help you debug any issues with the routing of your edges.

## Rendering

The rendering of the diagram is done by a separate system that takes the elements and edges, along with their positions and routes, and renders them on the screen. The rendering system will take into account the aspect ratio of the diagram, the size of the elements, and the routing of the edges to create a visually appealing diagram. The rendering system will also handle things like edge crossings, edge bundling, and edge labeling to make the diagram easier to read and understand.