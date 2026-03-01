# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Diagram rendering overhaul: proper grid layout, auto-layout, much larger nodes
- Diagram parser: skip comment lines, parse `icon:` and `pos:` metadata, detect all 5 arrow types (`->`, `<-`, `<->`, `--`, `-->`)
- Geometric fallback icons for 15+ node types (user, server, database, cloud, lock, api, cache, etc.)
- AI-generated diagram icons via `mdeck generate-icons <file.md>` command
- Icon images loaded from `media/diagram-icons/{name}.png` when available
- OpenAI DALL-E 3 and Google Gemini Imagen API support for icon generation
- `image_generation` config section for API provider and key
- Edge rendering with straight lines, nearest-face connection points, and proper arrowheads
- Dashed lines for `--` and `-->` arrow types
- Edge label pills with semi-transparent backgrounds
- Parallel edge offset when multiple edges connect the same node pair
- 8 unit tests for diagram parsing
- 4 unit tests for icon collection

### Changed

- Diagram nodes now render as rounded rectangles with icon + label (was: tiny pills in a single row)
- Diagram layout uses grid positioning or auto-layout (was: single horizontal row)

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
