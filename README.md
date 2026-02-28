<p align="center">
  <img src="https://raw.githubusercontent.com/mklab-se/presemd/main/media/presemd-horizontal.png" alt="presemd" width="600">
</p>

<h1 align="center">presemd</h1>

<p align="center">
  A markdown-based presentation tool.<br>
  Write your slides in standard markdown and present them beautifully.
</p>

<p align="center">
  <a href="https://github.com/mklab-se/presemd/actions/workflows/ci.yml"><img src="https://github.com/mklab-se/presemd/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/presemd"><img src="https://img.shields.io/crates/v/presemd.svg" alt="crates.io"></a>
  <a href="https://github.com/mklab-se/presemd/releases/latest"><img src="https://img.shields.io/github/v/release/mklab-se/presemd" alt="GitHub Release"></a>
  <a href="https://github.com/mklab-se/homebrew-tap/blob/main/Formula/presemd.rb"><img src="https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmklab-se%2Fhomebrew-tap%2Fmain%2FFormula%2Fpresemd.rb&search=%5Cd%2B%5C.%5Cd%2B%5C.%5Cd%2B&label=homebrew&prefix=v&color=orange" alt="Homebrew"></a>
  <a href="https://github.com/mklab-se/presemd/blob/main/LICENSE.md"><img src="https://img.shields.io/crates/l/presemd.svg" alt="License"></a>
</p>

## Installation

### Homebrew (macOS / Linux)

```bash
brew install mklab-se/tap/presemd
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/mklab-se/presemd/releases).

### Cargo

```bash
cargo install presemd
```

### Build from source

```bash
git clone https://github.com/mklab-se/presemd.git
cd presemd
cargo install --path crates/presemd
```

## Usage

```bash
presemd                # Launch the presentation viewer
presemd --version      # Show version
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
cargo run -p presemd     # Run the app
```

## License

MIT
