# mdeck

A markdown-based presentation tool.

## Design Principles

- **Visual appeal is paramount.** Every rendered element — text, code, transitions, scroll effects — must look polished and professional. Prefer smooth animations over instant state changes.
- **Simplicity over complexity.** Fewer controls, fewer options, fewer edge cases. The tool should feel effortless to use. When in doubt, leave it out.
- **Any markdown file should be presentable.** Overflow handling, layout inference, and sensible defaults mean users shouldn't need to tailor their markdown to the tool.

### Visualization Design Principles

All visualizations (charts, diagrams, etc.) must follow these principles:

- **Readability from distance.** This is a presentation tool — audiences may be far from the screen. All text must be large enough to read from the back of a room. Font sizes should be consistent across similar elements in all visualization types.
- **Space and margins.** Use generous padding and margins. Visualizations should fill available space without feeling cramped, but maintain a feeling of breathing room and negative space. Avoid tiny text crammed into corners.
- **Consistent font sizing.** Define a few standard font size categories (labels ~0.65-0.75, values ~0.55-0.65, grid labels ~0.55, legends ~0.65) and reuse them across all visualizations. Never go below 0.5 for any readable text.
- **Visual polish.** Smooth animations, subtle colors, no harsh borders between stacked elements. Prefer transparent overlapping areas (like in Venn/radar) over opaque blocking.

## Commands

```bash
cargo build              # Build all crates
cargo test --workspace   # Run all tests
cargo clippy --workspace -- -D warnings  # Lint (CI-enforced)
cargo fmt --all -- --check               # Format check (CI-enforced)
cargo run -p mdeck     # Run the app
```

## Architecture

Rust workspace with a single crate:

```
crates/
  mdeck/           # GUI binary (package and binary name: mdeck)
    src/
      main.rs        # Entry point, CLI bootstrap
      cli.rs         # Clap argument definitions (Cli, Commands, subcommands)
      app.rs         # GUI presentation app (eframe/egui rendering)
      banner.rs      # Version banner display
      config.rs      # Config struct, load/save (~/.config/mdeck/config.yaml)
      commands/
        mod.rs       # Re-exports
        ai.rs        # AI provider init/status/remove
        completion.rs # Shell completion generation
        config.rs    # Config show/set
        export.rs    # PNG export via headless eframe rendering
        spec.rs      # Format specification printer
      parser/          # Markdown-to-slide parser (frontmatter, blocks, inlines, splitter)
      render/          # Slide rendering engine
        mod.rs       # render_slide entry point, content height measurement
        text.rs      # Block-level drawing (headings, lists, code, tables, diagrams, images)
        syntax.rs    # Syntax highlighting via syntect (LazyLock-cached SyntaxSet/ThemeSet)
        transition.rs # Slide transitions (fade, slide, spatial) with easing
        layouts/     # Layout strategies (title, section, bullet, code, content, two_column, quote, image_slide)
        image_cache.rs # Async image loading and caching
      theme.rs       # Theme definitions (light, dark, solarized, etc.)
    doc/
      mdeck-spec.md  # Markdown format specification (included via include_str!)
```

- **Workspace root** `Cargo.toml` defines shared dependencies and metadata
- All crates inherit `version`, `edition`, `authors`, `license`, `repository` from workspace
- Single version bump in root `Cargo.toml` updates everything

## CLI Usage

```bash
mdeck <file.md>              # Launch presentation
mdeck <file.md> --check      # Validate presentation (exit code 1 if warnings)
mdeck ai                     # Show AI status (chat + image)
mdeck ai test                # Test AI integration
mdeck ai enable              # Enable AI features
mdeck ai disable             # Disable AI features
mdeck ai config              # Open AI config in editor
mdeck config show            # Display configuration
mdeck config set <key> <val> # Set config value (defaults.theme, defaults.transition, defaults.aspect)
mdeck export <file.md>       # Export slides as PNG images (1920x1080 default)
mdeck export <file.md> --width 3840 --height 2160  # Export at custom resolution
mdeck completion <shell>     # Generate shell completions (bash, zsh, fish, powershell)
mdeck spec                   # Print format specification
mdeck spec --short           # Print quick reference card
mdeck version                # Show version banner
mdeck --help                 # Show help
```

## Key Patterns

- **CLI framework:** `clap` with derive macros, `clap_complete` for shell completions
- **GUI framework:** `eframe` / `egui`
- **Config:** YAML via `serde_yaml`, stored at `~/.config/mdeck/config.yaml` (via `dirs`)
- **Interactive prompts:** `inquire` for selections (e.g., AI provider picker)
- **Terminal output:** `colored` for styled CLI output
- **Error handling:** `anyhow` for ergonomic error propagation
- **Rendering:** Scale factor `min(w/1920, h/1080)` applied to all pixel sizes for resolution independence
- **Syntax highlighting:** `syntect` with `LazyLock`-cached `SyntaxSet` / `ThemeSet`; theme maps to syntect theme via `Theme::syntect_theme_name()`
- **PNG export:** Headless eframe window using `ViewportCommand::Screenshot` / `Event::Screenshot`
- **Transitions:** fade, horizontal slide, spatial (directional pan), with smooth easing; animated overview zoom in/out
- **Scroll/overflow:** Per-slide smooth animated scroll with fade gradients; Up/Down keys; `scroll_targets` + lerp for animation
- **Keyboard:** Space/N/Right forward, P/Left back, Up/Down scroll, G grid, T transition, D theme, F fullscreen, H HUD, `.` blackout, Esc×2 exit
- **End slide:** Virtual "The End" slide with MDeck logo shown when navigating past the last slide
- **Diagrams:** Grid layout (when `pos:` specified) or auto-layout; geometric fallback icons; AI-generated icon images from `media/diagram-icons/`; 5 arrow types (`->`, `<-`, `<->`, `--`, `-->`)
- **AI integration:** `ailloy` crate for unified AI access (chat + image generation); config via `~/.config/ailloy/config.yaml`; async via `tokio`
- FPS overlay in top-right corner

## Releasing

1. Bump `version` in root `Cargo.toml`
2. Commit and push to main
3. Tag: `git tag v0.X.Y && git push origin v0.X.Y`
4. Release workflow builds binaries (Linux, macOS Intel+ARM, Windows), creates GitHub Release, updates Homebrew tap (`mklab-se/homebrew-tap`), publishes to crates.io

**Required GitHub secrets:**
- `CARGO_REGISTRY_TOKEN` (in `crates-io` environment)
- `HOMEBREW_TAP_TOKEN` (GitHub PAT with repo scope for `mklab-se/homebrew-tap`)

## Code Style

- Edition 2024, MSRV 1.85
- `cargo clippy` with `-D warnings` (zero warnings policy)
- `cargo fmt` enforced in CI
- **File size guideline:** When a source file exceeds ~500 lines, evaluate whether it would benefit from being split into smaller modules (`mod` in Rust). Look for natural boundaries: distinct type groups, self-contained algorithms, test helpers, or feature areas that could live in their own files. Propose a split plan before refactoring.

## Quality Requirements

### Testing
- **Always run the full test suite before declaring work complete:** `cargo test --workspace`
- **Always run the full CI check before pushing:** `cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo test --workspace`
- Write unit tests for all new functionality
- Test edge cases and error paths, not just the happy path
- **Every bug fix must include a regression test.** When fixing a bug, first write a test that reproduces it (fails before the fix, passes after). This prevents the bug from coming back and documents the expected behavior.
- When fixing incorrect tests, explain why the original assertion was wrong before updating it

### Visual Testing
- **Always verify rendering changes visually before declaring work complete.** Use the export command to generate slide PNGs and inspect them:
  ```bash
  cargo run -p mdeck -- export sample-presentations/test-code.md --output-dir /tmp/slides
  ```
  Then read the exported PNGs to check layout, syntax highlighting, spacing, and overall visual quality.
- Test presentations in `sample-presentations/` cover specific layouts: `test-bullet.md`, `test-code.md`, `poker-night.md`, etc.
- **Per-visualization test files** exist for efficient testing of individual visualization types:
  - `test-viz-barchart.md` — bar charts (vertical, horizontal, axis labels, reveal)
  - `test-viz-donutchart.md` — donut charts (center text, no center, reveal)
  - `test-viz-funnel.md` — funnel charts (basic, detailed, reveal)
  - `test-viz-kpi.md` — KPI cards (basic, many metrics, reveal)
  - `test-viz-linechart.md` — line charts (single/multi series, axis labels)
  - `test-viz-orgchart.md` — org charts (basic, deep hierarchy, reveal)
  - `test-viz-piechart.md` — pie charts (basic, many slices, reveal)
  - `test-viz-progress.md` — progress bars (basic, many bars, reveal)
  - `test-viz-radar.md` — radar charts (single/multi series, reveal)
  - `test-viz-scatter.md` — scatter plots (basic, sized, axis labels, reveal)
  - `test-viz-stacked-bar.md` — stacked bar charts (basic, axis labels, reveal)
  - `test-viz-timeline.md` — timelines (basic, long, reveal)
  - `test-viz-venn.md` — Venn diagrams (2-set, 3-set, reveal)
  - `test-viz-wordcloud.md` — word clouds (large, small, progressive reveal)
  - `test-diagram.md` — diagrams (auto-layout, grid, arrow types, reveal)
  - `test-all-visualizations.md` — comprehensive test with every visualization type
  When working on a specific visualization type, use its dedicated test file for faster iteration.
- When fixing visual issues, export before and after to confirm the fix.

### Runtime Testing
- **After making changes, run the application and check for runtime incidents.** Launch a relevant test presentation, navigate through slides, then check for errors:
  ```bash
  cargo run -p mdeck -- sample-presentations/test-all-visualizations.md
  ```
  After quitting, if the application reports incidents, read the log file and fix any issues. Incident logs are at `~/Library/Application Support/mdeck/logs/` (macOS) or `~/.config/mdeck/logs/` (Linux).
- Common issues to watch for: `time_jump` false positives (threshold must exceed the repaint heartbeat interval), rendering panics, and layout overflow.

### Documentation & Sample Presentations
- **Before considering any task done, ensure all documentation and sample presentations are up to date.** This is a blocking requirement — incomplete docs or outdated samples mean the task is not finished.
- **Review all documentation for accuracy before pushing or releasing:**
  - `README.md` — features, quick start, badges
  - `CHANGELOG.md` — new entries for every user-visible change
  - `CLAUDE.md` — architecture, commands, patterns
  - `crates/mdeck/doc/mdeck-spec.md` — format specification (embedded in binary via `mdeck spec`)
- **The format spec (`mdeck-spec.md`) must be updated whenever features are added or changed.** This includes new visualization types, directives, keyboard shortcuts, layouts, or any other user-facing feature. The spec is used by both humans and AI agents to understand how to write presentations.
- **Sample presentations must reflect all features.** When adding a new visualization type, layout, or feature:
  - Add it to `test-all-visualizations.md` (comprehensive showcase)
  - Create a dedicated `test-viz-<type>.md` or `test-<feature>.md` file
  - Update `introducing-mdeck.md` if the feature is significant enough for the intro presentation
- When adding new commands, flags, or crates, update all relevant docs in the same commit
- `CHANGELOG.md` must be updated for every release with a dated entry following Keep a Changelog format
