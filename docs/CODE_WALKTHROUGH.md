# Code Walkthrough

This document explains the current codebase from the outside in and shows where scan, cleanup, uninstall, macOS integration, scheduling, packaging, and GPUI responsibilities live.

## 1. Workspace overview

The workspace is split into four crates:

- `cleaner-core` — frontend/platform-neutral domain logic: scanners, plans, safety policy, execution, application inventory, uninstall evidence, orphan discovery, reports, cancellation.
- `cleaner-macos` — macOS-native integration and adapters: Finder, Trash, Full Disk Access, application discovery helpers, LaunchAgent scheduling.
- `cleaner-cli` — non-GUI consumer of the same Rust APIs and an architecture check against GUI coupling.
- `cleaner-gui` — GPUI desktop frontend for macOS.

Dependency direction:

```text
cleaner-core
    ↑
cleaner-macos / platform adapters
    ↑
stable Rust application/platform APIs
    ↑
cleaner-cli / cleaner-gui / future frontend
```

GPUI is not part of the engine. A future Flutter frontend must consume the same Rust behavior instead of recreating policy in Dart.

## 2. Smart Scan domain flow

Important core files include:

- `crates/cleaner-core/src/model.rs`
- `crates/cleaner-core/src/target.rs`
- `crates/cleaner-core/src/scanner.rs`
- `crates/cleaner-core/src/planner.rs`
- `crates/cleaner-core/src/action_policy.rs`
- `crates/cleaner-core/src/executor.rs`

Typed targets create approved `ScanRequest` values for User Cache, System Cache, Xcode, Homebrew, and Node caches.

`FileSystemScanner` emits frontend-neutral `ScanEvent` values and does not traverse discovered symlinks. `CancellationToken` provides cooperative cancellation across worker/UI boundaries.

The flow is:

```text
typed ScanRequest
    ↓
FileSystemScanner
    ↓
ScanEvent stream
    ↓
Planner::build
    ↓
CleanupPlan review
    ↓
ExecutionPolicy + CategoryActionPolicy
    ↓
CleanupExecutor
    ↓
ExecutionReport
```

## 3. Cleanup safety model

`ExecutionPolicy::default()` disables mutation.

Mutation requires explicit enabled policy plus pinned allow-list roots. Before each action, execution revalidates filesystem identity, category/root membership, protected roots, symlink state, and canonical-path stability.

GPUI uses Trash-only policy. Permanent-delete policy exists in core for selected generated/cache categories, but the macOS permanent-delete backend remains intentionally safety-locked.

Do not re-enable permanent deletion with path-only `remove_file` / `remove_dir_all` logic. The unresolved problem is ancestor-swap TOCTOU between validation and mutation; the acceptable future design requires anchored/no-follow filesystem mutation.

## 4. GPUI Smart Care

See `crates/cleaner-gui/src/main.rs` and `crates/cleaner-gui/src/execution.rs`.

GPUI owns presentation and transient state only. Filesystem scan and cleanup execution run off the event loop and communicate through channels.

Current Smart Care behavior:

- start/cancel Smart Scan
- live category metrics
- permission-denied status
- cleanup-plan review
- select/deselect safe items through core plan APIs
- Trash-only execution
- execution cancellation and report
- reviewed plan discarded after execution so another mutation requires a fresh scan

## 5. Application inventory and metadata

The uninstall flow begins in shared Rust APIs.

Inventory discovers applications under user/local/system application roots and treats discovered `.app` bundles as leaves. Directory symlinks are not followed.

Metadata includes bundle identifier/version data and signing TeamIdentifier when available. Missing metadata or unsigned applications remain visible; TeamIdentifier is context only and is never sufficient ownership evidence by itself.

CLI exposes the same inventory path through `dxtr-cleaner apps`.

## 6. Related-file evidence and confidence

Related-file matching is evidence-driven and read-only.

Confidence semantics:

- High — exact bundle-identifier-derived ownership path
- Medium — bundle-prefixed host-specific preference evidence
- Low — exact display-name directory evidence

Medium/Low evidence is review-only. Symlink candidates are excluded. Duplicate paths retain the strongest evidence.

CLI exposes the matcher through `dxtr-cleaner related <bundle-id>`.

## 7. System-app protection

Protection policy lives in `cleaner-core`.

Applications fail closed when classified as system applications, located under protected macOS system application roots, or using the exact `com.apple` namespace.

Ordinary third-party apps under `/Applications` remain eligible for reviewed uninstall.

Frontends render typed protection results rather than recreating protection rules.

## 8. Orphan discovery

Orphan discovery is frontend-neutral and read-only.

It uses the complete installed-application inventory as the authoritative live bundle-ID set. If inventory is partial, orphan classification fails closed and returns no destructive candidate set.

Only safe reverse-DNS-shaped entries in approved Library locations are considered; symlinks, live bundle IDs, and the `com.apple` namespace are excluded.

CLI exposes the same path through `dxtr-cleaner orphans`.

## 9. Reviewed uninstall execution

The GPUI uninstaller is a separate product flow from Smart Care.

The Rust-owned flow is:

```text
ApplicationInventory
    ↓
application protection
    ↓
related-file evidence
    ↓
UninstallPlan review
    ↓
fresh evidence + pinned execution roots
    ↓
Trash-only uninstall executor
    ↓
UninstallExecutionReport
```

High-confidence related data starts selected. Medium/Low review-only evidence requires explicit opt-in. Protected applications cannot execute.

Execution refreshes application/evidence state and revalidates safety roots immediately before mutation. Related data is trashed before the required application bundle so cancellation preferentially leaves the app installed.

After an attempt, the reviewed plan and cached application list are discarded to prevent stale reuse.

## 10. macOS platform boundary

See `crates/cleaner-macos/src/lib.rs` and focused modules in that crate.

macOS-native responsibilities include:

- Full Disk Access status and System Settings deep link
- Finder reveal
- move-to-Trash backend
- installed application discovery helpers
- LaunchAgent scheduling

Paths passed to Finder/AppleScript are arguments, not interpolated script source.

Frontend code should call platform APIs rather than execute native commands directly.

## 11. LaunchAgent scheduling foundation

See `crates/cleaner-macos/src/launch_agent.rs`.

Scheduling is deliberately read-only. The generated LaunchAgent runs only:

```text
dxtr-cleaner scan --category user
```

It cannot schedule Trash, uninstall, or permanent-delete mutation.

Important lower-level APIs validate label/path safety, render the plist, install/bootstrap it, inspect status, and uninstall/bootout it.

The plist path is under the user's LaunchAgents directory and logs are under `~/Library/Logs/DxtrCleaner`.

## 12. Frontend-ready scheduling coordinator

`LaunchAgentCoordinator` is the scheduling surface intended for desktop frontends.

It owns:

- HOME validation
- bundled CLI path resolution
- Daily interval policy
- LaunchAgent label/path details
- status inspection
- enable/disable operations
- stale executable detection
- App Translocation safety behavior

Frontends therefore do not know `launchctl`, plist layout, label strings, interval values, or bundle CLI structure.

Durability behavior:

- scheduled CLI is the sibling `Contents/MacOS/dxtr-cleaner`
- enabling/repairing is rejected when the GUI is running from App Translocation
- status/disable still work from a translocated launch so an existing schedule can be stopped
- if the application is moved/renamed after scheduling, status reports the configuration as stale rather than healthy

This boundary is suitable for reuse by a future frontend without moving scheduling policy into that frontend.

## 13. GPUI Scheduling Settings

See:

- `crates/cleaner-gui/src/main.rs`
- `crates/cleaner-gui/src/scheduling.rs`

`scheduling.rs` owns worker orchestration around `LaunchAgentCoordinator`.

The GPUI Settings page supports:

- load status
- Enable Daily Smart Scan
- Disable
- Repair stale app-location configuration
- Disabled / Enabled / Needs repair / Loading / Error states

Scheduling filesystem/native command work does not run on the GPUI event loop.

GPUI interprets coordinator results for presentation only; it does not construct plist paths or command lines.

## 14. Packaging

See `scripts/macos/package.sh` and `docs/M4_PACKAGING.md`.

The package script builds a release `.app` containing both:

- GPUI executable
- bundled `dxtr-cleaner` CLI

This in-bundle CLI is necessary so the LaunchAgent can target a stable path inside an installed application bundle.

The script supports Developer ID signing with hardened runtime and timestamping. If no signing identity is provided, ad-hoc signing is only a local smoke-test mode and is not release evidence.

The app bundle metadata is created with `plutil`, avoiding unsafe raw plist interpolation.

## 15. Notarization

See `scripts/macos/notarize.sh`.

The release path is:

```text
signed app + ZIP
    ↓
notarytool submit --wait
    ↓
stapler staple
    ↓
stapler validate
    ↓
spctl assessment
    ↓
rebuild final ZIP containing stapled app
```

Credentials stay external through the configured notary keychain profile.

Do not mark notarization complete from script existence alone; a real accepted Apple submission is required.

## 16. Homebrew cask generation

See `scripts/macos/generate_cask.sh` and `docs/HOMEBREW_CASK.md`.

The generator requires:

- explicit release version
- exact 64-hex SHA-256
- HTTPS release URL

It writes `Casks/dxtr-cleaner.rb` and validates Ruby syntax.

Generated Ruby values are serialized in non-interpolating literals so input such as `#{...}` cannot execute when Homebrew loads the cask.

The cask must reference the exact final notarized ZIP, not a placeholder or pre-notarization artifact.

## 17. Release verification gate

See `docs/M4_RELEASE_VERIFICATION.md`.

The remaining M4 work is physical release verification:

1. Developer ID signed build
2. code-signature/hardened-runtime verification
3. accepted notarization
4. staple validation
5. Gatekeeper assessment
6. fresh-machine launch smoke test
7. publish exact final ZIP
8. final SHA-256
9. generate cask from that artifact
10. Homebrew install/uninstall smoke test

M4 packaging/Homebrew roadmap items remain open until this evidence exists.

## 18. Formatting and CI

Quality commands:

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

`make prepush` performs real formatting before verification locally.

Development branches also have an auto-format workflow because connector/API writes do not run local hooks. The workflow may push `chore: apply rustfmt` as `github-actions[bot]`.

Important GitHub behavior: a workflow push made with the repository `GITHUB_TOKEN` does not recursively trigger arbitrary follow-on workflows. Therefore always verify that a real CI run exists for the **final PR head** before merging.

## 19. GPUI dependency policy

GPUI and `gpui_platform` are pinned to Zed commit:

```text
b05f40c5546b47bcf9561136dc0fcdcd9968cb63
```

Do not float the dependency during unrelated work. Upgrade in a dedicated PR with GUI validation.

## 20. Frontend and Windows portability

The current product frontend remains GPUI on macOS.

Future Windows support and optional Flutter desktop work must preserve the Rust ownership boundary. Shared scan, policy, planning, safety, execution, report, and scheduling semantics should not be rewritten per frontend.

Windows-specific behavior belongs behind Windows adapters/providers; Flutter, if adopted later, should own only presentation/state plus generated bindings to stable Rust APIs.

## 21. Where to start when changing code

Smart Scan behavior:

1. `cleaner-core` target/scanner model
2. core tests
3. GPUI only for presentation

Cleanup safety:

1. core safety/planner/action policy/executor
2. platform backend after policy is explicit
3. frontend last

Uninstaller behavior:

1. core inventory/evidence/protection/plan/execution APIs
2. macOS evidence/adapters
3. CLI validation surface
4. GPUI presentation

macOS scheduling:

1. `cleaner-macos/src/launch_agent.rs`
2. keep coordinator API frontend-neutral
3. `cleaner-gui/src/scheduling.rs` for worker orchestration
4. `main.rs` only for UI state/rendering

Release packaging:

1. `scripts/macos/package.sh`
2. `scripts/macos/notarize.sh`
3. `scripts/macos/generate_cask.sh`
4. `docs/M4_RELEASE_VERIFICATION.md`

Do not close a release gate because its automation script exists; close it only from real artifact evidence.
