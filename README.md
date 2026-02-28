# presemd

[![CI](https://github.com/mklab-se/presemd/actions/workflows/ci.yml/badge.svg)](https://github.com/mklab-se/presemd/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/presemd.svg)](https://crates.io/crates/presemd)
[![GitHub Release](https://img.shields.io/github/v/release/mklab-se/presemd)](https://github.com/mklab-se/presemd/releases)
[![Homebrew](https://img.shields.io/badge/homebrew-mklab--se%2Ftap-orange)](https://github.com/mklab-se/homebrew-tap)

A markdown-based presentation tool. Write your slides in standard markdown and present them beautifully.

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
