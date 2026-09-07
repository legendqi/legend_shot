<h1 align="center">Legend Shot</h1>

<p align="center">
  <strong>A lightweight, cross-platform screenshot and annotation tool written in Rust.</strong><br />
  Capture a region, annotate it, extract text with local OCR, then copy or save the result.
</p>

<p align="center">
  <a href="https://gitee.com/legendqi/legend_shot"><img alt="Version" src="https://img.shields.io/badge/version-0.1.0-orange.svg?style=flat-square" /></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-4c8bf5.svg?style=flat-square" />
  <img alt="Rust edition" src="https://img.shields.io/badge/Rust-edition%202024-dea584.svg?style=flat-square&logo=rust&logoColor=white" />
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Mulan%20PSL%20v2-blue.svg?style=flat-square" /></a>
</p>

<p align="center">
  <a href="#quickstart">Quickstart</a> ·
  <a href="#features">Features</a> ·
  <a href="#usage">Usage</a> ·
  <a href="#building">Building</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

## What is Legend Shot?

Legend Shot is a desktop screenshot utility built with Rust, `egui`, and `xcap`. On macOS and Linux it stays available in the top bar until you choose Capture; on Windows it opens the capture overlay immediately. The overlay lets you select a region and provides a compact toolbar for annotation, OCR, copying, and saving.

The project focuses on a fast capture workflow, native-resolution output, cross-platform behavior, and local processing. OCR inference runs locally after the required model files are available.

## Project status

Legend Shot is under active development. The current package version is `0.1.0`; public prebuilt installers have not been published yet. Build from source for development and evaluation. APIs, packaging, and platform integration may change before the first stable release.

<a id="features"></a>

## Features

| Capability | Description |
| --- | --- |
| Region capture | Select any rectangular region from the current desktop with a dimmed overlay. |
| Native-resolution output | Preserve source screenshot resolution, including Retina-scaled captures on macOS. |
| Annotation toolbar | Move the selection, draw freehand strokes, rectangles, arrows, text, mosaics, and numbered markers. |
| Multilingual text | Enter text through the operating system IME and render CJK text with an available system font. |
| Local OCR | Extract text using PP-OCRv5 mobile detection and recognition models through `oar-ocr`. |
| Copy and save | Copy the annotated image to the clipboard or save it as a PNG with a native file dialog. |
| Cross-platform UI | Run the same capture workflow on macOS, Windows, and X11-based Linux desktops. |
| Persistent preferences | Remember the last save directory and OCR result-window geometry. |

## Preview

A clean product screenshot will be added before the first packaged release.

<a id="quickstart"></a>

## Quickstart

### Prerequisites

- A current stable Rust toolchain with Cargo.
- Git and a native C/C++ build toolchain for your operating system.
- Network access on the first OCR run if the PP-OCRv5 model assets are not already cached.

Linux additionally requires an X11 desktop session, GTK 3, `libxdo`, and an AppIndicator implementation. `xclip` provides image clipboard support, and a CJK font such as Noto Sans CJK is recommended:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config xclip fonts-noto-cjk libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
```

If `libayatana-appindicator3-dev` is unavailable, `libappindicator3-dev` is an acceptable alternative. The desktop session must expose AppIndicator/StatusNotifier support; GNOME installations may require the AppIndicator extension.

### Run from source

```bash
git clone https://gitee.com/legendqi/legend_shot.git
cd legend_shot
cargo run
```

On macOS, the first Capture request asks for screen-recording permission for Legend Shot—or for the terminal application when running with `cargo run`. Grant it under **System Settings → Privacy & Security → Screen & System Audio Recording**, then choose Capture again; the process remains resident while permission is pending.

<a id="usage"></a>

## Usage

1. Start Legend Shot. On macOS/Linux, open its top-bar menu and choose **Capture**; the only other menu item is **Exit**. Windows enters capture immediately.
2. Drag across the screen to select a capture region.
3. Use the toolbar to annotate, run OCR, copy, or save the selection.
4. Finish a text annotation with `Ctrl+Enter` on Windows/Linux or `Command+Enter` on macOS.
5. On macOS/Linux, copy, save, or `Esc` returns to the tray; choose **Exit** from the tray menu to terminate the process. Windows exits after completing or cancelling capture.

| Input | Action |
| --- | --- |
| Drag on the overlay | Select a screenshot region. |
| Double-click inside the selection | Copy the selection, then return to the tray on macOS/Linux or exit on Windows. |
| `Esc` | Cancel the current operation; closing the outer capture returns to the tray on macOS/Linux or exits on Windows. |
| Move | Reposition the selected region. |
| Pen / Rectangle / Arrow | Draw visual annotations. |
| Text | Add multilingual text using the system input method. |
| Mosaic | Pixelate sensitive content. |
| Number | Add sequential numbered markers. |
| OCR | Recognize text in the selected region and open the result view. |
| Copy / Save | Export the selected region with annotations. |

## Command-line test mode

Legend Shot includes a non-interactive capture mode for development and smoke testing:

The `--test` lifecycle is unchanged and does not create a tray icon.

```bash
# Capture a region and copy it to the clipboard
cargo run -- --test "100,100,400,300" --action copy

# Capture a region and save it to a file
cargo run -- --test "100,100,400,300" --action save --output screenshot.png
```

The region format is `x,y,width,height` in screen coordinates. Supported actions are `copy` and `save`.

<a id="building"></a>

## Building

### Native release build

Build on the target operating system for the most reliable result:

```bash
cargo build --release
```

The executable is written to `target/release/legend_shot` (`legend_shot.exe` on Windows).

### Build helper

`build.sh` provides target-specific Cargo commands:

| Command | Target |
| --- | --- |
| `./build.sh setup` | Install the configured Rust compilation targets. |
| `./build.sh linux-amd64` | Linux x86_64. |
| `./build.sh linux-arm64` | Linux ARM64. |
| `./build.sh windows` | Windows x86_64. |
| `./build.sh macos-arm64` | macOS Apple Silicon. |
| `./build.sh macos-intel` | macOS Intel. |
| `./build.sh all` | Run every configured target build. |

Adding a Rust target alone is not always sufficient for desktop cross-compilation. Platform SDKs, native libraries, linkers, and OCR runtime dependencies must also be available. Native builds or CI runners for each operating system are recommended for release artifacts.

For `linux-arm64` cross-builds, install `gcc-aarch64-linux-gnu` and provide ARM64 GTK/AppIndicator development packages under `/usr/lib/aarch64-linux-gnu/pkgconfig`. The build preflight checks that target-specific pkg-config directory instead of accepting host-architecture libraries.

## Platform notes

| Platform | Notes |
| --- | --- |
| macOS | Starts in the menu bar. Screen & System Audio Recording permission is requested by the first capture action; development builds launched from a terminal use the terminal's permission identity. |
| Windows | Starts capture immediately and exits after completion. It uses the native clipboard implementation through `arboard`; Microsoft YaHei or SimHei is used when available for CJK rendering. |
| Linux | Starts in the top bar and currently targets X11. The session must support AppIndicator/StatusNotifier; `xclip` is required for copying PNG images. |

## OCR and privacy

Legend Shot uses the PP-OCRv5 mobile detection and recognition models provided through `oar-ocr`. Model files may be downloaded automatically on first use. Once loaded, OCR inference is performed locally; captured image data is not intentionally uploaded by Legend Shot.

## Configuration

The application stores `config.json` in the platform-standard configuration directory resolved by the Rust `directories` crate. It currently persists:

- The last directory used to save a screenshot.
- OCR result-window position and size.

<a id="architecture"></a>

## Architecture

```text
src/
├── main.rs           # CLI parsing, startup, fonts, and platform window setup
├── app_default.rs    # Application state, tools, annotations, and configuration
├── app.rs            # Main egui update loop and view/window transitions
├── app_draw.rs       # Screen rendering, selection, and pointer/keyboard input
├── app_toolbar.rs    # Annotation toolbar and on-canvas text editor
├── app_handle.rs     # Cropping, annotation export, clipboard, and file saving
├── app_ocr.rs        # OCR capture flow and result-state transitions
├── app_ocr_view.rs   # OCR result interface
├── ocr.rs            # OCR session, worker thread, timeout, and error model
├── ocr_oar.rs        # oar-ocr backend and PP-OCRv5 result normalization
├── tray.rs           # Native menu creation and macOS/Linux tray runtimes
└── ui.rs             # Icons, textures, fonts, and image drawing utilities
```

The central `ScreenshotApp` state is extended through separate `impl` blocks. Screen capture and UI drawing use distinct coordinate systems so that logical window coordinates can be mapped back to native screenshot pixels.

## Development and verification

```bash
cargo build
cargo test --all-targets
```

Some tests construct the live screenshot application and therefore require the same screen-recording and accessibility permissions as a development run.

## Roadmap

- Publish signed installers/packages for Windows, Linux, and macOS.
- Add clean product screenshots and release documentation.
- Expand automated platform testing and packaging in CI.

## Contributing

Issues and pull requests are welcome at [gitee.com/legendqi/legend_shot](https://gitee.com/legendqi/legend_shot). For bug reports, include your operating system, desktop/display configuration, reproduction steps, and relevant terminal output.

## License

Legend Shot is licensed under the [Mulan Permissive Software License, Version 2](LICENSE).

## Acknowledgements

Legend Shot is built with projects including [`egui`](https://github.com/emilk/egui), [`xcap`](https://github.com/nashaofu/xcap), [`arboard`](https://github.com/1Password/arboard), and [`oar-ocr`](https://github.com/GreatV/oar-ocr).
