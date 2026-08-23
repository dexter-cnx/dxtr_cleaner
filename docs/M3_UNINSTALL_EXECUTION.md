# M3 Trash-only Uninstall Execution

This slice adds the core execution boundary for reviewed application uninstall plans. The mutation mode is **Move to Trash only**. Permanent deletion remains safety-locked.

## Safety invariants

- A protected application cannot create an uninstall execution policy.
- The application identity is pinned from the reviewed plan using path, application location, and bundle identifier.
- Immediately before execution, the caller must provide a freshly inventoried current application. Identity mismatch is treated as a stale plan.
- System/Apple protection is re-evaluated against the current application immediately before mutation.
- The selected path set is pinned after review. Any selection change after policy creation invalidates execution as stale.
- Related-file items can be pinned only when the same path, kind, and confidence are still present in a fresh `RelatedFileReport`; the executor does not trust arbitrary plan paths as ownership evidence.
- Every selected path is checked with `symlink_metadata`, expected filesystem type, and canonicalization when the policy is pinned and again immediately before Trash.
- A canonical-path change between review and execution aborts execution.
- Broad protected roots are rejected before mutation.
- Symlinks are rejected.
- Related files are moved before the required application bundle. The app bundle is moved last so cancellation during residual cleanup preferentially leaves the application installed.
- Cancellation is cooperative through the shared `CancellationToken`.
- Backend failures are preserved as per-item records; the executor never falls back to permanent deletion.

## Mutation boundary

`UninstallExecutor` consumes the existing `TrashBackend` abstraction. On macOS, `SystemMacPlatform` already implements this backend through the existing Finder/Trash adapter.

There is intentionally no public CLI `uninstall --execute` command or GPUI execution wiring in this slice. User-facing mutation wiring should be added only after this core policy receives review and CI coverage.

## Remaining limitation

Trash mutation still uses path-based platform APIs after immediate canonical revalidation. Permanent deletion remains disabled until an anchored directory-descriptor/no-follow mutation design closes the stronger TOCTOU requirement.
