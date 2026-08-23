# M3 Reviewed Uninstall Planning

This slice defines the review model only. It performs no filesystem mutation.

## Core rules

- `UninstallPlan` lives in `cleaner-core` and is frontend-neutral.
- System/Apple-protected applications produce a fully locked plan with zero selected items.
- For an unprotected application, the `.app` bundle is a required primary item: it starts selected and cannot be deselected.
- High-confidence related files start selected.
- Medium- and Low-confidence related files are review-only, start unselected, and require explicit user opt-in.
- A frontend may toggle only selectable related-file items through the core plan API; it must not duplicate these rules.
- Duplicate installed applications sharing one bundle identifier are treated as ambiguous by the CLI validation surface and are not auto-selected.
- Incomplete application inventory prevents CLI plan construction.

## CLI validation

`dxtr-cleaner uninstall-plan <bundle-id>` builds and prints the same core plan. It is explicitly review-only and does not move, trash, or delete anything.

## Execution remains disabled

A later slice must define uninstall execution separately. That work must start Trash-only, revalidate every selected path at execution time, preserve system-app protection, and reject stale or escaped paths before any mutation.
