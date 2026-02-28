# presemd

A markdown-based presentation tool.

## Commands

```bash
cargo build              # Build all crates
cargo test --workspace   # Run all tests
cargo clippy --workspace -- -D warnings  # Lint (CI-enforced)
cargo fmt --all -- --check               # Format check (CI-enforced)
cargo run -p presemd     # Run the app
```

## Architecture

Rust workspace with a single crate:

```
crates/
  presemd/           # GUI binary (package and binary name: presemd)
    src/
      main.rs        # Entry point, slide rendering, transitions
```

- **Workspace root** `Cargo.toml` defines shared dependencies and metadata
- All crates inherit `version`, `edition`, `authors`, `license`, `repository`, `rust-version` from workspace
- Single version bump in root `Cargo.toml` updates everything

## Key Patterns

- GUI framework: `eframe` / `egui`
- Slide transitions: fade and horizontal slide with easing
- Keyboard navigation: arrow keys for forward/backward
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

## Quality Requirements

### Testing
- **Always run the full test suite before declaring work complete:** `cargo test --workspace`
- **Always run the full CI check before pushing:** `cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo test --workspace`
- Write unit tests for all new functionality
- Test edge cases and error paths, not just the happy path

### Documentation
- **Before pushing or releasing, review all documentation for accuracy:**
  - `README.md` — features, quick start, badges
  - `CHANGELOG.md` — new entries for every user-visible change
  - `CLAUDE.md` — architecture, commands, patterns
- When adding new commands, flags, or crates, update all relevant docs in the same commit
- `CHANGELOG.md` must be updated for every release with a dated entry following Keep a Changelog format
