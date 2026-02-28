---
name: release
description: "Release a new version: bump version, update docs, commit, push, and tag"
argument-hint: "<major|minor|patch>"
---

Release a new version of presemd.

## Input

$ARGUMENTS must be one of: `major`, `minor`, `patch`. If empty or invalid, stop and ask.

## Steps

### 1. Determine the new version

- Read the current version from the `version` field in the workspace `Cargo.toml`
- Apply the semver bump based on $ARGUMENTS:
  - `patch`: 0.2.0 -> 0.2.1
  - `minor`: 0.2.0 -> 0.3.0
  - `major`: 0.2.0 -> 1.0.0
- Show the user: "Releasing presemd v{OLD} -> v{NEW}"

### 2. Pre-flight checks

- Run `cargo fmt --all -- --check` — abort if formatting issues
- Run `cargo clippy --workspace -- -D warnings` — abort if warnings
- Run `cargo test --workspace` — abort if any test fails
- Run `git status` — abort if there are uncommitted changes that are NOT documentation or version files

### 3. Bump version numbers

- Update `version` in the root `Cargo.toml` `[workspace.package]` section

### 4. Update documentation

- **CHANGELOG.md**: Rename the `[Unreleased]` section to `[{NEW_VERSION}] - {TODAY}` (YYYY-MM-DD format). If there is no `[Unreleased]` section, create a new dated entry summarizing changes since the last release
- **README.md**: Review for accuracy — update any version references if present
- **CLAUDE.md**: Review for accuracy — no version references to update typically

### 5. Verify the build

- Run `cargo build --workspace` to ensure everything compiles with the new version
- Run `cargo test --workspace` once more after version bump

### 6. Commit, push, and tag

- Stage all changed files: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and any updated docs
- Commit with message: `Release v{NEW_VERSION}`
- Push to main: `git push`
- Create and push tag: `git tag v{NEW_VERSION} && git push origin v{NEW_VERSION}`

### 7. Confirm

- Tell the user the release is tagged and pushed
- Remind them that the GitHub Actions release workflow will now build binaries, publish to crates.io, and update the Homebrew tap
