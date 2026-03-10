<p align="center">
  <img src="https://raw.githubusercontent.com/mklab-se/mdeck/main/media/mdeck-horizontal.png" alt="mdeck" width="600">
</p>

<h1 align="center">MDeck</h1>

<p align="center">
  A markdown-based presentation tool.<br>
  Write your slides in standard markdown and present them beautifully.
</p>

<p align="center">
  <a href="https://github.com/mklab-se/mdeck/actions/workflows/ci.yml"><img src="https://github.com/mklab-se/mdeck/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/mdeck"><img src="https://img.shields.io/crates/v/mdeck.svg" alt="crates.io"></a>
  <a href="https://github.com/mklab-se/mdeck/releases/latest"><img src="https://img.shields.io/github/v/release/mklab-se/mdeck" alt="GitHub Release"></a>
  <a href="https://github.com/mklab-se/homebrew-tap/blob/main/Formula/mdeck.rb"><img src="https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmklab-se%2Fhomebrew-tap%2Fmain%2FFormula%2Fmdeck.rb&search=%5Cd%2B%5C.%5Cd%2B%5C.%5Cd%2B&label=homebrew&prefix=v&color=orange" alt="Homebrew"></a>
  <a href="https://github.com/mklab-se/mdeck/blob/main/LICENSE.md"><img src="https://img.shields.io/crates/l/mdeck.svg" alt="License"></a>
</p>

## Installation

### Homebrew (macOS / Linux)

```bash
brew install mklab-se/tap/mdeck
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/mklab-se/mdeck/releases).

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

## Usage

```bash
mdeck slides.md      # Present a markdown file
mdeck --help         # Show all commands
mdeck --version      # Show version
```

### Commands

```bash
mdeck spec                             # Print full format specification
mdeck spec --short                     # Print quick reference card
mdeck export slides.md                 # Export slides as PNG images
mdeck ai                               # Show AI status
mdeck ai test                          # Test AI integration
mdeck ai config                        # Open AI config in editor
mdeck ai generate slides.md            # Generate AI images for a presentation
mdeck ai generate-image --prompt "..." # Generate a single image
mdeck ai style list                    # List defined image styles
mdeck config show                      # Display current settings
mdeck config set defaults.theme dark   # Set a config value
mdeck completion zsh                   # Generate shell completions
```

The `mdeck spec` command outputs the complete MDeck markdown format specification, including all supported slide layouts, directives, diagram syntax, and visualization types. This is useful both for humans learning the format and for AI agents that need to understand how to write presentations for MDeck.

### Shell Completions

```bash
# Static completions
mdeck completion bash > ~/.bash_completion.d/mdeck
mdeck completion zsh > ~/.zfunc/_mdeck

# Dynamic completions (recommended)
source <(COMPLETE=bash mdeck)
source <(COMPLETE=zsh mdeck)
```

### Keyboard Controls

| Key | Action |
|-----|--------|
| Right Arrow | Next slide |
| Left Arrow | Previous slide |

## Development

```bash
cargo build              # Build
cargo test --workspace   # Run tests
cargo clippy --workspace -- -D warnings  # Lint
cargo fmt --all -- --check               # Format check
cargo run -p mdeck     # Run the app
```

## Documentation

The format specification ([`crates/mdeck/doc/mdeck-spec.md`](crates/mdeck/doc/mdeck-spec.md)) is the authoritative reference for the MDeck markdown format. It is embedded in the binary and available via `mdeck spec`.

**Important:** The spec must be kept up to date whenever features are added or changed. Any new slide layout, directive, visualization type, or keyboard shortcut must be documented in the spec before release.

## License

MIT
