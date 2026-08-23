# Project Handoff

## Current state

M0 and the main M1 Smart Scan interaction are merged. Final M1 hardening is active on
`feature/m1-scan-hardening`.

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
- User Cache excludes Homebrew to prevent double-counted metrics
- scan event sink with started/item/permission/finished/cancelled events
- cooperative `CancellationToken`
- symlink traversal protection retained
- root-level permission errors are not hidden by `Path::exists()`
- protected broad roots are rejected before filesystem traversal
- safety checks lexically normalize parent components such as `..`
- deterministic permission-denied error-classification coverage
- CLI builds requests from typed category targets

### M1 GPUI Smart Scan

- Smart Scan runs filesystem work on a worker thread
- scan events cross into GPUI through a channel
- event draining is bounded per tick so the UI and Cancel control remain responsive
- User Cache, Xcode, Homebrew, and Node cards show live bytes/items
- cancellation is wired to the core `CancellationToken`
- permission-denied paths are surfaced in scan status
- destructive execution remains disabled

### Important safety decision

Destructive execution remains disabled. `ExecutionPolicy::default()` disables destructive
actions and `SystemMacPlatform::move_to_trash` intentionally returns an error.

Protected broad roots are centralized in `cleaner-core` and shared by scan validation and
cleanup-plan execution validation. Descendant paths such as `/Library/Caches` remain eligible
for explicitly defined scanners while broad roots such as `/`, `/System`, and `/Library` are
rejected.

### GPUI dependency

GPUI and `gpui_platform` are pinned to Zed commit:

`b05f40c5546b47bcf9561136dc0fcdcd9968cb63`

Do not float the dependency. Upgrade in a dedicated dependency PR.

## Remaining M1 tasks

1. Validate this final hardening PR with `make ci`.
2. Address review feedback, if any.
3. Keep destructive execution disabled.
4. Merge and mark M1 complete.

## Validation

Before push on a development machine:

```bash
make prepush
```

GitHub Actions runs the non-mutating equivalent:

```bash
make ci
```
