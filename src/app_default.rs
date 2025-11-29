use device_query::{DeviceState, MousePosition};
use eframe::emath::{Pos2, Rect, Vec2};
use eframe::epaint::Color32;
use egui::Id;
use image::{ImageBuffer, Rgba};
use xcap::Monitor;
use crate::ui::get_screen_rect;

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
    Button
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
    pub widget_id: Id, // 添加widget_id用于焦点管理
    pub has_focus: bool, // 新增：跟踪焦点状态
    pub last_interaction_time: f64,
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
            last_interaction_time: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct MouseSelectionRect {
    pub start: MousePosition,
    pub end: MousePosition,
}

pub struct ScreenshotSegment {
    pub texture: egui::TextureHandle,
    pub rect: Rect, // 该片段在完整截图中的位置
}


pub struct ScreenshotApp {
    pub screens: Vec<Monitor>,
    pub screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub display_textures: Vec<ScreenshotSegment>,
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
    pub  brush_size: f32,
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
    pub device_state: DeviceState,

    pub screen_width: i32, // 屏幕宽度
    pub screen_height: i32, // 屏幕高度
}

impl Default for ScreenshotApp {
    fn default() -> Self {
        Self {
            screens: Vec::new(),
            screenshots: Vec::new(),
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
            window_rect: Rect::NOTHING,
            text_input_finalized: false,
            device_state: DeviceState::new(),
            screen_width: 0,
            screen_height: 0,
        }
    }
}

const MAX_SIZE: i32 = 2048;

impl ScreenshotApp {
    pub(crate) fn capture_screens(&mut self, ctx: &egui::Context) -> Result<(), Box<dyn std::error::Error>> {
        self.screens = Monitor::all()?;
        self.screenshots.clear();
        self.display_textures.clear();

        for screen in &self.screens {
            self.screen_width = screen.width().unwrap() as i32;
            self.screen_height = screen.height().unwrap() as i32;
            if self.screen_width <= MAX_SIZE && self.screen_height <= MAX_SIZE {
                let image = screen.capture_image()?;
                self.screenshots.push(image.clone());
                let texture = self.get_image_texture(image, ctx, get_screen_rect(screen));
                self.display_textures.push(texture);
            } else {
                let image_one = screen.capture_region(0u32, 0u32, MAX_SIZE as u32, self.screen_height as u32)?;
                self.screenshots.push(image_one.clone());
                let one_rect = Rect::from_min_size(
                    Pos2::new(screen.x().unwrap() as f32, screen.y().unwrap() as f32),
                    Vec2::new(MAX_SIZE as f32, screen.height().unwrap() as f32),
                );
                let texture_one = self.get_image_texture(image_one, ctx, one_rect);
                self.display_textures.push(texture_one);
                let image_two = screen.capture_region(MAX_SIZE as u32, 0u32, self.screen_width as u32 - MAX_SIZE as u32, self.screen_height as u32)?;
                self.screenshots.push(image_two.clone());
                let two_rect = Rect::from_min_size(
                    Pos2::new(screen.x().unwrap() as f32 + MAX_SIZE as f32, screen.y().unwrap() as f32),
                    Vec2::new(self.screen_width as f32 - MAX_SIZE as f32, screen.height().unwrap() as f32),
                );
                let texture_two = self.get_image_texture(image_two, ctx, two_rect);
                self.display_textures.push(texture_two);
            }
            // 转换为 image crate 的格式

        }

        Ok(())
    }

    fn get_image_texture(&self, image: ImageBuffer<Rgba<u8>, Vec<u8>>, ctx: &egui::Context, rect: Rect) -> ScreenshotSegment {
        let texture = ctx.load_texture(
            format!("screen_{}", self.display_textures.len()),
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                &image.to_vec(),
            ),
            egui::TextureOptions::LINEAR,
        );
        ScreenshotSegment {
            texture,
            rect,
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