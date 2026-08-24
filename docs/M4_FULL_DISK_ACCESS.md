# M4 Full Disk Access coordinator

`cleaner-macos` owns Full Disk Access probing and the System Settings deep link. Frontends and the CLI consume the adapter result instead of duplicating macOS permission heuristics.

## Probe semantics

The probe is read-only. It checks existing protected user-data directories in this order: `~/Library/Mail`, `~/Library/Messages`, and `~/Library/Safari`.

- `Granted`: `read_dir` succeeded for an existing protected probe directory.
- `Denied`: the operating system returned `PermissionDenied` for an existing probe directory.
- `Unknown`: `HOME` is unavailable, none of the probe directories exists, or the probe failed for a reason that cannot safely be classified as a permission decision.

`Granted` therefore means the protected-directory probe succeeded; it is not treated as a universal guarantee that every path on disk is readable. The coordinator intentionally preserves `Unknown` instead of inferring a permission state without evidence.

## CLI validation

```text
dxtr-cleaner permissions
dxtr-cleaner permissions --open
```

The first command prints status, probe path, and detail. The second delegates to `SystemMacPlatform::open_full_disk_access_settings()` and opens the macOS Full Disk Access settings pane when running on macOS.

No cleanup or uninstall mutation policy changes in this slice.
