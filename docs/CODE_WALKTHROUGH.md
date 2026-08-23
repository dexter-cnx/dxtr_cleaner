# Code Walkthrough

This document explains the codebase from the outside in and shows where scan, review, safety, execution, platform integration, and GPUI responsibilities live.

## 1. Workspace overview

The workspace is split into four crates:

- `cleaner-core` — domain models, scanners, cleanup planning, safety policy, action policy, execution, reports, and cancellation.
- `cleaner-macos` — macOS-specific operations such as Finder reveal, Trash integration, System Settings links, and other platform boundaries.
- `cleaner-cli` — a non-GUI consumer of the same Rust core APIs. It is useful as a frontend-independence check.
- `cleaner-gui` — the current GPUI desktop frontend for macOS.

The intended dependency direction is:

```text
cleaner-core
    ↑
platform adapters / application integration
    ↑
cleaner-cli / cleaner-gui / future frontend
```

GPUI is not part of the cleanup engine. A future Flutter frontend must consume the same Rust-owned behavior rather than reimplementing cleanup rules in Dart.

## 2. Core domain model

Start with `crates/cleaner-core/src/model.rs`.

Important types:

- `CleanupCategory` — shared category identity such as `UserCache`, `SystemCache`, `Xcode`, `Homebrew`, `Node`, `Docker`, and `LargeFiles`.
- `ScanItem` — one discovered filesystem candidate with path, category, byte size, and symlink state.
- `ScanSummary` — item count and total bytes.
- `CleanupPlanItem` — wraps a `ScanItem` with review selection state.
- `CleanupPlan` — reviewed set of cleanup candidates.

`CleanupPlan` owns frontend-neutral review operations such as:

- selected count / selected bytes
- select all safe items
- select/deselect by category
- toggle by path

Symlink items are intentionally prevented from becoming selected through these helpers.

## 3. Typed scan targets

See `crates/cleaner-core/src/target.rs`.

The scanner does not rely on GPUI-provided arbitrary paths. Scan roots are constructed through typed targets such as:

- `UserCacheScan`
- `SystemCacheScan`
- `XcodeScan`
- `HomebrewScan`
- `NodeScan`

Each target produces a `ScanRequest` containing:

- category
- roots
- excluded roots

This keeps platform and product scanning rules in Rust rather than the frontend.

## 4. Filesystem scanner and progress events

See `crates/cleaner-core/src/scanner.rs`.

`FileSystemScanner` recursively walks approved scan roots and emits frontend-neutral `ScanEvent` values:

- `Started`
- `ItemFound`
- `PermissionDenied`
- `Finished`
- `Cancelled`

The scanner uses `symlink_metadata` and does not traverse discovered symlinks.

`CancellationToken` is shared by scan and cleanup execution. It is backed by an atomic flag and can be cloned across worker/UI boundaries.

The GUI receives progress events, but scan ownership remains inside Rust.

## 5. Protected roots and scan safety

Protected broad filesystem locations are centralized in `cleaner-core` safety code and are reused by scan/execution validation.

Examples of broad roots that must not become cleanup roots include locations such as `/`, `/System`, and `/Library`.

A specific approved descendant such as a defined cache directory can still be scanned through a typed target.

The design rule is conservative: broad or ambiguous roots fail closed.

## 6. Building the cleanup plan

See `crates/cleaner-core/src/planner.rs`.

After scan completion, GPUI converts collected `ScanItem` values using:

```text
Planner::build(items)
```

The planner creates `CleanupPlanItem` values and leaves symlinks unselected by default.

The plan is then presented to the user for review before mutation.

## 7. Execution allow-list policy

`ExecutionPolicy` and `AllowedRoot` also live in `planner.rs`.

Destructive execution is disabled by default:

```text
ExecutionPolicy::default()
```

A caller must explicitly create an enabled policy with category-scoped allow-list roots.

`AllowedRoot::new` records the requested path and pins its canonical path at policy construction time.

Before each filesystem mutation, `Planner::validate_item_for_execution` revalidates the selected item:

1. destructive actions must be enabled
2. item must not be a symlink
3. item must not be a protected broad root
4. path must still exist
5. path is canonicalized again
6. matching allow-list root must have the same category
7. allow-list root must still exist and must not be a symlink
8. current canonical root must match the pinned canonical root
9. item must be a descendant of the pinned root, never the root itself

This validation is repeated immediately before each execution action rather than trusting the earlier review state.

## 8. Category action policy

See `crates/cleaner-core/src/action_policy.rs`.

`CategoryActionPolicy` decides the cleanup action in Rust.

Default behavior for every category is:

```text
MoveToTrash
```

Permanent delete requires explicit opt-in and is only policy-eligible for generated/cache categories:

- `UserCache`
- `Xcode`
- `Homebrew`
- `Node`

Permanent delete is rejected for:

- `SystemCache`
- `Docker`
- `LargeFiles`

The important boundary is that a frontend does not invent this policy.

The current GPUI flow deliberately uses:

```text
CategoryActionPolicy::trash_only()
```

so the UI cannot request permanent deletion.

## 9. Cleanup executor

See `crates/cleaner-core/src/executor.rs`.

`CleanupExecutor` coordinates reviewed execution.

Inputs include:

- `CleanupPlan`
- `ExecutionPolicy`
- `CategoryActionPolicy`
- `CancellationToken`
- platform `CleanupBackend`

For each selected item it:

1. checks cancellation
2. resolves the core action policy
3. revalidates the item against execution safety policy
4. invokes the appropriate backend operation
5. records success or failure

Safety failures are represented per item so an earlier successful mutation is not lost from the report if a later item fails revalidation.

`ExecutionReport` exposes:

- successful item count
- failed item count
- bytes moved to Trash
- bytes permanently deleted
- cancellation state

Moving to Trash is deliberately not reported as reclaimed disk capacity because the data may still occupy the same filesystem until Trash is emptied.

## 10. Platform backend boundary

See `crates/cleaner-macos/src/lib.rs`.

`SystemMacPlatform` owns macOS-specific operations.

Current important behaviors include:

- open Full Disk Access settings
- reveal a path in Finder
- move an item to Trash
- enumerate application roots

Trash integration currently uses Finder through `osascript`, passing the path as an argument rather than interpolating it into AppleScript source.

### Permanent-delete safety lock

The core contains category policy support for permanent delete, but the macOS backend currently refuses to perform permanent deletion.

Reason: simple path-based `remove_file` / `remove_dir_all` cannot fully close the ancestor-swap TOCTOU window after validation.

Permanent deletion must remain fail-closed until it can be implemented using an anchored directory descriptor / no-follow filesystem strategy that ties validation and mutation to the same filesystem identity.

Do not replace this lock with another path-based recheck.

## 11. GPUI Smart Scan flow

See `crates/cleaner-gui/src/main.rs`.

The current application flow is:

```text
Start Smart Scan
      ↓
typed ScanRequest values
      ↓
worker thread + FileSystemScanner
      ↓
ScanEvent channel
      ↓
GPUI metrics / status
      ↓
Planner::build
      ↓
Cleanup plan review
      ↓
Move selected to Trash
      ↓
CleanupExecutor on worker thread
      ↓
ExecutionReport
```

The UI event loop is not used for filesystem scan or cleanup work.

Messages cross worker/UI boundaries through channels and GPUI periodically drains them.

## 12. GPUI review state

The review panel displays:

- selected items / total items
- selected bytes
- first review rows with path and size
- selection / skipped / protected symlink state
- select-all-safe-items
- deselect-all

Selection behavior delegates to `CleanupPlan` rather than duplicating selection safety inside GPUI.

## 13. GPUI execution wiring

The M2 product-facing execution wiring is on `feature/m2-gpui-execution` / PR #11.

Important rules:

- execution policy roots come from the exact typed scan requests used for that scan
- GPUI uses `CategoryActionPolicy::trash_only()`
- cleanup work runs away from the GPUI event loop
- cleanup can be cancelled using the shared `CancellationToken`
- final report is displayed to the user
- after an execution attempt the plan is discarded
- another mutation requires a fresh scan

Discarding the plan prevents repeated execution against stale filesystem observations.

## 14. CLI as an architecture check

`cleaner-cli` consumes core scanner APIs without GPUI.

This is intentional. If new core cleanup logic starts requiring GUI-specific types, the dependency direction has been broken.

The CLI should remain a lightweight proof that scanning and policy remain frontend-neutral.

## 15. Formatting and CI

The repository quality gate is defined through the `Makefile`.

Important commands:

```bash
make format
make format-check
make test
make clippy
make gui-check
make verify
make prepush
make ci
```

`make prepush` performs formatting before verification for normal local git workflows.

Because connector/API file writes do not execute local git hooks, development branches also use `.github/workflows/auto-format.yml` to run real `cargo fmt --all` and commit formatter changes back to the branch when necessary.

CI still runs the non-mutating verification gate and remains authoritative.

## 16. GPUI dependency policy

GPUI and `gpui_platform` are pinned to this Zed commit:

```text
b05f40c5546b47bcf9561136dc0fcdcd9968cb63
```

Do not float this dependency during unrelated feature work. Upgrade GPUI only in a dedicated dependency PR with GUI validation.

## 17. Frontend portability

The current product frontend is GPUI on macOS.

The Rust engine must remain independent from that decision so a future Flutter frontend can be introduced without rewriting:

- scan rules
- cleanup planning
- selection safety
- action policy
- execution safety
- cleanup execution
- reports
- cancellation

A future Dart layer should own presentation/state only and call into stable Rust APIs/FFI.

## 18. Windows portability

Windows support is planned after the macOS flow stabilizes.

Expected Windows-specific work belongs in adapters/providers:

- Windows cleanup roots
- Recycle Bin integration
- permission/elevation handling
- protected-root policy
- packaging and signing

The shared core model and execution semantics should not be rewritten for Windows.

## 19. Where to start when modifying the project

For scan behavior:

1. `cleaner-core/src/target.rs`
2. `cleaner-core/src/scanner.rs`
3. scanner/core tests
4. GUI only for presentation

For cleanup safety:

1. `cleaner-core/src/safety.rs`
2. `cleaner-core/src/planner.rs`
3. `cleaner-core/src/action_policy.rs`
4. `cleaner-core/src/executor.rs`
5. platform adapter only after core policy is explicit

For macOS integration:

1. `cleaner-macos/src/lib.rs`
2. expose only the minimum frontend-neutral adapter contract needed

For UI changes:

1. `cleaner-gui/src/main.rs`
2. do not copy core policy into UI conditionals

## 20. Current milestone boundary

PR #11 completes the M2 product flow with Trash-only GPUI execution.

After M2, the next planned milestone is M3 App Uninstaller:

- installed app inventory
- bundle/team metadata
- related-file matcher
- confidence tiers
- system-app protection
- orphan finder

The same safety principle continues into M3: discovery and confidence remain separate from destructive execution, and execution continues through the shared core policy/executor boundary.
