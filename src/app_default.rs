use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use device_query::{DeviceState, MousePosition};
use eframe::emath::{Pos2, Rect};
use eframe::epaint::{Color32, ColorImage};
use egui::Id;
use egui_file_dialog::FileDialog;
use image::{GenericImageView, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use xcap::Monitor;

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

use crate::ocr::{OcrSession, OcrWorker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    Capture,
    OcrResult,
}

#[derive(Clone)]
pub struct CaptureSnapshot {
    pub selection_rect: Option<Rect>,
    pub mouse_selection_rect: Option<MouseSelectionRect>,
    pub annotations: Vec<Annotation>,
}

pub struct ScreenshotApp {
    pub is_first: bool,
    pub screens: Vec<Monitor>,
    pub screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub original_screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>, // 原始分辨率截图，用于保存
    pub screenshots_positions: Vec<(usize, usize, ImageBuffer<Rgba<u8>, Vec<u8>>)>,
    pub display_textures_split: Vec<(usize, usize, egui::TextureHandle)>,
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
    // 修复：窗口尺寸
    pub window_rect: Rect,

    // 新增：文本输入完成标记
    pub text_input_finalized: bool,
    pub device_state: Option<DeviceState>,

    pub screen_width: i32,  // 屏幕宽度
    pub screen_height: i32, // 屏幕高度

    pub screen_scale: f32,
    pub image_scale: f32,
    pub signal_sender: Option<Arc<Mutex<mpsc::Sender<AppSignal>>>>,
    pub signal_receiver: Option<Arc<Mutex<mpsc::Receiver<AppSignal>>>>,

    // 文件保存对话框
    pub save_dialog: FileDialog,
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
            is_first: true,
            screens: Vec::new(),
            screenshots: Vec::new(),
            original_screenshots: Vec::new(),
            screenshots_positions: Vec::new(),
            display_textures_split: Vec::new(),
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
            window_rect: Rect::NOTHING,
            text_input_finalized: false,
            device_state: None,
            screen_width: 0,
            screen_height: 0,
            screen_scale: 1.0,
            image_scale: 1.0,
            signal_sender: Some(Arc::new(Mutex::new(sender))),
            signal_receiver: Some(Arc::new(Mutex::new(receiver))),
            save_dialog: FileDialog::new()
                .title("保存截图")
                .add_save_extension("PNG 图片", "png")
                .add_save_extension("JPEG 图片", "jpg")
                .default_save_extension("PNG 图片")
                .default_file_name(&format!(
                    "screenshot_{}",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                ))
                .as_modal(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .allow_file_overwrite(true),
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
    pub fn with_config(config: AppConfig, config_path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut app = Self::default();
        app.config = config.clone();
        app.config_path = config_path;

        if let Some(ref dir) = config.last_save_dir {
            app.save_dialog.config_mut().initial_directory = dir.clone();
        }

        app.signal_sender = Some(Arc::new(Mutex::new(sender)));
        app.signal_receiver = Some(Arc::new(Mutex::new(receiver)));
        app.device_state = Some(DeviceState::new());
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
    pub fn capture_screens(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.screens = Monitor::all()?;
        self.screen_width = self.screens[0].width()? as i32;
        self.screen_height = self.screens[0].height()? as i32;
        self.screenshots.clear();
        self.original_screenshots.clear();
        for screen in &self.screens {
            self.screen_scale = screen.scale_factor().unwrap();
            let image = screen.capture_image()?;
            let (width, height) = image.dimensions();
            self.screen_width = width as i32;
            self.screen_height = height as i32;

            // 始终保存原始分辨率截图用于导出
            self.original_screenshots.push(image.clone());

            if width <= MAX_TEXTURE_SIZE as u32 && height <= MAX_TEXTURE_SIZE as u32 {
                self.screenshots_positions.push((0, 0, image.clone()));
                self.screenshots.push(image.clone());
            } else {
                // 分块纹理，处理大尺寸屏幕
                for y in (0..height).step_by(MAX_TEXTURE_SIZE) {
                    for x in (0..width).step_by(MAX_TEXTURE_SIZE) {
                        let tile_width = (width - x).min(MAX_TEXTURE_SIZE as u32);
                        let tile_height = (height - y).min(MAX_TEXTURE_SIZE as u32);
                        let tile = image.view(x, y, tile_width, tile_height).to_image();
                        self.screenshots_positions
                            .push((x as usize, y as usize, tile.clone()));
                        self.screenshots.push(tile);
                    }
                }
            }
        }
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

    pub fn get_combined_bounds(&self) -> Rect {
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
    use super::{AppConfig, OcrWindowState, ScreenshotApp, Tool};

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
    fn capture_window_uses_platform_appropriate_fullscreen_mode() {
        let style = crate::app::capture_window_style();

        assert_eq!(style.fullscreen, !cfg!(target_os = "macos"));
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
    fn non_macos_capture_window_keeps_existing_fullscreen_behavior() {
        let style = crate::app::capture_window_style_for(false);

        assert!(!style.accessory_application);
        assert!(style.fullscreen);
        assert!(style.decorations);
        assert!(style.resizable);
        assert!(style.close_button);
        assert!(style.minimize_button);
        assert!(style.maximize_button);
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
}
