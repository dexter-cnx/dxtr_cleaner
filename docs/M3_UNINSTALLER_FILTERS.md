# M3 Uninstaller Application Filters

The GPUI Uninstaller now exposes location filters for the installed-application inventory without changing uninstall planning or execution semantics.

## Filters

- All
- User (`$HOME/Applications`)
- Local (`/Applications`)
- System (`/System/Applications`)

The application list is no longer truncated to a fixed row count. The existing content pane remains vertically scrollable, so every application matching the selected filter can be reached.

Filtering is presentation-only. Each filtered row retains its original inventory index before invoking uninstall-plan construction, and the fresh-inventory/fresh-related-evidence checks still run immediately before every Trash execution attempt.

Text search is intentionally deferred to a separate GPUI input-focused slice so keyboard/focus/input-method handling can be reviewed independently from the uninstall list behavior.
