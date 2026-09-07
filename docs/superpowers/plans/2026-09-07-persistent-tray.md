# Legend Shot Persistent Tray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a persistent macOS menu-bar and Ubuntu top-bar icon whose native menu starts a capture or exits, while preserving the existing Windows and `--test` lifecycles.

**Architecture:** Keep `eframe` as the application event loop and isolate `tray-icon` integration in `src/tray.rs`. macOS creates the tray on the `eframe` main thread; Linux owns it on a dedicated GTK thread. Both platforms emit `TrayCommand` values into `ScreenshotApp`, which owns the resident lifecycle and hides rather than closes the root viewport after a capture.

**Tech Stack:** Rust 2024, `eframe`/`egui` 0.33.2, `tray-icon` 0.24.2, GTK 0.18.2 on Linux, `image` 0.25.8, macOS Core Graphics permission APIs.

**Spec:** `docs/superpowers/specs/2026-09-07-persistent-tray-design.md`

## Global Constraints

- The resident tray lifecycle applies only to macOS and Linux/Ubuntu.
- Windows must retain the existing start-capture-immediately and exit-after-capture behavior.
- `--test` must not create a tray, start an OCR worker, or enter the resident GUI lifecycle.
- The tray menu contains exactly two enabled items in this order: `截图`, `退出`.
- macOS screen-capture permission is requested only after the user selects `截图`; a denied or newly requested permission leaves the process resident.
- Capture, save, copy, double-click copy, OCR-result close, and ordinary `Esc` return macOS/Linux to tray idle; only the tray `退出` command terminates the resident process.
- Capture must occur while the overlay is hidden, and every new capture must discard previous selection, annotation, input, and texture state.
- The production icon is the approved monochrome focus-frame mark with a transparent background and no text.
- Do not add login startup, global shortcuts, settings, capture history, or new OCR menu items.
- Preserve all unrelated working-tree changes; stage only files belonging to the task being committed.

---

### Task 1: Create the focus-frame tray icon assets

**Files:**
- Create: `src/icon/tray-focus.svg`
- Create: `src/icon/tray-focus-16.png`
- Create: `src/icon/tray-focus-20.png`
- Create: `src/icon/tray-focus-24.png`
- Create: `src/icon/tray-focus-32.png`
- Create: `src/icon/tray-focus-64.png`
- Create: `src/icon/tray-focus-128.png`

**Interfaces:**
- Consumes: approved A “focus frame” direction from the spec.
- Produces: `src/icon/tray-focus-32.png`, embedded by `tray::load_icon`; the other sizes are release/package assets.

- [ ] **Step 1: Add the deterministic SVG master**

Create `src/icon/tray-focus.svg` with `apply_patch` using this exact geometry:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <g fill="none" stroke="#000" stroke-width="6" stroke-linecap="round">
    <path d="M24 8H14a6 6 0 0 0-6 6v10"/>
    <path d="M40 8h10a6 6 0 0 1 6 6v10"/>
    <path d="M56 40v10a6 6 0 0 1-6 6H40"/>
    <path d="M24 56H14a6 6 0 0 1-6-6V40"/>
  </g>
  <circle cx="32" cy="32" r="5" fill="#000"/>
</svg>
```

- [ ] **Step 2: Render a transparent 128 px source PNG on macOS**

Run:

```bash
mkdir -p /tmp/legend-shot-tray-icon
qlmanage -t -s 128 -o /tmp/legend-shot-tray-icon src/icon/tray-focus.svg
```

Expected: `/tmp/legend-shot-tray-icon/tray-focus.svg.png` exists and has a transparent background.

- [ ] **Step 3: Export every committed PNG size**

Run:

```bash
for size in 16 20 24 32 64 128; do
  sips -z "$size" "$size" /tmp/legend-shot-tray-icon/tray-focus.svg.png --out "src/icon/tray-focus-${size}.png"
done
```

Expected: six PNG files are written under `src/icon/`.

- [ ] **Step 4: Verify dimensions, alpha, and small-size legibility**

Run:

```bash
for icon in src/icon/tray-focus-*.png; do
  sips -g pixelWidth -g pixelHeight -g hasAlpha "$icon"
done
```

Expected: each width and height matches its filename and `hasAlpha: yes`. Open the 16 px and 20 px files at 1× scale and confirm the four corners remain separated and the center dot remains visible.

- [ ] **Step 5: Commit the icon assets**

```bash
git add src/icon/tray-focus.svg src/icon/tray-focus-16.png src/icon/tray-focus-20.png src/icon/tray-focus-24.png src/icon/tray-focus-32.png src/icon/tray-focus-64.png src/icon/tray-focus-128.png
git commit -m "feat: add focus-frame tray icon assets"
```

---

### Task 2: Add tray dependencies and the tested command boundary

**Files:**
- Modify: `Cargo.toml:6-31`
- Modify: `Cargo.lock`
- Modify: `src/main.rs:9-18`
- Create: `src/tray.rs`

**Interfaces:**
- Consumes: `src/icon/tray-focus-32.png` from Task 1.
- Produces: `TrayCommand::{Capture, Exit}`, `command_for_menu_id(&str) -> Option<TrayCommand>`, and `load_icon(&[u8]) -> Result<tray_icon::Icon, String>` for Task 3.

- [ ] **Step 1: Add target-scoped dependencies and module declaration**

Add to `Cargo.toml`:

```toml
[target.'cfg(any(target_os = "macos", target_os = "linux"))'.dependencies]
tray-icon = "0.24.2"

[target.'cfg(target_os = "linux")'.dependencies]
gtk = { version = "0.18.2", features = ["v3_24"] }
```

Add below the existing module declarations in `src/main.rs`:

```rust
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod tray;
```

- [ ] **Step 2: Write failing unit tests for menu mapping and icon decoding**

Create `src/tray.rs` with the enum, constants, and these tests, but leave `command_for_menu_id` and `load_icon` undefined:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayCommand {
    Capture,
    Exit,
}

const CAPTURE_MENU_ID: &str = "legend-shot.capture";
const EXIT_MENU_ID: &str = "legend-shot.exit";
const TRAY_ICON_PNG: &[u8] = include_bytes!("icon/tray-focus-32.png");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_known_menu_ids() {
        assert_eq!(command_for_menu_id(CAPTURE_MENU_ID), Some(TrayCommand::Capture));
        assert_eq!(command_for_menu_id(EXIT_MENU_ID), Some(TrayCommand::Exit));
        assert_eq!(command_for_menu_id("legend-shot.unknown"), None);
    }

    #[test]
    fn embedded_tray_icon_is_valid_rgba() {
        assert!(load_icon(TRAY_ICON_PNG).is_ok());
        assert!(load_icon(b"not a png").is_err());
    }
}
```

- [ ] **Step 3: Run the focused tests to verify they fail**

Run:

```bash
cargo test tray::tests -- --nocapture
```

Expected: compilation fails because `command_for_menu_id` and `load_icon` are not defined.

- [ ] **Step 4: Implement the pure mapping and decoder**

Add above the tests:

```rust
fn command_for_menu_id(id: &str) -> Option<TrayCommand> {
    match id {
        CAPTURE_MENU_ID => Some(TrayCommand::Capture),
        EXIT_MENU_ID => Some(TrayCommand::Exit),
        _ => None,
    }
}

fn load_icon(bytes: &[u8]) -> Result<tray_icon::Icon, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("托盘图标解码失败: {error}"))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|error| format!("托盘图标创建失败: {error}"))
}
```

- [ ] **Step 5: Run tests and refresh the lockfile**

Run:

```bash
cargo test tray::tests -- --nocapture
cargo check
```

Expected: both tray tests pass and `cargo check` succeeds; `Cargo.lock` records `tray-icon` and the target-scoped GTK dependency graph.

- [ ] **Step 6: Commit the command boundary**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/tray.rs
git commit -m "feat: add tray command boundary"
```

---

### Task 3: Implement macOS and Linux tray runtimes

**Files:**
- Modify: `src/tray.rs`

**Interfaces:**
- Consumes: `TrayCommand`, `command_for_menu_id`, and `load_icon` from Task 2.
- Produces: `TrayRuntime::start(egui::Context) -> Result<TrayRuntime, String>`, `TrayRuntime::try_recv(&self) -> Option<TrayCommand>`, and `TrayRuntime::shutdown(&mut self)` for `ScreenshotApp`.

- [ ] **Step 1: Add a failing menu-spec test**

Add a pure menu specification and this test:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayMenuSpec {
    id: &'static str,
    label: &'static str,
    command: TrayCommand,
}

fn menu_specs() -> [TrayMenuSpec; 2] {
    [
        TrayMenuSpec { id: CAPTURE_MENU_ID, label: "截图", command: TrayCommand::Capture },
        TrayMenuSpec { id: CAPTURE_MENU_ID, label: "截图", command: TrayCommand::Capture },
    ]
}

#[test]
fn menu_contains_only_capture_then_exit() {
    let specs = menu_specs();
    assert_eq!(specs.len(), 2);
    assert_eq!((specs[0].id, specs[0].label, specs[0].command),
               (CAPTURE_MENU_ID, "截图", TrayCommand::Capture));
    assert_eq!((specs[1].id, specs[1].label, specs[1].command),
               (EXIT_MENU_ID, "退出", TrayCommand::Exit));
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test tray::tests::menu_contains_only_capture_then_exit -- --nocapture
```

Expected: the second assertion fails because the temporary second item is another capture item.

- [ ] **Step 3: Implement the menu specification and native tray builder**

Replace `menu_specs` and add `build_tray`:

```rust
fn menu_specs() -> [TrayMenuSpec; 2] {
    [
        TrayMenuSpec { id: CAPTURE_MENU_ID, label: "截图", command: TrayCommand::Capture },
        TrayMenuSpec { id: EXIT_MENU_ID, label: "退出", command: TrayCommand::Exit },
    ]
}

fn build_tray(
    command_sender: std::sync::mpsc::Sender<TrayCommand>,
    ctx: egui::Context,
) -> Result<tray_icon::TrayIcon, String> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};

    let specs = menu_specs();
    let capture = MenuItem::with_id(specs[0].id, specs[0].label, true, None);
    let exit = MenuItem::with_id(specs[1].id, specs[1].label, true, None);
    let menu = Menu::with_items(&[&capture, &exit])
        .map_err(|error| format!("托盘菜单创建失败: {error}"))?;

    MenuEvent::set_event_handler(Some(move |event| {
        if let Some(command) = command_for_menu_id(event.id().as_ref()) {
            let _ = command_sender.send(command);
            ctx.request_repaint();
        }
    }));

    tray_icon::TrayIconBuilder::new()
        .with_tooltip("Legend Shot")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_icon(load_icon(TRAY_ICON_PNG)?)
        .with_icon_as_template(cfg!(target_os = "macos"))
        .build()
        .map_err(|error| format!("托盘创建失败: {error}"))
}
```

- [ ] **Step 4: Implement `TrayRuntime` with platform-correct ownership**

Add:

```rust
pub(crate) struct TrayRuntime {
    command_receiver: std::sync::mpsc::Receiver<TrayCommand>,
    #[cfg(target_os = "macos")]
    _icon: tray_icon::TrayIcon,
    #[cfg(target_os = "linux")]
    shutdown_sender: std::sync::mpsc::Sender<()>,
}

impl TrayRuntime {
    pub(crate) fn start(ctx: egui::Context) -> Result<Self, String> {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();

        #[cfg(target_os = "macos")]
        {
            let icon = build_tray(command_sender, ctx)?;
            return Ok(Self { command_receiver, _icon: icon });
        }

        #[cfg(target_os = "linux")]
        {
            use std::time::Duration;
            let (shutdown_sender, shutdown_receiver) = std::sync::mpsc::channel();
            let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);

            std::thread::Builder::new()
                .name("legend-shot-tray".to_string())
                .spawn(move || {
                    let result = gtk::init()
                        .map_err(|error| format!("GTK 初始化失败: {error}"))
                        .and_then(|_| build_tray(command_sender, ctx));
                    let icon = match result {
                        Ok(icon) => {
                            let _ = ready_sender.send(Ok(()));
                            icon
                        }
                        Err(error) => {
                            let _ = ready_sender.send(Err(error));
                            return;
                        }
                    };

                    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                        if shutdown_receiver.try_recv().is_ok() {
                            gtk::main_quit();
                            gtk::glib::ControlFlow::Break
                        } else {
                            gtk::glib::ControlFlow::Continue
                        }
                    });
                    gtk::main();
                    drop(icon);
                })
                .map_err(|error| format!("托盘线程启动失败: {error}"))?;

            ready_receiver
                .recv_timeout(Duration::from_secs(3))
                .map_err(|error| format!("等待托盘启动失败: {error}"))??;
            return Ok(Self { command_receiver, shutdown_sender });
        }
    }

    pub(crate) fn try_recv(&self) -> Option<TrayCommand> {
        self.command_receiver.try_recv().ok()
    }

    pub(crate) fn shutdown(&mut self) {
        tray_icon::menu::MenuEvent::set_event_handler(None::<fn(tray_icon::menu::MenuEvent)>);
        #[cfg(target_os = "linux")]
        let _ = self.shutdown_sender.send(());
    }
}
```

- [ ] **Step 5: Run unit tests and native compilation**

Run:

```bash
cargo fmt --check
cargo test tray::tests -- --nocapture
cargo check
```

Expected: formatting, all tray tests, and compilation pass on the current native platform.

- [ ] **Step 6: Commit the platform runtimes**

```bash
git add src/tray.rs
git commit -m "feat: add macOS and Linux tray runtimes"
```

---

### Task 4: Add resident application lifecycle and delayed macOS permission

**Files:**
- Modify: `src/app_default.rs:115-286`
- Modify: `src/app.rs:1-267`
- Modify: `src/main.rs:21-169`

**Interfaces:**
- Consumes: `TrayRuntime` and `TrayCommand` from Task 3; existing `capture_screens`, `screen_to_texture`, and `capture_window_style`.
- Produces: `AppLifecycle::{TrayIdle, Capturing, Exiting}`, `ScreenshotApp::install_tray`, `ScreenshotApp::begin_tray_capture`, `ScreenshotApp::hide_capture_window`, and `ScreenshotApp::exit_application` for Task 5.

- [ ] **Step 1: Write failing lifecycle and permission tests**

Add to the existing tests in `src/app_default.rs`:

```rust
#[test]
fn resident_lifecycle_starts_idle_and_rejects_reentrant_capture() {
    let mut lifecycle = AppLifecycle::TrayIdle;
    assert!(lifecycle.begin_capture());
    assert_eq!(lifecycle, AppLifecycle::Capturing);
    assert!(!lifecycle.begin_capture());
    lifecycle.finish_capture();
    assert_eq!(lifecycle, AppLifecycle::TrayIdle);
}

#[test]
fn exit_is_terminal_for_resident_lifecycle() {
    let mut lifecycle = AppLifecycle::Capturing;
    lifecycle.exit();
    assert_eq!(lifecycle, AppLifecycle::Exiting);
    assert!(!lifecycle.begin_capture());
}
```

Rename the macOS denial test to `macos_denied_screen_capture_permission_requests_and_stays_resident` and make it expect a resident result:

```rust
assert_eq!(
    macos_capture_permission_action(false),
    MacosCapturePermissionAction::RequestAndStayResident,
);
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test resident_lifecycle -- --nocapture
cargo test macos_denied_screen_capture_permission_requests_and_stays_resident -- --nocapture
```

Expected: compilation fails because `AppLifecycle` and `RequestAndStayResident` do not exist.

- [ ] **Step 3: Implement the pure lifecycle model and app fields**

Add to `src/app_default.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycle {
    TrayIdle,
    Capturing,
    Exiting,
}

impl AppLifecycle {
    pub fn begin_capture(&mut self) -> bool {
        if *self != Self::TrayIdle {
            return false;
        }
        *self = Self::Capturing;
        true
    }

    pub fn finish_capture(&mut self) {
        if *self != Self::Exiting {
            *self = Self::TrayIdle;
        }
    }

    pub fn exit(&mut self) {
        *self = Self::Exiting;
    }
}
```

Add these fields to `ScreenshotApp` and initialize them in `Default`:

```rust
pub lifecycle: AppLifecycle,
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub tray_runtime: Option<crate::tray::TrayRuntime>,
```

```rust
lifecycle: AppLifecycle::Capturing,
#[cfg(any(target_os = "macos", target_os = "linux"))]
tray_runtime: None,
```

- [ ] **Step 4: Change macOS permission checking from startup failure to capture-time decision**

In `src/main.rs`, replace `RequestAndExit` with `RequestAndStayResident` and replace `ensure_macos_screen_capture_permission` with:

```rust
#[cfg(target_os = "macos")]
pub(crate) fn macos_screen_capture_is_ready() -> bool {
    use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

    match macos_capture_permission_action(CGPreflightScreenCaptureAccess()) {
        MacosCapturePermissionAction::Capture => true,
        MacosCapturePermissionAction::RequestAndStayResident => {
            let _ = CGRequestScreenCaptureAccess();
            false
        }
    }
}
```

Remove the startup call to `ensure_macos_screen_capture_permission()?`.

- [ ] **Step 5: Implement tray polling and resident actions in `src/app.rs`**

At the start of `update`, call `self.poll_tray_commands(ctx)`. Add these methods:

```rust
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn install_tray(&mut self, tray_runtime: crate::tray::TrayRuntime) {
    self.tray_runtime = Some(tray_runtime);
    self.lifecycle = crate::app_default::AppLifecycle::TrayIdle;
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn poll_tray_commands(&mut self, ctx: &egui::Context) {
    let mut commands = Vec::new();
    if let Some(runtime) = &self.tray_runtime {
        while let Some(command) = runtime.try_recv() {
            commands.push(command);
        }
    }
    for command in commands {
        match command {
            crate::tray::TrayCommand::Capture => self.begin_tray_capture(ctx),
            crate::tray::TrayCommand::Exit => self.exit_application(ctx),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn begin_tray_capture(&mut self, ctx: &egui::Context) {
    if !self.lifecycle.begin_capture() {
        return;
    }

    #[cfg(target_os = "macos")]
    if !crate::macos_screen_capture_is_ready() {
        self.lifecycle.finish_capture();
        return;
    }

    self.reset_capture_state();
    if let Err(error) = self.capture_screens() {
        eprintln!("从托盘启动截图失败: {error}");
        self.lifecycle.finish_capture();
        return;
    }
    self.app_view = crate::app_default::AppView::Capture;
    self.ocr_window_configured = true;
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}

fn reset_capture_state(&mut self) {
    self.selection_rect = None;
    self.mouse_selection_rect = None;
    self.original_selection_rect = None;
    self.mouse_original_selection_rect = None;
    self.annotations.clear();
    self.current_annotation = None;
    self.text_input = None;
    self.number_input = None;
    self.show_toolbar = false;
    self.is_selecting = false;
    self.is_moving_box = false;
    self.display_textures_split.clear();
    self.screenshots_positions.clear();
}

pub(crate) fn hide_capture_window(&mut self, ctx: &egui::Context) {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        self.reset_capture_state();
        self.lifecycle.finish_capture();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        return;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn exit_application(&mut self, ctx: &egui::Context) {
    self.lifecycle.exit();
    if let Some(runtime) = &mut self.tray_runtime {
        runtime.shutdown();
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
```

- [ ] **Step 6: Wire tray creation and platform-specific startup in `main`**

Wrap the existing eager `capture_screens` calls and macOS monitor-derived viewport sizing so they run only on Windows/other non-resident targets. Keep `with_visible(false)` for macOS/Linux, and preserve the current initial capture behavior elsewhere. In the `run_native` creator, add:

```rust
#[cfg(any(target_os = "macos", target_os = "linux"))]
{
    let tray_runtime = crate::tray::TrayRuntime::start(cc.egui_ctx.clone())
        .map_err(|error| std::io::Error::other(error))?;
    app.install_tray(tray_runtime);
}
```

Do not execute this block from `run_test_mode`.

- [ ] **Step 7: Run focused and full tests**

Run:

```bash
cargo fmt --check
cargo test resident_lifecycle -- --nocapture
cargo test macos_ -- --nocapture
cargo test
cargo check
```

Expected: all tests pass and the application compiles on the current platform.

- [ ] **Step 8: Commit resident startup and permission handling**

```bash
git add src/app_default.rs src/app.rs src/main.rs
git commit -m "feat: add resident capture lifecycle"
```

---

### Task 5: Route every completion and close path back to the tray

**Files:**
- Modify: `src/app_draw.rs:129-385`
- Modify: `src/app_toolbar.rs:135-190`
- Modify: `src/app.rs:209-267`
- Modify: `src/app_ocr_view.rs:100-145`

**Interfaces:**
- Consumes: `ScreenshotApp::hide_capture_window` and lifecycle methods from Task 4.
- Produces: one consistent capture-completion path for copy, save, double-click, toolbar cancel, `Esc`, native close requests, and OCR-result close.

- [ ] **Step 1: Write a failing platform-disposition test**

Add to `src/app.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionDisposition {
    Hide,
    Close,
}

pub(crate) fn completion_disposition_for(resident_platform: bool) -> CompletionDisposition {
    let _ = resident_platform;
    CompletionDisposition::Close
}
```

Add tests:

```rust
#[test]
fn resident_platform_hides_after_capture() {
    assert_eq!(completion_disposition_for(true), CompletionDisposition::Hide);
}

#[test]
fn non_resident_platform_closes_after_capture() {
    assert_eq!(completion_disposition_for(false), CompletionDisposition::Close);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
cargo test disposition -- --nocapture
```

Expected: `resident_platform_hides_after_capture` fails because the temporary implementation always returns `Close`; the non-resident test passes.

- [ ] **Step 3: Implement disposition and use it in `hide_capture_window`**

Replace the stub with:

```rust
pub(crate) fn completion_disposition_for(resident_platform: bool) -> CompletionDisposition {
    if resident_platform {
        CompletionDisposition::Hide
    } else {
        CompletionDisposition::Close
    }
}
```

Refactor `hide_capture_window` to match on:

```rust
match completion_disposition_for(cfg!(any(target_os = "macos", target_os = "linux"))) {
    CompletionDisposition::Hide => {
        self.reset_capture_state();
        self.lifecycle.finish_capture();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
    CompletionDisposition::Close => {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
```

- [ ] **Step 4: Replace capture-flow `Close` commands with `hide_capture_window`**

Change only completion/cancel call sites:

- `src/app_draw.rs`: successful double-click copy and the outermost ordinary `Esc` branch.
- `src/app_toolbar.rs`: immediate copy success, delayed `AppSignal::Copy`, and the toolbar `Tool::Exit` action.
- `src/app.rs`: delayed copy completion and successful save-dialog completion.
- `src/app_ocr_view.rs`: successful OCR-result close button and its ordinary `Esc` close path.

Each site must call:

```rust
self.hide_capture_window(ctx);
```

Do not replace the `Close` inside `exit_application`; that is the only close command for the resident process.

- [ ] **Step 5: Intercept the native close button on resident platforms**

At the start of `ScreenshotApp::update`, before drawing the current view, add:

```rust
if cfg!(any(target_os = "macos", target_os = "linux"))
    && self.lifecycle != crate::app_default::AppLifecycle::Exiting
    && ctx.input(|input| input.viewport().close_requested())
{
    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    self.hide_capture_window(ctx);
    return;
}
```

This prevents an Ubuntu window-manager close action from bypassing the resident lifecycle.

- [ ] **Step 6: Verify there are no accidental resident close paths**

Run:

```bash
rg -n "ViewportCommand::Close" src
```

Expected: the remaining matches are the non-resident branch of `hide_capture_window` and `exit_application`; no drawing, toolbar, save, or OCR view directly closes the root viewport.

- [ ] **Step 7: Run the full Rust verification suite**

Run:

```bash
cargo fmt --check
cargo test
cargo check
git diff --check
```

Expected: every command exits successfully.

- [ ] **Step 8: Commit unified completion behavior**

```bash
git add src/app.rs src/app_draw.rs src/app_toolbar.rs src/app_ocr_view.rs
git commit -m "feat: return completed captures to tray"
```

---

### Task 6: Document Ubuntu dependencies and verify platform behavior

**Files:**
- Modify: `build.sh:1-70`
- Modify: `README.md:24-100,130-155,170-185`
- Modify: `README.zh-CN.md:24-100,130-155,170-185`

**Interfaces:**
- Consumes: final tray behavior and native dependency names from Tasks 2–5.
- Produces: actionable build diagnostics and accurate English/Chinese usage documentation.

- [ ] **Step 1: Add a Linux tray dependency preflight to `build.sh`**

Add:

```bash
check_linux_tray_dependencies() {
    if ! pkg-config --exists gtk+-3.0; then
        echo "缺少 GTK3 开发包：Ubuntu 请安装 libgtk-3-dev"
        return 1
    fi
    if ! pkg-config --exists ayatana-appindicator3-0.1 \
        && ! pkg-config --exists appindicator3-0.1; then
        echo "缺少 AppIndicator 开发包：Ubuntu 请安装 libayatana-appindicator3-dev 或 libappindicator3-dev"
        return 1
    fi
}
```

Call `check_linux_tray_dependencies` at the start of both `build_linux_amd64` and `build_linux_arm64`, before `cargo build`.

- [ ] **Step 2: Update Linux prerequisites in both READMEs**

Change the Ubuntu install command to include:

```bash
sudo apt install -y build-essential pkg-config xclip fonts-noto-cjk libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
```

State that `libappindicator3-dev` is an acceptable alternative when `libayatana-appindicator3-dev` is unavailable, and that the desktop session must expose AppIndicator/StatusNotifier support.

- [ ] **Step 3: Update startup and usage documentation in both languages**

Document these exact platform distinctions:

- macOS/Linux start in the top bar with a two-item menu: Capture/截图 and Exit/退出.
- Selecting Capture/截图 opens the overlay; copy, save, or `Esc` returns to the tray.
- macOS asks for screen-capture permission on the first capture request, not at process startup.
- Windows keeps the current immediate-capture and exit-after-completion flow.
- `--test` behavior is unchanged.

Add `tray.rs` to both architecture trees as the owner of menu creation and platform tray threads.

- [ ] **Step 4: Run automated verification**

Run:

```bash
bash -n build.sh
cargo fmt --check
cargo test
cargo check
git diff --check
```

Expected: all commands exit successfully.

- [ ] **Step 5: Check the Windows compatibility boundary**

On a machine/CI runner with the Windows GNU target and native OCR dependencies installed, run:

```bash
cargo check --target x86_64-pc-windows-gnu
```

Expected: compilation succeeds without compiling or referencing `src/tray.rs`; startup remains on the existing eager-capture branch.

- [ ] **Step 6: Perform macOS manual acceptance**

Run `cargo run` and verify:

1. Only the menu-bar focus-frame icon appears initially; there is no Dock icon or overlay.
2. The menu contains only `截图` and `退出`.
3. First `截图` requests permission when needed and remains resident afterward.
4. A later `截图` shows a fresh overlay; copy, save, and ordinary `Esc` each hide it.
5. Repeating capture works without stale selections, annotations, or textures.
6. `退出` removes the icon and terminates the process.

- [ ] **Step 7: Perform Ubuntu manual acceptance**

On Ubuntu GNOME with AppIndicator support, run `cargo run` and verify:

1. The focus-frame icon appears in the top bar and opens the two-item menu.
2. `截图` shows the overlay and supports repeated captures.
3. Copy, save, native-window close, and ordinary `Esc` return to the tray.
4. `退出` stops both the GTK tray thread and the process.

- [ ] **Step 8: Commit build and usage documentation**

```bash
git add build.sh README.md README.zh-CN.md
git commit -m "docs: document persistent tray requirements"
```

---

### Task 7: Final regression review

**Files:**
- Review: all files changed in Tasks 1–6

**Interfaces:**
- Consumes: complete persistent tray implementation.
- Produces: verified branch ready for code review and integration.

- [ ] **Step 1: Inspect the complete task diff without unrelated workspace changes**

Run from the implementation branch/worktree:

```bash
git log --oneline --decorate -7
git diff --stat HEAD~6..HEAD
git diff --check HEAD~6..HEAD
```

Expected: six focused implementation commits after the plan/spec commits, no whitespace errors, and no unrelated files.

- [ ] **Step 2: Run the final automated suite**

Run:

```bash
cargo fmt --check
cargo test
cargo check
bash -n build.sh
```

Expected: every command exits with status 0.

- [ ] **Step 3: Run the existing non-interactive smoke mode**

Run:

```bash
cargo run -- --test "100,100,400,300" --action save --output /tmp/legend-shot-tray-smoke.png
```

Expected: the command saves `/tmp/legend-shot-tray-smoke.png`, exits without starting a tray, and reports a successful capture.

- [ ] **Step 4: Review lifecycle and platform guards**

Confirm in the final diff:

- `TrayRuntime::start` is called only on macOS/Linux normal GUI startup.
- macOS permission is absent from process startup and present in `begin_tray_capture`.
- Windows and `--test` use no tray types.
- Only `exit_application` closes the resident root viewport.
- Every capture request clears old screenshots, positions, textures, selection, annotation, and input state before capturing.

- [ ] **Step 5: Request code review**

Use the `superpowers:requesting-code-review` workflow against the implementation range. Address findings before declaring the feature complete.
