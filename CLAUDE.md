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
        ai.rs        # AI provider init/status/remove/style management
        create.rs    # AI presentation creation from any content (mdeck ai create)
        generate.rs  # AI image generation for presentations (mdeck ai generate)
        completion.rs # Shell completion generation
        config.rs    # Config show/set
        export.rs    # PNG export via headless eframe rendering
        skill.rs     # AI agent skill setup, emit, and reference output
        spec.rs      # Format specification printer
      parser/          # Markdown-to-slide parser (frontmatter, blocks, inlines, splitter)
      render/          # Slide rendering engine
        mod.rs       # render_slide entry point, content height measurement
        text.rs      # Block-level drawing (headings, lists, code, tables, diagrams, images)
        syntax.rs    # Syntax highlighting via syntect (LazyLock-cached SyntaxSet/ThemeSet)
        transition.rs # Slide transitions (fade, slide, spatial) with easing
        layouts/     # Layout strategies (title, section, bullet, code, content, two_column, quote, image_slide)
        image_cache.rs # Async image loading and caching
      theme.rs       # Theme definitions (light, dark, nord)
      prompt.rs      # AI prompt construction helpers (image/icon style + orientation)
    doc/
      mdeck-spec.md  # Markdown format specification (included via include_str!)
      ai-reference-supplement.md  # AI reference docs (CLI + image generation guide)
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
mdeck ai create --input <file-or-text> --output <path>  # Create presentation from content
mdeck ai create -i           # Interactive presentation creation
mdeck ai create --prompt "..." # With audience/purpose context
mdeck ai generate <file.md>  # Generate all AI images in a presentation
mdeck ai generate-image --prompt "..." [--icon] [--output path] [--style name]
mdeck ai style add <name> <desc> [--icon]  # Add named style
mdeck ai style remove <name> [--icon]      # Remove named style
mdeck ai style list          # List all styles
mdeck ai style clear         # Remove all styles
mdeck ai style set-default <name>       # Set default image style
mdeck ai style set-icon-default <name>  # Set default icon style
mdeck ai style show-defaults            # Show current defaults
mdeck ai skill               # AI agent skill setup guide
mdeck ai skill --emit        # Output skill file for Claude Code
mdeck ai skill --reference   # Output full reference for AI agents
mdeck ai status              # Show AI status (explicit alias)
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
- **Keyboard:** Space/N/Right forward, P/Left back, Up/Down scroll, G grid, T transition, Shift+T theme, F fullscreen, H HUD, `.` blackout, Esc×2 exit
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

### Pre-Release Checklist

Before every release, verify these are up to date:

- **`GALLERY.md`** — Re-export gallery slides (`mdeck export samples/gallery.md --output-dir media/gallery`) and verify all screenshots reflect current rendering. If new visualization types, layouts, or features have been added, add them to `samples/gallery.md` and regenerate.
- **`README.md`** — Ensure features list, command reference, visualization table, and AI section are current. Check that gallery preview images look correct. The README is the first thing users see — it must provide an excellent experience.
- **`CHANGELOG.md`** — Dated entry with all user-visible changes.
- **`crates/mdeck/doc/mdeck-spec.md`** — Format spec reflects all current features.
- **`samples/`** — Test presentations cover all features; dedicated test files exist for each visualization type.
- **AI capabilities** — README documents AI image generation, style management, and diagram icon generation with clear examples.

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
  cargo run -p mdeck -- export samples/layouts/code.md --output-dir /tmp/slides
  ```
  Then read the exported PNGs to check layout, syntax highlighting, spacing, and overall visual quality.
- Test presentations in `samples/` are organized into subdirectories:
  - **`samples/visualizations/`** — per-visualization test files:
    - `barchart.md` — bar charts (vertical, horizontal, axis labels, reveal)
    - `donutchart.md` — donut charts (center text, no center, reveal)
    - `funnel.md` — funnel charts (basic, detailed, reveal)
    - `kpi.md` — KPI cards (basic, many metrics, reveal)
    - `linechart.md` — line charts (single/multi series, axis labels)
    - `orgchart.md` — org charts (basic, deep hierarchy, reveal)
    - `gantt.md` — gantt charts (basic, dependencies, delays, long timeline, many tasks, reveal)
    - `piechart.md` — pie charts (basic, many slices, reveal)
    - `progress.md` — progress bars (basic, many bars, reveal)
    - `radar.md` — radar charts (single/multi series, reveal)
    - `scatter.md` — scatter plots (basic, sized, axis labels, reveal)
    - `stacked-bar.md` — stacked bar charts (basic, axis labels, reveal)
    - `timeline.md` — timelines (basic, long, reveal)
    - `venn.md` — Venn diagrams (2-set, 3-set, reveal)
    - `wordcloud.md` — word clouds (large, small, progressive reveal)
    - `architecture.md` — architecture diagrams (auto-layout, grid, arrow types, reveal)
    - `all.md` — comprehensive test with every visualization type
  - **`samples/layouts/`** — per-layout test files:
    - `bullet.md`, `code.md`, `content.md`, `quote.md`, `section.md`, `title.md`, `two-column.md`
    - `image.md`, `image-layouts.md` — image split layouts (bullet+image, code+image, quote+image, content+image)
    - `image-generation.md` — AI image generation markers (image-generation, diagram icon prompts, @image-style)
    - `gallery.md` — gallery layout (2/3/4 images)
    - `layouts.md` — mixed layout test
  - **`samples/transitions/`** — per-transition test files: `fade.md`, `slide.md`, `spatial.md`, `none.md`
  - **`samples/features/`** — feature-specific test files:
    - `notes.md` — speaker notes with `???` separator
  - **Top-level `samples/`** — showcase presentations: `gallery.md`, `introducing-mdeck.md`, `poker-night.md`, `saloon-workshop.md`, `continents.md`
  When working on a specific visualization type, use its dedicated test file for faster iteration.
- When fixing visual issues, export before and after to confirm the fix.

### Runtime Testing
- **After making changes, run the application and check for runtime incidents.** Launch a relevant test presentation, navigate through slides, then check for errors:
  ```bash
  cargo run -p mdeck -- samples/visualizations/all.md
  ```
  After quitting, if the application reports incidents, read the log file and fix any issues. Incident logs are at `~/Library/Application Support/mdeck/logs/` (macOS) or `~/.config/mdeck/logs/` (Linux).
- Common issues to watch for: `time_jump` false positives (threshold must exceed the repaint heartbeat interval), rendering panics, and layout overflow.

### Documentation & Sample Presentations
- **Before considering any task done, ensure all documentation and sample presentations are up to date.** This is a blocking requirement — incomplete docs or outdated samples mean the task is not finished.
- **Review all documentation for accuracy before pushing or releasing:**
  - `README.md` — features, quick start, badges, gallery preview images
  - `GALLERY.md` — visual showcase with exported slide screenshots from `media/gallery/`
  - `CHANGELOG.md` — new entries for every user-visible change
  - `CLAUDE.md` — architecture, commands, patterns
  - `crates/mdeck/doc/mdeck-spec.md` — format specification (embedded in binary via `mdeck spec`)
- **The format spec (`mdeck-spec.md`) must be updated whenever features are added or changed.** This includes new visualization types, directives, keyboard shortcuts, layouts, or any other user-facing feature. The spec is used by both humans and AI agents to understand how to write presentations.
- **Sample presentations must reflect all features.** When adding a new visualization type, layout, or feature:
  - Add it to `samples/visualizations/all.md` (comprehensive showcase)
  - Create a dedicated file in `samples/visualizations/` or `samples/layouts/`
  - Update `introducing-mdeck.md` if the feature is significant enough for the intro presentation
- When adding new commands, flags, or crates, update all relevant docs in the same commit
- `CHANGELOG.md` must be updated for every release with a dated entry following Keep a Changelog format
