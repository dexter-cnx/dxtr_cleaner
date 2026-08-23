# Architecture

## Dependency direction

```text
cleaner-gui ─┐
             ├──> cleaner-core
cleaner-cli ─┘

cleaner-gui ───> cleaner-macos ───> cleaner-core
```

`cleaner-core` contains no GPUI, AppKit, Objective-C, or shell-command dependency.

## Cleanup lifecycle

```text
Scanner
  ↓
Vec<ScanItem>
  ↓
Planner
  ↓
CleanupPlan
  ↓
Review
  ↓
Safety validation
  ↓
Executor
  ↓
CleanupReport
```

M0 stops before destructive execution.

## Platform boundary

macOS functionality lives behind `MacPlatform` so future AppKit/objc2 integration
cannot leak into the scan domain.

Planned macOS responsibilities:

- Full Disk Access status and System Settings navigation
- Finder reveal / Trash integration
- installed application discovery
- LaunchServices / bundle metadata
- Spotlight metadata
- scheduled background launch integration
- privileged cleanup authorization where unavoidable

## GPUI policy

GPUI/Zed dependencies are pinned to an explicit commit in the workspace manifest.
Dependency bumps should be isolated and tested in dedicated changes.
