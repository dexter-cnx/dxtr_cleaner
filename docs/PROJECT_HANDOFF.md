# Project Handoff

## Current state

M0 and the M1 typed Smart Scan engine are merged. GPUI Smart Scan wiring is active on
`feature/m1-gpui-smart-scan`.

### Implemented foundation

- Rust workspace with four crates
- `cleaner-core` domain model and filesystem scanner
- cleanup planner and destructive-action safety lock
- `cleaner-macos` platform boundary
- `cleaner-cli` dry-run scan command
- `cleaner-gui` GPUI Smart Care shell
- CI and local `make prepush` quality gates
- architecture and roadmap docs

### M1 scan engine

- typed scan targets for User Cache, Xcode, Homebrew, and Node
- explicit exclusion roots in `ScanRequest`
- scan event sink with started/item/permission/finished/cancelled events
- cooperative `CancellationToken`
- symlink traversal protection retained
- root-level permission errors are not hidden by `Path::exists()`
- tests for events, exclusions, cancellation, missing optional roots, and symlink behavior
- CLI builds requests from typed category targets

### M1 GPUI wiring in progress

- Smart Scan starts filesystem work on a worker thread
- scan events cross into GPUI through a channel
- User Cache, Xcode, and Homebrew cards show live bytes/items
- cancellation is wired to the core `CancellationToken`
- permission-denied paths are surfaced in scan status
- destructive execution remains disabled

### Important safety decision

Destructive execution remains disabled. `ExecutionPolicy::default()` disables destructive
actions and `SystemMacPlatform::move_to_trash` intentionally returns an error.

### GPUI dependency

GPUI and `gpui_platform` are pinned to Zed commit:

`b05f40c5546b47bcf9561136dc0fcdcd9968cb63`

Do not float the dependency. Upgrade in a dedicated dependency PR.

## Remaining M1 tasks

1. Add protected-path policy to scan target/request construction.
2. Validate and harden GPUI background scan wiring against the pinned GPUI API.
3. Add Node to Smart Scan UI once the main scan interaction is stable.
4. Add a deterministic permission-denied test seam/fixture.
5. Keep execution disabled.

## Validation

Before push on a development machine:

```bash
make prepush
```

GitHub Actions runs the non-mutating equivalent:

```bash
make ci
```
