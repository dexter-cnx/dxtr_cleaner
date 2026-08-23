# Project Handoff

## Current state

M0 is complete. M1 Smart Scan is functionally complete on `feature/m1-complete-scanners` and is awaiting CI/review before merge.

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

- typed scan targets for User Cache, System Cache, Xcode, Homebrew, and Node
- Node target covers npm, pnpm, Yarn Classic, and Yarn Berry caches
- explicit exclusion roots in `ScanRequest`
- User Cache excludes Homebrew and Yarn caches to prevent double-counted metrics
- scan event sink with started/item/permission/finished/cancelled events
- cooperative `CancellationToken`
- symlink traversal protection retained
- root-level permission errors are not hidden by `Path::exists()`
- protected broad roots are rejected before filesystem traversal
- protected-root checks use filesystem-aware canonicalization when possible and conservative normalized fallback semantics
- deterministic permission-denied error-classification coverage
- CLI supports User Cache, System Cache, Xcode, Homebrew, and Node dry-run scans

### M1 GPUI Smart Scan

- Smart Scan runs filesystem work on a worker thread
- scan events cross into GPUI through a channel
- event draining is bounded per tick so the UI and Cancel control remain responsive
- User Cache, System Cache, Xcode, Homebrew, and Node cards show live bytes/items
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

## Next milestone after merge

M2 — review + cleaning:

1. cleanup plan review UI
2. move-to-trash where appropriate
3. permanent-delete policy by category
4. symlink-safe canonicalization and execution-time revalidation
5. allow-list enforcement
6. execution report
7. cancellation

Keep destructive execution disabled until the M2 review/revalidation boundaries are implemented and tested.

## Validation

Before push on a development machine:

```bash
make prepush
```

GitHub Actions runs the non-mutating equivalent:

```bash
make ci
```
