# Visualization Opportunities for MDeck

The following visualizations were identified during AI presentation generation as potentially valuable additions to mdeck. Each entry is formatted as a ready-to-submit GitHub issue.

If you find any of these valuable, please consider opening an issue at:
https://github.com/mklab-se/mdeck/issues/new

---

## 1. Feature Request: Git Flow Branch Diagram Visualization

### Context

MDeck is a markdown-based presentation tool that supports built-in visualizations (bar charts, timelines, architecture diagrams, etc.) rendered directly from text in fenced code blocks. During AI-powered presentation generation, a need was identified for a **Git Flow Branch Diagram** visualization that mdeck does not currently support. An AI-generated image was used as a fallback, but a native visualization would provide better precision, interactivity, and consistency with mdeck's other visualization types.

### What This Visualization Shows

A precise branching diagram showing main, develop, feature, release, and hotfix branches with their relationships and merge paths. This is critical for understanding Git Flow, as spatial relationships and timing matter.

### Data Structure

The visualization would display the following kind of data:

Branches (main, develop, feature/*, release/*, hotfix/*) and arrows indicating creation and merge directions over time.

### Suggested MDeck Syntax

A possible syntax for this visualization in mdeck markdown:

```
gitgraph { main -> develop; develop -> feature/*; develop -> release/* -> main; main -> hotfix/* -> develop }
```

### Why This Would Be Valuable

This visualization type would help presenters communicate a precise branching diagram showing main, develop, feature, release, and hotfix branches with their relationships and merge paths. this is critical for understanding git flow, as spatial relationships and timing matter. more effectively than bullet points or static images. It was identified by `mdeck ai create` during automated presentation generation, suggesting it's a common need when creating presentations from real-world content.

### Current Workaround

MDeck currently generates an AI image as a fallback using this prompt:

> A clean Git Flow diagram showing horizontal timelines for main and develop branches, with feature branches branching off develop and merging back, release branches branching from develop and merging into main and develop, and hotfix branches branching from main and merging back into both main and develop. Use clear labels and arrows.

While this produces a reasonable visual, a native interactive visualization would be more precise, data-driven, and consistent with mdeck's other visualization types.

---

## 2. Feature Request: Branch Lifecycle Flow Visualization

### Context

MDeck is a markdown-based presentation tool that supports built-in visualizations (bar charts, timelines, architecture diagrams, etc.) rendered directly from text in fenced code blocks. During AI-powered presentation generation, a need was identified for a **Branch Lifecycle Flow** visualization that mdeck does not currently support. An AI-generated image was used as a fallback, but a native visualization would provide better precision, interactivity, and consistency with mdeck's other visualization types.

### What This Visualization Shows

A step-by-step flow of how code moves from feature development to production, including release stabilization and hotfix paths. This requires precise sequencing that generic images often fail to capture clearly.

### Data Structure

The visualization would display the following kind of data:

Sequence of states: feature branch → develop → release branch → main, plus hotfix loop from main back to develop.

### Suggested MDeck Syntax

A possible syntax for this visualization in mdeck markdown:

```
flow { feature -> develop -> release -> main; main -> hotfix -> main; hotfix -> develop }
```

### Why This Would Be Valuable

This visualization type would help presenters communicate a step-by-step flow of how code moves from feature development to production, including release stabilization and hotfix paths. this requires precise sequencing that generic images often fail to capture clearly. more effectively than bullet points or static images. It was identified by `mdeck ai create` during automated presentation generation, suggesting it's a common need when creating presentations from real-world content.

### Current Workaround

MDeck currently generates an AI image as a fallback using this prompt:

> A flow diagram illustrating Git Flow lifecycle: feature branches merging into develop, then a release branch leading to main, and hotfix branches starting from main and merging back into both main and develop. Use arrows and clear stage labels.

While this produces a reasonable visual, a native interactive visualization would be more precise, data-driven, and consistent with mdeck's other visualization types.

---

