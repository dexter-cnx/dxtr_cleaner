# Dxtr Cleaner

A macOS-first system cleaner/uninstaller prototype built with **Rust + GPUI**.

## Goals

- Rust-first scan/cleanup engine.
- GPUI desktop interface.
- macOS-specific integration isolated behind a platform adapter.
- One engine shared by GUI and CLI.
- Safety-first cleanup lifecycle: **Scan → Plan → Review → Execute → Report**.
- Dry-run by default while the project is in early development.

## Workspace

```text
crates/
  cleaner-core/    Pure Rust domain, scanner, cleanup plan and safety policy
  cleaner-macos/   macOS adapter boundary
  cleaner-cli/     CLI using the same core engine
  cleaner-gui/     GPUI application
```

## Current milestone: M0

M0 deliberately does not delete anything.

Implemented:

- typed scan categories and scan events
- filesystem scan engine for configured roots
- cleanup-plan model
- explicit execution policy with destructive actions disabled by default
- macOS adapter contract
- CLI scan + plan demo
- GPUI shell/dashboard with Smart Scan summary cards
- CI formatting and unit-test workflow

## Run

macOS is the target platform for the GUI.

```bash
make format
cargo test -p cleaner-core -p cleaner-macos -p cleaner-cli
cargo run -p cleaner-cli -- scan --category dev
cargo run -p cleaner-gui
```

## Safety model

The scanner never deletes files. It only produces `ScanItem`s.

A separate planner creates a `CleanupPlan`. Execution requires an explicit
`ExecutionPolicy`; destructive execution is disabled in M0.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/ROADMAP.md`](docs/ROADMAP.md).
