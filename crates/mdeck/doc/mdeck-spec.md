# MDeck Markdown Syntax Specification

**Version:** 0.1
**Status:** Draft

MDeck is a markdown-based presentation tool. Authors write standard markdown; MDeck infers slide layout from content structure and renders it as a presentation.

---

## 1. Design Principles

1. **Readability over expressiveness.** A MDeck document should read as a natural markdown document. Someone reading the raw source should understand the content without knowing MDeck exists.

2. **Inference over configuration.** MDeck determines slide layout from content structure. Authors should almost never need to specify a layout explicitly.

3. **Standard markdown first.** Every feature uses standard CommonMark markdown when possible. The `@` directive system exists only for things markdown cannot express.

4. **Graceful degradation.** When rendered in a standard markdown viewer, a MDeck document should still be readable. Directives degrade to visible text; separators degrade to horizontal rules.

---

## 2. Document Structure

A MDeck document has two parts:

```
[frontmatter]       (optional, YAML metadata)
[slides]            (content separated by slide breaks)
```

### 2.1 Frontmatter

Standard YAML frontmatter, delimited by `---` on the first line and `---` on a subsequent line. Must be the very first content in the file (no preceding blank lines).

```yaml
---
title: "Building Resilient Systems"
author: "Jane Doe"
date: 2026-02-28
@theme: dark
@transition: slide
@aspect: 16:9
---
```

#### Standard fields

| Field    | Type   | Description                                     |
|----------|--------|-------------------------------------------------|
| `title`  | string | Presentation title (window title bar, metadata) |
| `author` | string | Author name                                     |
| `date`   | string | Presentation date                               |

#### MDeck fields (prefixed with `@`)

| Field         | Type   | Default   | Description                                        |
|---------------|--------|-----------|----------------------------------------------------|
| `@theme`      | string | `"light"` | Global theme: `"light"`, `"dark"`, `"nord"`, or custom name  |
| `@transition` | string | `"slide"` | Default transition: `"fade"`, `"slide"`, `"none"`  |
| `@aspect`     | string | `"16:9"`  | Aspect ratio: `"16:9"`, `"4:3"`, `"16:10"`        |
| `@code-theme`  | string | (theme)   | Syntax highlighting theme for code blocks          |
| `@footer`      | string | none      | Text shown in footer of every slide                |
| `@image-style` | string | none      | Default AI image generation style (name or description) |
| `@icon-style`  | string | none      | Default AI icon generation style (name or description)  |
| `@slide-level` | integer | (inferred) | Heading level that triggers slide breaks (1–6). E.g., `2` means H1 and H2 both split. When omitted, inferred from content. |

**Parser rule:** If the document starts with a line that is exactly `---`, begin parsing YAML until a closing `---` line. If no closing `---` is found before invalid YAML, treat the opening `---` as a slide separator instead (graceful recovery).

---

## 3. Slide Separation

Three mechanisms create slide breaks. When multiple overlap, a single break is produced (not multiple).

### 3.1 Explicit separator: `---`

A line of three or more dashes, with a blank line above and below:

```markdown
Content of slide one.

---

Content of slide two.
```

**Parser rule:** Pattern is `\n\n-{3,}\n\n`. The `---` line is consumed and not rendered as content.

### 3.2 Blank line gap: 3+ blank lines

Three or more consecutive blank lines create a slide break:

```markdown
Content of slide one.



Content of slide two.
```

**Parser rule:** Pattern is `\n{4,}` (three blank lines = four newline characters). Chosen over two blank lines because two blank lines are common in normal markdown formatting and would cause accidental breaks.

### 3.3 Heading inference

Headings start new slides when the current slide already has content. Which heading levels trigger splits depends on the **slide level**, determined as follows:

1. **Explicit:** Set `@slide-level: N` in frontmatter. Headings at level 1 through N all trigger splits.
2. **Inferred:** If `@slide-level` is not set:
   - **Single H1 (or no H1):** Infer slide level 2 — both `#` and `##` trigger splits. This handles "proper" markdown files where H1 is the title and H2s are sections.
   - **Multiple H1s:** Infer slide level 1 — only `#` triggers splits.

```markdown
# Title Slide

A subtitle

## First Topic

Content here — this is a separate slide because there's only one H1.

## Second Topic

More content — also a separate slide.
```

**Parser rule:** When a heading at or above the slide level is encountered and the current slide already contains rendered elements, insert a slide break before the heading.

### 3.4 Precedence

All three split mechanisms coexist and combine. When multiple overlap, a single break is produced.

- `---` within 3+ blank lines = single break, not two.
- Heading after `---` = the `---` creates the break, the heading belongs to the new slide.
- Frontmatter `---` delimiters are never treated as slide separators.
- `@slide-level` controls heading splits but does not affect `---` or blank-line splits.

### 3.5 Speaker Notes

Speaker notes can be added to any slide using the `???` separator. Everything after `???` until the next slide break is treated as notes — parsed but not rendered in the presentation.

```markdown
# Key Architecture Decisions

- Microservices over monolith
- Event-driven communication
+ gRPC for internal APIs

???

This slide sets the stage for the technical deep-dive. Emphasize that
the microservices decision was driven by **team autonomy**, not scale.
Ask the audience: "How many of you have migrated from a monolith?"

---

# Next Slide
```

**Parser rule:** A line whose trimmed content is three or more `?` characters (`???`, `????`, etc.) acts as a notes separator. The `???` line inside a fenced code block is ignored (not treated as a separator).

Notes content supports full markdown formatting (bold, italic, code, links) and is stored as raw text on the slide. Notes are stripped before layout classification, so they do not affect the inferred layout.

**Graceful degradation:** In a standard markdown viewer, `???` renders as visible text, acting as a natural separator between slide content and notes. This keeps notes readable in raw form — important for AI-generated presentations where notes explain slide intent and delivery guidance.

---

## 4. Slide Layout Inference

After parsing a slide's content into block elements, MDeck classifies them and matches against layout patterns. The first match wins, checked in the order below.

### Element types

| Symbol    | Meaning                              |
|-----------|--------------------------------------|
| `H1`      | Level-1 heading                      |
| `H2`      | Level-2 heading                      |
| `H3`      | Level-3 heading                      |
| `P`       | Paragraph                            |
| `UL`/`OL` | Unordered/ordered list               |
| `IMG`     | Image                                |
| `CODE`    | Fenced code block                    |
| `QUOTE`   | Blockquote                           |
| `DIAGRAM` | Architecture diagram block (`@architecture`) |

### Layout patterns

#### 1. Diagram Slide

**Match:** Contains a `DIAGRAM` element.
**Rendering:** Diagram is the primary content. Optional heading above becomes the slide title.

#### 2. Title Slide

**Match:** Only an `H1`, and optionally one of: a single `H2`, or a single short `P` (under 120 characters). No other elements.
**Rendering:** `H1` is rendered large and centered. `H2` or `P` is rendered below as subtitle.

```markdown
# Building Resilient Systems

A practical guide to fault tolerance
```

#### 3. Section Divider

**Match:** A single heading (`H1` or `H2`) with no other content.
**Rendering:** Heading rendered large and centered as a visual section break.

```markdown
# Part Two
```

#### 4. Image Slide

**Match:** A single `IMG`, optionally preceded by a heading, optionally followed by a short `P` (caption).
**Rendering:** Image fills the slide. Heading at top, caption at bottom.

```markdown
# System Architecture

![Architecture overview](arch.png)

The complete system at a glance.
```

#### 5. Gallery Slide

**Match:** Two or more `IMG` elements, with no other content or only a heading.
**Rendering:** Images in a grid. 2 images: side by side. 3: top 2, bottom 1 centered. 4: 2x2.

```markdown
# Comparison

![Before](before.png)
![After](after.png)
```

#### 6. Quote Slide

**Match:** A `QUOTE`, optionally followed by a `P` (attribution), optionally preceded by a heading. May include one image.
**Rendering:** Blockquote large and centered. Attribution below in smaller italic text, right-aligned. Leading `--` or `---` in the attribution is stripped. If an image is present, the quote renders in the left 55% and the image in the right 40% as a side panel.

```markdown
> The best way to predict the future is to invent it.

-- Alan Kay
```

#### 7. Code Slide

**Match:** A `CODE` block, optionally preceded by a heading and/or a short `P`. May include one image.
**Rendering:** Code block with syntax highlighting as primary element. Heading at top. If an image is present, the code renders in the left 55% and the image in the right 40% as a side panel.

```markdown
# Connection Pool

```rust
pub struct Pool {
    connections: Vec<Connection>,
    max_size: usize,
}
```
```

#### 8. Bullet Slide

**Match:** A heading followed by a `UL` or `OL`. May include one image.
**Rendering:** Heading at top, list below with generous spacing. Nesting supported up to 3 levels. If an image is present, the content renders in the left 55% and the image in the right 40% as a side panel.

```markdown
# Key Takeaways

- Resilience is a system property
- Failure is inevitable; recovery is a design choice
- Test your assumptions under stress
```

#### 9. Content Slide (Fallback)

**Match:** Anything not matching the above.
**Rendering:** Elements top-to-bottom in source order with reasonable spacing. Optional heading at top.

### 4.1 Explicit layout override

When inference produces the wrong result, force a layout with the `@layout` directive:

```markdown
@layout: two-column

# Comparison

Left column content...

+++

Right column content...
```

Available layout names: `title`, `section`, `bullets`, `image`, `gallery`, `quote`, `code`, `diagram`, `two-column`, `blank`, `content`.

---

## 5. Content Types

### 5.1 Headings

Standard ATX headings. Levels 1-3 are meaningful for layout; levels 4-6 are rendered as body-weight text.

```markdown
# Level 1 — Slide title / section
## Level 2 — Subtitle / subsection
### Level 3 — Minor heading within slide
```

### 5.2 Paragraphs and inline formatting

| Syntax               | Result          |
|----------------------|-----------------|
| `**bold**`           | **bold**        |
| `*italic*`           | *italic*        |
| `~~strikethrough~~`  | ~~strikethrough~~|
| `` `inline code` ``  | `inline code`   |
| `[text](url)`        | hyperlink       |

Links are rendered visually but are not clickable during presentation. The URL is shown on hover.

### 5.3 Lists

Both ordered and unordered lists with nesting up to 3 levels. List marker choice controls reveal behavior (see [Section 6](#6-incremental-reveal)).

```markdown
- First item
  - Nested item
    - Deep nested
- Second item

1. First step
2. Second step
   1. Sub-step
```

### 5.4 Images

Standard markdown image syntax:

```markdown
![Alt text](path/to/image.png)
```

Sizing directives can be placed in the alt text with the `@` prefix:

```markdown
![Architecture @width:80%](arch.png)
![Logo @height:100px](logo.png)
![Photo @fill](photo.jpg)
![Diagram @fit](diagram.svg)
![Banner @left](banner.png)
```

| Directive     | Description                                     |
|---------------|-------------------------------------------------|
| `@width:VAL`  | Set width (px, %, or `auto`)                    |
| `@height:VAL` | Set height (px, %, or `auto`)                   |
| `@fill`       | Fill the entire slide as background              |
| `@fit`        | Fit within available space, preserve aspect ratio (default) |
| `@left`       | Align left                                       |
| `@right`      | Align right                                      |
| `@center`     | Align center (default)                           |

When rendered in a standard markdown viewer, the `@` directives appear as visible alt text, which is acceptable degradation.

#### AI Image Generation

Use `image-generation` as the image path to mark an image for AI generation:

```markdown
![A futuristic cityscape at sunset](image-generation)
```

The alt text serves as the image prompt. Leave it empty for auto-prompting from slide context (requires chat capability):

```markdown
![](image-generation)
```

Run `mdeck ai generate <file.md>` to generate all marked images. The command:
- Detects orientation automatically (horizontal for full-slide, vertical for side-panel layouts)
- Applies the configured image style (via `@image-style` frontmatter, config default, or hardcoded fallback)
- Rewrites the markdown file, replacing `image-generation` with actual file paths

### 5.5 Code blocks

Standard fenced code blocks with optional language and line highlighting:

````markdown
```rust
fn main() {
    println!("Hello, world!");
}
```
````

Line highlighting uses `{lines}` notation after the language:

````markdown
```rust {3,5-7}
fn main() {
    let pool = Pool::new(10);
    pool.connect();           // highlighted
    let result = pool
        .query("SELECT")     // highlighted
        .fetch()              // highlighted
        .unwrap();            // highlighted
    println!("{:?}", result);
}
```
````

The `{...}` is parsed as comma-separated line numbers and ranges (e.g., `3`, `5-7`). Highlighted lines receive a distinct background. Code blocks without a language identifier render as plain monospace text with no highlighting.

### 5.6 Blockquotes

Standard markdown blockquotes:

```markdown
> This is a quotation that will be
> rendered prominently on the slide.
```

Nested blockquotes are supported and rendered with increasing indentation.

### 5.7 Tables

Standard pipe-delimited tables:

```markdown
| Feature   | Status  |
|-----------|---------|
| Parsing   | Done    |
| Rendering | WIP     |
```

Tables are rendered with theme-appropriate styling. They do not trigger a special layout; they are block elements within whatever layout the slide otherwise matches.

### 5.8 Horizontal rules within slides

Since `---` is reserved for slide breaks, use `***` or `___` for a visual rule within a slide:

```markdown
# Timeline

Phase 1: Research

***

Phase 2: Implementation
```

---

## 6. Incremental Reveal

MDeck uses the three standard markdown list markers to control how content is revealed during a presentation:

| Marker | Name           | Behavior                                                    |
|--------|----------------|-------------------------------------------------------------|
| `-`    | Static         | Visible immediately when the slide appears                  |
| `+`    | Next step      | Appears on the next forward press; advances the step counter |
| `*`    | Keep with previous | Appears at the same step as the preceding `+` item       |

### 6.1 In slide lists

```markdown
# Key Points

- Always visible context
+ First reveal
+ Second reveal
* Also part of second reveal
+ Third reveal
```

Presentation behavior:
1. Slide appears with "Always visible context" shown
2. Forward press: "First reveal" appears
3. Forward press: "Second reveal" and "Also part of second reveal" appear together
4. Forward press: "Third reveal" appears
5. Forward press: advance to next slide

### 6.2 In diagrams

The same markers control diagram element reveal. See [Section 8](#8-diagram-syntax) for full details.

### 6.3 Rules

- On a slide with steps, pressing forward reveals the next step rather than advancing to the next slide. Only after all steps have been revealed does forward advance to the next slide.
- A `*` without a preceding `+` on the same slide is treated as `-` (static).
- Ordered lists (`1.`, `2.`, etc.) are always static — they do not support incremental reveal.
- The step counter is per-slide and resets for each new slide.

---

## 7. Directives

Directives use the `@` prefix. They come in two forms:

### 7.1 Block directives

A standalone line at the beginning of a slide (before any content):

```markdown
@theme: dark
@transition: fade
@layout: two-column
```

**Syntax:** `@name: value`

Block directives at the start of a slide apply to that slide. In the frontmatter, they apply globally.

### 7.2 Fenced directives

For complex content, the fenced code block syntax with `@` on the language tag:

````markdown
```@architecture
...diagram content...
```
````

### 7.3 Directive reference

| Directive      | Scope          | Values                                    | Default        |
|----------------|----------------|-------------------------------------------|----------------|
| `@theme`       | global, slide  | `light`, `dark`, `nord`, custom name      | `light`        |
| `@transition`  | global, slide  | `fade`, `slide`, `none`                   | `slide`        |
| `@layout`      | slide          | layout name (see Section 4.1)             | auto-inferred  |
| `@background`  | slide          | color hex or image path                   | theme default  |
| `@footer`      | global, slide  | string                                    | none           |
| `@aspect`      | global         | `16:9`, `4:3`, `16:10`                    | `16:9`         |
| `@code-theme`  | global, slide  | theme name                                | theme-dependent|
| `@class`       | slide          | arbitrary string                          | none           |

**Scope resolution:** Slide-level directives override global. If not set at slide level, the global value applies. If not set globally, the default applies.

**Unknown directives** are ignored with a warning. They are not rendered as content.

---

## 8. Diagram Syntax

### 8.1 Basic form

Diagrams use a fenced code block with the `@architecture` language tag. In the simplest form, just write relationships — components are inferred:

````markdown
```@architecture
- User -> Server: Sends request
- Server -> Database: Queries data
- Server -> User: Returns response
```
````

### 8.2 Full form

For explicit layout, icons, and stepped reveal:

````markdown
```@architecture
# Components
- User        (icon: user,      pos: 1,1)
- Server      (icon: server,    pos: 2,1)
- Database    (icon: database,  pos: 2,2)
- Log Service (icon: logs,      pos: 3,1)

# Relationships
- Server -- Log Service: sends logs to
+ User -> Server: Sends request
+ Server -> Database: Queries data
+ Database -> Server: Returns results
* Server -> User: Sends response
```
````

In this example:
- All four components and the logging relationship are visible from the start (`-`)
- Forward press 1: "User -> Server: Sends request" appears
- Forward press 2: "Server -> Database: Queries data" appears
- Forward press 3: "Database -> Server: Returns results" and "Server -> User: Sends response" appear together (the `*` groups with the preceding `+`)

### 8.3 Components

```
- Name (key: value, key: value)
```

| Key     | Values                          | Default       | Description           |
|---------|---------------------------------|---------------|-----------------------|
| `icon`  | icon name from theme icon set   | `box`         | Visual icon           |
| `pos`   | `x,y` (integer grid coords)     | auto-layout   | Position hint         |
| `label` | string                          | component name| Display label         |
| `style`  | `primary`, `secondary`, `muted` | `primary`     | Visual emphasis       |
| `prompt` | quoted string                   | none          | AI icon generation prompt |

Use `icon: generate-image` with a `prompt` to mark a node for AI icon generation:

```
- Gateway (icon: generate-image, prompt: "An API gateway router icon", pos: 1,2)
```

Run `mdeck ai generate <file.md>` to generate all marked icons. The generated icon replaces `generate-image` with the actual filename.

If no components are explicitly declared, they are inferred from relationship lines. Each unique name becomes a component with default icon and auto-positioned layout.

### 8.4 Relationships

```
- Source -> Target: Label
```

Arrow types:

| Arrow   | Meaning                    |
|---------|----------------------------|
| `->`    | Solid arrow (directed)     |
| `<-`    | Reverse solid arrow        |
| `<->`   | Bidirectional solid arrow  |
| `--`    | Dashed line (undirected)   |
| `-->`   | Dashed arrow (directed)    |

The text after `:` is the label. If no `:` is present, the relationship has no label.

### 8.5 Comments

Lines starting with `#` inside a diagram block are comments / section headers. They are ignored by the parser but help organize the source.

### 8.6 Layout algorithm

The `pos: x,y` values are relative grid coordinates:
- `1,1` is the top-left of the diagram area
- Higher x moves right; higher y moves down
- The grid auto-scales to fill available space
- If no `pos` is specified for any component, MDeck uses an automatic layout algorithm (left-to-right for linear chains, tree layout for hierarchical structures)

### 8.7 Diagram type qualifier

For future extensibility, a type can be specified after `@architecture`:

````markdown
```@architecture sequence
- Alice -> Bob: Hello
- Bob -> Alice: Hi there
```
````

Supported types in v0.1:
- (default, no qualifier): architectural / component diagram
- `sequence`: sequence diagram with timeline ordering

Additional types (`flowchart`, `timeline`, etc.) are reserved for future versions.

### 8.8 Built-in icons

The built-in themes provide these icon names:

`user`, `server`, `database`, `cloud`, `browser`, `mobile`, `api`, `queue`, `cache`, `storage`, `function`, `container`, `network`, `lock`, `key`, `mail`, `logs`, `monitor`, `box`

An unrecognized icon name falls back to `box`. Icons are simple and clear line drawings, designed to be recognizable at presentation scale.

---

## 9. Theme System

### 9.1 Built-in themes

**`light`**

| Property        | Value           |
|-----------------|-----------------|
| Background      | `#FFFFFF`       |
| Primary text    | `#1A1A2E`       |
| Heading text    | `#16213E`       |
| Accent          | `#0F3460`       |
| Code background | `#F5F5F5`       |
| Quote border    | accent color    |

**`dark`**

| Property        | Value           |
|-----------------|-----------------|
| Background      | `#1E1E1E`       |
| Primary text    | `#C8C8C8`       |
| Heading text    | `#FFFFFF`       |
| Accent          | `#5294E2`       |
| Code background | `#2D2D2D`       |
| Quote border    | accent color    |

Both themes meet WCAG AA contrast requirements.

### 9.2 Theme properties

A theme defines:

| Property          | Description                               |
|-------------------|-------------------------------------------|
| `background`      | Slide background color or gradient        |
| `foreground`      | Primary text color                        |
| `heading-color`   | Heading color                             |
| `accent`          | Links, quote borders, highlights          |
| `code-background` | Code block background                     |
| `code-foreground` | Code block text color                     |
| `code-theme`      | Syntax highlighting theme name            |
| `font-family`     | Primary font                              |
| `font-family-mono`| Monospace font for code                   |
| `font-size-h1`    | H1 size                                   |
| `font-size-h2`    | H2 size                                   |
| `font-size-body`  | Body text size                            |
| `icon-set`        | Icon set for diagrams                     |
| `diagram-colors`  | Color palette for diagram components      |

### 9.3 Per-slide theme override

```markdown
@theme: dark

# The Dark Slide

This slide uses the dark theme even if the presentation is light.
```

Individual properties can be overridden:

```markdown
@background: #2C3E50

# Custom Background

This slide has a custom background color.
```

### 9.4 Custom themes

Custom themes (defined as external files) are reserved for a future version. The `@theme` field accepts arbitrary strings in anticipation of this.

---

## 10. Two-Column Layout

The two-column layout requires the `@layout: two-column` directive and uses `+++` as the column separator:

```markdown
@layout: two-column

# Comparison

**Before:**

Old approach with manual config.

+++

**After:**

New approach with auto-discovery.
```

Content before `+++` is the left column; content after is the right column. If no `+++` is found, all content goes in the left column.

The `+++` separator was chosen because it is visually distinct from `---` (slide break) and is not a standard markdown construct.

---

## 11. Edge Cases

### Content overflow
Text is never truncated silently. If content overflows, MDeck reduces font size (down to 60% of theme default). If it still overflows, content is clipped with a subtle fade indicator and a warning is emitted.

### Empty slides
A slide with no content renders as a blank slide with the theme's background. This is intentional, not an error.

### Adjacent separators
Multiple `---` separators in a row create empty slides between them.

### Frontmatter parse failures
If YAML in the frontmatter is malformed, MDeck warns and treats the entire frontmatter block as content on the first slide.

### Missing images
If an image path cannot be resolved, a placeholder box with the alt text is rendered, and a warning is emitted.

### Code blocks without language
Rendered as plain monospace text with no syntax highlighting.

### `+`/`*` markers inside code blocks
List markers are never interpreted inside fenced code blocks. This is standard markdown behavior: fenced block content is literal.

### `*` without preceding `+`
A `*` item with no preceding `+` on the same slide is treated as `-` (static).

---

## 12. Complete Example

```markdown
---
title: "Scaling Our Platform"
author: "Jane Doe"
date: 2026-02-28
@theme: dark
@transition: slide
---

# Scaling Our Platform

Engineering deep-dive, February 2026



# The Problem

+ 10x traffic growth in 6 months
+ P99 latency spiked from 50ms to 800ms
+ Database connection pool exhausted daily



# Architecture Before

![Old architecture @width:80%](old-arch.png)

A monolith struggling under load.



# The New Architecture

```@architecture
# Components
- User      (icon: user,      pos: 1,1)
- Gateway   (icon: api,       pos: 2,1)
- Service A (icon: container,  pos: 3,1)
- Service B (icon: container,  pos: 3,2)
- Database  (icon: database,   pos: 4,1)
- Cache     (icon: cache,     pos: 4,2)

# Flow
+ User -> Gateway: Request
+ Gateway -> Service A: Route
* Gateway -> Service B: Route
+ Service A -> Cache: Check cache
+ Service A -> Database: Query
```



# Key Code Change

```rust {3-5}
pub async fn handle_request(req: Request) -> Response {
    let key = req.cache_key();
    if let Some(cached) = cache.get(&key).await {
        return cached;
    }
    let result = db.query(req.query()).await?;
    cache.set(&key, &result, TTL).await;
    result
}
```



# Results

+ P99 latency: 800ms to 45ms
+ Connection pool usage: 95% to 12%
+ Zero downtime during the migration



> The best optimization is the one you don't have to make.

-- Our team's new motto

---

@layout: two-column

# Before and After

**Before:**

- Monolith
- Single database
- No caching
- Manual scaling

+++

**After:**

- Microservices
- Sharded database
- Redis cache layer
- Auto-scaling



# Questions?

Thank you for listening.
```

This example demonstrates: frontmatter, title slide, bullet slide with `+` reveal, image slide, diagram with `-`/`+`/`*` reveal, code slide with line highlighting, result slide with incremental reveal, quote slide with attribution, two-column layout, and a closing section divider.

---

## 13. Parser Grammar Summary

This section provides a condensed reference for implementation.

### 13.1 Phase 1: Split document into slides

```
Document     = Frontmatter? Slide (SlideSep Slide)*

Frontmatter  = "---\n" YAML_CONTENT "---\n"
               (only valid at document start, line 1)

SlideSep     = BlankGap | RuleSep | HeadingSep

BlankGap     = /\n{4,}/
               (3+ blank lines)

RuleSep      = /\n\n-{3,}\n\n/
               (--- with blank lines on both sides)

HeadingSep   = /^# /
               (H1 heading when current slide already has content)
```

### 13.2 Phase 2: Parse each slide into blocks

```
Slide        = Directive* Block*

Directive    = /^@\w[\w-]*:\s*.+$/

Block        = Heading | Paragraph | List | Image | CodeBlock
             | BlockQuote | DiagramBlock | Table | HRule

Heading      = /^#{1,6}\s+.+$/

Image        = /^!\[([^\]]*)\]\(([^)]+)\)$/

CodeBlock    = /^`{3,}(\w+)?(\s*\{[^}]+\})?\n/ CONTENT /\n`{3,}$/

DiagramBlock = /^`{3,}@architecture(\s+\w+)?\n/ CONTENT /\n`{3,}$/

BlockQuote   = /^>\s+.+$/  (one or more consecutive lines)

HRule        = /^(\*{3,}|_{3,})$/

List         = ListItem+
ListItem     = /^[-+*]\s+/ CONTENT        (unordered)
             | /^\d+\.\s+/ CONTENT         (ordered)
```

### 13.3 Phase 3: Parse diagram blocks

```
DiagramLine  = Comment | Relationship | Component

Comment      = /^#\s+.*/

Relationship = MARKER NAME ARROW NAME (":" LABEL)? Attrs?

Component    = MARKER NAME Attrs

MARKER       = /^[-+*]\s+/

ARROW        = "->" | "<-" | "<->" | "--" | "-->"

Attrs        = /\(([^)]+)\)/
               (comma-separated key: value pairs)

NAME         = /[A-Za-z][A-Za-z0-9 ]*/
               (parsing stops at '(' or ARROW token)
```

### 13.4 Phase 4: Classify layout

```
classify(elements) -> Layout:
    if has(DIAGRAM):           Diagram
    if has(VISUALIZATION):     Visualization
    if is_title_pattern():     Title
    if is_section_divider():   Section
    if single_image():         Image
    if multi_image():          Gallery
    if has(QUOTE):             Quote
    if has(CODE):              Code
    if heading_and_list():     Bullets
    else:                      Content
```

---

## 14. Visualization Syntax

MDeck supports data visualizations as fenced code blocks with `@` language tags. Each visualization type has its own format for data entry and optional directives.

### 14.1 Common Features

All visualization types share these features:

**Reveal markers:** The same `-`, `+`, `*` markers from Section 6 control progressive reveal of data items.

**Comment directives:** Lines starting with `#` inside visualization blocks are parsed as directives (e.g., `# orientation: horizontal`). Use them to configure the visualization.

**Axis labels:** Chart types with axes support `# x-label:` and `# y-label:` directives. The Y-axis label is rendered rotated 90° counter-clockwise.

**Automatic scaling:** All visualizations scale proportionally to the available slide area. Grid lines use "nice" round numbers (1, 2, 5, 10, 20, 25, 50, 100, ...).

### 14.2 Bar Chart (`@barchart`)

Vertical or horizontal bar chart with category labels and values.

````markdown
```@barchart
# orientation: vertical
# x-label: Programming Language
# y-label: Popularity Index
- JavaScript: 65
- Python: 48
+ TypeScript: 38
* Rust: 22
```
````

**Directives:**

| Directive       | Values                      | Default    | Description                |
|-----------------|-----------------------------|------------|----------------------------|
| `orientation`   | `vertical`, `horizontal`    | `vertical` | Bar direction              |
| `x-label`       | string                      | none       | Label for the X axis       |
| `y-label`       | string                      | Label for the Y axis (rotated 90° CCW) |

**Data format:** `- Label: value` or `- Label: value%` (the `%` suffix is stripped).

### 14.3 Line Chart (`@linechart`)

Line chart with one or more data series plotted over shared X-axis categories.

````markdown
```@linechart
# x-labels: Jan, Feb, Mar, Apr, May, Jun
# x-label: Month
# y-label: Temperature (°C)
- London: 5, 6, 10, 14, 17, 20
+ Madrid: 10, 12, 16, 19, 23, 28
```
````

**Directives:**

| Directive   | Values              | Default | Description                              |
|-------------|---------------------|---------|------------------------------------------|
| `x-labels`  | comma-separated     | none    | Category labels along the X axis         |
| `x-label`   | string              | none    | Label for the X axis                     |
| `y-label`   | string              | none    | Label for the Y axis (rotated 90° CCW)   |

**Data format:** `- Series Name: value1, value2, value3, ...`

Each series is a separate line. All series share the X-axis categories. A legend is displayed at the top-right.

### 14.4 Scatter Plot (`@scatter`)

2D scatter plot with labeled data points and optional custom sizes.

````markdown
```@scatter
# x-label: Hours Studied
# y-label: Test Score
- Alice: 80, 90
- Bob: 65, 75
- Carol: 90, 95 (size: 30)
```
````

**Directives:**

| Directive | Values | Default | Description                            |
|-----------|--------|---------|----------------------------------------|
| `x-label` | string | none    | Label for the X axis                   |
| `y-label` | string | none    | Label for the Y axis (rotated 90° CCW) |

**Data format:** `- Label: x, y` or `- Label: x, y (size: N)`

The optional `(size: N)` controls the radius of the data point. Without it, a default radius is used.

### 14.5 Stacked Bar Chart (`@stackedbar`)

Stacked bar chart showing multiple series stacked on top of each other for each category.

````markdown
```@stackedbar
# categories: Q1, Q2, Q3, Q4
# x-label: Quarter
# y-label: Revenue ($M)
- Product A: 40, 45, 50, 55
- Product B: 30, 35, 40, 45
+ Product C: 15, 20, 25, 30
```
````

**Directives:**

| Directive    | Values          | Default | Description                            |
|--------------|-----------------|---------|----------------------------------------|
| `categories` | comma-separated | none    | Category labels along the X axis       |
| `x-label`    | string          | none    | Label for the X axis                   |
| `y-label`    | string          | none    | Label for the Y axis (rotated 90° CCW) |

**Data format:** `- Series Name: value1, value2, value3, ...`

Each series provides one value per category. Values are stacked vertically. A legend is displayed at the top.

### 14.6 Pie Chart (`@piechart`)

Pie chart showing proportional segments. Values are automatically normalized to 100%.

````markdown
```@piechart
- Frontend: 35%
- Backend: 30%
+ DevOps: 20%
* Testing: 15%
```
````

**Data format:** `- Label: value%` or `- Label: value`

### 14.7 Donut Chart (`@donutchart`)

Like a pie chart but with a hollow center that can display a label.

````markdown
```@donutchart
# center: Total
- Completed: 65%
- In Progress: 25%
- Not Started: 10%
```
````

**Directives:**

| Directive | Values | Default | Description                |
|-----------|--------|---------|----------------------------|
| `center`  | string | none    | Text displayed in the center hole |

### 14.8 Word Cloud (`@wordcloud`)

Word cloud with words sized proportionally and laid out using spiral packing. Some words are automatically rotated 90° counter-clockwise for the classic word cloud aesthetic.

````markdown
```@wordcloud
- Artificial Intelligence (size: 50)
- Machine Learning (size: 45)
- Data Science (size: 42)
- Python (size: 38)
- Docker (size: 30)
- React (size: 27)
```
````

**Data format:** `- Word or Phrase (size: N)` or `- Word or Phrase`

The `size` value controls relative importance (larger = bigger font). Without a size, the default is 20. Words are placed largest-first using spiral search. Approximately 35% of words are rendered vertically (rotated 90° CCW) for visual variety; the two largest words are always horizontal.

For best results with word clouds, use 30-100 words with a good spread of sizes (e.g., 10-50).

### 14.9 Timeline (`@timeline`)

Horizontal or vertical timeline showing events in chronological order.

````markdown
```@timeline
- 2000: Y2K Bug
+ 2004: Facebook Founded
+ 2007: iPhone Released
+ 2010: Instagram Launched
```
````

**Data format:** `- Year/Date: Event description`

### 14.10 Funnel Chart (`@funnel`)

Funnel chart showing progressive narrowing stages.

````markdown
```@funnel
- Awareness: 10000
- Interest: 5000
- Consideration: 2500
- Decision: 1000
```
````

**Data format:** `- Stage: value`

### 14.11 KPI Cards (`@kpi`)

Key Performance Indicator cards showing metric values with optional trend indicators.

````markdown
```@kpi
- Revenue: $4.2M (trend: up, change: +12%)
- Users: 1.2M (trend: up, change: +8%)
- Churn: 2.1% (trend: down, change: -0.3%)
```
````

**Data format:** `- Metric: value (trend: up|down|flat, change: text)`

### 14.12 Progress Bars (`@progress`)

Horizontal progress bar indicators.

````markdown
```@progress
- Frontend: 85%
- Backend: 60%
- Testing: 30%
```
````

**Data format:** `- Label: value%`

### 14.13 Radar Chart (`@radar`)

Spider/radar chart comparing multiple items across shared axes.

````markdown
```@radar
# axes: Speed, Power, Range, Agility, Defense
- Fighter A: 9, 7, 5, 8, 6
+ Fighter B: 4, 9, 8, 6, 7
```
````

**Directives:**

| Directive | Values          | Default | Description                    |
|-----------|-----------------|---------|--------------------------------|
| `axes`    | comma-separated | none    | Names of the radar axes        |

**Data format:** `- Series: v1, v2, v3, ...` (one value per axis)

### 14.14 Venn Diagram (`@venn`)

Venn diagram showing set intersections (2-3 sets).

````markdown
```@venn
- Frontend: HTML, CSS, JavaScript, React
- Backend: Python, SQL, Redis, Docker
- DevOps: Docker, AWS, Terraform, CI/CD
```
````

**Data format:** `- Set Name: item1, item2, item3, ...`

Shared items between sets are automatically detected and displayed in the overlapping regions.

### 14.15 Organization Chart (`@orgchart`)

Hierarchical org chart with parent-child relationships.

````markdown
```@orgchart
- CEO
- CTO (parent: CEO)
- VP Engineering (parent: CTO)
- VP Product (parent: CTO)
- CFO (parent: CEO)
```
````

**Data format:** `- Name` or `- Name (parent: ParentName)`

The root node has no `parent` attribute. The chart is drawn as a tree with connecting lines.

### 14.16 Gantt Chart (`@gantt`)

Project timeline with tasks, durations, dependencies, and automatic time scaling.

````markdown
```@gantt
# title: Project Plan
- Research: 2024-01-15, 10d
- Design: 5d, after Research
- Frontend: 15d, after Design
- Backend: 15d, after Design
- Testing: 5wd, after Frontend
- Launch: 2d, after Testing + 3d
```
````

**Task specification:** Each task line has the format `- Task Name: spec1, spec2, ...` where specs can be:

| Spec | Description |
|------|-------------|
| `YYYY-MM-DD` | Absolute date (start or end) |
| `Nd` | Duration in calendar days |
| `Nwd` | Duration in working days (Mon-Fri) |
| `Nw` | Duration in weeks |
| `Nm` | Duration in months (~30 days) |
| `after TaskName` | Start when TaskName ends |
| `after TaskName + Nd` | Start N days after TaskName ends |

**Valid combinations:**
- Start date + end date: `2024-01-01, 2024-02-01`
- Start date + duration: `2024-01-01, 10d`
- Duration + dependency: `5d, after Research`
- Duration + dependency with delay: `3wd, after Design + 2d`

**Directives:**
- `# title: text` — Chart title displayed above the bars
- `# labels: inside` — Render task names inside the bars instead of in a left column. The left label area is removed, giving the full width to the timeline. When a bar is too short for the name, it falls back to showing the name to the right of the bar.

**Timeline auto-scaling:** The time axis automatically selects the appropriate unit:
- Days (for timelines up to ~3 weeks)
- Weeks (for timelines up to ~4 months)
- Months (for longer timelines)

Dependencies are shown as connector arrows between tasks.
