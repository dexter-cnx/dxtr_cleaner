# Project Handoff

## Current state

M0 and M1 Smart Scan are complete and merged to `main`.

M2 is in progress on `feature/m2-review-cleaning`. The first M2 slice adds a frontend-neutral cleanup-plan review model plus a GPUI review panel. Destructive execution remains disabled.

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

### M2 review slice

- scan results are retained as typed `ScanItem` values and converted through `Planner::build`
- `CleanupPlan` exposes frontend-neutral selected count/bytes and select-all/category/path selection helpers
- symlink entries remain impossible to select through the shared plan helpers
- GPUI shows cleanup-plan totals and the first review rows with path, size, and selection/protection state
- GPUI supports select-all-safe-items and deselect-all controls
- a new scan replaces the previous plan rather than carrying stale selections forward
- execution is intentionally not wired yet

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

## Frontend strategy

The current product direction remains **GPUI on macOS first**. Continue building and validating the macOS application with GPUI while keeping the Rust engine independent from GPUI-specific types and lifecycle assumptions.

GPUI is the current reference frontend, not part of the cleanup engine. The architecture must remain ready for a future Flutter desktop frontend without rewriting core cleanup behavior.

Intended dependency direction:

```text
Rust core/domain
      ↑
platform adapters/providers
      ↑
stable application API / event boundary
      ↑
 ┌────┴──────────────┐
 │                   │
GPUI frontend   Flutter frontend
(current)       (optional later)
```

Ownership rules:

- Rust owns scanning, cleanup policy, safety checks, planning, execution, reporting, and platform-native adapters.
- Frontends own presentation, interaction, localization, and frontend state only.
- GPUI-specific types must not leak into core/domain APIs.
- Cleanup rules and destructive-action policy must not be duplicated in GPUI or future Dart code.
- Long-running scan progress and cancellation should cross a frontend-neutral request/result/event boundary.
- Keep this boundary compatible with a future FFI layer such as `flutter_rust_bridge`, but do not introduce Flutter/FFI work before the Rust application API is stable.
- CLI should continue consuming the same Rust application/core APIs and serves as a useful frontend-independence check.

Windows remains a later target after the macOS flow is stable. A GPUI Windows feasibility spike is planned first, while a Flutter desktop spike remains an explicit alternative if GPUI Windows maturity or production ergonomics are insufficient.

## Next M2 slices

Continue M2 in safety-first order:

1. move-to-trash adapter contract and category action policy
2. permanent-delete policy by category
3. symlink-safe canonicalization and execution-time revalidation
4. allow-list enforcement
5. execution report and frontend-neutral execution events
6. execution cancellation
7. only then wire the destructive GPUI action

Keep destructive execution disabled until the revalidation and allow-list boundaries are implemented and tested.

## Validation

Before push on a development machine:

```bash
make prepush
```

GitHub Actions runs the non-mutating equivalent:

```bash
make ci
```
