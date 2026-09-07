use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use device_query::{DeviceQuery, DeviceState, MousePosition};
use eframe::emath::{Pos2, Rect};
use eframe::epaint::{Color32, ColorImage};
use egui::Id;
use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use xcap::Monitor;

use crate::display::{
    CaptureSession, CapturedDisplay, DisplayGeometry, PixelRect, ToolbarPlacement,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct OcrWindowState {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: f32,
    pub height: f32,
}

impl Default for OcrWindowState {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: 500.0,
            height: 500.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub last_save_dir: Option<PathBuf>,
    #[serde(default)]
    pub ocr_window: OcrWindowState,
}

#[derive(Debug, Clone, Copy)]
pub enum AppSignal {
    Save,
    Copy,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Select,
    Pen,
    Rectangle,
    Arrow,
    Text,
    MoveBox,
    Number,
    Mosaic,
    ColorPicker,
    Ocr,
    Save,
    Copy,
    Exit,
}

impl Tool {
    pub fn is_annotation_tool(self) -> bool {
        matches!(
            self,
            Self::Pen | Self::Rectangle | Self::Arrow | Self::Text | Self::Number | Self::Mosaic
        )
    }
}

#[derive(Clone)]
pub struct Annotation {
    pub tool: Tool,
    pub points: Vec<Pos2>,
    pub mouse_points: Vec<MousePosition>,
    pub color: Color32,
    pub stroke_width: f32,
    pub text: String,
    pub number: Option<i32>,
}

#[derive(Clone)]
pub struct TextInputState {
    pub position: Pos2,
    pub text: String,
    pub is_active: bool,
    pub widget_id: Id,   // 添加widget_id用于焦点管理
    pub has_focus: bool, // 新增：跟踪焦点状态
}

// 在创建TextInputState时初始化widget_id
impl TextInputState {
    pub fn new(position: Pos2) -> Self {
        Self {
            position,
            text: String::new(),
            is_active: true,
            // 使用下面这个windows下销毁输入框会报错
            // widget_id: Id::new(format!("text_input_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())),
            widget_id: Id::new("text_input".to_string()), // 使用固定ID或生成唯一ID
            has_focus: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MouseSelectionRect {
    pub start: MousePosition,
    pub end: MousePosition,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSnapshot {
    pub global_position: Pos2,
    pub primary_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryButtonTransition {
    Idle,
    Pressed,
    Held,
    Released,
}

pub fn primary_button_transition(was_down: bool, is_down: bool) -> PrimaryButtonTransition {
    match (was_down, is_down) {
        (false, false) => PrimaryButtonTransition::Idle,
        (false, true) => PrimaryButtonTransition::Pressed,
        (true, true) => PrimaryButtonTransition::Held,
        (true, false) => PrimaryButtonTransition::Released,
    }
}

use crate::ocr::{OcrSession, OcrWorker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    Capture,
    OcrResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycle {
    TrayIdle,
    Capturing,
    Exiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSaveDialogState {
    Idle,
    WaitingForHiddenFrame { restore_toolbar: bool },
    ReadyToOpen { restore_toolbar: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureRevealState {
    Idle,
    WaitingForHiddenFrame,
    ReadyToReveal,
}

impl CaptureRevealState {
    pub fn begin(&mut self) -> bool {
        if *self != Self::Idle {
            return false;
        }
        *self = Self::WaitingForHiddenFrame;
        true
    }

    pub fn finish_hidden_frame(&mut self) -> bool {
        if *self != Self::WaitingForHiddenFrame {
            return false;
        }
        *self = Self::ReadyToReveal;
        true
    }

    pub fn take_reveal(&mut self) -> bool {
        if *self != Self::ReadyToReveal {
            return false;
        }
        *self = Self::Idle;
        true
    }
}

impl NativeSaveDialogState {
    pub fn begin(&mut self, restore_toolbar: bool) -> bool {
        if *self != Self::Idle {
            return false;
        }
        *self = Self::WaitingForHiddenFrame { restore_toolbar };
        true
    }

    pub fn advance_frame(&mut self) -> Option<bool> {
        match *self {
            Self::Idle => None,
            Self::WaitingForHiddenFrame { restore_toolbar } => {
                *self = Self::ReadyToOpen { restore_toolbar };
                None
            }
            Self::ReadyToOpen { restore_toolbar } => {
                *self = Self::Idle;
                Some(restore_toolbar)
            }
        }
    }
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

#[derive(Clone)]
pub struct CaptureSnapshot {
    pub selection_rect: Option<Rect>,
    pub mouse_selection_rect: Option<MouseSelectionRect>,
    pub annotations: Vec<Annotation>,
}

pub struct DisplayTextureTile {
    pub pixel_rect: PixelRect,
    pub texture: egui::TextureHandle,
}

pub struct ScreenshotApp {
    pub lifecycle: AppLifecycle,
    pub capture_reveal_state: CaptureRevealState,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) tray_runtime: Option<crate::tray::TrayRuntime>,
    pub is_first: bool,
    pub screens: Vec<Monitor>,
    pub screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub original_screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>, // 原始分辨率截图，用于保存
    pub screenshots_positions: Vec<(usize, usize, ImageBuffer<Rgba<u8>, Vec<u8>>)>,
    pub display_textures_split: Vec<(usize, usize, egui::TextureHandle)>,
    pub capture_session: Option<CaptureSession>,
    pub display_textures: Vec<Vec<DisplayTextureTile>>,
    pub original_selection_rect: Option<Rect>,
    pub mouse_original_selection_rect: Option<MouseSelectionRect>,

    // 选择状态
    pub selection_rect: Option<Rect>,
    pub mouse_selection_rect: Option<MouseSelectionRect>,
    pub is_selecting: bool,
    pub selection_start: Pos2,
    pub selection_end: Pos2,
    pub mouse_start: MousePosition, // 添加鼠标位置变量 窗口中鼠标位置和屏幕中鼠标位置的坐标不一样，导致最后截图不对，故添加此参数
    pub mouse_end: MousePosition, // 添加鼠标位置变量 窗口中鼠标位置和屏幕中鼠标位置的坐标不一样，导致最后截图不对，故添加此参数
    pub is_moving_box: bool,
    pub move_start: Pos2,
    pub mouse_move_start: MousePosition,

    // 标注状态
    pub current_tool: Tool,
    pub annotations: Vec<Annotation>,
    pub current_annotation: Option<Annotation>,
    pub brush_size: f32,
    pub annotation_color: Color32,
    pub text_input: Option<TextInputState>,
    pub number_input: Option<i32>,
    pub tool_bar_focused: bool, // 添加工具栏焦点状态，主要是为了处理框选全屏时，工具栏在选框内部，工具栏无法点击的问题

    // UI 状态
    pub show_toolbar: bool,
    pub toolbar_position: Pos2,
    pub toolbar_placement: Option<ToolbarPlacement>,
    // 修复：窗口尺寸
    pub window_rect: Rect,

    // 新增：文本输入完成标记
    pub text_input_finalized: bool,
    pub device_state: Option<DeviceState>,
    pub pointer_snapshot: Option<PointerSnapshot>,
    pub last_primary_down: bool,

    pub screen_width: i32,  // 屏幕宽度
    pub screen_height: i32, // 屏幕高度

    pub screen_scale: f32,
    pub image_scale: f32,
    pub signal_sender: Option<Arc<Mutex<mpsc::Sender<AppSignal>>>>,
    pub signal_receiver: Option<Arc<Mutex<mpsc::Receiver<AppSignal>>>>,

    // 文件保存对话框
    pub native_save_dialog_state: NativeSaveDialogState,
    pub pending_save_image: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub config: AppConfig,
    pub config_path: PathBuf,

    // OCR 状态
    pub app_view: AppView,
    pub ocr_session: OcrSession,
    pub ocr_worker: Option<OcrWorker>,
    pub ocr_capture_snapshot: Option<CaptureSnapshot>,
    pub ocr_window_configured: bool,
    pub pending_ocr_window: Option<(OcrWindowState, std::time::Instant)>,
    pub ocr_copied_until: Option<std::time::Instant>,

    // 双击检测
    pub last_click_time: f64,
    pub last_click_pos: Pos2,
}

impl Default for ScreenshotApp {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            lifecycle: AppLifecycle::Capturing,
            capture_reveal_state: CaptureRevealState::Idle,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            tray_runtime: None,
            is_first: true,
            screens: Vec::new(),
            screenshots: Vec::new(),
            original_screenshots: Vec::new(),
            screenshots_positions: Vec::new(),
            display_textures_split: Vec::new(),
            capture_session: None,
            display_textures: Vec::new(),
            original_selection_rect: None,
            mouse_original_selection_rect: None,
            selection_rect: None,
            mouse_selection_rect: None,
            is_selecting: false,
            selection_start: Pos2::ZERO,
            selection_end: Pos2::ZERO,
            mouse_start: (0, 0),
            mouse_end: (0, 0),
            is_moving_box: false,
            move_start: Pos2::ZERO,
            mouse_move_start: (0, 0),
            current_tool: Tool::Select,
            annotations: Vec::new(),
            current_annotation: None,
            brush_size: 3.0,
            annotation_color: Color32::RED,
            text_input: None,
            number_input: None,
            tool_bar_focused: false,
            show_toolbar: false,
            toolbar_position: Pos2::ZERO,
            toolbar_placement: None,
            window_rect: Rect::NOTHING,
            text_input_finalized: false,
            device_state: None,
            pointer_snapshot: None,
            last_primary_down: false,
            screen_width: 0,
            screen_height: 0,
            screen_scale: 1.0,
            image_scale: 1.0,
            signal_sender: Some(Arc::new(Mutex::new(sender))),
            signal_receiver: Some(Arc::new(Mutex::new(receiver))),
            native_save_dialog_state: NativeSaveDialogState::Idle,
            pending_save_image: None,
            config: AppConfig::default(),
            config_path: PathBuf::new(),
            app_view: AppView::Capture,
            ocr_session: OcrSession::new(),
            ocr_worker: None,
            ocr_capture_snapshot: None,
            ocr_window_configured: false,
            pending_ocr_window: None,
            ocr_copied_until: None,
            last_click_time: 0.0,
            last_click_pos: Pos2::ZERO,
        }
    }
}

impl ScreenshotApp {
    pub fn begin_global_selection(&mut self, position: Pos2) {
        self.is_selecting = true;
        self.selection_start = position;
        self.selection_end = position;
        self.selection_rect = Some(Rect::from_min_max(position, position));
        let mouse_position = (position.x.round() as i32, position.y.round() as i32);
        self.mouse_start = mouse_position;
        self.mouse_end = mouse_position;
        self.mouse_selection_rect = Some(MouseSelectionRect {
            start: mouse_position,
            end: mouse_position,
        });
    }

    pub fn update_global_selection(&mut self, position: Pos2) {
        if !self.is_selecting {
            return;
        }
        self.selection_end = position;
        self.selection_rect = Some(Rect::from_two_pos(self.selection_start, self.selection_end));
        self.mouse_end = (position.x.round() as i32, position.y.round() as i32);
        self.mouse_selection_rect = Some(MouseSelectionRect {
            start: (
                self.mouse_start.0.min(self.mouse_end.0),
                self.mouse_start.1.min(self.mouse_end.1),
            ),
            end: (
                self.mouse_start.0.max(self.mouse_end.0),
                self.mouse_start.1.max(self.mouse_end.1),
            ),
        });
    }

    pub fn finish_global_selection(&mut self) {
        if !self.is_selecting {
            return;
        }
        self.update_global_selection(self.selection_end);
        self.is_selecting = false;
        if self.selection_rect.is_some_and(|rect| rect.area() > 0.0) {
            self.current_tool = Tool::MoveBox;
        }
    }

    pub fn poll_pointer(&mut self) -> Option<(PointerSnapshot, PrimaryButtonTransition)> {
        let mouse = self.device_state.as_ref()?.get_mouse();
        let snapshot = PointerSnapshot {
            global_position: Pos2::new(mouse.coords.0 as f32, mouse.coords.1 as f32),
            primary_down: mouse.button_pressed.get(1).copied().unwrap_or(false),
        };
        let transition = primary_button_transition(self.last_primary_down, snapshot.primary_down);
        self.last_primary_down = snapshot.primary_down;
        self.pointer_snapshot = Some(snapshot);
        Some((snapshot, transition))
    }

    pub fn with_config(config: AppConfig, config_path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut app = Self::default();
        app.config = config.clone();
        app.config_path = config_path;

        app.signal_sender = Some(Arc::new(Mutex::new(sender)));
        app.signal_receiver = Some(Arc::new(Mutex::new(receiver)));
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            app.device_state = Some(DeviceState::new());
        }
        app
    }

    pub fn save_config(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.config) {
            if let Some(parent) = self.config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&self.config_path, json);
        }
    }
}

pub const MAX_TEXTURE_SIZE: usize = 2048;

impl ScreenshotApp {
    pub fn install_capture_session(&mut self, session: CaptureSession) {
        self.display_textures = (0..session.displays.len()).map(|_| Vec::new()).collect();
        self.capture_session = Some(session);
    }

    pub fn try_install_captured_displays(
        &mut self,
        displays: Vec<CapturedDisplay>,
    ) -> Result<(), String> {
        let session = CaptureSession::new(displays)?;
        self.install_capture_session(session);
        Ok(())
    }

    pub fn capture_screens(&mut self) -> Result<(), String> {
        let screens = Monitor::all().map_err(|error| format!("枚举显示器失败: {error}"))?;
        if screens.is_empty() {
            return Err("未检测到可截图的显示器".to_string());
        }

        let mut captured_displays = Vec::with_capacity(screens.len());
        for (session_index, screen) in screens.iter().enumerate() {
            let x = screen
                .x()
                .map_err(|error| format!("读取显示器 {session_index} 的 X 坐标失败: {error}"))?;
            let y = screen
                .y()
                .map_err(|error| format!("读取显示器 {session_index} 的 Y 坐标失败: {error}"))?;
            let logical_width = screen
                .width()
                .map_err(|error| format!("读取显示器 {session_index} 的宽度失败: {error}"))?;
            let logical_height = screen
                .height()
                .map_err(|error| format!("读取显示器 {session_index} 的高度失败: {error}"))?;
            let image = screen
                .capture_image()
                .map_err(|error| format!("捕获显示器 {session_index}（{x},{y}）失败: {error}"))?;
            let geometry = DisplayGeometry::new(
                session_index,
                Rect::from_min_size(
                    Pos2::new(x as f32, y as f32),
                    egui::vec2(logical_width as f32, logical_height as f32),
                ),
                image.dimensions(),
            )?;
            captured_displays.push(CapturedDisplay::from_image(
                geometry,
                image,
                MAX_TEXTURE_SIZE as u32,
            )?);
        }

        let session = CaptureSession::new(captured_displays)?;

        // 旧渲染与导出路径会在后续任务中迁移；在此之前由同一原子会话派生兼容数据。
        self.screens = screens;
        self.screenshots.clear();
        self.original_screenshots = session
            .displays
            .iter()
            .map(|display| display.original_image.clone())
            .collect();
        self.screenshots_positions.clear();
        for display in &session.displays {
            for tile in &display.tiles {
                self.screenshots.push(tile.image.clone());
                self.screenshots_positions.push((
                    tile.pixel_rect.x as usize,
                    tile.pixel_rect.y as usize,
                    tile.image.clone(),
                ));
            }
        }
        if let Some(first) = session.displays.first() {
            self.screen_width = first.geometry.pixel_size.0 as i32;
            self.screen_height = first.geometry.pixel_size.1 as i32;
            self.screen_scale = first.geometry.pixel_scale.x;
        }
        self.install_capture_session(session);
        Ok(())
    }

    // fn split_screenshot(&self, image: &RgbaImage, max_tile_size: u32) -> Vec<(usize, usize, RgbaImage)> {
    //     let (width, height) = image.dimensions();
    //     let mut tiles = Vec::new();
    //
    //     for y in (0..height).step_by(max_tile_size as usize) {
    //         for x in (0..width).step_by(max_tile_size as usize) {
    //             let tile_width = (width - x).min(max_tile_size);
    //             let tile_height = (height - y).min(max_tile_size);
    //
    //             let tile: ImageBuffer<Rgba<u8>, Vec<u8>> = image.view(x, y, tile_width, tile_height).to_image();
    //             tiles.push((x as usize, y as usize, tile));
    //         }
    //     }
    //     tiles
    // }

    pub fn screen_to_texture(&mut self, ctx: &egui::Context) {
        self.ensure_display_textures(ctx);
        for (x, y, image) in self.screenshots_positions.clone() {
            let size = [image.width() as usize, image.height() as usize];
            let pixels = image.into_raw();
            // 注意：这里假设 image crate 返回的是 RGBA 字节，与 egui 的 ColorImage 匹配
            let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
            let texture = ctx.load_texture(
                format!("screenshot_{}_{}", x, y),
                color_image,
                Default::default(),
            );
            self.display_textures_split.push((x, y, texture));
        }
    }

    pub fn ensure_display_textures(&mut self, ctx: &egui::Context) {
        let Some(session) = &self.capture_session else {
            return;
        };
        if self.display_textures.iter().any(|tiles| !tiles.is_empty()) {
            return;
        }

        self.display_textures = session
            .displays
            .iter()
            .map(|display| {
                display
                    .tiles
                    .iter()
                    .map(|tile| {
                        let size = [tile.image.width() as usize, tile.image.height() as usize];
                        let color_image =
                            ColorImage::from_rgba_unmultiplied(size, tile.image.as_raw());
                        let texture = ctx.load_texture(
                            format!(
                                "screenshot_{}_{}_{}",
                                display.geometry.session_index,
                                tile.pixel_rect.x,
                                tile.pixel_rect.y
                            ),
                            color_image,
                            Default::default(),
                        );
                        DisplayTextureTile {
                            pixel_rect: tile.pixel_rect,
                            texture,
                        }
                    })
                    .collect()
            })
            .collect();
    }

    pub fn get_combined_bounds(&self) -> Rect {
        if let Some(session) = &self.capture_session {
            return session.desktop_bounds;
        }
        if self.screens.is_empty() {
            return Rect::NOTHING;
        }

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for screen in &self.screens {
            min_x = min_x.min(screen.x().unwrap());
            min_y = min_y.min(screen.y().unwrap());
            max_x = max_x.max(screen.x().unwrap() + screen.width().unwrap() as i32);
            max_y = max_y.max(screen.y().unwrap() + screen.height().unwrap() as i32);
        }

        Rect::from_min_max(
            Pos2::new(min_x as f32, min_y as f32),
            Pos2::new(max_x as f32, max_y as f32),
        )
    }
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, Rect, Vec2};
    use image::RgbaImage;

    use crate::display::{CaptureSession, CapturedDisplay, DisplayGeometry};

    use super::{
        AppConfig, AppLifecycle, CaptureRevealState, NativeSaveDialogState, OcrWindowState,
        PrimaryButtonTransition, ScreenshotApp, Tool, primary_button_transition,
    };

    fn test_display(
        index: usize,
        bounds: (f32, f32, f32, f32),
        pixels: (u32, u32),
    ) -> CapturedDisplay {
        let geometry = DisplayGeometry::new(
            index,
            Rect::from_min_size(Pos2::new(bounds.0, bounds.1), Vec2::new(bounds.2, bounds.3)),
            pixels,
        )
        .unwrap();
        CapturedDisplay::from_image(
            geometry,
            RgbaImage::new(pixels.0, pixels.1),
            super::MAX_TEXTURE_SIZE as u32,
        )
        .unwrap()
    }

    fn test_capture_session(displays: Vec<CapturedDisplay>) -> CaptureSession {
        CaptureSession::new(displays).unwrap()
    }

    #[test]
    fn capture_waits_for_one_hidden_frame_before_reveal() {
        let mut state = CaptureRevealState::Idle;

        assert!(state.begin());
        assert!(!state.take_reveal());
        state.finish_hidden_frame();
        assert!(state.take_reveal());
        assert_eq!(state, CaptureRevealState::Idle);
    }

    #[test]
    fn native_save_dialog_waits_for_hidden_toolbar_frame_before_opening() {
        let mut state = NativeSaveDialogState::Idle;

        assert!(state.begin(true));
        assert_eq!(state.advance_frame(), None);
        assert_eq!(state.advance_frame(), Some(true));
        assert_eq!(state, NativeSaveDialogState::Idle);
    }

    #[test]
    fn native_save_dialog_rejects_duplicate_open_requests() {
        let mut state = NativeSaveDialogState::Idle;

        assert!(state.begin(false));
        assert!(!state.begin(true));
    }

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

    #[test]
    fn default_ocr_window_size_is_500_square() {
        let state = OcrWindowState::default();

        assert_eq!(state.width, 500.0);
        assert_eq!(state.height, 500.0);
        assert_eq!(state.x, None);
        assert_eq!(state.y, None);
    }

    #[test]
    fn old_config_without_ocr_window_state_remains_compatible() {
        let config: AppConfig = serde_json::from_str(r#"{"last_save_dir":null}"#).unwrap();

        assert_eq!(config.ocr_window.width, 500.0);
        assert_eq!(config.ocr_window.height, 500.0);
    }

    #[test]
    fn saved_window_position_must_intersect_current_monitor() {
        let visible = OcrWindowState {
            x: Some(100.0),
            y: Some(100.0),
            width: 500.0,
            height: 500.0,
        };
        let hidden = OcrWindowState {
            x: Some(2500.0),
            y: Some(100.0),
            width: 500.0,
            height: 500.0,
        };

        assert!(crate::app::saved_position_is_visible(
            visible,
            egui::vec2(1920.0, 1080.0)
        ));
        assert!(!crate::app::saved_position_is_visible(
            hidden,
            egui::vec2(1920.0, 1080.0)
        ));
    }

    #[test]
    fn ocr_window_geometry_waits_until_fullscreen_has_exited() {
        assert!(!crate::app::ocr_window_geometry_can_be_applied(Some(true)));
        assert!(crate::app::ocr_window_geometry_can_be_applied(Some(false)));
        assert!(!crate::app::ocr_window_geometry_can_be_applied(None));
    }

    #[test]
    fn capture_coordinator_is_never_fullscreen() {
        let style = crate::app::capture_window_style();

        assert!(!style.fullscreen);
    }

    #[test]
    fn macos_capture_window_is_a_borderless_fixed_overlay() {
        let style = crate::app::capture_window_style_for(true);

        assert!(style.accessory_application);
        assert!(!style.fullscreen);
        assert!(!style.decorations);
        assert!(!style.resizable);
        assert!(!style.close_button);
        assert!(!style.minimize_button);
        assert!(!style.maximize_button);
    }

    #[test]
    fn non_macos_capture_coordinator_stays_hidden_and_windowless() {
        let style = crate::app::capture_window_style_for(false);

        assert!(!style.accessory_application);
        assert!(!style.fullscreen);
        assert!(!style.decorations);
        assert!(!style.resizable);
        assert!(!style.close_button);
        assert!(!style.minimize_button);
        assert!(!style.maximize_button);
    }

    #[test]
    fn default_does_not_start_ocr_worker() {
        let app = ScreenshotApp::default();

        assert!(app.ocr_worker.is_none());
    }

    #[test]
    fn default_constructor_does_not_initialize_os_pointer_source() {
        let app = ScreenshotApp::default();

        assert!(app.device_state.is_none());
    }

    #[test]
    fn ocr_is_an_action_not_an_annotation_tool() {
        assert!(!Tool::Ocr.is_annotation_tool());
        assert!(Tool::Pen.is_annotation_tool());
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
            0,
            (0.0, 0.0, 100.0, 100.0),
            (100, 100),
        )]));

        assert!(app.try_install_captured_displays(Vec::new()).is_err());

        assert_eq!(app.capture_session.as_ref().unwrap().displays.len(), 1);
        assert_eq!(app.display_textures.len(), 1);
    }

    #[test]
    fn primary_button_transition_is_emitted_once() {
        assert_eq!(
            primary_button_transition(false, true),
            PrimaryButtonTransition::Pressed
        );
        assert_eq!(
            primary_button_transition(true, true),
            PrimaryButtonTransition::Held
        );
        assert_eq!(
            primary_button_transition(true, false),
            PrimaryButtonTransition::Released
        );
        assert_eq!(
            primary_button_transition(false, false),
            PrimaryButtonTransition::Idle
        );
    }

    #[test]
    fn global_selection_can_start_left_and_end_on_right_monitor() {
        let mut app = ScreenshotApp::default();

        app.begin_global_selection(Pos2::new(-200.0, 100.0));
        app.update_global_selection(Pos2::new(300.0, 500.0));
        app.finish_global_selection();

        assert_eq!(
            app.selection_rect,
            Some(Rect::from_min_max(
                Pos2::new(-200.0, 100.0),
                Pos2::new(300.0, 500.0),
            ))
        );
        assert!(!app.is_selecting);
    }
}
