# Project Handoff

## Current state

M0–M3 are complete and merged to `main`.

M4 macOS integration is functionally implemented through GPUI scheduling controls, signed/notarized packaging scripts, and Homebrew cask generation. The remaining M4 work is **real release verification**, not feature implementation: Developer ID signing, notarization/stapling/Gatekeeper evidence, publication of the exact final ZIP, and Homebrew install/uninstall smoke testing.

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

`cleaner-macos::launch_agent::LaunchAgentCoordinator` is the stable frontend-facing scheduling surface. GPUI does not own:

- LaunchAgent label
- plist path construction
- `launchctl`
- interval policy
- app-bundle CLI layout

The coordinator exposes status, enable-daily, and disable behavior.

Important durability rules:

- the scheduled CLI is the bundled `Contents/MacOS/dxtr-cleaner`
- enabling/repairing is rejected from App Translocation
- an existing schedule can still be inspected and disabled from a translocated launch
- status detects when a plist points at an old app location after the app is moved/renamed
- stale schedules are surfaced to GPUI as needing repair rather than silently reported as healthy

### GPUI Settings

The Settings page is now a real scheduling UI.

It supports:

- load current schedule status
- Enable Daily Smart Scan
- Disable
- Repair a stale app-location schedule
- Disabled / Enabled / Needs repair / Error / Loading states

Scheduling I/O and `launchctl` work run off the GPUI event loop through a worker/channel boundary.

### Packaging

`scripts/macos/package.sh` builds the release app bundle and ZIP.

The bundle contains both:

- GPUI application executable
- `dxtr-cleaner` CLI used by scheduled scan integration

The packaging script supports Developer ID signing and hardened runtime. Ad-hoc signing remains local-smoke-only and does not satisfy the release gate.

### Notarization

`scripts/macos/notarize.sh` submits the final ZIP through `notarytool`, staples the app, validates the staple, performs Gatekeeper assessment, and rebuilds the ZIP after stapling.

Real Apple credentials are external and must never be committed.

### Homebrew cask

`scripts/macos/generate_cask.sh` generates `Casks/dxtr-cleaner.rb` only from an explicit version, real 64-hex SHA-256, and HTTPS release URL.

The generator does not invent placeholder hashes and protects generated Ruby literals from interpolation.

Do not publish a cask that points to an ad-hoc-signed or pre-notarization artifact.

## Permanent-delete safety lock

Permanent deletion remains blocked in the macOS backend.

Reason: path-based `remove_file` / `remove_dir_all` cannot close the ancestor-swap TOCTOU window between validation and mutation.

Do not re-enable permanent delete with another path recheck. The acceptable future implementation requires anchored directory-descriptor / no-follow filesystem mutation tying validation and deletion to the same filesystem identity.

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

M4 code implementation should not expand further unless release verification exposes a real defect.

The next work is the physical release gate in [`M4_RELEASE_VERIFICATION.md`](./M4_RELEASE_VERIFICATION.md):

1. build with the real Developer ID Application certificate
2. verify code signing and hardened runtime
3. submit to Apple notarization
4. staple and pass Gatekeeper assessment
5. smoke-test the exact final ZIP on a clean/fresh macOS context
6. publish that exact ZIP
7. calculate its final SHA-256
8. generate the Homebrew cask from the published artifact
9. run Homebrew install/uninstall smoke tests
10. retain non-secret release evidence

Do **not** mark signed/notarized packaging or Homebrew as complete in the roadmap until those real-artifact checks pass.

After M4 release validation, the planned next engineering milestone is M5 Windows feasibility/platform work. A future Flutter desktop frontend remains an explicit optional path after the Rust application boundary is stable enough to support it without policy duplication.
