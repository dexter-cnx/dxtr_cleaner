# Project Handoff

## Current state

M0, M1 Smart Scan, and M2 review + Trash-only execution are complete and merged to `main`. M3.1 installed-application inventory, M3.2 bundle/team metadata, M3.3 related-file matching + confidence tiers, and M3.4 system-app protection are merged. Permanent delete remains safety-locked in the macOS backend and is not exposed by GPUI.

M3.5 is implemented on `feature/m3-orphan-finder`: the frontend-neutral orphan report/finder API lives in `cleaner-core`, while `cleaner-macos` scans high-confidence bundle-shaped Library entries against the installed bundle-ID set. `dxtr-cleaner orphans` is the CLI validation surface. This slice is read-only and adds no uninstall mutation.

For a code-oriented tour of the repository, read [`CODE_WALKTHROUGH.md`](./CODE_WALKTHROUGH.md) before changing scan, cleanup, execution, GPUI, or platform-boundary code.

### Implemented foundation

- Rust workspace with four crates
- `cleaner-core` domain model and filesystem scanner
- cleanup planner and execution safety policy
- `cleaner-macos` platform boundary
- `cleaner-cli` dry-run scan command
- `cleaner-gui` GPUI Smart Care shell
- CI and local `make prepush` quality gates
- development-branch auto-format workflow using real `cargo fmt`
- architecture, roadmap, handoff, and code-walkthrough docs

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

### M3 installed application inventory

- `ApplicationInventory` is a frontend-neutral core trait
- inventory results contain typed application location (`user`, `local`, `system`), display name, bundle path, and optional application metadata
- partial traversal failures are preserved as structured warnings rather than discarding successful results
- macOS roots are `/Applications`, `/System/Applications`, and `~/Applications`
- nested application folders are traversed, but a discovered `.app` bundle is treated as a leaf
- directory symlinks, including symlinked inventory roots, are never followed
- output is sorted deterministically for stable UI/CLI behavior and tests
- `dxtr-cleaner apps` exercises the same Rust API without introducing GPUI-specific inventory logic
- this slice is read-only; it adds no uninstall or mutation path

### M3 application metadata

- `ApplicationMetadata` remains frontend-neutral and optional
- macOS extraction reads `CFBundleIdentifier`, `CFBundleVersion`, and `CFBundleShortVersionString` from `Contents/Info.plist`
- signing metadata captures `TeamIdentifier` from `codesign` when available
- unsigned apps or missing metadata do not remove the application from inventory
- bundle identifier is the primary ownership signal for later related-file matching
- TeamIdentifier is supporting metadata only and must never be treated as ownership evidence by itself because one team can sign multiple applications

### M3 related-file matcher + confidence tiers

- `RelatedFileMatcher`, `RelatedFileCandidate`, `MatchEvidence`, `MatchConfidence`, and `RelatedFileKind` live in `cleaner-core`
- High confidence requires an exact bundle-identifier-derived path
- bundle identifiers are treated as untrusted metadata and must pass strict safe-component validation before any path construction or prefix matching
- High candidates currently cover Application Support, Caches, Containers, HTTPStorages, Preferences plist, and Saved Application State paths
- Medium confidence covers bundle-identifier-prefixed `~/Library/Preferences/ByHost` entries
- Low confidence covers exact display-name directories under Application Support and Caches
- Medium and Low are explicitly review-only through core policy semantics
- candidate symlinks are excluded with `symlink_metadata`
- duplicate candidate paths retain the strongest available evidence
- display-name evidence is never upgraded merely because a developer TeamIdentifier matches
- matcher remains read-only; it does not create an uninstall execution plan or mutate files
- `dxtr-cleaner related <bundle-id>` exposes the same matcher for CLI validation

### M3 system application protection

- `ApplicationProtectionPolicy` lives in `cleaner-core`
- applications classified as `ApplicationLocation::System` are protected
- paths under `/System/Applications` and known CoreServices application roots are protected defensively
- the exact `com.apple` bundle namespace is protected even when an Apple app appears outside the system roots
- lookalike prefixes such as `com.appleish.*` are not treated as Apple
- ordinary third-party apps under `/Applications` remain unprotected
- typed protection reasons are exposed for GPUI/Flutter/CLI presentation
- this is a policy boundary only; no uninstall mutation was added
- the implementation must remain compatible with the declared Rust 1.85 MSRV

### M3 orphan finder

- `OrphanFinder`, `OrphanCandidate`, `OrphanFinderIssue`, and `OrphanReport` live in `cleaner-core`
- shared bundle-ID validation is centralized in `cleaner-core` so related-file and orphan logic use the same path-safety rules
- installed applications supply the authoritative live bundle-identifier set
- macOS scans exact bundle-shaped entries under `~/Library/Application Support`, `Caches`, `Containers`, `HTTPStorages`, `Preferences`, and `Saved Application State`
- an orphan identifier must pass path-safe validation and contain at least two reverse-DNS-style components, preventing generic folders such as `Adobe` from being classified as app ownership evidence
- candidates whose bundle identifier is still installed are excluded
- the exact `com.apple` namespace is always excluded defensively, even if inventory misses a system application
- symlink roots are rejected with a warning and symlink candidates are never returned
- unreadable roots/entries preserve partial-result issues instead of discarding successful candidates
- duplicate paths retain the strongest confidence evidence
- `Preferences/ByHost` is intentionally omitted from orphan inference because the bundle-ID/host-suffix boundary cannot be reconstructed reliably enough for a missing app
- current orphan candidates are High confidence exact bundle-ID shapes only; lower-confidence name inference is intentionally not used
- `dxtr-cleaner orphans` exercises the same API without GPUI-specific logic
- this slice remains read-only and creates no uninstall plan or mutation path

### Important safety decision

`ExecutionPolicy::default()` disables mutation. A caller must explicitly construct an enabled execution policy with pinned allow-list roots before `CleanupExecutor` performs any mutation.

GPUI exposes only **Move selected to Trash**. It does not expose permanent deletion. The product badge explicitly states `Safe mode · Trash only`.

Protected broad roots are centralized in `cleaner-core` and shared by scan validation and execution validation. Descendant paths such as `/Library/Caches` remain eligible for explicitly defined scanners while broad roots such as `/`, `/System`, and `/Library` are rejected.

No M3 uninstall execution exists yet. Inventory, metadata, related-file matching, system protection, and orphan discovery are evidence/policy foundations only. Any future uninstall execution must build a reviewed plan from these APIs, revalidate at execution time, keep system-app protection mandatory, and remain Trash-only initially.

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

- Rust owns scanning, cleanup policy, safety checks, planning, execution, reporting, application inventory, related-file matching, system-app protection, orphan discovery, and platform-native adapters.
- Frontends own presentation, interaction, localization, and frontend state only.
- GPUI-specific types must not leak into core/domain APIs.
- Cleanup and uninstall rules must not be duplicated in GPUI or future Dart code.
- Long-running scan/execution progress and cancellation should cross a frontend-neutral request/result/event boundary.
- Keep this boundary compatible with a future FFI layer such as `flutter_rust_bridge`, but do not introduce Flutter/FFI work before the Rust application API is stable.
- CLI should continue consuming the same Rust application/core APIs and serves as a useful frontend-independence check.

Windows remains a later target after the macOS flow is stable. A GPUI Windows feasibility spike is planned first, while a Flutter desktop spike remains an explicit alternative if GPUI Windows maturity or production ergonomics are insufficient.

## Next step

After merging M3.5, the evidence/policy foundation for the app-uninstaller milestone is complete. The next safety-first slice is **reviewed uninstall planning**, not direct deletion:

1. define a frontend-neutral uninstall plan model that includes the selected app, protection state, and related-file candidates with confidence/evidence
2. protected applications must be impossible to select for uninstall in core policy
3. Medium/Low related-file candidates remain review-only and default unselected
4. stale plans must be invalidated/revalidated before execution
5. initial execution should move the app and explicitly selected related files to Trash only
6. permanent deletion remains outside the uninstall flow until anchored/no-follow mutation is implemented and separately reviewed

Do not infer ownership from display names alone. Exact bundle identifiers remain the strongest ownership evidence; lower-confidence matches stay review-only. TeamIdentifier is useful for context/disambiguation but is never sufficient ownership evidence by itself.

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
