---
title: "Layout Test: Diagram Slides"
@theme: dark
@transition: fade
---

# Layout Test: Diagram Slides
Focused tests for the diagram layout


# Request Flow

```@diagram
- Client -> Server: HTTP request
- Server -> Database: Query
- Database -> Server: Results
- Server -> Client: JSON response
```


# Microservices Architecture

```@diagram
# Components
- Gateway  (icon: api,      pos: 1,1)
- Auth     (icon: lock,     pos: 2,1)
- Users    (icon: user,     pos: 2,2)
- Cache    (icon: cache,    pos: 3,1)
- DB       (icon: database, pos: 3,2)

# Relationships
- Gateway -> Auth: validates
- Gateway -> Users: routes to
- Auth --> Cache: checks token
- Users -> DB: queries
```


# Simple Pipeline

```@diagram
- Source -> Build: triggers
- Build -> Test: on success
- Test -> Deploy: all green
- Deploy -> Monitor: reports to
```


# Hub and Spoke

```@diagram
# Central hub with many connections
- API  (icon: api,      pos: 2,2)
- Web  (icon: browser,  pos: 1,1)
- App  (icon: mobile,   pos: 3,1)
- Auth (icon: lock,     pos: 1,3)
- DB   (icon: database, pos: 3,3)
- Logs (icon: logs,     pos: 2,1)
- Mail (icon: mail,     pos: 2,3)

- Web -> API: requests
- App -> API: requests
- API -> Auth: validates
- API -> DB: queries
- API -> Logs: writes
- API -> Mail: sends
```


# Mixed Arrow Types

```@diagram
- Frontend -> Backend: REST calls
- Backend -> Database: queries
- Database <- Cache: fills
- Frontend <-> WebSocket: real-time
- Monitoring -- Logging: associated
- CI --> Staging: deploys
```


# Layered Architecture

```@diagram
- Browser   (icon: browser,  pos: 1,1)
- Mobile    (icon: mobile,   pos: 2,1)
- Gateway   (icon: api,      pos: 1,2)
- Auth      (icon: lock,     pos: 2,2)
- Users     (icon: user,     pos: 1,3)
- Products  (icon: container, pos: 2,3)
- Database  (icon: database, pos: 1,4)
- Cache     (icon: cache,    pos: 2,4)

- Browser -> Gateway
- Mobile -> Gateway
- Gateway -> Auth: validates
- Gateway -> Users: routes
- Gateway -> Products: routes
- Users -> Database: reads/writes
- Products -> Database: reads/writes
- Products -> Cache: caches
```


# Large System (10 nodes)

```@diagram
- Client    (icon: browser,   pos: 1,1)
- CDN       (icon: cloud,     pos: 2,1)
- LB        (icon: network,   pos: 3,1)
- Web       (icon: server,    pos: 1,2)
- API       (icon: api,       pos: 2,2)
- Worker    (icon: function,  pos: 3,2)
- DB        (icon: database,  pos: 1,3)
- Cache     (icon: cache,     pos: 2,3)
- Queue     (icon: queue,     pos: 3,3)
- Monitor   (icon: monitor,   pos: 2,4)

- Client -> CDN: static assets
- Client -> LB: API calls
- LB -> Web: serves pages
- LB -> API: routes
- API -> DB: queries
- API -> Cache: reads
- API -> Queue: publishes
- Queue -> Worker: processes
- Worker -> DB: writes
- Monitor -- API: observes
- Monitor -- Worker: observes
```


# Minimal Diagram

```@diagram
- A -> B: connects
```


# Auto-Layout (no positions)

```@diagram
- User -> Load Balancer: request
- Load Balancer -> Server 1: route
- Load Balancer -> Server 2: route
- Load Balancer -> Server 3: route
- Server 1 -> Database: query
- Server 2 -> Database: query
- Server 3 -> Database: query
```


# Reveal: Incremental Build

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


# Reveal: Pipeline Growth

```@diagram
- Source (icon: storage, pos: 1,1)
+ Build  (icon: container, pos: 2,1)
+ Source -> Build: triggers
+ Test   (icon: function, pos: 3,1)
* Build -> Test: on success
+ Deploy (icon: cloud, pos: 4,1)
* Test -> Deploy: all green
```


# Dense Routing Test

```@diagram
# 3x3 grid with many crossing connections
- A (pos: 1,1)
- B (pos: 2,1)
- C (pos: 3,1)
- D (pos: 1,2)
- E (pos: 2,2)
- F (pos: 3,2)
- G (pos: 1,3)
- H (pos: 2,3)
- I (pos: 3,3)

# Edges across the grid
- A -> E: diagonal
- C -> G: far-diagonal-1
- B -> H: vertical
- D -> F: horizontal
- A -> I: far-diagonal-2
- G -> C: far-diagonal-3
- E -> A: back
- E -> C: right
- E -> G: down-left
- E -> I: down-right
```
