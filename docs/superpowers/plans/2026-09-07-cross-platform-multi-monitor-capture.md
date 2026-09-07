# Cross-Platform Multi-Monitor Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add reliable multi-monitor screenshot selection across Windows, macOS, and Linux X11, including cross-screen dragging, mixed DPI composition, annotations, clipboard, save, and OCR.

**Architecture:** Keep the eframe root viewport as a coordinator and create one borderless overlay viewport per captured display. Store selection and annotations in global logical coordinates, render monitor-local projections, and route save/copy/OCR through one tested RGBA compositor.

**Tech Stack:** Rust 2024, egui/eframe 0.33.2 multi-viewports, xcap 0.9.4, device_query 4.0.1, image 0.25.8, rfd 0.15.4.

**Spec:** `docs/superpowers/specs/2026-09-07-cross-platform-multi-monitor-capture-design.md`

## Global Constraints

- Supported platforms are Windows, macOS, and Linux X11; Linux Wayland must fail with an explicit unsupported-session message.
- One capture session uses an immutable display topology and refreshes topology at the start of the next capture.
- Every selection and annotation point is stored in signed global logical coordinates; viewport-local and native-pixel coordinates exist only at conversion boundaries.
- A selection may span any number of monitors and may contain desktop gaps; gap pixels in the composed RGBA result are transparent.
- Mixed-DPI output uses the maximum X and Y pixel scales of intersected displays and rejects output over `100_000_000` pixels.
- The existing `MAX_TEXTURE_SIZE` remains `2048`; tiling is per display and texture keys include the display session index.
- Do not add a Wayland or portal dependency in this implementation.
- Preserve the user's existing uncommitted changes in `src/app.rs`, `src/app_default.rs`, `src/tray.rs`, and tray icon files. Inspect each overlapping diff before editing, stage only task files, and never discard unrelated hunks.
- Follow TDD: observe each new test fail for the expected reason before adding its production implementation.

## File Map

- Create `src/display.rs`: typed display geometry, capture-session data, texture tile specs, composition planning/compositing, toolbar placement, and pure unit tests.
- Modify `src/main.rs`: register `display`, keep the root viewport hidden during capture, validate Linux session type, and adapt test mode to the canonical global selection.
- Modify `src/app_default.rs`: replace parallel single-screen vectors/scalars with `CaptureSession`, per-display textures, and global pointer/selection state; make capture assignment atomic.
- Modify `src/app.rs`: coordinate overlay viewport creation/reveal/close, shared repainting, lifecycle errors, and OCR root-window transitions.
- Modify `src/app_draw.rs`: render a specified display, translate global geometry to local coordinates, and update active interactions from the system pointer.
- Modify `src/app_toolbar.rs`: assign the toolbar to exactly one display and convert its global placement to that viewport's local coordinates.
- Modify `src/app_handle.rs`: replace single-monitor crop logic with shared multi-monitor composition and a single annotation-to-output transform.
- Modify `src/app_ocr.rs`: consume the same composed selection and update snapshot/tests after removal of dual mouse coordinates.
- Modify `src/ui.rs`: remove or stop using `get_screen_rect` once display geometry owns coordinate conversion.
- Modify `README.md`: document multi-monitor platform support, X11-only Linux scope, and Wayland behavior.

---

### Task 1: Typed Display Geometry and Texture Tiles

**Files:**
- Create: `src/display.rs`
- Modify: `src/main.rs:9-20`

**Interfaces:**
- Consumes: `egui::{Pos2, Rect, Vec2}` and `image::RgbaImage`.
- Produces: `PixelRect`, `DisplayGeometry::new`, `DisplayGeometry::global_to_local_rect`, `DisplayGeometry::logical_intersection_to_pixels`, `DisplayTile`, `CapturedDisplay::from_image`, `CaptureSession::new`, `CaptureSession::display_at`, and `tile_rects`.

- [ ] **Step 1: Register the module and write failing geometry tests**

Add `mod display;` beside the other module declarations in `src/main.rs`. Create `src/display.rs` with tests that define the required behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(index: usize, bounds: Rect, pixels: (u32, u32)) -> DisplayGeometry {
        DisplayGeometry::new(index, bounds, pixels).unwrap()
    }

    #[test]
    fn maps_negative_origin_global_selection_to_native_pixels() {
        let display = geometry(
            0,
            Rect::from_min_size(Pos2::new(-1920.0, -200.0), Vec2::new(1920.0, 1080.0)),
            (1920, 1080),
        );
        assert_eq!(
            display.logical_intersection_to_pixels(Rect::from_min_max(
                Pos2::new(-1820.0, -150.0),
                Pos2::new(-1780.0, -120.0),
            )),
            Some(PixelRect { x: 100, y: 50, width: 40, height: 30 })
        );
    }

    #[test]
    fn retina_mapping_floors_min_and_ceils_max_edges() {
        let display = geometry(
            0,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1512.0, 982.0)),
            (3024, 1964),
        );
        assert_eq!(
            display.logical_intersection_to_pixels(Rect::from_min_max(
                Pos2::new(300.25, 400.25),
                Pos2::new(985.25, 607.25),
            )),
            Some(PixelRect { x: 600, y: 800, width: 1371, height: 415 })
        );
    }

    #[test]
    fn desktop_bounds_include_left_and_upper_displays() {
        let displays = vec![
            CapturedDisplay::blank(geometry(0, Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0)), (1920, 1080))),
            CapturedDisplay::blank(geometry(1, Rect::from_min_size(Pos2::new(-1280.0, -1024.0), Vec2::new(1280.0, 1024.0)), (1280, 1024))),
        ];
        let session = CaptureSession::new(displays).unwrap();
        assert_eq!(session.desktop_bounds.min, Pos2::new(-1280.0, -1024.0));
        assert_eq!(session.desktop_bounds.max, Pos2::new(1920.0, 1080.0));
    }

    #[test]
    fn tiles_one_display_without_reusing_another_display_identity() {
        assert_eq!(
            tile_rects((4097, 2050), 2048),
            vec![
                PixelRect { x: 0, y: 0, width: 2048, height: 2048 },
                PixelRect { x: 2048, y: 0, width: 2048, height: 2048 },
                PixelRect { x: 4096, y: 0, width: 1, height: 2048 },
                PixelRect { x: 0, y: 2048, width: 2048, height: 2 },
                PixelRect { x: 2048, y: 2048, width: 2048, height: 2 },
                PixelRect { x: 4096, y: 2048, width: 1, height: 2 },
            ]
        );
    }
}
```

- [ ] **Step 2: Run the new test module and verify failure**

Run: `cargo test display::tests -- --nocapture`

Expected: compilation fails because the geometry and session types are not defined.

- [ ] **Step 3: Implement the minimal typed geometry model**

Implement these public shapes and exact invariants in `src/display.rs`:

```rust
use egui::{Pos2, Rect, Vec2};
use image::RgbaImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayGeometry {
    pub session_index: usize,
    pub logical_bounds: Rect,
    pub pixel_size: (u32, u32),
    pub pixel_scale: Vec2,
}

#[derive(Clone)]
pub struct DisplayTile {
    pub pixel_rect: PixelRect,
    pub image: RgbaImage,
}

#[derive(Clone)]
pub struct CapturedDisplay {
    pub geometry: DisplayGeometry,
    pub original_image: RgbaImage,
    pub tiles: Vec<DisplayTile>,
}

pub struct CaptureSession {
    pub displays: Vec<CapturedDisplay>,
    pub desktop_bounds: Rect,
}
```

`DisplayGeometry::new` must reject non-finite bounds, non-positive logical dimensions, and zero pixel dimensions. Compute `pixel_scale` as `(pixel_width / logical_width, pixel_height / logical_height)`. `logical_intersection_to_pixels` intersects first, floors minimum edges, ceils maximum edges, clamps to the captured image, and returns `None` for an empty result. `global_to_local_rect` subtracts `logical_bounds.min`. `CaptureSession::new` rejects an empty display list and unions all logical bounds. `display_at` returns the containing display, otherwise the display with the smallest squared distance from the point to its rectangle.

`CapturedDisplay::from_image(geometry, image, max_tile_size)` verifies `image.dimensions() == geometry.pixel_size`, crops every rectangle returned by `tile_rects`, and returns the populated display. `CapturedDisplay::blank` exists only under `#[cfg(test)]` and calls `from_image` with a transparent image and `2048`.

Build each `DisplayTile` by cropping `tile_rects(pixel_size, MAX_TEXTURE_SIZE as u32)` from its display image; never put tiles from different displays in one unkeyed vector.

- [ ] **Step 4: Run geometry tests and the existing suite**

Run: `cargo test display::tests -- --nocapture`

Expected: all new geometry tests pass.

Run: `cargo test`

Expected: all existing tests pass.

- [ ] **Step 5: Commit the geometry foundation**

```bash
git add src/display.rs src/main.rs
git commit -m "feat: add multi-monitor display geometry"
```

---

### Task 2: Mixed-DPI Composition Core

**Files:**
- Modify: `src/display.rs`

**Interfaces:**
- Consumes: `DisplayGeometry`, `CapturedDisplay`, and `PixelRect` from Task 1.
- Produces: `OutputTransform`, `ComposedSelection`, `plan_composition(displays: &[DisplayGeometry], selection: Rect) -> Result<CompositionPlan, String>`, and `compose_selection(displays: &[CapturedDisplay], selection: Rect) -> Result<ComposedSelection, String>`.

- [ ] **Step 1: Write failing composition tests**

Add this helper in the existing `display.rs` test module, then add the tests:

```rust
fn solid_display(
    index: usize,
    bounds: (f32, f32, f32, f32),
    pixels: (u32, u32),
    color: [u8; 4],
) -> CapturedDisplay {
    let geometry = geometry(
        index,
        Rect::from_min_size(
            Pos2::new(bounds.0, bounds.1),
            Vec2::new(bounds.2, bounds.3),
        ),
        pixels,
    );
    let image = RgbaImage::from_pixel(pixels.0, pixels.1, image::Rgba(color));
    CapturedDisplay::from_image(geometry, image, 2048).unwrap()
}

#[test]
fn composition_keeps_monitor_gap_transparent() {
    let left = solid_display(0, (0.0, 0.0, 100.0, 100.0), (100, 100), [255, 0, 0, 255]);
    let right = solid_display(1, (150.0, 0.0, 100.0, 100.0), (100, 100), [0, 0, 255, 255]);
    let composed = compose_selection(
        &[left, right],
        Rect::from_min_size(Pos2::ZERO, Vec2::new(250.0, 100.0)),
    ).unwrap();
    assert_eq!(composed.image.dimensions(), (250, 100));
    assert_eq!(composed.image.get_pixel(10, 10).0, [255, 0, 0, 255]);
    assert_eq!(composed.image.get_pixel(125, 10).0, [0, 0, 0, 0]);
    assert_eq!(composed.image.get_pixel(200, 10).0, [0, 0, 255, 255]);
}

#[test]
fn composition_uses_highest_scale_without_changing_apparent_size() {
    let normal = solid_display(0, (0.0, 0.0, 100.0, 100.0), (100, 100), [255, 0, 0, 255]);
    let retina = solid_display(1, (100.0, 0.0, 100.0, 100.0), (200, 200), [0, 255, 0, 255]);
    let composed = compose_selection(
        &[normal, retina],
        Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0)),
    ).unwrap();
    assert_eq!(composed.image.dimensions(), (400, 200));
    assert_eq!(composed.transform.scale, Vec2::new(2.0, 2.0));
    assert_eq!(composed.image.get_pixel(100, 100).0, [255, 0, 0, 255]);
    assert_eq!(composed.image.get_pixel(300, 100).0, [0, 255, 0, 255]);
}

#[test]
fn composition_rejects_empty_and_oversized_outputs() {
    let small = solid_display(0, (0.0, 0.0, 1.0, 1.0), (1, 1), [0, 0, 0, 255]);
    assert!(compose_selection(&[small], Rect::ZERO).is_err());
    let display = geometry(
        0,
        Rect::from_min_size(Pos2::ZERO, Vec2::new(20_000.0, 20_000.0)),
        (20_000, 20_000),
    );
    let error = plan_composition(
        &[display],
        Rect::from_min_size(Pos2::ZERO, Vec2::new(20_000.0, 20_000.0)),
    ).unwrap_err();
    assert!(error.contains("100000000"));
}
```

- [ ] **Step 2: Run composition tests and verify failure**

Run: `cargo test display::tests::composition -- --nocapture`

Expected: compilation fails because composition interfaces do not exist.

- [ ] **Step 3: Implement planning and alpha composition**

Add:

```rust
pub const MAX_OUTPUT_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputTransform {
    pub selection: Rect,
    pub scale: Vec2,
}

impl OutputTransform {
    pub fn global_to_output(&self, point: Pos2) -> Pos2 {
        let local = point - self.selection.min;
        Pos2::new(local.x * self.scale.x, local.y * self.scale.y)
    }
}

pub struct ComposedSelection {
    pub image: RgbaImage,
    pub transform: OutputTransform,
}
```

`plan_composition` must normalize selection edges, collect only intersecting displays, reject no intersections, select maximum per-axis scale, calculate output dimensions with `ceil`, use checked `u64` multiplication, and reject more than `MAX_OUTPUT_PIXELS`. Each `CompositionPiece` records display index, source `PixelRect`, and destination `PixelRect`.

`compose_selection` allocates `RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]))`, crops each native source, resizes mismatched pieces with `image::imageops::FilterType::Lanczos3`, and copies them into the planned destination. Assert through returned errors that every source and destination is in bounds instead of silently skipping a display.

- [ ] **Step 4: Run focused and full tests**

Run: `cargo test display::tests::composition -- --nocapture`

Expected: all composition tests pass with transparent gaps and a `400x200` mixed-DPI result.

Run: `cargo test`

Expected: all tests pass.

- [ ] **Step 5: Commit the compositor**

```bash
git add src/display.rs
git commit -m "feat: compose selections across displays"
```

---

### Task 3: Atomic Capture Session and Per-Display Textures

**Files:**
- Modify: `src/app_default.rs:217-470`
- Modify: `src/app.rs:219-253,422-463`

**Interfaces:**
- Consumes: `CaptureSession`, `CapturedDisplay`, `DisplayGeometry`, `DisplayTile`, and `tile_rects`.
- Produces: `ScreenshotApp::capture_screens() -> Result<(), String>`, `ScreenshotApp::install_capture_session(session: CaptureSession)`, `DisplayTextureTile`, and `ScreenshotApp::ensure_display_textures(ctx: &egui::Context)`.

- [ ] **Step 1: Write failing state and atomic-assignment tests**

In `app_default.rs` tests, build two small `CapturedDisplay` values and verify replacement is all-or-nothing:

```rust
fn test_display(
    index: usize,
    bounds: (f32, f32, f32, f32),
    pixels: (u32, u32),
) -> CapturedDisplay {
    let geometry = DisplayGeometry::new(
        index,
        Rect::from_min_size(
            Pos2::new(bounds.0, bounds.1),
            Vec2::new(bounds.2, bounds.3),
        ),
        pixels,
    ).unwrap();
    CapturedDisplay::from_image(geometry, RgbaImage::new(pixels.0, pixels.1), 2048).unwrap()
}

fn test_capture_session(displays: Vec<CapturedDisplay>) -> CaptureSession {
    CaptureSession::new(displays).unwrap()
}

#[test]
fn installing_capture_session_replaces_all_display_state_at_once() {
    let mut app = ScreenshotApp::default();
    let session = test_capture_session(vec![
        test_display(0, (0.0, 0.0, 100.0, 100.0), (100, 100)),
        test_display(1, (100.0, 0.0, 100.0, 100.0), (200, 200)),
    ]);
    app.install_capture_session(session);
    assert_eq!(app.capture_session.as_ref().unwrap().displays.len(), 2);
    assert_eq!(app.display_textures.len(), 2);
    assert!(app.display_textures.iter().all(Vec::is_empty));
}

#[test]
fn failed_session_build_does_not_replace_existing_capture() {
    let mut app = ScreenshotApp::default();
    app.install_capture_session(test_capture_session(vec![test_display(
        0, (0.0, 0.0, 100.0, 100.0), (100, 100),
    )]));
    assert!(app.try_install_captured_displays(Vec::new()).is_err());
    assert_eq!(app.capture_session.as_ref().unwrap().displays.len(), 1);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test app_default::tests::installing_capture_session -- --nocapture`

Run: `cargo test app_default::tests::failed_session_build -- --nocapture`

Expected: compilation fails because `capture_session` and install methods are absent.

- [ ] **Step 3: Replace parallel screen state with session state**

Add:

```rust
pub struct DisplayTextureTile {
    pub pixel_rect: crate::display::PixelRect,
    pub texture: egui::TextureHandle,
}
```

Replace `screens`, `screenshots`, `original_screenshots`, `screenshots_positions`, `display_textures_split`, `screen_width`, `screen_height`, `screen_scale`, and `image_scale` with:

```rust
pub capture_session: Option<crate::display::CaptureSession>,
pub display_textures: Vec<Vec<DisplayTextureTile>>,
```

Keep the old fields only until every compiler error in later tasks has a planned migration; do not maintain both models after Task 7. `install_capture_session` sizes `display_textures` to the session's display count before assigning the session. `try_install_captured_displays` constructs `CaptureSession` first and assigns only on success.

Rewrite `capture_screens` to collect all xcap metadata and images into a local vector. For each `Monitor`, read signed `x/y`, logical `width/height`, capture its image, create `DisplayGeometry`, pass it and the image to `CapturedDisplay::from_image`, and only then call `try_install_captured_displays`. Convert every xcap error to a message containing the session index and monitor origin. Do not mutate `ScreenshotApp` inside the enumeration loop.

`ensure_display_textures` creates textures named `screenshot_{session_index}_{x}_{y}` and stores them under the corresponding display index. `reset_capture_state` drops the complete session and texture matrix.

- [ ] **Step 4: Run state tests, full tests, and build**

Run: `cargo test app_default::tests -- --nocapture`

Expected: atomic-assignment tests pass.

Run: `cargo test && cargo build`

Expected: all tests and the host build pass. Temporary compatibility accessors may be used only if they are deleted by Task 7.

- [ ] **Step 5: Commit session capture migration**

```bash
git add src/app_default.rs src/app.rs
git commit -m "refactor: capture displays as one session"
```

---

### Task 4: Overlay Viewport Coordinator and Platform Guard

**Files:**
- Modify: `src/app.rs:11-154,181-217,255-275,387-463`
- Modify: `src/app_draw.rs:1-135`
- Modify: `src/main.rs:40-148`

**Interfaces:**
- Consumes: `CaptureSession.displays`, `DisplayGeometry.logical_bounds`, and the per-display texture matrix from Task 3.
- Produces: `OverlayWindowSpec`, `overlay_specs(session: &CaptureSession) -> Vec<OverlayWindowSpec>`, `overlay_viewport_id(index: usize) -> egui::ViewportId`, `ScreenshotApp::update_capture_viewports`, `ScreenshotApp::close_capture_viewports`, `ScreenshotApp::render_display_viewport`, `ScreenshotApp::draw_display`, and `linux_x11_session_supported`.

- [ ] **Step 1: Write failing viewport and Linux-session tests**

Add pure tests in `app.rs`:

```rust
fn test_session_with_bounds(bounds: &[(f32, f32, f32, f32)]) -> CaptureSession {
    let displays = bounds.iter().enumerate().map(|(index, &(x, y, width, height))| {
        let pixels = (width as u32, height as u32);
        let geometry = DisplayGeometry::new(
            index,
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
            pixels,
        ).unwrap();
        CapturedDisplay::from_image(
            geometry,
            RgbaImage::new(pixels.0, pixels.1),
            MAX_TEXTURE_SIZE as u32,
        ).unwrap()
    }).collect();
    CaptureSession::new(displays).unwrap()
}

#[test]
fn overlay_specs_preserve_negative_monitor_origins() {
    let session = test_session_with_bounds(&[
        (-1440.0, 0.0, 1440.0, 900.0),
        (0.0, -1080.0, 1920.0, 1080.0),
    ]);
    let specs = overlay_specs(&session);
    assert_eq!(specs[0].position, egui::pos2(-1440.0, 0.0));
    assert_eq!(specs[1].position, egui::pos2(0.0, -1080.0));
    assert_ne!(specs[0].viewport_id, specs[1].viewport_id);
    assert!(specs.iter().all(|spec| spec.always_on_top && !spec.decorated));
}

#[test]
fn linux_guard_rejects_wayland_and_requires_display() {
    assert!(linux_x11_session_supported(Some("x11"), Some(":0")));
    assert!(!linux_x11_session_supported(Some("wayland"), Some(":0")));
    assert!(!linux_x11_session_supported(None, None));
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run: `cargo test app::tests::overlay_specs -- --nocapture`

Run: `cargo test app::tests::linux_guard -- --nocapture`

Expected: compilation fails because overlay specs and the Linux guard do not exist.

- [ ] **Step 3: Implement coordinator-owned viewports**

Define `OverlayWindowSpec` as viewport id, position, size, decoration, and always-on-top values. `overlay_viewport_id` hashes `("capture-overlay", session_index)`.

In `update_capture_viewports`, call `ensure_display_textures`, clone the small list of display geometries/specs needed to avoid borrowing the session through UI closures, then call `ctx.show_viewport_immediate` once per display using:

```rust
egui::ViewportBuilder::default()
    .with_title("Legend Shot")
    .with_position(spec.position)
    .with_inner_size(spec.size)
    .with_decorations(false)
    .with_resizable(false)
    .with_transparent(true)
    .with_window_level(egui::WindowLevel::AlwaysOnTop)
    .with_visible(self.capture_reveal_state == CaptureRevealState::Idle)
```

The callback invokes `self.render_display_viewport(display_index, viewport_ctx)`. In this task, that method must render the specified display's texture tiles at `(tile.x / scale_x, tile.y / scale_y)` and apply a full local dim layer; it must not center or fit a monitor image. Task 5 adds selection projection, annotations, and input to this already working per-display renderer. If any child reports `close_requested`, cancel the entire session. While capture is active, the root calls `ctx.request_repaint_after(Duration::from_millis(16))`; Task 5 narrows this to active interactions where practical.

`close_capture_viewports` sends `ViewportCommand::Close` to every overlay id before resetting state. Resident platforms keep the root hidden; Windows closes the root after its overlays close. OCR results reuse the decorated root viewport and explicitly close overlays before revealing/configuring it.

Replace the root fullscreen capture builder with a hidden coordinator builder (`visible(false)`, no fullscreen). Preserve the existing macOS accessory activation policy and tray behavior.

On Linux, call `linux_x11_session_supported(XDG_SESSION_TYPE, DISPLAY)` before starting capture. The pure function accepts `Option<&str>` so tests do not mutate process environment. Reject `wayland` case-insensitively even when `DISPLAY` exists; accept only a non-empty `DISPLAY` when session type is absent or `x11`.

- [ ] **Step 4: Run tests and host build**

Run: `cargo test app::tests -- --nocapture`

Expected: viewport-spec, lifecycle, and Linux guard tests pass.

Run: `cargo build`

Expected: the native host build succeeds and no root-fullscreen assumption remains.

- [ ] **Step 5: Commit viewport orchestration**

```bash
git add src/app.rs src/app_draw.rs src/main.rs
git commit -m "feat: coordinate one overlay per display"
```

---

### Task 5: Global Pointer State and Per-Display Rendering

**Files:**
- Modify: `src/app_default.rs:70-110,210-350`
- Modify: `src/app_draw.rs:1-520`
- Modify: `src/app.rs:422-463`

**Interfaces:**
- Consumes: `DisplayGeometry::global_to_local_rect`, Task 4's per-display renderer, and `device_query::MouseState { coords, button_pressed }`.
- Produces: `PointerSnapshot`, `PrimaryButtonTransition`, `primary_button_transition`, `ScreenshotApp::poll_pointer`, `ScreenshotApp::draw_overlay_for_display`, and `ScreenshotApp::handle_input_for_display`.

- [ ] **Step 1: Write failing pointer-transition and cross-screen selection tests**

Add pure tests in `app_default.rs`:

```rust
#[test]
fn primary_button_transition_is_emitted_once() {
    assert_eq!(primary_button_transition(false, true), PrimaryButtonTransition::Pressed);
    assert_eq!(primary_button_transition(true, true), PrimaryButtonTransition::Held);
    assert_eq!(primary_button_transition(true, false), PrimaryButtonTransition::Released);
    assert_eq!(primary_button_transition(false, false), PrimaryButtonTransition::Idle);
}

#[test]
fn global_selection_can_start_left_and_end_on_right_monitor() {
    let mut app = ScreenshotApp::default();
    app.begin_global_selection(egui::pos2(-200.0, 100.0));
    app.update_global_selection(egui::pos2(300.0, 500.0));
    app.finish_global_selection();
    assert_eq!(
        app.selection_rect,
        Some(egui::Rect::from_min_max(
            egui::pos2(-200.0, 100.0),
            egui::pos2(300.0, 500.0),
        ))
    );
    assert!(!app.is_selecting);
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run: `cargo test app_default::tests::primary_button_transition -- --nocapture`

Run: `cargo test app_default::tests::global_selection -- --nocapture`

Expected: compilation fails because global pointer and selection helpers do not exist.

- [ ] **Step 3: Make global logical coordinates canonical**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSnapshot {
    pub global_position: Pos2,
    pub primary_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryButtonTransition { Idle, Pressed, Held, Released }
```

`poll_pointer` converts `device_state.get_mouse().coords` directly to signed global logical `Pos2` and reads `button_pressed.get(1).copied().unwrap_or(false)` because device_query mouse buttons are one-based. Store `last_primary_down` and update it once per root frame.

Use only `selection_start`, `selection_end`, and normalized `selection_rect` for new interaction logic. Moving a selection applies global pointer delta and clamps the unchanged rectangle size against `CaptureSession.desktop_bounds`, which may have negative minima. Keep the legacy mouse selection fields temporarily so the pre-Task-7 export code still compiles, but do not read them in viewport input/rendering; Task 7 migrates all remaining consumers and deletes them.

Treat `Annotation.points` as global points. Until Task 7 migrates image export, mirror the same signed global values into legacy `mouse_points`; do not perform a second coordinate conversion. Task 7 removes `mouse_points`. Convert a viewport-local egui click to global coordinates by adding that display's `logical_bounds.min` before creating text or annotation state.

- [ ] **Step 4: Render one display and drive active interactions globally**

`render_display_viewport` creates a frameless central panel and calls, in order:

```rust
self.draw_display(display_index, ui);
self.draw_overlay_for_display(display_index, ui);
self.draw_annotations_for_display(display_index, ui);
self.handle_input_for_display(display_index, ui, viewport_ctx, pointer_snapshot);
self.draw_text_input_for_display(display_index, ui);
```

`draw_display` places each texture tile at `(tile.x / scale_x, tile.y / scale_y)` with its logical tile size. It never centers or scales an entire monitor image to the current viewport.

`draw_overlay_for_display` fills the local viewport, intersects the global selection with the display's global bounds, converts the intersection to local coordinates, and draws the existing four surrounding dim rectangles plus border. `draw_annotations_for_display` subtracts the display origin from every global point and relies on the viewport clip rectangle.

A local egui primary press may begin an action only in the viewport under the pointer. Once active, held/released updates come from `PointerSnapshot`, so crossing a native viewport does not interrupt the action. A `Released` transition finalizes selection or annotation exactly once and triggers OCR recapture or toolbar display as today.

- [ ] **Step 5: Run focused tests, all tests, and build**

Run: `cargo test app_default::tests::primary_button_transition -- --nocapture`

Run: `cargo test app_default::tests::global_selection -- --nocapture`

Expected: pointer transitions and cross-screen selection pass.

Run: `cargo test && cargo build`

Expected: all tests pass and viewport input/rendering no longer reads the legacy dual-coordinate fields.

- [ ] **Step 6: Commit global input/rendering**

```bash
git add src/app_default.rs src/app_draw.rs src/app.rs
git commit -m "feat: track selections across overlay windows"
```

---

### Task 6: Toolbar Ownership and Cross-Screen Annotation Preview

**Files:**
- Modify: `src/display.rs`
- Modify: `src/app_default.rs:243-288`
- Modify: `src/app_draw.rs:416-520`
- Modify: `src/app_toolbar.rs:1-120,500-575`

**Interfaces:**
- Consumes: global selection/annotation state and `CaptureSession::display_at`.
- Produces: `ToolbarPlacement`, `place_toolbar`, `ScreenshotApp::update_toolbar_placement`, and display-index-aware toolbar/text rendering.

- [ ] **Step 1: Write failing toolbar placement tests**

In `display.rs`, add:

```rust
#[test]
fn toolbar_uses_release_monitor_and_flips_above_at_bottom_edge() {
    let displays = vec![
        geometry(0, Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0)), (1000, 800)),
        geometry(1, Rect::from_min_size(Pos2::new(1000.0, 0.0), Vec2::new(1000.0, 800.0)), (1000, 800)),
    ];
    let placement = place_toolbar(
        &displays,
        Rect::from_min_max(Pos2::new(900.0, 600.0), Pos2::new(1800.0, 790.0)),
        Pos2::new(1700.0, 790.0),
        Vec2::new(300.0, 50.0),
        8.0,
    ).unwrap();
    assert_eq!(placement.display_index, 1);
    assert!(placement.global_position.y < 600.0);
    assert!(placement.global_position.x >= 1000.0);
    assert!(placement.global_position.x + 300.0 <= 2000.0);
}

#[test]
fn toolbar_chooses_nearest_display_when_endpoint_is_in_gap() {
    let displays = vec![
        geometry(0, Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)), (100, 100)),
        geometry(1, Rect::from_min_size(Pos2::new(200.0, 0.0), Vec2::new(100.0, 100.0)), (100, 100)),
    ];
    assert_eq!(
        place_toolbar(&displays, Rect::from_min_max(Pos2::ZERO, Pos2::new(250.0, 80.0)), Pos2::new(180.0, 50.0), Vec2::new(50.0, 20.0), 4.0).unwrap().display_index,
        1
    );
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test display::tests::toolbar -- --nocapture`

Expected: compilation fails because toolbar placement does not exist.

- [ ] **Step 3: Implement deterministic toolbar placement**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarPlacement {
    pub display_index: usize,
    pub global_position: Pos2,
}
```

`place_toolbar` chooses the containing/nearest endpoint display, centers the toolbar on the selection horizontally, clamps X to monitor bounds, tries `selection.max.y + margin`, and flips to `selection.min.y - toolbar_height - margin` when the lower position does not fit. Clamp the final Y to the chosen display. The current implementation uses display logical bounds as the documented work-area fallback because the selected windowing API does not expose a separate work area here.

Store `toolbar_placement: Option<ToolbarPlacement>` instead of a window-local `toolbar_position`. Render `draw_toolbar` only when its display index matches the current viewport and pass `global_position - display.logical_bounds.min` to egui `Area::fixed_pos`.

Text input uses the same ownership rule based on its global anchor. Annotation previews render on every intersected display, clipped locally; toolbar button signals remain shared and are consumed once by the coordinator.

- [ ] **Step 4: Run focused and regression tests**

Run: `cargo test display::tests::toolbar -- --nocapture`

Expected: toolbar chooses display 1 in both tests and flips above the bottom-edge selection.

Run: `cargo test && cargo build`

Expected: all tests and build pass.

- [ ] **Step 5: Commit toolbar and preview behavior**

```bash
git add src/display.rs src/app_default.rs src/app_draw.rs src/app_toolbar.rs
git commit -m "feat: place capture toolbar on release display"
```

---

### Task 7: Route Save, Clipboard, Test Mode, and OCR Through One Composer

**Files:**
- Modify: `src/app_handle.rs:1-380`
- Modify: `src/app_ocr.rs:1-360`
- Modify: `src/main.rs:171-263`
- Modify: `src/ui.rs:181-189`
- Modify: `src/app_default.rs:210-350`

**Interfaces:**
- Consumes: `compose_selection`, `ComposedSelection`, `OutputTransform`, global `Annotation.points`, and `ScreenshotApp.capture_session`.
- Produces: `ScreenshotApp::compose_current_selection(&[Annotation]) -> Result<RgbaImage, String>` and `ScreenshotApp::draw_annotations_on_composed(image, transform, annotations)`.

- [ ] **Step 1: Write failing consumer and annotation-transform tests**

In `app_handle.rs` tests, create an app with a two-display test session and a cross-screen selection:

```rust
fn solid_display(
    index: usize,
    origin_x: f32,
    logical_width: f32,
    pixels: (u32, u32),
    color: [u8; 4],
) -> CapturedDisplay {
    let geometry = DisplayGeometry::new(
        index,
        Rect::from_min_size(Pos2::new(origin_x, 0.0), Vec2::new(logical_width, 50.0)),
        pixels,
    ).unwrap();
    CapturedDisplay::from_image(
        geometry,
        RgbaImage::from_pixel(pixels.0, pixels.1, Rgba(color)),
        2048,
    ).unwrap()
}

fn app_with_two_solid_displays() -> ScreenshotApp {
    let mut app = ScreenshotApp::default();
    app.install_capture_session(CaptureSession::new(vec![
        solid_display(0, 0.0, 100.0, (100, 50), [255, 0, 0, 255]),
        solid_display(1, 100.0, 100.0, (100, 50), [0, 0, 255, 255]),
    ]).unwrap());
    app
}

fn app_with_retina_second_display() -> ScreenshotApp {
    let mut app = ScreenshotApp::default();
    app.install_capture_session(CaptureSession::new(vec![
        solid_display(0, 0.0, 100.0, (100, 50), [0, 0, 0, 255]),
        solid_display(1, 100.0, 100.0, (200, 100), [0, 0, 0, 255]),
    ]).unwrap());
    app
}

fn rectangle_annotation(start: Pos2, end: Pos2) -> Annotation {
    Annotation {
        tool: Tool::Rectangle,
        points: vec![start, end],
        color: Color32::RED,
        stroke_width: 1.0,
        text: String::new(),
        number: None,
    }
}

#[test]
fn current_selection_composes_both_displays_for_every_consumer() {
    let mut app = app_with_two_solid_displays();
    app.selection_rect = Some(Rect::from_min_size(Pos2::new(50.0, 0.0), Vec2::new(100.0, 50.0)));
    let image = app.compose_current_selection(&[]).unwrap();
    assert_eq!(image.dimensions(), (100, 50));
    assert_eq!(image.get_pixel(10, 10).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(90, 10).0, [0, 0, 255, 255]);
}

#[test]
fn annotation_global_point_uses_composed_output_transform() {
    let mut app = app_with_retina_second_display();
    app.selection_rect = Some(Rect::from_min_size(Pos2::new(50.0, 0.0), Vec2::new(100.0, 50.0)));
    let annotation = rectangle_annotation(Pos2::new(75.0, 10.0), Pos2::new(125.0, 40.0));
    let image = app.compose_current_selection(&[annotation]).unwrap();
    assert_eq!(image.dimensions(), (200, 100));
    assert_eq!(image.get_pixel(50, 20).0[..3], [255, 0, 0]);
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run: `cargo test app_handle::tests::current_selection -- --nocapture`

Run: `cargo test app_handle::tests::annotation_global_point -- --nocapture`

Expected: compilation fails because consumers still use single-monitor crop paths.

- [ ] **Step 3: Implement the shared consumer path**

Implement:

```rust
pub fn compose_current_selection(
    &self,
    annotations: &[Annotation],
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
    let selection = self.selection_rect.ok_or_else(|| "请选择截图区域".to_string())?;
    let session = self.capture_session.as_ref().ok_or_else(|| "截图会话不存在".to_string())?;
    let mut composed = crate::display::compose_selection(&session.displays, selection)?;
    self.draw_annotations_on_composed(&mut composed.image, composed.transform, annotations);
    Ok(composed.image)
}
```

Change annotation image drawing so every global point is transformed through `OutputTransform::global_to_output`; scale stroke width by `transform.scale.x.min(transform.scale.y).max(1.0)`. Mosaic samples from a clone of the fully composed unannotated background.

Make `save_screenshot`, `trigger_save_dialog`, `copy_to_clipboard`, OCR submission, and `crop_selection_for_test` call `compose_current_selection`. Delete `crop_region_for_global_selection`, `crop_region_for_global_selection_in_pixels`, `crop_rgba_region`, non-macOS local-pixel loops, and `ui::get_screen_rect` once no callers remain.

Update `CaptureSnapshot` and OCR recapture to save/restore only canonical `selection_rect` and global annotations. In CLI test mode, interpret `--test x,y,width,height` as global logical coordinates; set `selection_rect` only, allowing negative origins and cross-monitor regions.

- [ ] **Step 4: Run focused, OCR, and full tests**

Run: `cargo test app_handle::tests -- --nocapture`

Expected: both cross-display consumer tests pass.

Run: `cargo test app_ocr::tests -- --nocapture`

Expected: OCR state and recapture tests pass using the shared compositor.

Run: `cargo test && cargo build`

Expected: all tests/build pass and `rg "mouse_selection_rect|mouse_points|get_screen_rect|original_screenshots|screenshots_positions" src` returns no production-code matches.

- [ ] **Step 5: Commit consumer unification**

```bash
git add src/app_handle.rs src/app_ocr.rs src/main.rs src/ui.rs src/app_default.rs
git commit -m "feat: export cross-monitor selections"
```

---

### Task 8: Error UX, Documentation, and Release Verification

**Files:**
- Modify: `src/app.rs`
- Modify: `src/main.rs`
- Modify: `README.md`
- Test: all inline `#[cfg(test)]` modules

**Interfaces:**
- Consumes: all prior task interfaces.
- Produces: final platform error reporting, synchronized overlay teardown, documented support matrix, and verification evidence.

- [ ] **Step 1: Write failing lifecycle/error tests**

Add tests that keep session cleanup independent from native windows:

```rust
#[test]
fn capture_failure_returns_resident_app_to_idle_without_partial_session() {
    let mut app = ScreenshotApp::default();
    app.lifecycle = AppLifecycle::Capturing;
    app.capture_session = None;
    app.finish_failed_capture(true);
    assert_eq!(app.lifecycle, AppLifecycle::TrayIdle);
    assert!(app.capture_session.is_none());
    assert!(app.display_textures.is_empty());
}

#[test]
fn closing_capture_clears_all_overlay_ids() {
    let mut app = ScreenshotApp::default();
    app.install_capture_session(test_session_with_bounds(&[
        (0.0, 0.0, 100.0, 100.0),
        (100.0, 0.0, 100.0, 100.0),
    ]));
    assert_eq!(app.active_overlay_ids().len(), 2);
    app.clear_capture_session();
    assert!(app.active_overlay_ids().is_empty());
}
```

- [ ] **Step 2: Run lifecycle tests and verify failure**

Run: `cargo test app::tests::capture_failure -- --nocapture`

Run: `cargo test app::tests::closing_capture -- --nocapture`

Expected: compilation fails because cleanup helpers are not defined.

- [ ] **Step 3: Implement complete error and cleanup behavior**

Centralize cleanup in `clear_capture_session`: close/hide all overlay viewport IDs first, clear textures and session, reset interaction/toolbar/text/OCR recapture state, then apply the existing resident-vs-close completion policy. `finish_failed_capture(resident_platform: bool)` uses the explicit argument in tests and is called with `cfg!(any(target_os = "macos", target_os = "linux"))` in production.

Capture-start failures call `rfd::MessageDialog::new().set_title("Legend Shot").set_description(error).set_level(rfd::MessageLevel::Error).show()` because overlays are not visible. Errors after reveal are stored in an app error field, rendered in the owning viewport for one frame, then close the session after acknowledgement. Always mirror the detailed error to stderr.

Use these user-facing messages:

```rust
const WAYLAND_UNSUPPORTED: &str = "当前版本仅支持 Linux X11，暂不支持 Wayland 截图。";
const NO_MONITORS: &str = "未检测到可截图的显示器。";
const POINTER_UNAVAILABLE: &str = "无法读取系统鼠标位置；请检查辅助功能或输入权限。";
```

Keep the existing macOS screen-recording preflight/request and report pointer failure separately as accessibility permission. Treat any overlay close request or invalidated display viewport as cancellation of the entire active capture, never as removal of one display from the session.

- [ ] **Step 4: Document support and manual checks**

Add a README section stating:

```markdown
## Multi-monitor support

Legend Shot supports selections spanning multiple displays on Windows, macOS, and Linux X11, including displays with negative coordinates and different scale factors. Linux Wayland is not supported yet; run under an X11 session. Desktop gaps inside a cross-monitor selection are exported as transparent pixels.
```

Add the manual matrix from the design spec as release-check bullets: single screen, left/right, above/below, negative origin, mixed DPI, cross-gap selection, every annotation tool, save/copy/OCR/Escape, primary-display change, and reconnect between sessions.

- [ ] **Step 5: Run formatting and automated verification**

Run: `cargo fmt --all -- --check`

Expected: exit 0. If it fails, run `cargo fmt --all`, inspect the formatting diff, then rerun the check.

Run: `cargo test`

Expected: exit 0 with all old and new tests passing.

Run: `cargo build`

Expected: exit 0 on the development host.

Run: `cargo clippy --all-targets -- -D warnings`

Expected: exit 0 with no warnings. Fix warnings in task-owned code; do not suppress them broadly.

Run: `git diff --check`

Expected: exit 0.

- [ ] **Step 6: Perform native smoke tests before claiming universal readiness**

On macOS, Windows, and Linux X11, run the manual matrix recorded in the README/design. Save at least one cross-monitor PNG per OS and verify dimensions, transparent gaps, screenshot alignment, annotation alignment, clipboard content, and OCR input visually. If only the current host is available, report the other OS checks as unverified rather than claiming cross-platform completion.

- [ ] **Step 7: Commit final error UX and documentation**

```bash
git add src/app.rs src/main.rs README.md
git commit -m "docs: describe multi-monitor platform support"
```

---

## Final Review Checklist

- Confirm the spec's display model, per-monitor overlays, global input, toolbar ownership, composition, platform scope, lifecycle, and test matrix each map to a completed task above.
- Run `rg "mouse_selection_rect|mouse_points|get_screen_rect|screen_width|screen_height|screen_scale|image_scale|screenshots_positions|display_textures_split" src`; no removed production model may remain.
- Inspect `git status --short` and every staged diff before each commit so the user's pre-existing changes and tray icons are never staged accidentally.
- Do not claim Windows/macOS/Linux X11 support until the corresponding native smoke test has actually run; distinguish automated geometry coverage from native-window verification.
