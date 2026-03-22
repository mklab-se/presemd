# Visualization Opportunities for MDeck

Each section below is a self-contained feature request ready to be submitted as a GitHub issue. Copy the section you're interested in and paste it at:
https://github.com/mklab-se/mdeck/issues/new

---

## 1. Feature Request: `@branchgraph` Visualization

### Summary

A directed graph showing how branches diverge and merge over time. This is critical for understanding Git Flow because relationships between branches (feature, develop, release, main) are inherently spatial and temporal. Bullet points or static images cannot precisely convey merge paths, branch origins, and reintegration points.

### Data Model

Entities include branches (nodes/lanes), commits (points on lanes), and merges (edges between lanes). Each branch has a timeline, and commits are ordered sequentially. Merges connect commits across branches. Additional metadata includes branch type (feature, release, etc.) and timestamps.

### Rendering Specification

Horizontal timeline with parallel lanes for each branch. Commits are dots along lines; merges are curved or angled connectors between lanes. Use color coding per branch type (e.g., green=main, blue=develop, yellow=feature, red=hotfix). Labels for branch names and merge points should be clearly placed.

### Visual Mockup

```
main:    ●──────●────────────●
           \           /
develop:    ●────●────●────●
                 \        /
feature:          ●────●

(merges shown as diagonal connections)
```

### Proposed Syntax

````markdown
```@branchgraph
- branch: main
  commits: m1, m2
- branch: develop
  commits: d1, d2, d3
- branch: feature/login
  commits: f1, f2
- merge: feature/login -> develop at d3
- merge: develop -> main at m2
```
````

### Implementation Notes

MDeck renders visualizations from fenced code blocks with `@` language tags (e.g., `@barchart`, `@timeline`, `@architecture`). Each visualization type is implemented as a Rust rendering function in `crates/mdeck/src/render/`. The parser detects the `@` tag in `crates/mdeck/src/parser/blocks.rs` and creates a corresponding `Block` variant. Progressive reveal is supported via `+` and `*` list markers.

