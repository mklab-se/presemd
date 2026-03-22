# Visualization Opportunities for MDeck

Each section below is a self-contained feature request ready to be submitted as a GitHub issue. Copy the section you're interested in and paste it at:
https://github.com/mklab-se/mdeck/issues/new

---

## 1. Feature Request: `@workflowstatediagram` Visualization

### Summary

A directed flow diagram showing how work moves between different states (e.g., feature, develop, release, production). This helps clarify transitions and rules, which are hard to fully grasp via bullet points and too precise for AI-generated images.

### Data Model

Nodes represent states or branch types (feature, develop, release, main). Directed edges represent allowed transitions (e.g., feature → develop, develop → release). Each edge may include conditions or actions (e.g., code review, testing).

### Rendering Specification

Nodes arranged left-to-right following the lifecycle: feature → develop → release → main. Arrows clearly labeled with actions. Different colors for branch types (e.g., green for main, blue for develop, orange for release, purple for features).

### Visual Mockup

```
feature ---> develop ---> release ---> main
                 \                         ^
                  -------- hotfix ----------
```

### Proposed Syntax

````markdown
```@workflowstatediagram
- feature -> develop : merge after PR
- develop -> release : cut release branch
- release -> main : deploy
- main -> hotfix : branch for urgent fix
- hotfix -> develop : merge back
```
````

### Implementation Notes

MDeck renders visualizations from fenced code blocks with `@` language tags (e.g., `@barchart`, `@timeline`, `@architecture`). Each visualization type is implemented as a Rust rendering function in `crates/mdeck/src/render/`. The parser detects the `@` tag in `crates/mdeck/src/parser/blocks.rs` and creates a corresponding `Block` variant. Progressive reveal is supported via `+` and `*` list markers.

