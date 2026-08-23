# Project Handoff

## Current state

M0 is merged. M1 Smart Scan work is active on `feature/m1-smart-scan`.

### Implemented foundation

- Rust workspace with four crates
- `cleaner-core` domain model and filesystem scanner
- cleanup planner and destructive-action safety lock
- `cleaner-macos` platform boundary
- `cleaner-cli` dry-run scan command
- `cleaner-gui` GPUI Smart Care shell
- CI and local `make prepush` quality gates
- architecture and roadmap docs

### M1 scan engine work

- typed scan targets for User Cache, Xcode, Homebrew, and Node
- explicit exclusion roots in `ScanRequest`
- scan event sink with started/item/permission/finished/cancelled events
- cooperative `CancellationToken`
- symlink traversal protection retained
- tests for events, exclusions, cancellation, and symlink behavior
- CLI now builds requests from typed category targets

### Important safety decision

Destructive execution remains disabled. `ExecutionPolicy::default()` disables destructive
actions and `SystemMacPlatform::move_to_trash` intentionally returns an error.

### GPUI dependency

GPUI and `gpui_platform` are pinned to Zed commit:

`b05f40c5546b47bcf9561136dc0fcdcd9968cb63`

Do not float the dependency. Upgrade in a dedicated dependency PR.

## Remaining M1 tasks

1. Add protected-path policy to scan target/request construction.
2. Wire Smart Scan button to background scan execution in GPUI.
3. Show live bytes/items per category from scan events.
4. Wire cancellation control into GPUI.
5. Add a deterministic permission-denied test seam/fixture.
6. Keep execution disabled.

## Validation

Before push on a development machine:

```bash
make prepush
```

GitHub Actions runs the non-mutating equivalent:

```bash
make ci
```
