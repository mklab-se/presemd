<p align="center">
  <img src="https://raw.githubusercontent.com/mklab-se/mdeck/main/media/mdeck-horizontal.png" alt="mdeck" width="600">
</p>

<h1 align="center">MDeck</h1>

<p align="center">
  Stunning presentations from markdown.<br>
  Write content. MDeck handles the rest.
</p>

<p align="center">
  <a href="https://github.com/mklab-se/mdeck/actions/workflows/ci.yml"><img src="https://github.com/mklab-se/mdeck/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/mdeck"><img src="https://img.shields.io/crates/v/mdeck.svg" alt="crates.io"></a>
  <a href="https://github.com/mklab-se/mdeck/releases/latest"><img src="https://img.shields.io/github/v/release/mklab-se/mdeck" alt="GitHub Release"></a>
  <a href="https://github.com/mklab-se/homebrew-tap/blob/main/Formula/mdeck.rb"><img src="https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmklab-se%2Fhomebrew-tap%2Fmain%2FFormula%2Fmdeck.rb&search=%5Cd%2B%5C.%5Cd%2B%5C.%5Cd%2B&label=homebrew&prefix=v&color=orange" alt="Homebrew"></a>
  <a href="https://github.com/mklab-se/mdeck/blob/main/LICENSE.md"><img src="https://img.shields.io/crates/l/mdeck.svg" alt="License"></a>
</p>

<p align="center">
  <a href="GALLERY.md"><strong>Gallery</strong></a> &middot;
  <a href="crates/mdeck/doc/mdeck-spec.md"><strong>Format Spec</strong></a> &middot;
  <a href="CHANGELOG.md"><strong>Changelog</strong></a>
</p>

---

## What is MDeck?

MDeck creates stunning presentations from standard markdown files. No proprietary formats, no complex setup — just write markdown and present beautifully.

- **Any `.md` file is instantly presentable** — intelligent layout inference picks the right slide design from your content structure: title slides, bullet lists, code blocks, quotes, images, tables, and more
- **Built-in visualizations** — architecture diagrams, Gantt charts, word clouds, bar/line/pie charts, KPI dashboards, org charts, timelines, radar charts, scatter plots, Venn diagrams, and more — all from simple text in your markdown
- **AI-native** — since presentations are just markdown, any AI can help you write them. But MDeck also has AI built directly in: generate images and icons in your chosen style, right from the command line. Your slides, your aesthetic
- **Built in Rust** — fast, lightweight, GPU-accelerated rendering with smooth transitions and animations

<p align="center">
  <img src="media/gallery/slide-08.png" width="45%">&nbsp;&nbsp;
  <img src="media/gallery/slide-22.png" width="45%">
</p>
<p align="center">
  <img src="media/gallery/slide-25.png" width="45%">&nbsp;&nbsp;
  <img src="media/gallery/slide-26.png" width="45%">
</p>

<p align="center"><em>See the full <a href="GALLERY.md">Gallery</a> for all layouts and visualization types.</em></p>

---

## Installation

### Homebrew (macOS / Linux)

```bash
brew install mklab-se/tap/mdeck
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/mklab-se/mdeck/releases) — available for macOS (Intel + ARM), Linux, and Windows.

### Cargo

```bash
cargo install mdeck
```

### Build from source

```bash
git clone https://github.com/mklab-se/mdeck.git
cd mdeck
cargo install --path crates/mdeck
```

---

## Quick Start

```bash
# Present a markdown file
mdeck slides.md

# Export slides as PNG images
mdeck export slides.md

# Show all commands
mdeck --help
```

Write a file called `talk.md`:

```markdown
---
title: "My Talk"
@theme: dark
---

# Welcome

This is my first MDeck presentation.

---

## Key Points

- Write in standard markdown
- Slides are separated by `---`
- Layout is inferred automatically

---

## Architecture

​```@diagram
- Client -> Server: requests
- Server -> Database: queries
- Database -> Server: results
​```
```

Then present it: `mdeck talk.md`

---

## Features

### Automatic Layout Inference

MDeck detects what kind of slide you're writing and picks the best layout:

| Content | Layout |
|---------|--------|
| Heading + subtitle | Title |
| Lone heading | Section divider |
| Heading + bullet list | Bullet |
| Heading + code block | Code |
| Blockquote + attribution | Quote |
| Single image | Full-screen image |
| Bullets + image | Split layout |
| `+++` separator | Two-column |
| `@diagram` code block | Diagram |
| `@barchart`, `@piechart`, etc. | Visualization |

Override with `@layout: name` when needed.

### Visualizations

Write data visualizations directly in markdown using fenced code blocks:

| Type | Tag | Example |
|------|-----|---------|
| Bar chart | `@barchart` | `- Python: 48` |
| Line chart | `@linechart` | `- Revenue: 100, 150, 200` |
| Pie chart | `@piechart` | `- Frontend: 35%` |
| Donut chart | `@donut` | `- Complete: 78` |
| Stacked bar | `@stackedbar` | `- Product A: 40, 45, 50` |
| Scatter plot | `@scatter` | `- Alice: 80, 90` |
| Radar chart | `@radar` | `- Speed: 9, 7, 5, 3` |
| Funnel | `@funnel` | `- Visitors: 10000` |
| KPI cards | `@kpi` | `- Revenue: $4.2M (trend: +12%)` |
| Progress bars | `@progress` | `- Design: 100%` |
| Timeline | `@timeline` | `- 2024: Project launch` |
| Word cloud | `@wordcloud` | `- AI (size: 50)` |
| Venn diagram | `@venn` | `- Set A & Set B: Overlap` |
| Org chart | `@orgchart` | `- CEO -> CTO` |
| Gantt chart | `@gantt` | `- Design: 8d, after Research` |
| Diagram | `@diagram` | `- Client -> Server` |

All visualizations support progressive reveal with `+` markers.

### Diagrams

Architecture and flow diagrams from text:

```markdown
​```@diagram
- Browser   (icon: browser,  pos: 1,1)
- API       (icon: api,      pos: 2,1)
- Database  (icon: database, pos: 2,2)

- Browser -> API: requests
- API -> Database: queries
​```
```

Features: grid positioning, 20+ built-in icons, 5 arrow types (`->`, `<-`, `<->`, `--`, `-->`), labeled connections, and AI-generated custom icons.

### Themes

Built-in themes: **light**, **dark**, and **nord**. Cycle with `Shift+T` during presentation.

Set globally in frontmatter or per-slide:

```yaml
---
@theme: dark
@transition: spatial
---
```

### Transitions

Smooth animated transitions between slides: **fade**, **slide**, **spatial**, and **none**. Cycle with `T` during presentation.

### Slide Splitting

Three mechanisms create slide breaks (all combine):

1. **`---`** separator with blank lines on both sides
2. **Three blank lines** between content
3. **Heading inference** — headings automatically start new slides

Smart heading inference: if your file has one `#` title and uses `##` for sections, both levels split. Control explicitly with `@slide-level: 2` in frontmatter.

---

## AI Features

MDeck integrates AI for image and icon generation. Configure with `mdeck ai enable`.

### Generate Images for a Presentation

Add image placeholders to your markdown:

```markdown
## African Savanna

- Home to 54 countries
- The Sahara is the size of the United States

![A sweeping savanna at golden hour with acacia trees](image-generation)
```

Then generate all images at once:

```bash
mdeck ai generate slides.md
```

MDeck scans for `image-generation` markers, generates images using AI, saves them to an `images/` directory, and rewrites your markdown with the actual file paths.

### Style Control

Control the visual style of generated images:

```yaml
---
@image-style: "Cinematic photography, vivid colors, dramatic lighting"
@icon-style: "Clean minimalist icon, subtle 3D feel"
---
```

Or manage named styles:

```bash
mdeck ai style add Cinematic "Vivid colors, dramatic lighting, sweeping vistas"
mdeck ai style set-default Cinematic
mdeck ai style list
```

### Ad-Hoc Image Generation

Generate individual images from the command line:

```bash
mdeck ai generate-image --prompt "A futuristic cityscape at sunset"
mdeck ai generate-image --prompt "A database server" --icon --output db.png
```

### Diagram Icons

Generate custom icons for diagram nodes:

```markdown
​```@diagram
- Gateway (icon: generate-image, prompt: "An API gateway router")
- Auth    (icon: generate-image, prompt: "A security lock shield")
​```
```

Run `mdeck ai generate` to create and cache the icons.

---

## Commands

```bash
mdeck <file.md>              # Launch presentation
mdeck <file.md> --check      # Validate presentation (exit 1 if warnings)
mdeck export <file.md>       # Export slides as PNG images (1920x1080)
mdeck export <file.md> --width 3840 --height 2160  # Custom resolution
mdeck spec                   # Print full format specification
mdeck spec --short           # Print quick reference card
mdeck completion <shell>     # Generate shell completions
mdeck config show            # Display current settings
mdeck config set <key> <val> # Set config value
mdeck ai                     # Show AI status
mdeck ai enable              # Enable AI features
mdeck ai disable             # Disable AI features
mdeck ai test                # Test AI integration
mdeck ai config              # Open AI config in editor
mdeck ai generate <file.md>  # Generate all AI images in a presentation
mdeck ai generate-image      # Generate a single image from a prompt
mdeck ai style list          # List defined image styles
mdeck ai style add <n> <d>   # Add a named style
mdeck ai style set-default   # Set the default image style
```

### Keyboard Controls

| Key | Action |
|-----|--------|
| Space / N / Right | Next slide |
| P / Left | Previous slide |
| Up / Down | Scroll overflowed content |
| G | Grid overview |
| Shift+T | Cycle theme |
| T | Cycle transition |
| F | Toggle fullscreen |
| H | Toggle HUD |
| `.` | Blackout screen |
| Esc Esc | Quit |

### Shell Completions

```bash
# Static completions
mdeck completion bash > ~/.bash_completion.d/mdeck
mdeck completion zsh > ~/.zfunc/_mdeck

# Dynamic completions (recommended)
source <(COMPLETE=bash mdeck)
source <(COMPLETE=zsh mdeck)
```

---

## Documentation

- **[Gallery](GALLERY.md)** — Visual showcase of all layouts and visualizations
- **[Format Specification](crates/mdeck/doc/mdeck-spec.md)** — Complete reference for the MDeck markdown format (also available via `mdeck spec`)
- **[Changelog](CHANGELOG.md)** — Release history and what's new

The format spec is embedded in the binary and available via `mdeck spec`. It covers all slide layouts, directives, visualization syntax, diagram features, and keyboard shortcuts.

---

## Development

```bash
cargo build              # Build
cargo test --workspace   # Run tests
cargo clippy --workspace -- -D warnings  # Lint (CI-enforced)
cargo fmt --all -- --check               # Format check (CI-enforced)
cargo run -p mdeck       # Run the app
```

---

## License

MIT
