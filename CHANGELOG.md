# Changelog

All notable changes to this project will be documented in this file.

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
- Sample presentations for testing (`sample-presentations/`)

## [0.1.1] - 2026-02-28

### Added

- Initial implementation with hardcoded demo slides
- Slide transitions: fade and horizontal slide with easing
- Keyboard navigation with arrow keys
- FPS overlay
- `--version` flag support
