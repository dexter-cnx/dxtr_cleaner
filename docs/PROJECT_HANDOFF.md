# Project Handoff

## Current state

M0–M3 are complete and merged to `main`.

M4 macOS integration is functionally implemented through GPUI scheduling controls, signed/notarized packaging, release evidence verification, Homebrew cask generation, and a two-phase release runner. The remaining M4 work is **physical release verification only**, not feature implementation: run the real Developer ID/notarization flow on macOS, publish the exact prepared ZIP, verify the quarantined download against the prepared digest, then perform Homebrew install/uninstall smoke testing.

Permanent delete remains deliberately safety-locked in `cleaner-macos`; GPUI exposes Trash-only cleanup/uninstall execution.

For a code-oriented tour, read [`CODE_WALKTHROUGH.md`](./CODE_WALKTHROUGH.md). For the final M4 release gate, follow [`M4_RELEASE_VERIFICATION.md`](./M4_RELEASE_VERIFICATION.md).

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

## M4 — macOS integration

### Full Disk Access

`cleaner-macos` owns Full Disk Access probing and the System Settings deep link. Status remains explicit `Granted` / `Denied` / `Unknown`.

### Finder and Trash

Finder reveal and Trash integration remain macOS platform operations. Trash uses Finder/AppleScript with paths passed as arguments rather than source interpolation.

### LaunchAgent scheduling

Scheduling is intentionally read-only.

The scheduled command is limited to:

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

### GPUI Settings

The Settings page supports load status, Enable Daily Smart Scan, Disable, Repair stale configuration, and explicit Disabled / Enabled / Needs repair / Error / Loading states. Scheduling I/O and `launchctl` work stay off the GPUI event loop.

### Packaging and release tooling

`scripts/macos/package.sh` builds the signed `.app` and ZIP. The bundle contains both the GPUI executable and the bundled `dxtr-cleaner` CLI. CI includes a behavioral packaging-contract test so removing/signing the wrong binary cannot silently pass.

`scripts/macos/notarize.sh` submits the ZIP with `notarytool`, staples the app, validates the staple, performs Gatekeeper assessment, and rebuilds the final ZIP after stapling.

`scripts/macos/generate_cask.sh` generates `Casks/dxtr-cleaner.rb` only from an explicit version, real SHA-256, and a URL constrained to this repository's GitHub Releases path. It rejects traversal/dot-segment URL tricks and remains compatible with macOS Bash 3.2.

`scripts/macos/verify_release.sh` verifies the app extracted from the exact ZIP being hashed, requires quarantine evidence by default, writes evidence atomically only after success, and can require a prepared expected SHA so a different valid/notarized build cannot satisfy the final release gate.

### Canonical two-phase release flow

Phase 1 — prepare on the release Mac:

```bash
SIGNING_IDENTITY="Developer ID Application: ..." \
NOTARY_PROFILE=<profile> \
VERSION=<release-version> \
URL="https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v<release-version>/Dxtr%20Cleaner.zip" \
make prepare-macos-release
```

This performs Developer ID packaging → notarization/stapling → prepublish verification → prepared SHA persistence → cask generation. It intentionally does not count local no-quarantine evidence as the final Gatekeeper gate.

Phase 2 — after publishing and downloading the exact ZIP through a quarantine-applying path:

```bash
ZIP_PATH="/path/to/downloaded/Dxtr Cleaner.zip" \
EXPECTED_SHA256_FILE="/path/to/prepared-expected-sha256" \
make verify-macos-release
```

The downloaded ZIP must match the prepared digest byte-for-byte. A different signed/notarized build must fail.

Real Apple credentials remain external and must never be committed.

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

The repository also has a development-branch auto-format workflow. Because GitHub's `GITHUB_TOKEN` does not recursively trigger arbitrary follow-on workflows from bot-pushed formatting commits, always ensure a real CI run exists for the final PR head before merging.

## GPUI dependency

GPUI and `gpui_platform` are pinned to Zed commit:

```text
b05f40c5546b47bcf9561136dc0fcdcd9968cb63
```

Upgrade only in a dedicated dependency PR.

## What remains now

M4 code implementation should not expand further unless physical release verification exposes a real defect.

The only remaining M4 gate is:

1. run `make prepare-macos-release` with the real Developer ID Application identity and notary profile
2. retain accepted notarization/staple/prepublish evidence and the prepared SHA
3. publish that exact ZIP to the release URL used by the cask
4. download it through a quarantining path
5. run `make verify-macos-release` against the prepared expected SHA
6. perform first-launch / Settings / LaunchAgent smoke checks from a durable install location
7. run Homebrew install/uninstall smoke tests using the generated cask
8. retain non-secret evidence
9. only then check off signed/notarized packaging and Homebrew in `ROADMAP.md`

After M4 release validation, the planned next engineering milestone is M5 Windows feasibility/platform work. A future Flutter desktop frontend remains an optional path after the Rust application boundary is stable enough to support it without policy duplication.
