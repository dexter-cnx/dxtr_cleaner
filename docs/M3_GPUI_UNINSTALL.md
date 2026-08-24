# M3 GPUI Uninstall Flow

The GPUI Uninstaller is a frontend for the shared Rust uninstall planning and Trash-only execution APIs.

## Review flow

- Installed applications are inventoried off the UI thread.
- Incomplete inventory fails closed instead of showing a partial uninstallable set.
- Selecting an application builds the shared `UninstallPlan`.
- Protected Apple/system applications remain locked.
- The application bundle is required for an unprotected uninstall plan.
- High-confidence related files start selected; Medium/Low review-only evidence requires explicit user opt-in through the core plan API.
- The uninstall review renders every plan item rather than truncating the list, so no selected path can be executed without being exposed in the review surface.
- The content pane is vertically scrollable, keeping all review rows and execution controls reachable in the bounded desktop window.

## Execution flow

Each execution attempt refreshes application inventory and related-file evidence before mutation. The worker then creates a fresh `UninstallExecutionPolicy`, pins the current related-file execution roots, and runs `UninstallExecutor` through the macOS Trash backend.

Smart Care cleanup and application uninstall share a destructive-operation guard. Only one Trash mutation flow may execute or cancel at a time, avoiding races between cleanup of a related path and uninstall of its containing application data.

The reviewed plan and cached application list are discarded after every execution attempt, including cancellation or failure, so another mutation requires fresh inventory and review.

Runtime safety failures preserve partial execution records for anything that was already moved to Trash. Permanent deletion remains unavailable.
