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

Permanent delete remains an explicit core policy opt-in. The default is Trash for every category; M2 permits permanent delete only for generated/cache categories (`UserCache`, `Xcode`, `Homebrew`, `Node`) and rejects it for `SystemCache`, `Docker`, and `LargeFiles`.

## M3 — app uninstaller

- [ ] installed app inventory
- [ ] bundle/team metadata
- [ ] related-file matcher
- [ ] confidence tiers
- [ ] system-app protection
- [ ] orphan finder

## M4 — macOS integration

Primary implementation path: continue shipping the macOS application with GPUI while keeping frontend dependencies outside the Rust engine boundary.

- [ ] Full Disk Access coordinator
- [ ] Finder reveal
- [ ] System Settings deep links
- [ ] scheduling / launch agent
- [ ] signed + notarized packaging
- [ ] Homebrew cask

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

Target Windows only after the core scan/cleanup model and primary macOS flow are stable. The first Windows experiment may reuse GPUI, but the Rust engine must not depend on that choice.

### M5.0 — GPUI Windows feasibility spike

- [ ] create `spike/windows-gpui`
- [ ] build and launch the GPUI shell on Windows 11
- [ ] validate sidebar/layout rendering and basic interaction
- [ ] scan a user-selected directory and stream results into the GUI
- [ ] delete a disposable test file through the platform adapter
- [ ] record GPUI/API gaps and decide whether GPUI Windows can become a supported production target

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
