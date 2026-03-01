# Contributing to mdeck

Thank you for considering contributing to MDeck! This guide will help you get started.

## Getting Started

1. Fork the repository and clone your fork
2. Install Rust 1.85+ via [rustup](https://rustup.rs/)
3. Build the project: `cargo build`
4. Run tests: `cargo test`

## Development Workflow

### Project Structure

```
crates/
  mdeck/    # GUI binary and presentation engine
```

### Running Tests

```bash
cargo test                    # All tests
cargo test -p mdeck           # Single crate
cargo test test_name          # Single test
cargo clippy                  # Lint check
```

### Code Style

- Run `cargo clippy` before submitting -- CI will check this
- Follow existing patterns in the codebase
- Add tests for new functionality

## Making Changes

### Bug Fixes

1. Create a branch: `git checkout -b fix/description`
2. Write a test that reproduces the bug
3. Fix the bug
4. Verify all tests pass: `cargo test`
5. Open a pull request

### New Features

1. Open an issue to discuss the feature first
2. Create a branch: `git checkout -b feature/description`
3. Implement with tests
4. Update the README if the feature is user-facing
5. Open a pull request

## Pull Requests

- Keep PRs focused -- one feature or fix per PR
- Include tests for new code paths
- Write a clear description of what changed and why
- CI must pass (build, test, clippy)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
