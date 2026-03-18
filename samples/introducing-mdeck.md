---
title: "Introducing MDeck"
author: "MDeck Team"
@theme: dark
@transition: slide
---

# Introducing MDeck

Presentations from plain markdown

---

# The Problem

- Presentation tools are heavy and lock you in
- Your content lives in proprietary formats
- Formatting slides takes longer than writing content
- Sharing and version control are painful

---

# What is MDeck?

MDeck is a **markdown-based presentation tool** built in Rust.

Write standard markdown. Get polished slides.

- Any `.md` file is instantly presentable
- Layout is inferred from content structure
- No special syntax required for basic use
- Keyboard-driven, fast, and lightweight

---

# Getting Started

```bash
# Install via Homebrew
brew install mklab-se/tap/mdeck

# Or via Cargo
cargo install mdeck

# Present any markdown file
mdeck my-talk.md
```

That's it. No config files, no build step, no setup.

---

# How Slides Work

Three ways to separate slides:

```markdown
# Heading starts a new slide

Content below the heading.

---

An explicit separator also works.



Three blank lines create a break too.
```

MDeck figures out the rest. Headings, separators, and blank lines all do what you'd expect.

---

# Layout Inference

MDeck detects what kind of slide you're writing:

- **Title** -- a heading with an optional subtitle
- **Bullets** -- a heading followed by a list
- **Code** -- a heading with a fenced code block
- **Quote** -- a blockquote, optionally with attribution
- **Image** -- a single image, fills the slide
- **Two-column** -- split content with `+++`
- **Diagram** -- architecture diagrams from text
- **Visualization** -- charts and data from text

No `@layout` directive needed in most cases.

---

# Code Slides

Fenced code blocks get syntax highlighting automatically:

```rust
fn main() {
    let slides = parse_markdown("talk.md");
    for slide in &slides {
        render(slide);
    }
}
```

Add line highlighting with `{lines}` after the language tag.

---

# Visualizations

MDeck renders charts directly from your markdown. Here's project status at a glance:

```@progress
- Parsing: 100%
- Rendering: 100%
- Themes: 85%
- Export: 90%
- AI Features: 60%
```

---

# Visualizations: Charts

```@barchart
# orientation: horizontal
# x-label: Stars
- Ease of Use: 92
- Visual Quality: 88
- Speed: 95
- Portability: 90
- Extensibility: 75
```

---

# Architecture

```@architecture
- Markdown File  (icon: storage,   pos: 1,1)
- Parser         (icon: function,  pos: 2,1)
- Layout Engine  (icon: container, pos: 3,1)
- Renderer       (icon: monitor,   pos: 3,2)
- Theme System   (icon: browser,   pos: 4,2)

- Markdown File -> Parser: reads
- Parser -> Layout Engine: slide blocks
- Layout Engine -> Renderer: positioned elements
- Theme System -> Renderer: colors and fonts
```

---

# Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Space / N / Right | Next slide |
| P / Left | Previous slide |
| G | Grid overview |
| D | Cycle theme |
| T | Cycle transition |
| F | Toggle fullscreen |
| Up / Down | Scroll overflowed content |
| Esc Esc | Quit |

---

# Themes and Customization

Set a theme globally in frontmatter or per-slide:

```markdown
---
@theme: dark
---
```

Built-in themes: **light**, **dark**, and **nord**.

Override per slide with `@theme:`, `@background:`, or `@transition:`.

---

> Any markdown file should be presentable.

-- MDeck design principle

---

# Learn More

- **GitHub:** github.com/mklab-se/mdeck
- **Format spec:** `mdeck spec` prints the full reference
- **Quick ref:** `mdeck spec --short`
- **Export:** `mdeck export talk.md` renders slides to PNG

Start presenting in seconds:

```bash
mdeck your-talk.md
```
