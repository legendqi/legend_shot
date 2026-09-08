# Capture Controls Implementation Plan

> **For agentic workers:** Use independent agents for the isolated hotkey and selection modules; the primary agent integrates editing history and application lifecycle in this session.

**Goal:** Implement global screenshot shortcuts, precise region adjustment, and complete undo/redo.

**Architecture:** Keep ScreenshotApp as the coordinator. Isolate OS hotkey registration in hotkey.rs, pure selection geometry and aids in selection.rs, and edit history in app_history.rs. Preserve the existing display composition and OCR worker.

**Tech Stack:** Rust, egui/eframe 0.33, tray-icon, global-hotkey, existing image/display model.

**Spec:** ../specs/2026-09-08-capture-controls-design.md

## Global Constraints

- Support macOS, Windows and Linux X11; preserve the independent CLI test mode.
- Default shortcut: macOS Command+Shift+A; Windows/Linux Ctrl+Shift+A; configurable.
- Logical coordinate movement: 1; Shift movement: 10; minimum selection dimension: 1.
- History stores edit state only, at most 100 actions; never copies capture pixels.
- Respect text editor focus and IME; report export failures without losing edits.

## Tasks

- [x] Hotkeys: add hotkey.rs with parsing and transactional registration tests; extend tray.rs with settings command and Windows runtime; update Cargo dependencies. Interface: default_capture_shortcut() -> String; HotkeyRuntime::new(ctx) -> Result<Self, String>, set_shortcut(&mut self, &str) -> Result<(), String>, take_triggered(&self) -> bool. Run focused tests after observing missing behavior.
- [x] Selection: add selection.rs with eight-handle hit testing, bounded resizing and nudging; test negative origins, edge crossing and minimum sizes before implementation. Expose ResizeHandle, hit_test_handle, resize_selection, nudge_selection and draw_selection_aids(&ScreenshotApp, usize, &mut egui::Ui).
- [x] History: add tests for annotation undo/redo, numbering, branch invalidation, selection moves, text focus and session reset; observe failure; implement app_history.rs and wire commit points in app_draw.rs, toolbar actions and OCR snapshot restoration.
- [x] Integration: add settings viewport/config fields, initialize hotkeys on the UI thread, poll events while tray-idle, unify Windows tray lifecycle, connect resize drag and keyboard routing, show actionable export errors.
- [x] Validation: run focused tests during development; run full tests, formatting and clippy after integration; inspect the final diff and GUI where available; update README shortcuts/platform behavior and record native platform limitations.

## Baseline

2026-09-08: cargo test --all-targets --offline: 101 passed, 0 failed.

## Final verification

- `cargo fmt --all --check`: passed.
- `cargo clippy --all-targets --offline --locked -- -D warnings`: passed.
- `cargo test --all-targets --offline --locked --quiet`: 146 passed, 0 failed.
- `cargo build --offline --locked`: passed on macOS arm64.
- A minimal harness directly importing current hotkey.rs and tray.rs passed `cargo check --target x86_64-pc-windows-gnu --offline --locked`.
- Full Windows cross-check stopped in ring because `x86_64-w64-mingw32-gcc` is not installed. Linux and Windows native runtime testing was not available.
- Native macOS development app was launched; computer-use could only inspect the hidden coordinator and could not reliably access the tray or trigger the global shortcut. End-to-end GUI behavior is therefore not claimed verified. Test processes were stopped to release their hotkeys.
- Independent code review identified and rechecked three fixes: no nudging during an unfinished annotation; drop/disable hotkey triggers during capture/settings and modal dialogs; use each key event's modifier state and preserve multiple events in a frame. Regression tests cover these paths.
