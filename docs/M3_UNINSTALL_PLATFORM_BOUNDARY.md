# M3 Uninstall Platform Boundary

This slice keeps the reviewed uninstall/execution model frontend-neutral while moving macOS-specific mutation-root construction behind the macOS adapter.

## Boundary

- `cleaner-core` owns uninstall planning, selection policy, execution pinning, stale-plan checks, path revalidation, and execution reporting.
- `cleaner-macos` owns the allow-listed macOS related-file execution roots derived from the current HOME directory.
- `cleaner-gui` requests those roots from `SystemMacPlatform` and does not construct `~/Library/...` mutation roots itself.

If HOME is unavailable, the macOS adapter refuses to provide execution roots and uninstall remains blocked before mutation.

The supported related-file roots are Application Support, Caches, Containers, HTTPStorages, Preferences, and Saved Application State. Group Containers remain excluded from ownership inference and uninstall execution.

The adapter only supplies the platform root set; `cleaner-core` still pins canonical roots and revalidates them immediately before Trash, so moving root construction out of GPUI does not weaken execution-time safety checks.
