# Project Handoff

## Current state

M0–M3 are complete and merged to `main`.

M4 macOS integration is functionally implemented through GPUI scheduling controls, signed/notarized packaging, release evidence verification, Homebrew cask generation, and a two-phase release runner. However, macOS is **not considered finished yet**. The project must now return to macOS and complete product usability, trust/safety UX, recovery/reporting polish, and physical release verification before any further Windows product work.

M5 Windows work has already progressed beyond the original feasibility spike into platform adapters, Smart Scan providers, Recycle Bin integration, and the shared Smart Scan cleanup coordinator. Preserve that work as-is, but **pause further Windows implementation now**. In particular, do not continue Windows GPUI review/mutation wiring until the macOS completion gate below is closed.

Permanent delete remains deliberately safety-locked in `cleaner-macos`; GPUI exposes Trash-only cleanup/uninstall execution.

For a code-oriented tour, read [`CODE_WALKTHROUGH.md`](./CODE_WALKTHROUGH.md). For the final M4 release gate, follow [`M4_RELEASE_VERIFICATION.md`](./M4_RELEASE_VERIFICATION.md).

## Priority rule — macOS first

The current engineering priority is:

```text
Finish macOS product + release quality
            ↓
Close macOS completion gate
            ↓
Resume Windows M5
            ↓
Consider optional Flutter frontend later
```

Do not start another Windows feature slice merely because the shared Windows core is ready. macOS is the reference product and must reach a stable, trustworthy, releasable state first.

Windows work may resume only after all macOS completion items in this handoff are done or explicitly deferred with a documented reason.

## Architecture direction

The current product frontend is GPUI on macOS, but GPUI is not part of the cleanup engine.

Dependency direction:

```text
cleaner-core
    ↑
cleaner-macos / platform adapters
    ↑
stable Rust application/platform APIs
    ↑
cleaner-cli / cleaner-gui / future Flutter frontend
```

Rules:

- Rust owns scan rules, cleanup/uninstall policy, planning, safety validation, execution, reports, application inventory, related-file matching, orphan discovery, and platform-native operations.
- GPUI owns presentation and transient UI state only.
- A future Flutter frontend must reuse the same Rust behavior rather than duplicate policy in Dart.
- CLI remains a frontend-independence validation surface.
- GPUI-specific types must not leak into `cleaner-core` or platform APIs.

## Completed milestones

### M0 — foundation

- Rust workspace and crate boundaries
- frontend-neutral core models
- macOS platform adapter boundary
- CLI and GPUI shells
- CI, `make prepush`, and auto-format workflow

### M1 — Smart Scan

- User Cache
- System Cache
- Xcode
- Homebrew
- Node/npm/pnpm/Yarn caches
- protected roots and exclusions
- permission-denied reporting
- live progress events
- cooperative cancellation
- symlink-safe traversal

### M2 — review + Trash-only cleanup

- shared cleanup planner
- review UI
- pinned execution allow-list roots
- execution-time revalidation
- structured execution report
- cancellation
- GPUI Trash-only execution
- permanent-delete category policy exists in core but macOS mutation remains safety-locked

### M3 — app uninstaller

- installed application inventory
- bundle/version/team metadata
- related-file matcher
- confidence tiers
- system-app protection
- orphan finder with fail-closed partial-inventory behavior
- reviewed uninstall planning
- Trash-only uninstall execution
- GPUI application filters, review, execution, cancellation, and reports
- stale reviewed plans are discarded after execution attempts

## M4 — macOS integration already implemented

### Full Disk Access

`cleaner-macos` owns Full Disk Access probing and the System Settings deep link. Status remains explicit `Granted` / `Denied` / `Unknown`.

### Finder and Trash

Finder reveal and Trash integration remain macOS platform operations. Trash uses Finder/AppleScript with paths passed as arguments rather than source interpolation.

### LaunchAgent scheduling

Scheduling is intentionally read-only. The scheduled command is limited to:

```text
dxtr-cleaner scan --category user
```

Trash, uninstall, and permanent-delete mutation are not schedulable.

`cleaner-macos::launch_agent::LaunchAgentCoordinator` is the stable frontend-facing scheduling surface. GPUI does not own LaunchAgent labels, plist paths, `launchctl`, interval policy, or app-bundle CLI layout.

Important durability/safety rules:

- the scheduled CLI is bundled `Contents/MacOS/dxtr-cleaner`
- enabling/repairing is rejected from App Translocation
- existing schedules can still be inspected/disabled from a translocated launch
- status detects stale app paths after the bundle moves
- plist mutation fails closed on unsafe symlink/non-file states
- bootstrap failure restores the previous plist and reactivates the prior job when appropriate

### Packaging and release tooling

`scripts/macos/package.sh` builds the signed `.app` and ZIP. The bundle contains both the GPUI executable and bundled `dxtr-cleaner` CLI.

`scripts/macos/notarize.sh` submits the ZIP with `notarytool`, staples the app, validates the staple, performs Gatekeeper assessment, and rebuilds the final ZIP after stapling.

`scripts/macos/generate_cask.sh` generates `Casks/dxtr-cleaner.rb` only from an explicit version, real SHA-256, and this repository's GitHub Releases URL.

`scripts/macos/verify_release.sh` verifies the app extracted from the exact ZIP being hashed, requires quarantine evidence by default, and can require a prepared expected SHA.

## macOS completion program — do this now

The macOS application is the current product target. Complete the following in order before returning to Windows.

### MAC-1 — cleanup usability and layout

Make the normal macOS Smart Scan → Review → Clean flow comfortable and obvious in the real GPUI app.

- verify cleanup works end-to-end on a real Mac, not only through core/CLI tests
- fix remaining layout density, clipping, awkward spacing, scrolling, disabled-state, and action-hierarchy issues
- ensure scan progress, review, execution, cancellation, empty states, partial results, and error states are understandable
- verify the Uninstaller flow has the same usability quality bar
- test at realistic macOS window sizes rather than one development size
- keep long-running filesystem work off the GPUI event loop

### MAC-2 — cleanup trust UX

Implement the behavior-level ideas selected from the Tidy review without copying GPL implementation code.

1. **Honest disk accounting**
   - distinguish logical size from allocated/on-disk size where macOS exposes it reliably
   - distinguish `moved to Trash` from `actually reclaimed`
   - never claim that Trash-only execution immediately freed the reported bytes

2. **Richer per-item execution outcomes**
   - expose typed outcomes such as executed, skipped, changed since scan, permission denied, protected, missing, failed, and cancelled
   - preserve useful partial-success details instead of collapsing them into one aggregate error

3. **User-facing safety tiers**
   - keep evidence/confidence policy in Rust
   - present clear semantics such as Safe / Review / Risky
   - only genuinely safe items default selected
   - inferred ownership/orphans remain review-only

4. **Permission and scan-coverage health**
   - show Full Disk Access state and scan coverage
   - distinguish Full / Partial scan
   - explain skipped locations
   - provide Open Settings and Re-check actions
   - missing permission must never become a misleading confident zero

5. **Smart Care aggregation polish**
   - orchestrate existing typed categories/providers in one useful surface
   - preserve independent policy, review state, and execution allow-lists
   - do not duplicate cleanup rules in GPUI

Suggested first branch: `macos-cleanup-trust-ux`.

### MAC-3 — cleanup history and recovery

Build on Trash-only mutation rather than adding more destructive behavior.

- record cleanup sessions with timestamp, category/source, item outcomes, and byte accounting
- make previous cleanup activity inspectable
- support Reveal in Trash where appropriate
- add trustworthy restore/Put Back behavior only where macOS/platform semantics can be verified
- never imply recoverability for an operation that cannot actually be restored

Suggested branch: `cleanup-history-and-restore`.

### MAC-4 — Space Lens foundation

After cleanup/recovery UX is stable:

- add frontend-neutral Rust directory-size aggregation/top-offender APIs
- use allocated size where appropriate and clearly define accounting semantics
- support cancellation/progress for large trees
- add GPUI visualization only after the data API is stable
- keep treemap/visualization concepts out of core domain models

Suggested branch: `space-lens-foundation`.

This is still within the cleaner/uninstaller product scope. Clipboard history, network metering, and AI-usage tracking remain out of scope.

### MAC-5 — physical release verification

Complete the real release gate after product polish is ready.

Phase 1 — prepare on the release Mac:

```bash
SIGNING_IDENTITY="Developer ID Application: ..." \
NOTARY_PROFILE=<profile> \
VERSION=<release-version> \
URL="https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v<release-version>/Dxtr%20Cleaner.zip" \
make prepare-macos-release
```

Phase 2 — after publishing and downloading the exact ZIP through a quarantine-applying path:

```bash
ZIP_PATH="/path/to/downloaded/Dxtr Cleaner.zip" \
EXPECTED_SHA256_FILE="/path/to/prepared-expected-sha256" \
make verify-macos-release
```

Required physical checks:

1. real Developer ID packaging succeeds
2. notarization accepted and staple validated
3. prepared SHA retained
4. exact prepared ZIP published
5. quarantined download matches prepared SHA byte-for-byte
6. Gatekeeper verification succeeds on that download
7. first launch from a durable install location succeeds
8. Smart Scan / Review / Trash cleanup smoke succeeds
9. Uninstaller review / Trash smoke succeeds
10. Settings / Full Disk Access state behaves correctly
11. LaunchAgent enable/status/disable smoke succeeds
12. Homebrew install/uninstall smoke succeeds with the generated cask
13. non-secret release evidence is retained
14. only then check off signed/notarized packaging and Homebrew in `ROADMAP.md`

## macOS completion gate

Do **not** resume Windows feature development until these are true:

- [ ] normal macOS cleanup flow is usable and visually stable
- [ ] macOS Uninstaller flow is usable and visually stable
- [ ] honest allocated/logical/Trash accounting is implemented or explicitly documented where unavailable
- [ ] partial-scan and permission health UX is implemented
- [ ] richer execution outcomes are visible and understandable
- [ ] safety-tier presentation is implemented without moving policy into GPUI
- [ ] cleanup history/recovery decision is implemented and validated
- [ ] Space Lens foundation is completed or explicitly deferred after review
- [ ] real signed/notarized release verification passes
- [ ] quarantined-download Gatekeeper verification passes
- [ ] Homebrew install/uninstall smoke passes
- [ ] final macOS smoke test is recorded

When this checklist closes, macOS becomes the stable reference implementation for cross-platform parity.

## Windows status — paused, preserve work

Already completed/progressed Windows work must remain intact:

- GPUI Windows feasibility harness
- Windows platform adapters
- Windows Smart Scan providers
- Recycle Bin integration
- Windows Smart Scan cleanup coordinator using the shared planner/executor and provider-root allow-listing

Do not delete or redesign this work merely because it is paused.

After the macOS completion gate closes, resume M5 with the next planned slice:

- retain reviewed Windows Smart Scan items in the Windows GPUI spike
- invoke the shared Windows cleanup coordinator
- do not duplicate execution policy in Windows GPUI
- then continue the remaining Windows release/platform work

## Permanent-delete safety lock

Permanent deletion remains blocked in the macOS backend.

Reason: path-based `remove_file` / `remove_dir_all` cannot close the ancestor-swap TOCTOU window between validation and mutation.

Do not re-enable permanent delete with another path recheck. A future implementation requires anchored directory-descriptor / no-follow filesystem mutation tying validation and deletion to the same filesystem identity.

## Quality gates

Local development:

```bash
make prepush
```

CI equivalent:

```bash
make ci
```

Always ensure a real CI run exists for the final PR head before merging code changes. A documentation-only handoff update made after an already-green code head may be merged without waiting for another CI cycle once the final diff is confirmed to contain no code, workflow, build, or configuration changes.

## GPUI dependency

GPUI and `gpui_platform` are pinned to Zed commit:

```text
b05f40c5546b47bcf9561136dc0fcdcd9968cb63
```

Upgrade only in a dedicated dependency PR.

## What remains now

**Next work is macOS, not Windows.**

Start with `macos-cleanup-trust-ux` and the remaining real macOS cleanup/layout issues. Continue through recovery/history and the physical release gate until the macOS completion checklist is closed. Only then return to the paused Windows GPUI mutation slice.

A future Flutter desktop frontend remains optional and should not interrupt either the macOS completion program or the later Windows continuation.
