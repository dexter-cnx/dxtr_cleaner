# M3 Orphan Scan Fail-Closed Policy

Orphan classification now fails closed when any configured orphan-scan root cannot be examined safely.

## Invariant

If the orphan scan records any `OrphanFinderIssue`, the returned `OrphanReport` contains no candidates. This applies even when candidates were discovered successfully in earlier roots during the same scan.

Examples that make the scan incomplete include unreadable roots, symlinked roots, non-directory roots, `read_dir` failures, and entry metadata failures.

This complements the existing application-inventory rule: incomplete application inventory skips orphan classification entirely. Orphan evidence remains review-only until nested bundle inventory is implemented, and no orphan candidate is attached automatically to an installed application's uninstall plan.
