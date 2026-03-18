# MDeck Gallery

A visual showcase of what you can create with MDeck — layouts, charts, diagrams, and more. Everything below was generated from a single markdown file using `mdeck export`.

> **Source:** [`samples/gallery.md`](samples/gallery.md)
>
> **Try it yourself:** `mdeck samples/gallery.md`

---

## Layouts

MDeck automatically infers the right layout from your content structure. No configuration needed.

### Title Slide

A heading with a short subtitle — detected automatically.

<img src="media/gallery/slide-01.png" width="720">

### Section Divider

A lone heading becomes a section divider between topics.

<img src="media/gallery/slide-03.png" width="720">

### Bullet Points

A heading followed by a list renders as a bullet slide.

<img src="media/gallery/slide-04.png" width="720">

### Code Highlight

Fenced code blocks get automatic syntax highlighting with language detection.

<img src="media/gallery/slide-05.png" width="720">

### Blockquote

Blockquotes with attribution render as elegant quote slides.

<img src="media/gallery/slide-06.png" width="720">

### Data Table

Standard markdown tables render with clean formatting.

<img src="media/gallery/slide-07.png" width="720">

### Bullet Slide with Image

Add a single image to a bullet slide and it automatically renders as a split layout — content on the left, image on the right.

<img src="media/gallery/slide-26.png" width="720">

### Full-Screen Image

A slide with just an image fills the entire slide area.

<img src="media/gallery/slide-27.png" width="720">

### Two-Column Layout

Split content into two columns using the `+++` separator.

<img src="media/gallery/slide-28.png" width="720">

---

## Diagrams

Architecture and flow diagrams rendered from simple text descriptions. Supports grid positioning, icons, labeled arrows, and multiple arrow types.

### Architecture Diagram

Grid-positioned nodes with icons and labeled connections.

<img src="media/gallery/slide-08.png" width="720">

### Flow Diagram

Auto-layout pipeline showing process flow.

<img src="media/gallery/slide-09.png" width="720">

---

## Charts & Visualizations

All visualizations are written as fenced code blocks with `@` language tags. Data is specified as simple `- Label: value` lines.

### Bar Chart

Vertical bar chart with axis labels.

<img src="media/gallery/slide-10.png" width="720">

### Horizontal Bar Chart

<img src="media/gallery/slide-11.png" width="720">

### Line Chart

Multi-series line chart with shared X-axis categories.

<img src="media/gallery/slide-12.png" width="720">

### Pie Chart

Proportional segments with automatic percentage labels.

<img src="media/gallery/slide-13.png" width="720">

### Donut Chart

Pie chart variant with a center label.

<img src="media/gallery/slide-14.png" width="720">

### Stacked Bar Chart

Multiple series stacked per category.

<img src="media/gallery/slide-15.png" width="720">

### Scatter Plot

2D scatter plot with labeled data points and axis descriptions.

<img src="media/gallery/slide-16.png" width="720">

### Radar Chart

Multi-axis comparison between data series.

<img src="media/gallery/slide-17.png" width="720">

### Funnel Chart

Progressive narrowing stages — great for conversion metrics.

<img src="media/gallery/slide-18.png" width="720">

### KPI Dashboard

Key metrics with trend indicators.

<img src="media/gallery/slide-19.png" width="720">

### Progress Bars

Horizontal progress indicators for project status.

<img src="media/gallery/slide-20.png" width="720">

### Timeline

Chronological events along a visual timeline.

<img src="media/gallery/slide-21.png" width="720">

### Word Cloud

Words sized proportionally to importance with automatic layout.

<img src="media/gallery/slide-22.png" width="720">

### Venn Diagram

Set intersections with automatic overlap detection.

<img src="media/gallery/slide-23.png" width="720">

### Organization Chart

Hierarchical tree with parent-child relationships.

<img src="media/gallery/slide-24.png" width="720">

### Gantt Chart

Project timeline with task dependencies and automatic time scaling.

<img src="media/gallery/slide-25.png" width="720">

---

## AI-Generated Images

MDeck integrates with AI image generation. Add `![prompt](image-generation)` to your slides, then run `mdeck ai generate` to create images automatically.

The images below were generated using `mdeck ai generate` with the style: *"Cinematic landscape photography style. Vivid colors, dramatic lighting, sweeping vistas."*

<img src="media/gallery/africa.png" width="720">

*African savanna at golden hour — generated from a text prompt and automatically placed in the slide.*

<img src="media/gallery/antarctica.png" width="720">

*Antarctic ice shelf with aurora — another AI-generated image used in the [continents presentation](samples/continents.md).*

---

## Getting Started

```bash
# Install
brew install mklab-se/tap/mdeck

# Present any markdown file
mdeck your-talk.md

# Export slides as PNG
mdeck export your-talk.md

# See the full format specification
mdeck spec
```

See the [README](README.md) for full installation and usage instructions.
