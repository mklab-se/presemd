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
