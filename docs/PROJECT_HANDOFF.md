# Project Handoff

## Current state

M0 and M1 Smart Scan are complete and merged to `main`.

M2 core execution safety, execution engine, and category action policy are complete. The active branch `feature/m2-gpui-execution` wires the reviewed cleanup plan into the GPUI product flow using Trash-only execution. Permanent delete remains safety-locked in the macOS backend and is not exposed by GPUI.

### Implemented foundation

- Rust workspace with four crates
- `cleaner-core` domain model and filesystem scanner
- cleanup planner and execution safety policy
- `cleaner-macos` platform boundary
- `cleaner-cli` dry-run scan command
- `cleaner-gui` GPUI Smart Care shell
- CI and local `make prepush` quality gates
- development-branch auto-format workflow using real `cargo fmt`
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

### M2 review + execution foundation

- scan results are retained as typed `ScanItem` values and converted through `Planner::build`
- `CleanupPlan` exposes frontend-neutral selected count/bytes and select-all/category/path selection helpers
- symlink entries remain impossible to select through the shared plan helpers
- GPUI shows cleanup-plan totals and review rows with path, size, and selection/protection state
- GPUI supports select-all-safe-items and deselect-all controls
- a new scan replaces the previous plan rather than carrying stale selections forward
- `ExecutionPolicy` keeps mutation opt-in and pins category-scoped allow-list roots
- execution-time revalidation uses `symlink_metadata` and canonicalization immediately before each mutation
- allow-list roots retain their pinned canonical path and fail closed if the root is replaced by a symlink or redirected
- broad protected roots remain rejected
- `CleanupExecutor` returns structured per-item execution records and preserves partial success when a later item fails safety revalidation
- cooperative execution cancellation uses the shared `CancellationToken`
- successful move-to-Trash bytes are reported as moved bytes rather than falsely claiming reclaimed disk capacity
- `cleaner-macos` implements move-to-Trash through Finder/AppleScript with the path passed as argv

### M2 category action policy

The default cleanup action for every category is `MoveToTrash`.

Permanent delete requires explicit core-policy opt-in and is only policy-eligible for clearly generated/cache categories:

- `UserCache`
- `Xcode`
- `Homebrew`
- `Node`

Permanent delete is rejected for the more sensitive or ambiguous categories:

- `SystemCache`
- `Docker`
- `LargeFiles`

The frontend must never decide this policy itself. GPUI and any future Flutter frontend consume core behavior rather than duplicating category rules.

The macOS permanent-delete backend intentionally fails closed. A previous path-based implementation was removed because ancestor replacement between validation and mutation creates a TOCTOU escape. Permanent deletion must not be enabled until mutation can be performed relative to an anchored directory descriptor with no-follow semantics.

### M2 GPUI execution flow

- GPUI retains the exact typed `ScanRequest` values used by the completed Smart Scan
- when scanning completes, GPUI builds an `ExecutionPolicy` allow-list from those scan roots rather than arbitrary UI paths
- cleanup is always launched through `CleanupExecutor`
- GPUI passes `CategoryActionPolicy::trash_only()` and therefore cannot request permanent delete
- cleanup runs on a worker thread and keeps the GPUI event loop responsive
- cleanup cancellation uses a dedicated shared `CancellationToken`
- the UI displays execution completion/cancellation/failure status, moved bytes, and per-run success/failure counts
- after any completed execution the cleanup plan is discarded, forcing a fresh scan before another mutation and avoiding stale-plan reuse

### Important safety decision

`ExecutionPolicy::default()` disables mutation. A caller must explicitly construct an enabled execution policy with pinned allow-list roots before `CleanupExecutor` performs any mutation.

GPUI now exposes only **Move selected to Trash**. It does not expose permanent deletion. The product badge explicitly states `Safe mode · Trash only`.

Protected broad roots are centralized in `cleaner-core` and shared by scan validation and execution validation. Descendant paths such as `/Library/Caches` remain eligible for explicitly defined scanners while broad roots such as `/`, `/System`, and `/Library` are rejected.

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
- Long-running scan/execution progress and cancellation should cross a frontend-neutral request/result/event boundary.
- Keep this boundary compatible with a future FFI layer such as `flutter_rust_bridge`, but do not introduce Flutter/FFI work before the Rust application API is stable.
- CLI should continue consuming the same Rust application/core APIs and serves as a useful frontend-independence check.

Windows remains a later target after the macOS flow is stable. A GPUI Windows feasibility spike is planned first, while a Flutter desktop spike remains an explicit alternative if GPUI Windows maturity or production ergonomics are insufficient.

## Next step

After the GPUI execution PR is merged, M2 is complete and work can proceed to M3 app uninstaller in safety-first slices:

1. installed application inventory
2. bundle/team metadata extraction
3. related-file matcher with confidence tiers
4. system-app protection
5. orphan finder
6. only then reviewed uninstall execution

Keep permanent deletion safety-locked until anchored/no-follow filesystem mutation is implemented and separately reviewed.

## Validation

Before push on a development machine:

```bash
make prepush
```

GitHub Actions runs the non-mutating equivalent:

```bash
make ci
```
