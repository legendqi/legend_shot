# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Legend_Shot is a cross-platform screenshot tool built with Rust and egui/eframe. It captures screens, allows annotations (pen, rectangle, arrow, text, mosaic, numbered markers), and supports saving to file or copying to clipboard.

## Build Commands

```bash
# Development build
cargo build

# Release build (optimized for size)
cargo build --release

# Run the application
cargo run

# Cross-platform builds via build.sh
./build.sh setup              # Install compilation targets
./build.sh linux-amd64        # Build for Linux x86_64
./build.sh linux-arm64        # Build for Linux ARM64
./build.sh windows            # Build for Windows
./build.sh macos-arm64        # Build for macOS ARM (M1/M2)
./build.sh macos-intel        # Build for macOS Intel
./build.sh all                # Build all platforms
```

## Architecture

The codebase follows a modular structure with `ScreenshotApp` as the central struct:

```
src/
├── main.rs           # Entry point, eframe setup
├── app_default.rs    # ScreenshotApp struct, state, screen capture
├── app.rs            # App trait impl, main update loop
├── app_draw.rs       # Drawing screens, overlay, input handling
├── app_toolbar.rs    # Toolbar UI, annotation rendering
├── app_handle.rs     # Save/copy operations, image cropping
└── ui.rs             # Utilities, icon loading, image compression
```

### Key Patterns

- **State Management**: All state lives in `ScreenshotApp` struct in `app_default.rs`
- **Trait Implementation**: `app.rs` implements `eframe::App` trait
- **Extension Methods**: Other modules implement methods on `ScreenshotApp` via separate `impl` blocks
- **Dual Coordinate Systems**: Window coordinates (`Pos2`) and screen/mouse coordinates (`MousePosition`) are tracked separately to handle DPI scaling correctly

### Important Constants

- `MAX_TEXTURE_SIZE: usize = 2048` - Textures larger than this are compressed to avoid GPU limitations

### Annotation Tools

Defined in `Tool` enum: `Select`, `Pen`, `Rectangle`, `Arrow`, `Text`, `MoveBox`, `Number`, `Mosaic`, `ColorPicker`, `Save`, `Copy`, `Exit`

## Dependencies

- **egui/eframe** - Immediate mode GUI framework
- **xcap** - Screen capture
- **arboard** - Clipboard operations
- **image** - Image processing (PNG, JPEG)
- **device_query** - Mouse position tracking
- **rfd** - Native file dialogs
- **chrono** - Timestamp generation

## Platform Considerations

- The release profile is optimized for minimal binary size (`opt-level = "z"`, LTO, stripped symbols)
- Cross-compilation configured for ARM64 Linux target
- Windows: Uses `x11` feature for egui; fullscreen mode handles taskbar differently