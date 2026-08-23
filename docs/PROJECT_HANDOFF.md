# Project Handoff

## Current state

M0, M1 Smart Scan, and M2 review + Trash-only execution are complete and merged to `main`. M3.1 installed-application inventory, M3.2 bundle/team metadata, M3.3 related-file matching/confidence tiers, and M3.4 system-app protection are merged. Permanent delete remains safety-locked in the macOS backend and is not exposed by GPUI.

M3.5 orphan discovery is implemented on `feature/m3-orphan-finder`. The finder is read-only and frontend-neutral in `cleaner-core`; `cleaner-macos` scans bundle-shaped Library entries and `dxtr-cleaner orphans` provides a CLI validation surface.

### M3 orphan safety policy

- orphan discovery consumes the full `ApplicationInventoryReport`, not just the discovered application slice
- if application inventory is partial (`inventory.issues` is non-empty), orphan classification fails closed and returns no candidates
- missing `HOME` is an explicit orphan-finder issue rather than an empty-success result
- only safe reverse-DNS-shaped bundle identifiers and expected filesystem entry kinds are considered
- live top-level bundle identifiers and the exact `com.apple` namespace are excluded
- symlinks are excluded
- `Preferences/ByHost` is intentionally not inferred because the bundle-ID/host-suffix boundary is ambiguous
- current application inventory does not enumerate embedded `.appex`/`.xpc` bundle identifiers, so orphan candidates are conservatively **Medium confidence / review-only**
- High-confidence orphan ownership must not be restored until nested bundle inventory closes that ownership gap
- this slice performs no mutation and creates no uninstall execution plan

## Next step

After M3.5 is merged, design reviewed uninstall planning. Protected applications must be impossible to select; related-file confidence remains visible; lower-confidence/review-only data stays default-unselected; execution remains Trash-only and must revalidate paths at mutation time.

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
