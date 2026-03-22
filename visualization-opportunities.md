# Visualization Opportunities for MDeck

The following visualizations were identified during AI presentation generation as potentially valuable additions to mdeck. Each entry is formatted as a ready-to-submit GitHub issue.

If you find any of these valuable, please consider opening an issue at:
https://github.com/mklab-se/mdeck/issues/new

---

## 1. Feature Request: Git Flow Branch Diagram Visualization

### Context

While generating a presentation, MDeck identified that the slide "How It All Fits Together" would benefit from a **Git Flow Branch Diagram** visualization. Currently, mdeck does not support this visualization type, so an AI-generated image was used as a fallback.

### What This Visualization Shows

A branching diagram showing main, develop, feature, release, and hotfix branches and how they merge over time

### Data Structure

The visualization would display the following kind of data:

Branches as lanes, arrows showing merges, timeline progression left to right

### Suggested MDeck Syntax

A possible syntax for this visualization in mdeck markdown:

```
architecture
```

### Why This Would Be Valuable

This visualization type would help presenters communicate a branching diagram showing main, develop, feature, release, and hotfix branches and how they merge over time more effectively than bullet points or static images. It was identified by `mdeck ai create` during automated presentation generation, suggesting it's a common need when creating presentations from real-world content.

### Current Workaround

MDeck currently generates an AI image as a fallback using this prompt:

> A clean Git branching diagram showing Git Flow: two main horizontal branches labeled 'main' and 'develop'. Feature branches branch off develop and merge back. Release branches branch from develop and merge into both main and develop. Hotfix branches branch from main and merge back into both main and develop. Use color coding and arrows to show flow over time.

While this produces a reasonable visual, a native interactive visualization would be more precise, data-driven, and consistent with mdeck's other visualization types.

**Source:** Auto-detected by `mdeck ai create`
**Slide:** "How It All Fits Together"

---

## 2. Feature Request: Process Flow Diagram Visualization

### Context

While generating a presentation, MDeck identified that the slide "Example: Feature to Production" would benefit from a **Process Flow Diagram** visualization. Currently, mdeck does not support this visualization type, so an AI-generated image was used as a fallback.

### What This Visualization Shows

A step-by-step flow of a feature moving from creation to deployment

### Data Structure

The visualization would display the following kind of data:

Boxes representing steps: feature branch → develop → release → main → deploy

### Why This Would Be Valuable

This visualization type would help presenters communicate a step-by-step flow of a feature moving from creation to deployment more effectively than bullet points or static images. It was identified by `mdeck ai create` during automated presentation generation, suggesting it's a common need when creating presentations from real-world content.

### Current Workaround

MDeck currently generates an AI image as a fallback using this prompt:

> A horizontal flow diagram showing the lifecycle of a feature in Git Flow: starting with 'feature/login branch', merging into 'develop', then into 'release/1.2', then into 'main', ending with 'production deployment'. Use arrows and simple labeled boxes.

While this produces a reasonable visual, a native interactive visualization would be more precise, data-driven, and consistent with mdeck's other visualization types.

**Source:** Auto-detected by `mdeck ai create`
**Slide:** "Example: Feature to Production"

---

