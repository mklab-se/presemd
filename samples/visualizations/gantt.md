---
title: Gantt Chart Tests
@theme: dark
---

# Gantt Chart — Short Timeline (Weekend Shading)

```@gantt
- Setup: 2024-01-15, 3d
- Development: 2024-01-17, 5d
- Code Review: 2024-01-22, 2d
- Bug Fixes: 2024-01-24, 3d
```


# Gantt Chart — Basic

```@gantt
- Research: 2024-01-15, 10d
- Design: 2024-01-22, 8d
- Development: 2024-01-29, 20d
- Testing: 2024-02-12, 10d
- Launch: 2024-02-22, 3d
```


# Gantt Chart — Dependencies

```@gantt
- Planning: 2024-03-01, 5d
- Design: 8d, after Planning
- Frontend: 15d, after Design
- Backend: 15d, after Design
- Integration: 5d, after Frontend
- QA: 10d, after Integration
- Release: 2d, after QA
```


# Gantt Chart — With Delays

```@gantt
- Requirements: 2024-06-01, 10d
- Architecture: 5d, after Requirements + 2d
- Sprint 1: 10wd, after Architecture
- Sprint 2: 10wd, after Sprint 1
- Code Review: 3d, after Sprint 2 + 6wd
- Deployment: 2d, after Code Review
```


# Gantt Chart — Long Timeline

```@gantt
# title: Product Roadmap 2024
- Discovery: 2024-01-01, 2024-02-15
- MVP Development: 2024-02-01, 2024-05-31
- Beta Testing: 2024-05-01, 2024-07-15
- Marketing Prep: 2024-06-01, 2024-08-31
- Public Launch: 2024-08-15, 2024-09-15
- Post-launch Support: 2024-09-01, 2024-12-31
```


# Gantt Chart — Many Tasks

```@gantt
- Kickoff: 2024-04-01, 2d
- Requirements: 3d, after Kickoff
- UI Design: 5d, after Requirements
- API Design: 4d, after Requirements
- Database Schema: 3d, after API Design
- Auth Module: 5d, after Database Schema
- User Service: 7d, after Database Schema
- Dashboard UI: 8d, after UI Design
- Reporting: 6d, after User Service
- Testing: 5d, after Auth Module
- Documentation: 4d, after Testing
- Deployment: 2d, after Documentation
```


# Gantt Chart — Labels Inside

```@gantt
# labels: inside
- Planning: 2024-03-01, 5d
- Design: 8d, after Planning
- Frontend: 15d, after Design
- Backend: 15d, after Design
- Integration: 5d, after Frontend
- QA: 10d, after Integration
- Release: 2d, after QA
```


# Gantt Chart — Labels Inside (Many Tasks)

```@gantt
# labels: inside
- Kickoff: 2024-04-01, 2d
- Requirements: 3d, after Kickoff
- UI Design: 5d, after Requirements
- API Design: 4d, after Requirements
- Database Schema: 3d, after API Design
- Auth Module: 5d, after Database Schema
- User Service: 7d, after Database Schema
- Dashboard UI: 8d, after UI Design
- Reporting: 6d, after User Service
- Testing: 5d, after Auth Module
- Documentation: 4d, after Testing
- Deployment: 2d, after Documentation
```


# Gantt Chart — Incremental Reveal

```@gantt
- Phase 1: 2024-01-01, 15d
+ Phase 2: 10d, after Phase 1
+ Phase 3: 20d, after Phase 2
* Support: 20d, after Phase 2
+ Phase 4: 5d, after Phase 3
```
