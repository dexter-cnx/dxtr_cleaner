# Project Handoff

## Current state

Milestone M0 foundation scaffold is complete.

### Implemented

- Rust workspace with four crates
- `cleaner-core` domain model and filesystem scanner
- cleanup planner and destructive-action safety lock
- `cleaner-macos` platform boundary
- `cleaner-cli` dry-run scan command
- `cleaner-gui` GPUI Smart Care shell
- CI workflow for format/test/clippy on macOS
- architecture and roadmap docs

### Important safety decision

M0 cannot delete or trash files. `ExecutionPolicy::default()` disables destructive
actions and `SystemMacPlatform::move_to_trash` intentionally returns an error.

### GPUI dependency

GPUI and `gpui_platform` are pinned to Zed commit:

`b05f40c5546b47bcf9561136dc0fcdcd9968cb63`

Do not float the dependency. Upgrade in a dedicated dependency PR.

## Next branch

Suggested: `feature/m1-smart-scan`

### M1 tasks

1. Add typed category scanners instead of generic roots.
2. Add exclusions and protected-path policy.
3. Add scan progress/event sink.
4. Wire Smart Scan button to background scan execution in GPUI.
5. Show live bytes/items per category.
6. Add cancellation.
7. Add test fixtures for permission denied and symlink traversal.
8. Keep execution disabled.

## Validation needed on a Mac

The generation environment did not contain a Rust toolchain, so run:

```bash
rustup show
cargo fmt --all
cargo test -p cleaner-core -p cleaner-macos -p cleaner-cli
cargo clippy -p cleaner-core -p cleaner-macos -p cleaner-cli --all-targets -- -D warnings
cargo check -p cleaner-gui
cargo run -p cleaner-gui
```

Fix any GPUI API drift before starting M1.
