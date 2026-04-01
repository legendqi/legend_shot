# AGENTS.md

Guidelines for AI coding agents working in this repository.

## Build Commands

```bash
# Development build
cargo build

# Release build (optimized for size)
cargo build --release

# Run application
cargo run

# Cross-platform builds
./build.sh setup              # Install compilation targets
./build.sh linux-amd64        # Build for Linux x86_64
./build.sh linux-arm64        # Build for Linux ARM64
./build.sh windows            # Build for Windows
./build.sh macos-arm64        # Build for macOS ARM
./build.sh macos-intel        # Build for macOS Intel

# Install to system
sudo cp target/release/legend_shot /usr/local/bin/
```

## Testing

No automated tests currently. Manual testing:
```bash
# Test mode with auto-capture region
cargo run -- --test "100,100,400,300" --action copy
cargo run -- --test "100,100,400,300" --action save --output test.png
```

## Architecture

```
src/
├── main.rs           # Entry point, CLI args, eframe setup
├── app_default.rs    # ScreenshotApp struct, state, screen capture
├── app.rs            # App trait impl, main update loop
├── app_draw.rs       # Drawing screens, overlay, input handling
├── app_toolbar.rs    # Toolbar UI, annotation rendering
├── app_handle.rs     # Save/copy operations, image cropping
└── ui.rs             # Utilities, icon loading, image compression
```

**Pattern**: `ScreenshotApp` is central struct. Other modules extend it via separate `impl` blocks.

## Code Style

### Imports
- Group by scope: std → external crates → crate modules
- Use `use crate::` for internal imports

```rust
use std::sync::{Arc, Mutex, mpsc};

use device_query::{DeviceState, MousePosition};
use eframe::emath::{Pos2, Rect};
use egui::Id;

use crate::ui::get_compress_image;
```

### Struct Definition
- Large structs with many fields are acceptable
- Use `pub` for fields accessed across modules
- Document complex fields with inline comments

### Error Handling
- Use `Result<T, String>` for fallible operations
- Return `Err("描述".to_string())` for user-facing errors
- Use `.map_err(|e| e.to_string())?` for library errors

```rust
pub fn save_screenshot(&mut self) -> Result<(), String> {
    if self.selection_rect.is_none() {
        return Err("请选择要保存的图片".to_string());
    }
    // ...
}
```

### Naming Conventions
- Snake_case for functions and variables
- PascalCase for types and enums
- Descriptive names: `mouse_selection_rect`, `annotation_color`

### Platform-Specific Code
Use `#[cfg(target_os = "linux")]` attributes:
```rust
#[cfg(target_os = "linux")]
{
    // Linux-specific clipboard handling
}

#[cfg(not(target_os = "linux"))]
{
    // Other platforms
}
```

## Key Patterns

### Dual Coordinate Systems
- `Pos2` (egui): Window coordinates for rendering
- `MousePosition` (i32 tuple): Screen coordinates for capture
- Always track both to handle DPI scaling correctly

### State Management
- All state in `ScreenshotApp` struct
- Mutable methods modify state in place
- Use `Option<T>` for nullable fields

### Annotation System
```rust
pub struct Annotation {
    pub tool: Tool,           // Tool enum
    pub points: Vec<Pos2>,    // Window coordinates
    pub mouse_points: Vec<MousePosition>, // Screen coordinates
    pub color: Color32,
    pub stroke_width: f32,
    pub text: String,
    pub number: Option<i32>,
}
```

### Threading for UI Operations
Use channels to communicate between threads:
```rust
let sender_clone = self.signal_sender.clone();
std::thread::spawn(move || {
    signal_sender.lock().unwrap().send(AppSignal::Save).ok();
});
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| egui/eframe | Immediate mode GUI |
| xcap | Screen capture |
| arboard | Clipboard operations |
| rfd | File dialogs (gtk3 feature for Linux fullscreen) |
| image | Image processing |
| device_query | Mouse position tracking |
| clap | CLI arguments |
| chrono | Timestamps |

## Important Constants

```rust
pub const MAX_TEXTURE_SIZE: usize = 2048;
```

Textures larger than 2048px are compressed to avoid GPU limitations.

## Platform Notes

- **Linux**: Use `gtk3` feature for rfd to fix fullscreen dialog issues
- **Windows**: Use `x11` feature for egui; fullscreen handles taskbar differently
- **Release**: Optimized for size (`opt-level = "z"`, LTO, stripped symbols)