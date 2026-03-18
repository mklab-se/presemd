---
name: release
description: "Release a new version: bump version, update docs, commit, push, and tag"
argument-hint: "<major|minor|patch>"
---

Release a new version of mdeck.

## Input

$ARGUMENTS must be one of: `major`, `minor`, `patch`. If empty or invalid, stop and ask.

## Steps

### 1. Determine the new version

- Read the current version from the `version` field in the workspace `Cargo.toml`
- Apply the semver bump based on $ARGUMENTS:
  - `patch`: 0.2.0 -> 0.2.1
  - `minor`: 0.2.0 -> 0.3.0
  - `major`: 0.2.0 -> 1.0.0
- Show the user: "Releasing mdeck v{OLD} -> v{NEW}"

### 2. Pre-flight checks

- Run `cargo update` to update dependencies to the latest compatible versions
- Run `cargo fmt --all -- --check` — abort if formatting issues
- Run `cargo clippy --workspace -- -D warnings` — abort if warnings
- Run `cargo test --workspace` — abort if any test fails
- Run `git status` — abort if there are uncommitted changes that are NOT documentation or version files

### 3. Verify documentation and spec are up to date

- **`crates/mdeck/doc/mdeck-spec.md`**: Review the format spec against current features. Run `cargo run -p mdeck -- spec` and `cargo run -p mdeck -- spec --short` to verify the output looks correct and covers all implemented features. If new visualizations, layouts, directives, or keyboard shortcuts have been added since the last release, update the spec (and the short reference in `commands/spec.rs`) before proceeding.
- **`README.md`**: Verify features list, command reference, visualization table, and gallery preview images are current.
- **`GALLERY.md`**: If rendering has changed, re-export gallery slides (`cargo run -p mdeck -- export sample-presentations/gallery.md --output-dir media/gallery`) and verify screenshots reflect current rendering.
- **`CLAUDE.md`**: Verify architecture, commands, and patterns sections are accurate.
- If any documentation is out of date, update it now before proceeding to the version bump.

### 4. Bump version numbers

- Update `version` in the root `Cargo.toml` `[workspace.package]` section

### 5. Update CHANGELOG

- **CHANGELOG.md**: Rename the `[Unreleased]` section to `[{NEW_VERSION}] - {TODAY}` (YYYY-MM-DD format). If there is no `[Unreleased]` section, create a new dated entry summarizing changes since the last release

### 6. Verify the build

- Run `cargo build --workspace` to ensure everything compiles with the new version
- Run `cargo test --workspace` once more after version bump

### 7. Commit, push, and tag

- Stage all changed files: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and any updated docs
- Commit with message: `Release v{NEW_VERSION}`
- Push to main: `git push`
- Create and push tag: `git tag v{NEW_VERSION} && git push origin v{NEW_VERSION}`

### 8. Confirm

- Tell the user the release is tagged and pushed
- Remind them that the GitHub Actions release workflow will now build binaries, publish to crates.io, and update the Homebrew tap
