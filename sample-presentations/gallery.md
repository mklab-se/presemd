---
title: "MDeck Gallery"
author: "MDeck"
@theme: dark
@transition: spatial
---

# MDeck Gallery

A showcase of layouts, visualizations, and diagrams

---

# Title Slide

This is a title layout — a heading with a short subtitle.

---

## Section Divider

---

## Bullet Points

### Key Features

- Write slides in **standard markdown**
- Layout is *inferred* from content structure
- Supports code highlighting, images, and diagrams
- Built-in themes: light, dark, and nord
- Export slides as PNG with `mdeck export`

---

## Code Highlight

```rust
use mdeck::parser;

fn main() {
    let presentation = parser::parse("slides.md");
    for slide in &presentation.slides {
        println!("Layout: {:?}", slide.layout);
    }
}
```

---

## Blockquote

> The best way to predict the future is to invent it.

-- Alan Kay

---

## Data Table

| Feature | MDeck | Traditional Tools |
|---------|-------|-------------------|
| Format | Markdown | Proprietary |
| Version Control | Git-friendly | Difficult |
| File Size | Kilobytes | Megabytes |
| Portability | Universal | Vendor-locked |
| AI Integration | Built-in | Plugins |

---

## Architecture Diagram

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

---

## Flow Diagram

```@diagram
- Source -> Build: triggers
- Build -> Test: on success
- Test -> Deploy: all green
- Deploy -> Monitor: reports to
```

---

## Bar Chart

```@barchart
# x-label: Language
# y-label: Popularity
- JavaScript: 65
- Python: 48
- TypeScript: 38
- Go: 28
- Rust: 22
```

---

## Horizontal Bar Chart

```@barchart
# orientation: horizontal
# x-label: Score
- Ease of Use: 92
- Visual Quality: 88
- Speed: 95
- Portability: 90
- Extensibility: 75
```

---

## Line Chart

```@linechart
# x-labels: Jan, Feb, Mar, Apr, May, Jun
# y-label: Revenue ($K)
- Product A: 120, 145, 162, 190, 215, 248
- Product B: 80, 92, 105, 118, 130, 155
- Product C: 40, 55, 62, 78, 95, 110
```

---

## Pie Chart

```@piechart
- Frontend: 35%
- Backend: 30%
- DevOps: 20%
- Testing: 15%
```

---

## Donut Chart

```@donut
# center: 78%
- Complete: 78
- In Progress: 15
- Not Started: 7
```

---

## Stacked Bar Chart

```@stackedbar
# categories: Q1, Q2, Q3, Q4
# y-label: Revenue ($M)
- Enterprise: 40, 45, 50, 55
- SMB: 30, 35, 40, 45
- Consumer: 20, 25, 30, 35
```

---

## Scatter Plot

```@scatter
# x-label: Development Time (weeks)
# y-label: User Satisfaction
- Dashboard: 8, 92
- Auth Flow: 3, 78
- Search: 12, 95
- Reports: 6, 85
- Settings: 2, 70
- Onboarding: 5, 88
```

---

## Radar Chart

```@radar
# axes: Speed, Reliability, Scalability, Security, Usability
- Current System: 6, 8, 5, 7, 9
- Target State: 9, 9, 9, 9, 8
```

---

## Funnel Chart

```@funnel
- Website Visitors: 100000
- Sign Ups: 25000
- Activated Users: 12000
- Paid Subscribers: 4800
- Enterprise Clients: 960
```

---

## KPI Dashboard

```@kpi
- Revenue: $4.2M (trend: +12%)
- Active Users: 1.2M (trend: +8%)
- Churn Rate: 3.2% (trend: -0.5%)
- NPS Score: 72 (trend: +5)
```

---

## Progress Bars

```@progress
- Design: 100%
- Frontend: 85%
- Backend: 70%
- Testing: 45%
- Documentation: 30%
```

---

## Timeline

```@timeline
- 2020: Project inception and initial prototyping
- 2021: Core parser and rendering engine complete
- 2022: Theme system and transitions added
- 2023: Diagram and visualization support
- 2024: AI integration and image generation
- 2025: Public release on crates.io
```

---

## Word Cloud

```@wordcloud
- Markdown (size: 50)
- Presentations (size: 48)
- Slides (size: 42)
- Rust (size: 40)
- Open Source (size: 38)
- Diagrams (size: 36)
- Charts (size: 35)
- Themes (size: 34)
- Export (size: 30)
- AI (size: 28)
- Visualizations (size: 45)
- Code (size: 32)
- Images (size: 29)
- Tables (size: 26)
- Transitions (size: 24)
- Keyboard (size: 22)
- Layout (size: 38)
- Inference (size: 20)
- Fullscreen (size: 18)
- GPU (size: 16)
```

---

## Venn Diagram

```@venn
- Frontend (size: 35)
- Backend (size: 35)
- DevOps (size: 35)
- Frontend & Backend: Fullstack
- Backend & DevOps: SRE
- Frontend & DevOps: Platform
```

---

## Organization Chart

```@orgchart
- CEO
- CEO -> CTO
- CEO -> CFO
- CEO -> COO
- CTO -> VP Engineering
- CTO -> VP Product
- CFO -> Controller
- COO -> VP Operations
```

---

## Gantt Chart

```@gantt
# title: Product Launch
- Research: 2024-06-01, 10d
- Design: 8d, after Research
- Frontend: 15d, after Design
- Backend: 15d, after Design
- Integration: 5d, after Frontend
- QA Testing: 10d, after Integration
- Launch: 2d, after QA Testing
```

---

## Bullet Slide with Image

- The Sahara Desert is roughly the size of the United States
- Lake Victoria is the world's second-largest freshwater lake
- Home to 54 countries — more than any other continent
- Africa has more languages than any other continent (~2,000)

![African savanna at golden hour](images/golden-savanna-acacias.png)

---

## Image Slide

![A vast Antarctic ice shelf meeting deep blue ocean](images/antarctic-aurora-icebergs.png)

---

@layout: two-column

## Two-Column Layout

### Markdown Source

Write standard markdown on the left, see polished slides on the right.

- Bullet points
- **Bold** and *italic*
- Code snippets
- Images and diagrams

+++

### Rendered Output

```python
def present(slides):
    for slide in slides:
        render(slide)
```

Split content with the `+++` separator.
