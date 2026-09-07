# Cross-Platform Multi-Monitor Capture Design

## Summary

`legend_shot` currently enumerates and captures every monitor, but it still renders and crops as if there were only one screen. In particular, screenshot tiles from different monitors reuse the same local origin, the capture window covers only one monitor, and the non-macOS crop path compares global selections with monitor-local image pixels. This produces overlapping screenshots and incorrect selections when an external display is connected.

This change will support Windows, macOS, and Linux X11 with one capture model: a hidden coordinator owns one borderless overlay viewport per monitor, while all viewports share a global selection and annotation state. A selection may begin on any monitor and span any number of monitors. Linux Wayland is explicitly out of scope for this iteration.

## Goals

- Show the correct frozen screenshot and dimming overlay on every connected monitor at the same time.
- Allow a selection to start on one monitor and end on another.
- Support monitors placed left, right, above, or below the primary monitor, including negative desktop coordinates.
- Preserve consistent apparent sizes when monitors use different DPI scale factors.
- Allow moving a selection and creating annotations across monitor boundaries.
- Produce one RGBA result for save, clipboard, and OCR, with transparent pixels where the selected desktop rectangle is not covered by a monitor.
- Refresh the monitor topology at the beginning of every capture so changes made while the application is resident take effect on the next capture.
- Preserve all existing single-monitor behavior.

## Non-Goals

- Linux Wayland screen capture and input tracking.
- Automatic window or UI-element detection.
- Stitching displays into a non-rectangular output image.
- Reacting in place to monitor hot-plug events during an already active capture. The active session uses a fixed topology; a topology change cancels or invalidates that session, and the next capture enumerates displays again.

## Architecture

### Capture coordinator and overlay viewports

The eframe root viewport becomes a hidden capture coordinator. For an active capture, it creates one borderless, transparent, always-on-top viewport for each monitor. Each overlay is positioned at that monitor's global logical origin and sized to its logical dimensions. The same arrangement is used on Windows, macOS, and Linux X11 instead of maintaining a giant-window path and a multi-window path.

The coordinator owns all mutable capture state. Overlay callbacks receive a monitor index and render a monitor-local view of that shared state. This gives all windows a single lifecycle: starting a capture creates the complete set; cancel, save, copy, OCR completion, or application exit closes or hides the complete set.

The existing resident behavior remains platform-specific: macOS and Linux return to the tray after a capture, while Windows may exit according to the existing completion policy. Window creation and rendering behavior is otherwise shared.

### Display snapshot model

Introduce a platform-neutral display model in `src/display.rs`:

```rust
struct DisplaySnapshot {
    session_index: usize,
    logical_bounds: GlobalRect,
    pixel_size: (u32, u32),
    pixel_scale: (f32, f32),
    original_image: RgbaImage,
    tiles: Vec<DisplayTile>,
}

struct CaptureSession {
    displays: Vec<DisplaySnapshot>,
    desktop_bounds: GlobalRect,
}
```

The session index only identifies a monitor for the lifetime of one capture; it does not claim to be stable across hot-plug events. `pixel_scale` is derived independently on each axis from captured pixel dimensions divided by logical monitor dimensions. This is more reliable than assuming one global scale factor and also makes rounding behavior explicit.

The session constructor validates that at least one display exists, all logical and pixel sizes are positive, all coordinate calculations fit their integer types, and every display image was captured. Session creation is atomic: a failed display capture does not open a partial set of overlays.

### Coordinate spaces

Three coordinate spaces are named and kept separate:

1. **Global logical coordinates** describe the virtual desktop and are the canonical coordinates for selections, pointer positions, toolbar ownership, and annotations.
2. **Viewport-local logical coordinates** start at `(0, 0)` in one overlay. Rendering converts by subtracting that display's global logical origin.
3. **Display-native pixel coordinates** address the captured image. Cropping converts a logical offset using the display's independent X/Y pixel scales and clamps the rounded result to image bounds.

Selections and annotation points are stored only in global logical coordinates. Raw system pointer coordinates are normalized into that space at the input boundary. Image-space coordinates do not leak into interaction state.

## Capture and Rendering Flow

1. A capture request hides existing application UI before pixels are acquired.
2. The coordinator enumerates monitors and captures each original image.
3. It builds `CaptureSession`, calculates virtual desktop bounds, and splits each monitor image into texture tiles no larger than `MAX_TEXTURE_SIZE`.
4. It creates one overlay viewport per display and reveals the set only after the first hidden render frame, preventing the overlays from appearing in screenshots.
5. Each viewport draws only its own display's tiles at monitor-local positions. Tile names include the session index and tile offset so textures cannot collide between monitors.
6. Each viewport draws a full local dimming layer, then removes dimming from the intersection of the global selection and that display.
7. Global annotations are clipped and transformed for the viewport currently being rendered.

This replaces `screenshots_positions` and the single `screen_width`, `screen_height`, `screen_scale`, and `image_scale` assumptions. Large-texture splitting remains, but it is performed and positioned within an individual display snapshot.

## Cross-Monitor Input

An input adapter exposes a `PointerSnapshot` containing the current global logical position and primary-button state. A local egui press begins the interaction and records which action is active. While selecting, moving, resizing, or drawing, the coordinator polls the system pointer source and updates the action from global coordinates. This prevents a drag from ending merely because the pointer left its originating native window.

The active interaction is finalized exactly once when the system primary button transitions from down to up. All overlay viewports request repaint while an interaction is active so the selection remains visually synchronized across screens.

Rules for existing interactions:

- A selection may include portions of the virtual desktop not occupied by a monitor.
- Moving a selection clamps its rectangular bounds to the virtual desktop bounding rectangle, not to one monitor.
- Annotation tools use global points and may cross monitor boundaries.
- Text editing remains owned by the viewport containing the text anchor; the resulting annotation is stored globally.
- Double-click copy continues to operate on the complete current selection.
- Escape first cancels the active text/editing state or selection as it does today, then closes all overlays together.

## Toolbar Placement

When selection ends, the global pointer position determines the owning monitor. If the pointer is in a desktop gap, use the intersected monitor nearest to the pointer, preferring the monitor containing the selection endpoint when available.

The toolbar is positioned below the selection within the owning monitor's available work area. If there is insufficient space below, it is placed above. Horizontal position is clamped to the work area. Where the windowing API does not expose a distinct work area, the monitor's logical bounds are used as the fallback. Only the owning viewport renders and accepts input for the toolbar.

## Multi-Monitor Composition

Save, clipboard, and OCR share one composition function; none of them implements monitor lookup independently.

1. Normalize the global selection rectangle and reject empty dimensions.
2. Find every display whose logical bounds intersect the selection.
3. Choose the maximum X and Y pixel scales among the intersected displays as the output scale. This preserves high-DPI detail and keeps content from different monitors the same apparent size.
4. Allocate a transparent RGBA output image sized from the logical selection dimensions times the output scale.
5. For each display intersection, convert the intersection to native pixel bounds, crop it, resize it to the intersection's output dimensions when its scale differs, and alpha-copy it at the corresponding global selection offset.
6. Render annotations into the composed output using the same global-to-output transform. Mosaic uses a clone of the composed background, preserving existing ordering semantics.

Rounding uses an edge-based rule: floor minimum edges, ceil maximum edges, and clamp both to their source or destination bounds. This avoids one-pixel seams between adjacent displays. The output allocation uses checked arithmetic and rejects images over `100_000_000` pixels (approximately 400 MB for the base RGBA buffer) with a user-facing error rather than risking an out-of-memory abort.

Pixels inside the rectangular selection but outside every monitor remain transparent. PNG save and clipboard preserve alpha when supported. OCR receives the composed RGBA image; transparent gaps are converted to the OCR backend's normal RGB background conversion behavior.

## Platform Scope

### Windows

Use the shared per-monitor overlay model and the existing xcap/device-query stack. Negative coordinates for monitors left or above the primary display are valid and must remain signed throughout geometry calculations.

### macOS

Use borderless accessory-application overlays rather than native fullscreen spaces. Existing screen-recording and accessibility permission checks remain prerequisites. Per-display viewports avoid macOS restrictions and inconsistent scaling associated with one window spanning multiple displays.

### Linux X11

Use per-monitor override-style borderless overlays through the existing eframe X11 backend. Global pointer tracking and screenshot capture continue through the current libraries. At startup or capture time, a non-X11 session is rejected with a clear message that Wayland is not yet supported.

### Linux Wayland

Wayland is not supported by this design. Correct support would require a separate portal/PipeWire capture flow and compositor-dependent interaction design; it must not silently fall back to incorrect X11 assumptions.

## Errors and Lifecycle

- Monitor enumeration failure, zero monitors, invalid geometry, or a failed display capture aborts the capture before overlays are revealed.
- Viewport creation failure closes any viewports already created for that session and reports the failing display.
- Missing macOS permissions produce a targeted permission message.
- A Linux non-X11 environment produces an explicit unsupported-session message.
- If monitor topology changes during an active capture and the overlay set becomes invalid, cancel the current session and ask the user to start a new capture.
- User-facing capture-start errors use a cross-platform native message dialog because no capture viewport is guaranteed to be visible. Errors that occur after overlays are visible use the in-application error UI before the overlay set closes. Diagnostic details are also written to stderr.

## Code Changes

- Add `src/display.rs` for display/session data, coordinate transforms, intersections, composition layout, and pure unit tests.
- Update `src/app_default.rs` to own `CaptureSession` and global interaction state; remove single-screen dimension and scale fields after callers migrate.
- Update `src/app.rs` to coordinate per-monitor viewport creation, synchronized repainting, and all-overlay teardown.
- Update `src/app_draw.rs` to render one display per viewport and translate global interaction state to viewport-local drawing coordinates.
- Update `src/app_toolbar.rs` so one selected viewport owns the toolbar and placement is clamped to that monitor.
- Update `src/app_handle.rs` so save, clipboard, test mode, and OCR use the shared multi-display composer.
- Update `src/main.rs` so the root viewport acts as the hidden coordinator rather than a fullscreen single-monitor capture surface.
- Update `src/ui.rs` only where old monitor rectangle helpers are superseded by the typed display geometry API.

Unrelated existing working-tree modifications must be preserved. Implementation commits will stage only files changed for this feature.

## Testing

### Automated tests

Pure geometry and composition tests will cover:

- a single monitor at `(0, 0)` for backward compatibility;
- a monitor with a negative X or Y origin;
- left/right and above/below monitor arrangements;
- a selection crossing two and three monitors;
- a rectangular selection containing a monitor gap, verifying transparent pixels;
- adjacent monitors with different scale factors, verifying output dimensions and absence of seams;
- Retina-style logical-to-native cropping;
- toolbar monitor ownership and above/below edge clamping;
- movement within virtual desktop bounds;
- overflow, empty selection, and the output pixel limit;
- a partial display capture failure producing no usable session;
- session-wide close/cancel state transitions.

Run `cargo test` and `cargo build` on the development host. Cross-target checks should be run where the required Rust targets and native dependencies are available.

### Manual platform matrix

Validate on macOS, Windows, and Linux X11 with:

- one monitor;
- two monitors arranged left/right;
- two monitors arranged above/below;
- a non-primary display with negative coordinates;
- mixed DPI/scale factors;
- a selection contained in each individual monitor;
- a selection spanning monitor boundaries and a desktop gap;
- move, pen, rectangle, arrow, text, number, and mosaic across displays;
- copy, PNG save, OCR, double-click copy, and Escape;
- changing the primary monitor and reconnecting a monitor between capture sessions.

Because native multi-window behavior cannot be fully proven by unit tests or cross-compilation, completion requires at least a smoke test on each supported operating-system family before claiming universal release readiness.

## Acceptance Criteria

- The overlapping and duplicated monitor image shown in the reported macOS reproduction no longer occurs.
- Every connected monitor shows its own frozen screenshot under a synchronized dimming overlay.
- A drag can begin on one monitor, cross native window boundaries, and finish on another without interruption.
- The saved/copied image contains correctly positioned content from every intersected monitor at a consistent apparent scale.
- Desktop gaps are transparent, and annotations align with their preview after composition.
- A single-monitor capture remains visually and functionally equivalent to the existing behavior.
- Windows, macOS, and Linux X11 complete the manual smoke-test matrix; Wayland reports that it is unsupported.
