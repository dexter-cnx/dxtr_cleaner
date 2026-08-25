# Roadmap

## M0 — foundation

- [x] Rust workspace
- [x] core scan model
- [x] safe planner
- [x] CLI scan demo
- [x] macOS adapter boundary
- [x] GPUI dashboard shell
- [x] formatting/test CI
- [ ] keep core/domain APIs platform-neutral so Windows adapters can be added without rewriting scan/cleanup logic
- [ ] keep Rust engine independent from GPUI so an alternate frontend can be added without rewriting core behavior

## M1 — real Smart Scan

- [x] user cache scanner
- [x] system cache scanner
- [x] Xcode DerivedData scanner
- [x] Homebrew cache scanner
- [x] Node/npm/pnpm/yarn cache scanners
- [x] exclusions and protected roots
- [x] live GUI progress events

## M2 — review + cleaning

- [x] cleanup plan review UI
- [x] move-to-trash where appropriate
- [x] permanent-delete policy by category
- [x] symlink-safe canonicalization and revalidation
- [x] allow-list enforcement
- [x] execution report
- [x] cancellation
- [x] GPUI Trash-only execution wiring

The GPUI product flow is intentionally Trash-only. It derives the execution allow-list from the typed scan requests used for the completed scan, pins that policy before review execution, runs cleanup through the shared Rust `CleanupExecutor`, exposes cooperative cancellation, and discards the stale cleanup plan after execution so another mutation requires a fresh scan.

Permanent delete remains an explicit core policy capability rather than a frontend toggle. The core policy permits opt-in only for generated/cache categories (`UserCache`, `Xcode`, `Homebrew`, `Node`) and rejects it for `SystemCache`, `Docker`, and `LargeFiles`. The macOS permanent-delete backend remains safety-locked until an anchored directory-descriptor/no-follow implementation closes the ancestor-swap TOCTOU gap.

## M3 — app uninstaller

- [x] installed app inventory
- [x] bundle/team metadata
- [x] related-file matcher
- [x] confidence tiers
- [x] system-app protection
- [x] orphan finder
- [x] reviewed uninstall planning
- [x] Trash-only core execution foundation
- [x] GPUI uninstall review + execution wiring

The inventory slice discovers `.app` bundles under `/Applications`, `/System/Applications`, and `~/Applications`, including nested application folders. App bundles are treated as leaves, directory symlinks are never followed, unreadable subtrees are reported as partial-result warnings, and the shared inventory model remains frontend-neutral. `dxtr-cleaner apps` provides a CLI validation surface for the same Rust inventory API.

Application metadata extraction records bundle identifier/version data from `Info.plist` and signing TeamIdentifier when available without dropping unsigned or partially readable applications from inventory.

Related-file matching is evidence-driven and read-only. Exact bundle-identifier paths are High confidence, bundle-prefixed `Preferences/ByHost` entries are Medium confidence, and exact display-name directory matches are Low confidence. Medium and Low candidates are explicitly review-only. TeamIdentifier is not sufficient evidence by itself because multiple applications from the same developer can share one team identity. Candidate symlinks are excluded and duplicate paths keep the strongest available evidence.

System-app protection is centralized in `cleaner-core`. Applications are protected when inventory classifies them as system applications, when their path is under known macOS system application roots, or when their bundle identifier uses the exact `com.apple` namespace. Protection produces typed reasons for frontend display and is intentionally conservative: ordinary third-party applications in `/Applications` remain unprotected, while Apple/system applications fail closed before any uninstall execution policy can be constructed.

Orphan discovery is also read-only. The installed application inventory provides the authoritative live bundle-identifier set, while the macOS adapter scans exact bundle-shaped entries under Application Support, Caches, Containers, HTTPStorages, Preferences, and Saved Application State. Candidates must pass path-safe bundle-ID validation and a reverse-DNS-shaped check, symlinks are excluded, live bundle IDs are excluded, and the `com.apple` namespace is always excluded defensively. `Preferences/ByHost` is intentionally not inferred as orphan ownership because separating a missing bundle ID from host-specific suffixes is not reliable enough. `dxtr-cleaner orphans` exposes the same API for CLI validation.

Reviewed uninstall planning is frontend-neutral. The application bundle is the required primary item for unprotected apps, High-confidence related files start selected, and Medium/Low evidence remains review-only and default-unselected. Policy-bearing selection fields stay private behind the core plan API.

The Trash-only execution foundation pins the post-review selected set, requires fresh related-file evidence before a related path can be pinned, and pins an explicit canonical execution root for each related-file kind. Those roots are rejected if symlinked and are revalidated immediately before Trash, preventing ancestor-symlink or root-swap escapes. Current application identity and protection are also revalidated immediately before execution, while selected paths are checked for symlinks, protected roots, expected filesystem types, and canonical-path drift. Runtime safety failures preserve all earlier execution records in the report and stop further mutation instead of hiding partial side effects. Related data is trashed before the required app bundle so cancellation preferentially leaves the application installed. Permanent deletion remains disabled.

The GPUI Uninstaller is a separate frontend flow from Smart Care. Installed apps and related-file plans are loaded off the UI thread. Protected apps render locked; High-confidence evidence starts selected, while review-only candidates require explicit opt-in through the core plan API. Every Trash attempt refreshes application inventory and related-file evidence, creates a new execution policy, revalidates safety roots, and discards the reviewed plan and cached application list after the attempt so stale state cannot be reused.

## M4 — macOS integration

Primary implementation path: continue shipping the macOS application with GPUI while keeping frontend dependencies outside the Rust engine boundary.

- [x] Full Disk Access coordinator
- [x] Finder reveal platform adapter
- [x] System Settings Full Disk Access deep link
- [x] scheduling / LaunchAgent read-only foundation
- [x] GPUI scheduling controls / stable frontend surface
- [ ] signed + notarized packaging — scripts and release gate implemented; real Developer ID/notarization verification still required
- [ ] Homebrew cask — generator implemented; publish only after a real notarized release artifact exists

Full Disk Access probing is macOS-only and remains explicit `Granted` / `Denied` / `Unknown`. Finder reveal and the Full Disk Access System Settings deep link live behind `cleaner-macos`. LaunchAgent scheduling is deliberately read-only: it can schedule `dxtr-cleaner scan --category user`, but it cannot schedule Trash, uninstall, or permanent-delete mutation. The stable `LaunchAgentCoordinator` owns label/path/interval/launchctl behavior and validates the in-bundle CLI target. GPUI Settings consumes only coordinator status and enable/disable operations on a background worker, including stale-configuration detection when the app bundle moves. Packaging includes both the GPUI executable and CLI in one `.app` bundle so the LaunchAgent can target a stable absolute in-bundle CLI path.

The Homebrew cask is generated from the exact final release ZIP using an explicit version, SHA-256 digest, and HTTPS URL. The generator does not manufacture placeholder hashes or point Homebrew at an unnotarized artifact. M4 packaging and cask tasks remain open until a real Developer ID build passes notarization/Gatekeeper verification and the resulting published ZIP passes Homebrew install/uninstall smoke tests.

## Frontend architecture strategy

The current product frontend remains **GPUI on macOS**. GPUI is the reference frontend while the core engine and platform adapters are developed.

The architecture must also remain ready for a future **Flutter desktop frontend** without moving scan, safety, cleanup, or policy logic into Dart.

Target dependency direction:

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

Design rules:

- [ ] GPUI-specific types must not leak into core/domain models
- [ ] filesystem scan, rule evaluation, cleanup planning, safety checks, and execution remain in Rust
- [ ] frontend consumes stable request/result/event models rather than internal engine structures
- [ ] long-running scans expose progress/cancellation through a frontend-neutral event boundary
- [ ] platform-native behavior remains behind Rust adapters/providers
- [ ] no duplicated cleanup policy in GPUI or future Dart code
- [ ] keep the boundary compatible with a future generated FFI layer such as `flutter_rust_bridge`, without requiring it today
- [ ] CLI remains another consumer of the same Rust application/core APIs and serves as a frontend-independence check

## M5 — Windows support

Target Windows only after the core scan/cleanup model and primary macOS flow are stable. M5 feasibility work may proceed in parallel with the remaining physical M4 release verification, but it must not be used to mark M4 release gates complete. The first Windows experiment may reuse GPUI, but the Rust engine must not depend on that choice.

### M5.0 — GPUI Windows feasibility spike

- [x] create `spike/windows-gpui`
- [ ] build and launch the GPUI shell on Windows 11 — isolated spike crate and Windows CI compile probe added; interactive launch still required
- [ ] validate sidebar/layout rendering and basic interaction
- [ ] scan a user-selected directory and stream results into the GUI — read-only path + event-stream harness implemented; real Windows smoke still required
- [ ] delete a disposable test file through the platform adapter
- [ ] record GPUI/API gaps and decide whether GPUI Windows can become a supported production target

See [`M5_WINDOWS_GPUI_SPIKE.md`](./M5_WINDOWS_GPUI_SPIKE.md) for the isolated feasibility harness and evidence rules.

### M5.1 — Windows platform adapters

- [ ] Windows filesystem/path adapter
- [ ] Recycle Bin integration
- [ ] Windows disk/storage information adapter
- [ ] permissions/elevation boundary where required
- [ ] Windows-safe protected roots and allow-list policy
- [ ] normalize platform-specific cleanup locations behind shared provider interfaces

### M5.2 — Windows Smart Scan providers

- [ ] `%TEMP%` and user temp data
- [ ] `%LOCALAPPDATA%` application caches
- [ ] Windows Error Reporting data
- [ ] thumbnail/cache candidates where safe
- [ ] browser cache providers where policy allows
- [ ] developer-tool caches shared with macOS where applicable
- [ ] Windows-specific exclusions and confidence rules

### M5.3 — Windows desktop integration + release

- [ ] native window/menu/shortcut validation
- [ ] Windows-specific GPUI behavior regression tests
- [ ] CI build/test job on Windows
- [ ] `.exe`/installer packaging
- [ ] signing and release artifact verification
- [ ] Windows 11 smoke-test matrix

## M6 — optional Flutter frontend

Do not start this milestone until the Rust application API/event boundary is stable enough to prove that frontend replacement does not require changes to cleanup behavior.

### M6.0 — Flutter frontend feasibility spike

- [ ] create a minimal Flutter desktop shell consuming the Rust engine through FFI
- [ ] validate macOS first using the same scan request/result models as GPUI
- [ ] stream scan progress/events from Rust to Dart
- [ ] validate cancellation and execution reporting
- [ ] compare packaging, accessibility, localization, UI velocity, and binary/runtime tradeoffs against GPUI
- [ ] verify that the same Flutter frontend can build on Windows using Windows Rust platform adapters
- [ ] decide whether Flutter becomes an additional frontend, the preferred Windows frontend, or remains only a fallback option

### M6.1 — productionization if adopted

- [ ] generated/stable Dart bindings
- [ ] frontend-only state/presentation layer in Dart
- [ ] localization and accessibility coverage
- [ ] macOS and Windows desktop packaging
- [ ] cross-frontend behavioral parity tests against shared Rust fixtures
- [ ] document ownership boundaries so business logic cannot drift into Flutter

## Cross-platform design rule

New core features should avoid hard-coding macOS paths, APIs, GPUI types, or frontend assumptions. Platform behavior belongs behind adapters/providers selected with `cfg(target_os = ...)`; shared scan models, cleanup planning, policy, reporting, and application events should remain platform- and frontend-neutral whenever practical.
