# M5.0 Windows GPUI Feasibility Spike

This spike is intentionally isolated from the production macOS GPUI application.

## Goal

Determine whether the currently pinned GPUI revision can support a Windows 11 frontend while the Rust engine remains frontend- and platform-neutral.

## Spike crate

`crates/cleaner-gui-windows-spike`

Dependencies:

- `cleaner-core`
- `gpui`
- `gpui_platform`

It deliberately does **not** depend on `cleaner-macos`.

## Current probe

The spike accepts a disposable directory path as its first command-line argument and streams `FileSystemScanner` events into a minimal GPUI window.

Example on Windows:

```powershell
cargo run -p cleaner-gui-windows-spike -- C:\path\to\disposable\fixture
```

The current slice is read-only. No Recycle Bin or delete operation is wired yet.

## CI evidence

The `windows-gpui-spike` job runs on `windows-latest` and executes:

```text
cargo check -p cleaner-gui-windows-spike
```

A passing job proves that the pinned GPUI/core combination compiles for Windows. It does not prove GUI launch/runtime behavior; that still requires a real Windows 11 interactive smoke test.

## Next gates

1. Windows compile probe passes.
2. Launch the shell on a real Windows 11 desktop.
3. Validate layout and click handling.
4. Scan a disposable selected directory and confirm streamed results.
5. Add a Windows platform adapter for a disposable Recycle Bin mutation test.
6. Record GPUI/API gaps and decide whether GPUI Windows is viable for production.

M4 physical macOS release verification remains independently open and must not be marked complete from this spike.
