# Changelog

All notable changes to this project will be documented in this file.

## [0.14.0] - 2026-03-22

### Added

- **AI presentation creation** (`mdeck ai create`) — create complete presentations from any content source. Supports text prompts, PDF files, DOCX files, markdown, plain text, and piped stdin input. AI analyzes the content, identifies key points, and generates a structured presentation with speaker notes, visualizations, and image generation markers. Includes interactive mode (`-i`) for guided creation with audience/purpose context, and custom prompt support (`--prompt`) for tailored presentations.
- **Speaker notes** (`???` separator) — add presenter-only notes to any slide. Notes are parsed and stored but never rendered in the presentation. Supports full markdown formatting. Designed to help presenters understand slide intent, especially valuable in AI-generated presentations where notes explain delivery guidance and talking points.
- **Git graph visualization** (`@gitgraph`) — precise, data-driven branch diagrams showing branches as horizontal lanes with commits, forks, and merges. Supports Git Flow and any branching strategy. Progressive reveal builds the graph step by step.

### Changed

- Upgraded ailloy dependency from 0.5 to 0.6.

### Dependencies

- Added `pdf-extract` for PDF text extraction.
- Added `zip` for DOCX text extraction.

## [0.13.0] - 2026-03-20

### Added

- **AI agent skill command** (`mdeck ai skill`) — setup guide and skill file emitter for AI agents like Claude Code. `--emit` outputs a ready-to-save skill file, `--reference` outputs the full format spec and AI reference documentation at runtime.
- **Explicit `mdeck ai status` subcommand** — alias for running `mdeck ai` without arguments.
- **AI reference supplement** (`ai-reference-supplement.md`) — comprehensive CLI and AI image generation reference bundled into the binary for AI agent consumption.

## [0.12.3] - 2026-03-19

### Added

- **Interactive AI config wizard** (`mdeck ai config`) — guided setup for AI providers and models, replacing the previous "open in editor" approach. Powered by ailloy's `config-tui` module.
- **Interactive style creation** (`mdeck ai style add -i`) — AI-assisted style crafting with interactive prompts. `set` is now an alias for `add`.
- **Color-coded edge labels** — architecture diagram edge labels now use the edge's color as background, making it easy to see which label belongs to which connection.

### Changed

- Upgraded ailloy dependency from 0.4 to 0.5 with `config-tui` feature for shared AI status/enable/disable logic.
- AI status, enable, and disable commands now delegate to ailloy's `config_tui` module for consistent behavior across ailloy-based tools.
- Reorganized sample presentations from `sample-presentations/` to `samples/` with subdirectories (`visualizations/`, `layouts/`, `transitions/`).
- Edge label horizontal padding increased for better readability.

## [0.12.2] - 2026-03-11

### Added

- **Nord theme** — an arctic, blue-gray theme inspired by the polar landscape. Calm, muted, and professional. Theme cycling is now dark → light → nord → dark (press `D`).
- **Standardized visualization design tokens** — all 15 visualization types now share centralized constants for font sizes, stroke widths, corner radii, opacities, and swatch sizes, ensuring visual consistency within each theme.
- **Theme-aware trend colors** — KPI cards now use theme-appropriate green/red instead of hardcoded values, ensuring readability across all three themes.

### Changed

- Synchronized font sizes, stroke widths, corner radii, and legend styling across bar charts, stacked bars, line charts, scatter plots, pie/donut charts, radar charts, Venn diagrams, funnel charts, KPI cards, org charts, gantt charts, progress bars, timelines, and word clouds.
- Stacked bar charts now have rounded corners matching regular bar charts.
- Radar chart axis labels reduced from 0.75 to 0.65 for consistency with other visualizations.
- Timeline date/description fonts adjusted for better readability at distance.

## [0.12.1] - 2026-03-11

### Changed

- **Improved README** — rewritten "What is MDeck?" section emphasizing presentation quality, built-in visualizations, and AI-native workflow. Removed minor features from the hero section.
- **Updated gallery images** — refreshed AI-generated visuals in GALLERY.md.

## [0.12.0] - 2026-03-11

### Added

- **AI image generation (`mdeck ai generate`):** Scan a presentation for `![prompt](image-generation)` markers and diagram nodes with `icon: generate-image`, then generate all images in one command. Automatically detects orientation (horizontal for full-slide, vertical for side-panel layouts, square for icons) and rewrites the markdown with actual file paths.
- **Style management (`mdeck ai style`):** Define named image and icon styles in config, set defaults, and override per-presentation via `@image-style` / `@icon-style` frontmatter directives. Hardcoded defaults ensure good results out of the box.
- **Ad-hoc image generation (`mdeck ai generate-image`):** Generate a single image from a prompt with `--prompt`, `--style`, `--output`, and `--icon` flags.
- **Diagram prompt metadata:** Diagram nodes now support `prompt: "..."` in parenthetical metadata for AI icon generation (e.g., `Gateway (icon: generate-image, prompt: "An API gateway")`).
- **Diagram icon aspect ratio preservation:** Non-square icon images are now rendered with correct aspect ratio instead of being stretched.
- **Ungenerated image warning:** Launching a presentation with `image-generation` markers prints a terminal warning suggesting `mdeck ai generate`.
- **Enhanced `mdeck ai test`:** Image generation test now lets you choose between normal image and icon, using the configured default styles.
- **Smart heading-level slide splitting:** Files with a single H1 heading (the common "title + H2 sections" pattern) now automatically split on both H1 and H2 headings. Files with multiple H1s keep the original behavior (only H1 splits). This makes standard markdown files work as presentations without needing explicit `---` separators.
- **`@slide-level` frontmatter directive:** Explicitly control which heading level triggers slide breaks (e.g., `@slide-level: 2` means H1 and H2 both split). Overrides the automatic inference when set.
- **Visual gallery (`GALLERY.md`):** Comprehensive showcase of all layouts, diagrams, and visualizations with exported slide screenshots. Linked from README.
- **Revamped `README.md`:** Restructured with feature overview, visualization table, AI documentation, gallery preview images, and navigation links.

## [0.11.2] - 2026-03-10

### Added

- **Image-aware layouts for Bullet, Code, and Quote slides:** Adding a single image to a bullet, code, or quote slide now renders the content on the left (55%) with the image as a side panel on the right (40%), instead of falling through to the generic Content layout. The Content (fallback) layout also gains the same image-split behavior.

## [0.11.1] - 2026-03-10

### Added

- **Gantt chart visualization (`@gantt`):** Project timelines with tasks, durations, dependencies, and automatic time scaling. Supports absolute dates (`YYYY-MM-DD`), calendar days (`Nd`), working days (`Nwd`), weeks (`Nw`), months (`Nm`), and dependency chains (`after Task`, `after Task + 3d`). Timeline auto-scales between days, weeks, and months based on project span.
- **Gantt weekend shading:** Non-working days (Saturday/Sunday) are shown as subtle gray columns when the timeline is at day-level scale.
- **Gantt labels inside bars (`# labels: inside`):** Option to render task names inside their bars instead of in a left column, giving the full width to the timeline.

### Removed

- **`architecture-diagrams.md`:** Removed redundant standalone diagram documentation. All specifications are now consolidated in `mdeck-spec.md`.

## [0.11.0] - 2026-03-10

### Added

- **Ten new visualization types:** Donut chart (`@donutchart`), line chart (`@linechart`), scatter plot (`@scatter`), stacked bar (`@stackedbar`), funnel chart (`@funnel`), KPI cards (`@kpi`), progress bars (`@progress`), radar chart (`@radar`), Venn diagram (`@venn`), org chart (`@orgchart`)
- **Chart axis labels:** `# x-label:` and `# y-label:` directives for bar chart, line chart, scatter plot, and stacked bar
- **Word cloud improvements:** Elliptical cloud shape, non-linear font size contrast (`t^1.5`), rotation restricted to smallest words only
- **Format specification command:** `mdeck spec` prints the full format spec, `mdeck spec --short` prints a quick reference card
- **Per-visualization test files:** Individual test presentations for each visualization type
- **MDeck intro presentation:** `introducing-mdeck.md` — a real presentation about MDeck itself

### Changed

- **Reorganized sample presentations:** Removed redundant files, added comprehensive `test-all-visualizations.md`

## [0.10.0] - 2026-03-10

### Added

- **Four new visualization types:** Word cloud (`@wordcloud`), timeline (`@timeline`), pie chart (`@piechart`), and bar chart (`@barchart`) — all using the same code-block DSL as diagrams with `@` language tags
- **Reveal step support for visualizations:** All new visualization types support `-` (static), `+` (next step), and `*` (with previous) reveal markers for progressive disclosure
- **Bar and pie chart reveal animations:** Bars grow from zero height/width and pie slices sweep from zero angle when revealed, with smooth ease-in-out easing over 0.4 seconds
- **Mixed content slides:** Visualization layout supports heading + text blocks + visualization on the same slide
- **Bar chart orientations:** Vertical (default) and horizontal via `# orientation: horizontal` directive
- **Bar chart grid labels:** Nice-number algorithm for clean axis labels (20, 40, 60 instead of 23.3, 46.7)
- **Word cloud layout:** Dense spiral placement with area-proportional font sizing, cached for stable positions across frames
- Sample presentation `test-visualizations.md` covering all visualization types

### Changed

- **Larger fonts across all visualizations and diagrams** for better readability in presentation settings: diagram node labels (0.55x → 0.8x), diagram edge labels (0.45x → 0.65x), timeline dates (0.55x → 0.85x), timeline descriptions (0.45x → 0.7x), pie chart legend (0.45x → 0.65x), bar chart labels (0.4x → 0.6x)

## [0.9.1] - 2026-03-10

### Fixed

- **Diagram reveal ordering:** Interleaved nodes and edges now reveal in file order instead of all nodes first then all edges. This fixes diagrams like "Pipeline Growth" where `+ Source -> Build` should appear between `+ Build` and `+ Test`, not after all nodes.
- **False time-jump warnings on Linux:** Raised the time-jump detection threshold from 200ms to 2000ms. The Linux repaint keepalive (500ms) was triggering spurious "power-state gap" incidents every frame cycle, flooding the incident log.

## [0.9.0] - 2026-03-06

### Changed

- **AI integration rewrite:** Migrated from custom AI provider system (direct OpenAI/Gemini API calls via `ureq`) to the [`ailloy`](https://github.com/mklab-se/ailloy) crate for unified AI access with async support
- New AI subcommands: `ai test`, `ai enable`, `ai disable`, `ai config` replace the old `ai init`, `ai status`, `ai remove`
- `ai` (no subcommand) now shows status directly
- `ai test` supports interactive testing of both chat completion and image generation with inline terminal image display (iTerm2, Kitty)
- `ai config` opens the ailloy configuration file in your editor

### Removed

- `generate-icons` command (AI icon generation now handled via ailloy)
- Custom `AiConfig`, `AiProvider`, and `ImageGenProvider` types from config (replaced by ailloy's config system)
- `ureq` and `serde_json` dependencies (replaced by `ailloy` and `tokio`)

## [0.8.1] - 2026-03-04

### Added

- **Power-state resilience (Linux):** More aggressive repaint keepalive (500ms vs 4s) prevents GPU context instability when presenting on battery or while screen-sharing
- **Time-jump detection:** Frame deltas >200ms are detected and all in-flight animation timestamps (transitions, overview zoom, pen strokes, arrows, toasts, reveal steps) are shifted forward so animations resume smoothly instead of snapping to completion
- Time-jump incidents are logged to the incident log for diagnostics
- Incident log header now includes `XDG_CURRENT_DESKTOP` and `DESKTOP_SESSION` environment variables for better desktop environment diagnostics

## [0.8.0] - 2026-03-03

### Added

- **Incident logging:** Lightweight `IncidentLog` module records all recovered and fatal errors (display errors, file watcher errors, reload failures) to `~/.config/mdeck/logs/incident-YYYY-MM-DD-HHMMSS.log` for diagnostics
- Log files are created lazily — no file is written during normal operation
- At session end, if any incidents occurred, the log file path is printed to stderr
- Log header includes version, presentation file, OS/arch, and display-related environment variables (DISPLAY, WAYLAND_DISPLAY, XDG_SESSION_TYPE) for Linux debugging
- File watcher errors are now logged (previously silently ignored)
- File reload errors are now logged in addition to the existing toast notification

## [0.7.1] - 2026-03-02

### Removed

- Debug frame profiling that wrote `/tmp/mdeck-profile.log` on every exit

## [0.7.0] - 2026-03-02

### Added

- **"The End" slide:** Virtual end slide shown when navigating past the last slide, with centered "The End" title and MDeck logo/attribution in the bottom-right corner
- **Blackout mode:** Press `.` (period) to toggle screen to solid black for audience attention; press `.` again to resume
- **`--check` CLI flag:** Validate presentations without launching the GUI — reports diagram routing warnings with exit code 1 on problems, 0 on success
- Structured warning system (`CheckReport`, `CheckWarning`, `CheckCategory`) for extensible presentation validation
- Diagram route warnings collected once during background precache instead of per-frame `eprintln!` spam
- Brief one-liner warning summary printed to stderr in GUI mode when routing issues are found

### Changed

- Replaced `precache_all_diagrams_background` with `precache_all_diagrams_with_report` that returns a `CheckReport` via channel
- Removed noisy per-frame `eprintln!("ROUTE WARNING: ...")` from `draw_diagram_sized`; fallback drawing logic preserved
- HUD (press H) now shows `.` blackout shortcut

## [0.6.0] - 2026-03-02

### Added

- Background pre-caching of diagram routes: all diagrams are pre-computed on a background thread at startup and after file reload, making transitions to diagram slides instant
- Diagram scale-to-fit: large diagrams (3+ rows) that overflow the slide area are automatically scaled down to fit
- `# scale:` directive in diagram blocks: `fit` (default), `scroll`, or a numeric factor (e.g. `0.7`)

### Changed

- Diagram route cache upgraded from thread-local `RefCell` to global `Mutex`, enabling cross-thread cache sharing between background precache and render threads
- Removed per-frame adjacent-slide precaching in favor of whole-presentation background precaching

## [0.5.0] - 2026-03-02

### Added

- Live file watching: presentation auto-reloads when the markdown file is saved, with slide position preservation
- Configurable routing cost weights (`routing.length`, `routing.turn`, `routing.lane_change`, `routing.crossing`) in config
- Crossing-aware edge routing: A* search now penalizes routes that cross existing edges
- Crossing detection at junctions and empty cell centers for perpendicular and pass-through segments
- Turn-conflict detection for lanes adjacent to turning routes
- 37 new unit tests for crossing avoidance, routing weights, and file watcher

### Changed

- Edge routing engine uses weighted cost function (length + turns + lane changes + crossings) instead of simple path length

## [0.4.0] - 2026-03-02

### Added

- Diagram rendering overhaul: proper grid layout, auto-layout, much larger nodes
- Diagram parser: skip comment lines, parse `icon:` and `pos:` metadata, detect all 5 arrow types (`->`, `<-`, `<->`, `--`, `-->`)
- Geometric fallback icons for 15+ node types (user, server, database, cloud, lock, api, cache, etc.)
- AI-generated diagram icons via `mdeck generate-icons <file.md>` command
- Icon images loaded from `media/diagram-icons/{name}.png` when available
- OpenAI DALL-E 3 and Google Gemini Imagen API support for icon generation
- `image_generation` config section for API provider and key
- Orthogonal edge routing engine with A* pathfinding and lane allocation
- Edge rendering with rounded corners, proper arrowheads, and lane-aligned connections
- Dashed lines for `--` and `-->` arrow types
- Edge label pills with semi-transparent backgrounds
- Diagram debug overlay (press R) showing routing details
- Gallery layout for image-heavy slides
- 244 unit tests covering parsing, routing, and rendering

### Changed

- Diagram nodes now render as rounded rectangles with icon + label (was: tiny pills in a single row)
- Diagram layout uses grid positioning or auto-layout (was: single horizontal row)

### Fixed

- Arrow port offsets now derived from lane assignments, eliminating diagonal "lane-switching" segments
- Entry face computation corrected with `.opposite()` to match routing direction
- Edge labels moved to 20% along polyline to prevent overlap on opposing edges (A->B and B->A)
- Debug overlay route format now shows lane labels between coordinates per routing spec

## [0.3.0] - 2026-02-28

### Changed

- Renamed project from `presemd` to `mdeck` across the entire codebase
- Binary name changed from `presemd` to `mdeck`
- Config directory changed from `~/.config/presemd/` to `~/.config/mdeck/`
- Crate name changed from `presemd` to `mdeck` on crates.io
- Homebrew formula changed from `presemd` to `mdeck`
- Repository URL changed from `mklab-se/presemd` to `mklab-se/mdeck`

## [0.2.0] - 2026-02-28

### Added

- Full CLI with clap: `mdeck <file.md>` to launch presentations
- Subcommands: `ai init/status/remove`, `config show/set`, `completion`, `export`, `spec`, `version`
- Shell completions for bash, zsh, fish, and powershell (static and dynamic)
- AI provider configuration with auto-detection (Claude, Codex, Copilot, Ollama)
- YAML-based configuration at `~/.config/mdeck/config.yaml`
- Configurable defaults: theme, transition, aspect ratio, start mode
- Global flags: `--verbose`, `--quiet`, `--no-color`, `--windowed`
- `--slide <N>` flag to start on a specific slide (1-indexed)
- `--overview` flag to start in grid overview mode
- `defaults.start_mode` config setting (`first`, `overview`, or slide number)
- Grid overview: mouse hover highlight, click to select slide, mouse wheel scrolling
- Grid overview: fade gradients at screen edges when content overflows
- Grid overview: presentation title shown instead of generic "Slide Overview"
- Freehand pen annotations (left-drag) with outline/glow effect
- Arrow annotations (right-drag) with large arrowhead and drop shadow
- Distinct colors: pen strokes in cyan/blue, arrows in yellow-orange/red
- ESC clears drawings on current slide before double-tap-to-quit
- Mouse input: left-click forward, right-click backward, scroll wheel for content
- PNG export via `mdeck export` with configurable resolution
- Format specification via `mdeck spec` (full and `--short` quick reference)
- Sample presentations for testing (`samples/`)

## [0.1.1] - 2026-02-28

### Added

- Initial implementation with hardcoded demo slides
- Slide transitions: fade and horizontal slide with easing
- Keyboard navigation with arrow keys
- FPS overlay
- `--version` flag support
