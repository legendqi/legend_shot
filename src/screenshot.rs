use arboard::Clipboard;
use device_query::{DeviceQuery, DeviceState};
use eframe::App;
use eframe::epaint::StrokeKind;
use egui::{Color32, Id, Pos2, Rect, Shape, Stroke, Vec2};
use image::{ImageBuffer, Rgba};
use xcap::{Monitor};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Select,
    Brush,
    Rectangle,
    Arrow,
    Text,
    MoveBox
}

#[derive(Clone)]
struct Annotation {
    tool: Tool,
    points: Vec<Pos2>,
    color: Color32,
    stroke_width: f32,
    text: String,
}

#[derive(Clone)]
struct TextInputState {
    position: Pos2,
    text: String,
    is_active: bool,
    widget_id: Id, // 添加widget_id用于焦点管理
    has_focus: bool, // 新增：跟踪焦点状态
}

// 在创建TextInputState时初始化widget_id
impl TextInputState {
    fn new(position: Pos2) -> Self {
        Self {
            position,
            text: String::new(),
            is_active: true,
            widget_id: Id::new(format!("text_input_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())), // 使用固定ID或生成唯一ID
            has_focus: false,
        }
    }
}


pub struct ScreenshotApp {
    screens: Vec<Monitor>,
    screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    display_textures: Vec<egui::TextureHandle>,
    original_selection_rect: Option<Rect>,
    mouse_original_selection_rect: Option<Rect>,


    // 选择状态
    selection_rect: Option<Rect>,
    mouse_selection_rect: Option<Rect>,
    is_selecting: bool,
    selection_start: Pos2,
    selection_end: Pos2,
    mouse_start: Pos2, // 添加鼠标位置变量 窗口中鼠标位置和屏幕中鼠标位置的坐标不一样，导致最后截图不对，故添加此参数
    mouse_end: Pos2, // 添加鼠标位置变量 窗口中鼠标位置和屏幕中鼠标位置的坐标不一样，导致最后截图不对，故添加此参数
    is_moving_box: bool,
    move_start: Pos2,
    mouse_move_start: Pos2,

    // 标注状态
    current_tool: Tool,
    annotations: Vec<Annotation>,
    current_annotation: Option<Annotation>,
    brush_size: f32,
    annotation_color: Color32,
    text_input: Option<TextInputState>,

    // UI 状态
    show_toolbar: bool,
    toolbar_position: Pos2,
    // 修复：窗口尺寸
    window_rect: Rect,

    // 新增：文本输入完成标记
    text_input_finalized: bool,
    device_state: DeviceState,

    screen_with: u32, // 屏幕宽度
    screen_height: u32, // 屏幕高度
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
            mouse_start: Pos2::ZERO,
            mouse_end: Pos2::ZERO,
            is_moving_box: false,
            move_start: Pos2::ZERO,
            mouse_move_start: Pos2::ZERO,
            current_tool: Tool::Select,
            annotations: Vec::new(),
            current_annotation: None,
            brush_size: 3.0,
            annotation_color: Color32::RED,
            text_input: None,
            show_toolbar: false,
            toolbar_position: Pos2::ZERO,
            window_rect: Rect::NOTHING,
            text_input_finalized: false,
            device_state: DeviceState::new(),
            screen_with: 0,
            screen_height: 0,
        }
    }
}

impl ScreenshotApp {
    fn capture_screens(&mut self, ctx: &egui::Context) -> Result<(), Box<dyn std::error::Error>> {
        self.screens = Monitor::all()?;
        self.screenshots.clear();
        self.display_textures.clear();

        for screen in &self.screens {
            let image = screen.capture_image()?;
            self.screen_with = image.width();
            self.screen_height = image.height();
            // 转换为 image crate 的格式
            let img_buffer = ImageBuffer::from_raw(
                image.width(),
                image.height(),
                image.to_vec(),
            ).ok_or("Failed to create image buffer")?;

            self.screenshots.push(img_buffer);

            // 创建 egui 纹理
            let texture = ctx.load_texture(
                format!("screen_{}", self.display_textures.len()),
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    &image.to_vec(),
                ),
                egui::TextureOptions::LINEAR,
            );

            self.display_textures.push(texture);
        }

        Ok(())
    }

    fn get_combined_bounds(&self) -> Rect {
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

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.window_rect = ctx.viewport_rect();
        // 首次运行截图
        if self.screenshots.is_empty() {
            if let Err(e) = self.capture_screens(ctx) {
                eprintln!("Failed to capture screens: {}", e);
            }
        }
        // 主界面
        egui::Area::new(Id::from("screenshot_area".to_string()))
            .order(egui::Order::Background)
            .fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            self.draw_screens(ui);
            self.draw_overlay(ui);        // 覆盖层和选择框
            self.draw_annotations(ui);    // 标注在覆盖层之上
            self.handle_input(ui, ctx);   // 输入处理，包括更新选择框和标注
            self.draw_text_input(ui);     // 添加文本输入UI

            if self.show_toolbar {
                self.draw_toolbar(ctx);
            }
        });
    }
}

fn get_screen_rect(screen: &Monitor) -> Rect {
    Rect::from_min_size(
        Pos2::new(screen.x().unwrap() as f32, screen.y().unwrap() as f32),
        Vec2::new(screen.width().unwrap() as f32, screen.height().unwrap() as f32),
    )
}

impl ScreenshotApp {
    fn draw_screens(&self, ui: &mut egui::Ui) {
        for (_i, (screen, texture)) in self.screens.iter().zip(&self.display_textures).enumerate() {
            let screen_rect = get_screen_rect(screen);

            // 绘制屏幕截图
            ui.put(screen_rect, egui::Image::new(texture).fit_to_exact_size(screen_rect.size()));
        }
    }

    fn draw_overlay(&mut self, ui: &mut egui::Ui) {
        let combined_bounds = self.get_combined_bounds();

        // 绘制半透明灰色覆盖层，但排除选择区域
        if let Some(selection) = self.selection_rect {
            let selection_min = selection.min.max(combined_bounds.min);
            let selection_max = selection.max.min(combined_bounds.max);
            let clipped_selection = Rect::from_min_max(selection_min, selection_max);

            if clipped_selection.area() > 0.0 {
                let overlay_color = Color32::from_rgba_unmultiplied(0, 0, 0, 100);

                // 将覆盖层分割成4个矩形区域（选择区域周围的区域）
                // 上方区域
                if combined_bounds.min.y < clipped_selection.min.y {
                    let top_rect = Rect::from_min_max(
                        combined_bounds.min,
                        Pos2::new(combined_bounds.max.x, clipped_selection.min.y)
                    );
                    ui.painter().add(Shape::rect_filled(
                        top_rect,
                        egui::CornerRadius::ZERO,
                        overlay_color,
                    ));
                }

                // 下方区域
                if combined_bounds.max.y > clipped_selection.max.y {
                    let bottom_rect = Rect::from_min_max(
                        Pos2::new(combined_bounds.min.x, clipped_selection.max.y),
                        combined_bounds.max
                    );
                    ui.painter().add(Shape::rect_filled(
                        bottom_rect,
                        egui::CornerRadius::ZERO,
                        overlay_color,
                    ));
                }

                // 左方区域
                if combined_bounds.min.x < clipped_selection.min.x {
                    let left_rect = Rect::from_min_max(
                        Pos2::new(combined_bounds.min.x, clipped_selection.min.y),
                        Pos2::new(clipped_selection.min.x, clipped_selection.max.y)
                    );
                    ui.painter().add(Shape::rect_filled(
                        left_rect,
                        egui::CornerRadius::ZERO,
                        overlay_color,
                    ));
                }

                // 右方区域
                if combined_bounds.max.x > clipped_selection.max.x {
                    let right_rect = Rect::from_min_max(
                        Pos2::new(clipped_selection.max.x, clipped_selection.min.y),
                        Pos2::new(combined_bounds.max.x, clipped_selection.max.y)
                    );
                    ui.painter().add(Shape::rect_filled(
                        right_rect,
                        egui::CornerRadius::ZERO,
                        overlay_color,
                    ));
                }

                // 绘制选择框边框
                ui.painter().add(Shape::rect_stroke(
                    clipped_selection,
                    egui::CornerRadius::ZERO,
                    Stroke::new(2.0, Color32::RED),
                    StrokeKind::Inside
                ));
            } else {
                // 选择区域完全在边界外，绘制完整覆盖层
                ui.painter().add(Shape::rect_filled(
                    combined_bounds,
                    egui::CornerRadius::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                ));
            }
        } else {
            // 没有选择时绘制完整覆盖层
            ui.painter().add(Shape::rect_filled(
                combined_bounds,
                egui::CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, 100),
            ));
        }
    }


    fn handle_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pointer_pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO);
        let mouse_pos = self.device_state.get_mouse().coords;
        // 如果有活动的文本输入，优先处理文本输入
        if let Some(text_state) = &mut self.text_input {
            if text_state.is_active {
                // 文本输入激活时，不处理其他工具
                self.handle_text_input(ui, ctx);
                // 如果文本输入已经完成，立即返回
                if self.text_input_finalized {
                    self.finalize_text_input();
                    return;
                }
            }
        }

        // 鼠标按下开始选择
        if ui.input(|i| i.pointer.primary_pressed()) {
            if self.current_tool == Tool::Select && !self.show_toolbar {
                self.is_selecting = true;
                self.selection_start = pointer_pos;
                self.selection_end = pointer_pos;
                self.mouse_start = Pos2::new(mouse_pos.0 as f32, mouse_pos.1 as f32);
                self.mouse_end = Pos2::new(mouse_pos.0 as f32, mouse_pos.1 as f32);
            } else if self.current_tool == Tool::MoveBox && self.selection_rect.is_some() {
                // 开始移动选择框
                if self.selection_rect.unwrap().contains(pointer_pos) {
                    self.is_moving_box = true;
                    self.show_toolbar = false;
                    self.move_start = pointer_pos;
                    self.mouse_move_start = Pos2::new(mouse_pos.0 as f32, mouse_pos.1 as f32);
                    // 保存选择框的原始位置
                    self.original_selection_rect = self.selection_rect;
                    self.mouse_original_selection_rect = self.mouse_selection_rect;
                }
            } else if self.current_tool != Tool::Select && self.current_tool != Tool::MoveBox {
                // 检查是否在选择区域内才允许开始标注
                if let Some(selection_rect) = self.selection_rect {
                    if selection_rect.contains(pointer_pos) {
                        // 如果是文本工具，开始文本输入
                        if self.current_tool == Tool::Text {
                            if self.text_input.is_none() {
                                // 创建文本输入状态
                                let text_state = TextInputState::new(pointer_pos);
                                self.text_input = Some(text_state);
                                self.text_input_finalized = false;
                                self.start_annotation(pointer_pos);
                            } else {
                                self.finalize_text_input();
                            }
                        } else {
                            // 否则，开始标注
                            self.start_annotation(pointer_pos);
                        }
                    } else {
                        // 点击区域外的地方, 取消文本输入
                        if self.text_input.is_some() || self.current_tool == Tool::Text {
                            // 创建文本输入状态
                            self.finalize_text_input();
                        }
                    }
                }
            }
        }

        // 处理键盘输入（仅在文本输入激活时）
        if let Some(text_state) = &mut self.text_input {
            if text_state.is_active {
                // 使用正确的事件处理方式
                ctx.input(|input| {
                    // 处理字符输入
                    for event in &input.events {
                        match event {
                            egui::Event::Text(text) => {
                                // 过滤控制字符，只添加可打印字符
                                if !text.chars().next().map_or(false, |c| c.is_control()) {
                                    text_state.text.push_str(text);
                                }
                            }
                            _ => {}
                        }
                    }

                    // 处理特殊键
                    if input.key_pressed(egui::Key::Enter) {
                        text_state.text.push('\n');
                    }

                    if input.key_pressed(egui::Key::Backspace) {
                        text_state.text.pop();
                    }
                });

                // ESC 键取消文本输入
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.text_input = None;
                    self.current_annotation = None; // 同时取消当前标注
                }
            }
        }

        // 鼠标拖动
        if ui.input(|i| i.pointer.primary_down()) {
            if self.is_selecting {
                self.selection_end = pointer_pos;
                self.mouse_end = Pos2::new(mouse_pos.0 as f32, mouse_pos.1 as f32);

                self.update_selection_rect();
            } else if self.is_moving_box {
                // 移动选择框 - 基于原始位置计算偏移
                if let (Some(original_rect), Some(mouse_original_rect), Some(combined_bounds)) = (self.original_selection_rect, self.mouse_original_selection_rect, Some(self.get_combined_bounds())) {
                    let delta = pointer_pos - self.move_start;
                    let mouse_delta = Pos2::new(mouse_pos.0 as f32, mouse_pos.1 as f32) - self.mouse_move_start;
                    // 应用偏移到原始位置
                    let mut new_rect = original_rect;
                    let mut new_mouse_rect = mouse_original_rect;
                    new_rect.min += delta;
                    new_rect.max += delta;

                    new_mouse_rect.min += mouse_delta;
                    new_mouse_rect.max += mouse_delta;

                    // 限制选择框在屏幕范围内
                    new_rect.min = new_rect.min.max(combined_bounds.min);
                    new_rect.max = new_rect.max.min(combined_bounds.max);

                    new_mouse_rect.min = new_mouse_rect.min.max(Pos2::new(0.0, 0.0));
                    new_mouse_rect.max = new_mouse_rect.max.min(Pos2::new(self.screen_with as f32, self.screen_height as f32));

                    // 确保选择框大小不变
                    let width = original_rect.width();
                    let height = original_rect.height();

                    let mouse_width = mouse_original_rect.width();
                    let mouse_height = mouse_original_rect.height();
                    // 限制选择框在屏幕范围内，考虑选择框的大小
                    let max_x = combined_bounds.max.x - width;
                    let max_y = combined_bounds.max.y - height;

                    let max_mouse_x = self.screen_with as f32 - mouse_width;
                    let max_mouse_y = self.screen_height as f32 - mouse_height;

                    new_rect.min.x = new_rect.min.x.clamp(combined_bounds.min.x, max_x);
                    new_rect.min.y = new_rect.min.y.clamp(combined_bounds.min.y, max_y);

                    new_mouse_rect.min.x = new_mouse_rect.min.x.clamp(0.0, max_mouse_x);
                    new_mouse_rect.min.y = new_mouse_rect.min.y.clamp(0.0, max_mouse_y);

                    // 根据调整后的min重新计算max
                    new_rect.max.x = new_rect.min.x + width;
                    new_rect.max.y = new_rect.min.y + height;

                    new_mouse_rect.max.x = new_mouse_rect.min.x + mouse_width;
                    new_mouse_rect.max.y = new_mouse_rect.min.y + mouse_height;

                    self.selection_rect = Some(new_rect);
                    self.mouse_selection_rect = Some(new_mouse_rect);

                    // ✅ 更新 selection_start 和 selection_end
                    self.selection_start = new_rect.min;
                    self.selection_end = new_rect.max;

                    self.mouse_start = new_mouse_rect.min;
                    self.mouse_end = new_mouse_rect.max;
                    println!("mouse_start: {:?}, mouse_end: {:?}", self.mouse_start, self.mouse_end);

                    // 更新工具栏位置
                    self.update_toolbar_position(new_rect);
                }
            } else if let Some(ref mut annotation) = self.current_annotation {
                // 检查拖动点是否在选择区域内
                if let Some(selection_rect) = self.selection_rect {
                    if selection_rect.contains(pointer_pos) {
                        annotation.points.push(pointer_pos);
                    }
                }
            }
        }

        // 鼠标释放
        if ui.input(|i| i.pointer.primary_released()) {
            if self.is_selecting {
                self.is_selecting = false;
                self.selection_end = pointer_pos;
                self.mouse_end = Pos2::new(mouse_pos.0 as f32, mouse_pos.1 as f32);
                self.update_selection_rect();
                if let Some(rect) = self.selection_rect {
                    if rect.area() > 100.0 { // 最小区域阈值
                        self.show_toolbar = true;
                        self.update_toolbar_position(rect);
                    }
                }
            } else if self.is_moving_box && self.current_tool == Tool::MoveBox {
                // let mut current_pos = mouse_pos;
                // let x_distance = current_pos.0 - self.mouse_move_start.0;
                // let y_distance = current_pos.1 - self.mouse_move_start.1;
                // println!("mouse_start: {:?}, mouse_end: {:?}", self.mouse_start, self.mouse_end);
                // println!("x_distance: {}, y_distance: {}", x_distance, y_distance);
                // self.mouse_start.0 += x_distance;
                // self.mouse_start.1 += y_distance;
                // self.mouse_end.0 += x_distance;
                // self.mouse_end.1 += y_distance;
                // self.mouse_start.0 = self.mouse_start.0.min((self.screen_with - 1) as i32);
                // self.mouse_start.0 = self.mouse_start.0.max(0);
                // self.mouse_start.1 = self.mouse_start.1.min((self.screen_height - 1) as i32);
                // self.mouse_start.1 = self.mouse_start.1.max(0);
                // self.mouse_end.0 = self.mouse_end.0.min((self.screen_with - 1) as i32);
                // self.mouse_end.0 = self.mouse_end.0.max(0);
                // self.mouse_end.1 = self.mouse_end.1.min((self.screen_height - 1) as i32);
                // self.mouse_end.1 = self.mouse_end.1.max(0);
                //
                // println!("mouse_start: {:?}, mouse_end: {:?}", self.mouse_start, self.mouse_end);
                self.is_moving_box = false;
                self.show_toolbar = true;
                self.original_selection_rect = None;
                self.mouse_original_selection_rect = None;
            } else if let Some(annotation) = &self.current_annotation {
                // 对于非文本工具，直接完成标注
                if self.current_tool != Tool::Text {
                    if annotation.points.len() > 1 {
                        self.annotations.push(annotation.clone());
                    }
                    self.current_annotation = None;
                }
                // 文本工具的完成由文本输入处理
            }
        }

        // ESC 键退出
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some(text_state) = &mut self.text_input {
                if text_state.is_active {
                    self.text_input = None;
                    return;
                }
            }
            if self.show_toolbar {
                self.show_toolbar = false;
                self.selection_rect = None;
                self.mouse_selection_rect = None;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn handle_text_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(text_state) = &mut self.text_input.clone() {
            if !text_state.is_active {
                return;
            }

            // 确保焦点
            if !text_state.has_focus {
                ui.memory_mut(|mem| mem.request_focus(text_state.widget_id));
                text_state.has_focus = true;
            }

            // 处理键盘输入
            ctx.input(|input| {
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        if !text.chars().next().map_or(false, |c| c.is_control()) {
                            text_state.text.push_str(text);
                        }
                    }
                }

                if input.key_pressed(egui::Key::Enter) {
                    if input.modifiers.ctrl {
                        // Ctrl+Enter 完成输入
                        self.finalize_text_input();
                    } else {
                        // 普通回车换行
                        text_state.text.push('\n');
                    }
                }

                if input.key_pressed(egui::Key::Backspace) {
                    text_state.text.pop();
                }
            });

            // ESC 键取消
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel_text_input();
            }
        }
    }

    fn finalize_text_input(&mut self) {
        if let Some(text_state) = self.text_input.take() {
            if let Some(mut annotation) = self.current_annotation.take() {
                annotation.text = text_state.text;
                if !annotation.text.trim().is_empty() || annotation.points.len() > 1 {
                    self.annotations.push(annotation);
                }
            }
        }
        self.text_input_finalized = false;
    }

    fn cancel_text_input(&mut self) {
        self.text_input = None;
        self.current_annotation = None;
        self.text_input_finalized = false;
    }

    fn update_selection_rect(&mut self) {
        let min_x = self.selection_start.x.min(self.selection_end.x);
        let min_y = self.selection_start.y.min(self.selection_end.y);
        let max_x = self.selection_start.x.max(self.selection_end.x);
        let max_y = self.selection_start.y.max(self.selection_end.y);

        let mouse_min_x = self.mouse_start.x.min(self.mouse_end.x);
        let mouse_min_y = self.mouse_start.y.min(self.mouse_end.y);
        let mouse_max_x = self.mouse_start.x.max(self.mouse_end.x);
        let mouse_max_y = self.mouse_start.y.max(self.mouse_end.x);

        self.mouse_selection_rect = Some(Rect::from_min_max(
            Pos2::new(mouse_min_x, mouse_min_y),
            Pos2::new(mouse_max_x, mouse_max_y),
        ));

        self.selection_rect = Some(Rect::from_min_max(
            Pos2::new(min_x, min_y),
            Pos2::new(max_x, max_y),
        ));
        // 框选完成后，设置默认工具为 MoveBox
        if self.selection_rect.is_some() && self.current_tool != Tool::MoveBox {
            self.current_tool = Tool::MoveBox;
        }
    }

    fn update_toolbar_position(&mut self, selection_rect: Rect) {
        // 工具栏显示在选择框右下角
        self.toolbar_position = Pos2::new(
            selection_rect.max.x,
            selection_rect.max.y,
        );
    }

    fn start_annotation(&mut self, pos: Pos2) {
        self.current_annotation = Some(Annotation {
            tool: self.current_tool,
            points: vec![pos],
            color: self.annotation_color,
            stroke_width: self.brush_size,
            text: "".to_string(),
        });
    }
}

// 标注和工具栏
impl ScreenshotApp {
    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        if let Some(selection_rect) = self.selection_rect {
            let toolbar_size = Vec2::new(100.0, 40.0);
            // 计算工具栏位置：在选择框右下角，并与选择框右对齐
            let mut toolbar_pos = Pos2::new(
                selection_rect.max.x - toolbar_size.x, // 右对齐：工具栏右侧与选择框右侧对齐
                selection_rect.max.y,                  // 在选择框下方
            );

            // 确保工具栏在屏幕内
            let screen_rect = ctx.viewport_rect();
            
            // 如果工具栏超出右边界，向左调整
            if toolbar_pos.x + toolbar_size.x > screen_rect.max.x {
                toolbar_pos.x = screen_rect.max.x - toolbar_size.x;
            }
            
            // 如果工具栏超出左边界，确保至少显示一部分
            if toolbar_pos.x < screen_rect.min.x {
                toolbar_pos.x = screen_rect.min.x;
            }
            
            // 如果工具栏超出下边界，显示在选择框上方
            if toolbar_pos.y + toolbar_size.y > screen_rect.max.y {
                toolbar_pos.y = selection_rect.min.y - toolbar_size.y;
            }
            
            // 如果工具栏超出上边界，确保至少显示一部分
            if toolbar_pos.y < screen_rect.min.y {
                toolbar_pos.y = screen_rect.min.y;
            }


            egui::Area::new(Id::from("annotation_toolbar".to_string()))
                .fixed_pos(toolbar_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // 工具选择
                                self.tool_button(ui, Tool::MoveBox, "↔");
                                self.tool_button(ui, Tool::Brush, "✎");
                                self.tool_button(ui, Tool::Rectangle, "□");
                                self.tool_button(ui, Tool::Arrow, "→");
                                self.tool_button(ui, Tool::Text, "T");

                                ui.separator();

                                // 颜色选择
                                ui.color_edit_button_srgba(&mut self.annotation_color);

                                // 画笔大小
                                // ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=20.0));

                                ui.separator();


                                if ui.button("copy").clicked() {
                                    self.copy_to_clipboard();
                                }

                            });
                        });
                });
        }
    }

    fn tool_button(&mut self, ui: &mut egui::Ui, tool: Tool, icon: &str) -> egui::Response {
        let is_selected = self.current_tool == tool;
        let response = ui.selectable_label(is_selected, icon);

        if response.clicked() {
            self.current_tool = tool;
        }

        response
    }

    fn draw_annotations(&self, ui: &mut egui::Ui) {
        let painter = ui.painter();

        for annotation in &self.annotations {
            self.draw_single_annotation(painter, annotation);
        }

        if let Some(annotation) = &self.current_annotation {
            self.draw_single_annotation(painter, annotation);
        }
    }

    // 文本输入
    fn draw_text_input(&mut self, ui: &mut egui::Ui) {
        if let Some(text_state) = &mut self.text_input {
            if text_state.is_active {
                let max_x = self.selection_end.x;
                let current_x = text_state.position.x;
                let desired_width = (max_x - current_x).max(max_x - current_x);
                // 创建文本输入区域
                let _text_response = egui::Area::new(text_state.widget_id)
                    .fixed_pos(text_state.position)
                    .order(egui::Order::Foreground)
                    .show(ui.ctx(), |ui| {
                        // egui::Frame::window(ui.style())
                        egui::Frame::NONE
                            .show(ui, |ui| {
                                let text_edit = egui::TextEdit::multiline(&mut text_state.text)
                                    .font(egui::FontId::proportional(16.0))
                                    .desired_width(desired_width)
                                    .desired_rows(1)
                                    .min_size(Vec2::ZERO)
                                    .frame(true)
                                    .text_color(self.annotation_color)
                                    .lock_focus(true)
                                    .hint_text("")
                                    .id(text_state.widget_id);
                                // 设置焦点
                                if !text_state.has_focus {
                                    ui.memory_mut(|mem| mem.request_focus(text_state.widget_id));
                                    text_state.has_focus = true;
                                }
                                // 更新输入框位置
                                if let Some(annotation) = &self.current_annotation {
                                    if let Some(&pos) = annotation.points.first() {
                                        if text_state.position != pos {
                                            text_state.position = pos;
                                        }
                                    }
                                }
                                let response = ui.add(text_edit);
                                response
                            }).inner
                    }).response;
                // 确保光标持续可见
                if text_state.has_focus {
                    ui.ctx().request_repaint(); // 确保光标闪烁动画持续
                }
            }
        }
    }

    fn draw_single_annotation(&self, painter: &egui::Painter, annotation: &Annotation) {
        if annotation.points.len() < 2 {
            return;
        }

        let stroke = Stroke::new(annotation.stroke_width, annotation.color);
        match annotation.tool {
            Tool::Brush => {
                // 绘制自由画笔
                for window in annotation.points.windows(2) {
                    if let [start, end] = window {
                        painter.line_segment([*start, *end], stroke);
                    }
                }
            }
            Tool::Rectangle => {
                // 绘制矩形
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    let rect = Rect::from_two_pos(start, end);
                    painter.rect_stroke(rect, egui::CornerRadius::ZERO, stroke, StrokeKind::Middle);
                }
            }
            Tool::Arrow => {
                // 绘制箭头
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    // painter.arrow(start, end - start, stroke);
                    // 1️⃣ 先画箭杆（线，和原来一样）
                    painter.line_segment([start, end], stroke);

                    // 2️⃣ 计算箭头头的三个点（实心三角形！）
                    let dir = end - start;
                    let dir_len = dir.length();
                    if dir_len < 1.0 { return; } // 防止太短

                    let dir_norm = dir / dir_len; // 方向单位向量

                    // ✅ 修正：手动计算垂直向量（egui中没有perp()方法）
                    let perp = Vec2::new(-dir_norm.y, dir_norm.x); // 旋转90度

                    // 箭头尺寸（可调，我设了10x5，你按需改）
                    let arrow_length = 15.0;
                    let arrow_width = 5.0;

                    let tip = end; // 箭头尖端
                    let left = end - dir_norm * arrow_length + perp * arrow_width;
                    let right = end - dir_norm * arrow_length - perp * arrow_width;

                    // 3️⃣ 用凸多边形实心填充箭头头
                    painter.add(Shape::convex_polygon(
                        vec![tip, left, right],
                        annotation.color,
                        Stroke::NONE,
                    ));
                }
            }
            Tool::Text => {
                // 显示已保存的文本（仅在文本输入不活动时）
                if let Some(&pos) = annotation.points.first() {
                    if !annotation.text.is_empty() {
                        // 按换行符分割文本
                        let lines: Vec<&str> = annotation.text.lines().collect();
                        let line_height = 16.0; // 与字体大小一致
                        // 逐行绘制
                        for (i, line) in lines.iter().enumerate() {
                            painter.text(
                                Pos2::new(
                                    pos.x + 4f32,
                                    pos.y + (i as f32) * line_height + 2f32, // 逐行下移
                                ),
                                // x和y加的4和2为为了避免文本向左上角移动
                                egui::Align2::LEFT_TOP,
                                line.to_string(),
                                egui::FontId::proportional(16.0),
                                annotation.color,
                            );
                        }
                        // painter.text(
                        //     pos,
                        //     egui::Align2::LEFT_TOP,
                        //     annotation.text.clone(),
                        //     egui::FontId::proportional(16.0),
                        //     annotation.color,
                        // );
                    }
                }
            }
            _ => {}
        }
    }
}

// 处理截图
impl ScreenshotApp {

    // fn save_screenshot(&self, ctx: &egui::Context) {
        // if let Some(selection_rect) = self.selection_rect {
        //     if let Some(cropped_image) = self.crop_selection(selection_rect, &self.annotations) {
        //         // 使用文件对话框选择保存位置
        //         let task = rfd::AsyncFileDialog::new()
        //             .set_title("save")
        //             .add_filter("PNG", &["png".to_string()])
        //             .add_filter("JPEG", &["jpg".to_string(), "jpeg".to_string()])
        //             .save_file();
        //
        //         let ctx = ctx.clone();
        //         wasm_bindgen_futures::spawn_local(async move {
        //             if let Some(file) = task.await {
        //                 let path = file.path();
        //                 if let Err(e) = cropped_image.save(path.to_path_buf()) {
        //                     eprintln!("Failed to save screenshot: {}", e);
        //                 }
        //             }
        //             ctx.request_repaint();
        //         });
        //     }
        // }
    // }

    fn copy_to_clipboard(&self) {
        if let Some(selection_rect) = self.selection_rect {

            // 确保当前文本输入完成
            let mut annotations = self.annotations.clone();
            if let Some(text_state) = &self.text_input {
                if let Some(annotation) = &self.current_annotation {
                    let mut new_annotation = annotation.clone();
                    new_annotation.text = text_state.text.clone();
                    annotations.push(new_annotation);
                }
            }

            if let Some(cropped_image) = self.crop_selection(selection_rect, &annotations) {
                // 转换为剪贴板格式
                if let Ok(mut clipboard) = Clipboard::new() {
                    let image_data = arboard::ImageData {
                        width: cropped_image.width() as usize,
                        height: cropped_image.height() as usize,
                        bytes: std::borrow::Cow::Borrowed(&cropped_image.as_raw()),
                    };

                    if let Err(e) = clipboard.set_image(image_data) {
                        eprintln!("Failed to copy to clipboard: {}", e);
                    }
                }
            }
        }
    }

    fn crop_selection(&self, selection_rect: Rect, annotations: &Vec<Annotation>) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {

        let x = self.mouse_start.x as u32;
        let y = self.mouse_start.y as u32;
        let width = self.mouse_end.x as u32 - x;
        let height = self.mouse_end.y as u32 - y;
        // 创建新的图像缓冲区 - 直接使用选择框的大小
        let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        // 用白色背景填充
        for pixel in cropped_image.pixels_mut() {
            *pixel = Rgba([255, 255, 255, 255]);
        }

        // 查找包含选择区域的屏幕
        for (screen, screenshot) in self.screens.iter().zip(&self.screenshots) {
            let screen_rect = get_screen_rect(screen);

            if screen_rect.contains(selection_rect.center()) {
                // 计算在屏幕图像中的相对位置

                if width > 0 && height > 0 {
                    // 创建新的图像缓冲区
                    let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

                    // 复制原始截图内容
                    for src_y in y..(y + height) {
                        for src_x in x..(x + width) {
                            let dst_x = src_x - x;
                            let dst_y = src_y - y;

                            let pixel = screenshot.get_pixel(src_x, src_y);
                            cropped_image.put_pixel(dst_x, dst_y, pixel.clone());
                        }
                    }

                    // 添加标注内容
                    self.add_annotations_to_image(&mut cropped_image, selection_rect, annotations);

                    return Some(cropped_image);
                }
            }
        }
        None
    }

    fn add_annotations_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, selection_rect: Rect, annotations: &Vec<Annotation>) {
        for annotation in annotations {
            self.draw_single_annotation_to_image(image, annotation, selection_rect);
        }
    }

    fn draw_single_annotation_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotation: &Annotation, selection_rect: Rect) {
        if annotation.points.len() < 2 {
            return;
        }

        // 转换为相对于裁剪图像的坐标
        let rel_rect = Rect::from_min_size(
            Pos2::new(selection_rect.min.x, selection_rect.min.y),
            Vec2::new(selection_rect.width(), selection_rect.height())
        );

        let color = annotation.color;

        match annotation.tool {
            Tool::Brush => {
                // 绘制自由画笔
                for window in annotation.points.windows(2) {
                    if let [start, end] = window {
                        let start_rel = Pos2::new(start.x - rel_rect.min.x, start.y - rel_rect.min.y);
                        let end_rel = Pos2::new(end.x - rel_rect.min.x, end.y - rel_rect.min.y);

                        // 绘制线条
                        // 由于代码较长，这里只展示基本逻辑
                        let mut x = start_rel.x as usize;
                        let mut y = start_rel.y as usize;
                        let dx = end_rel.x - start_rel.x;
                        let dy = end_rel.y - start_rel.y;
                        let steps = (dx.hypot(dy)).ceil() as usize;

                        for _ in 0..steps {
                            if x < image.width() as usize && y < image.height() as usize {
                                image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                            }
                            x += dx.signum() as usize;
                            y += dy.signum() as usize;
                        }
                    }
                }
            }
            Tool::Rectangle => {
                // 绘制矩形
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    let rect = Rect::from_two_pos(start, end);
                    let rect_rel = Rect::from_min_max(
                        Pos2::new(rect.min.x - rel_rect.min.x, rect.min.y - rel_rect.min.y),
                        Pos2::new(rect.max.x - rel_rect.min.x, rect.max.y - rel_rect.min.y)
                    );

                    // 绘制矩形边框
                    let x1 = rect_rel.min.x as usize;
                    let y1 = rect_rel.min.y as usize;
                    let x2 = rect_rel.max.x as usize;
                    let y2 = rect_rel.max.y as usize;

                    // 顶部和底部边
                    for x in x1..=x2 {
                        if y1 < image.height() as usize {
                            image.put_pixel(x as u32, y1 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                        if y2 < image.height() as usize {
                            image.put_pixel(x as u32, y2 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                    }

                    // 左侧和右侧边
                    for y in y1..=y2 {
                        if x1 < image.width() as usize {
                            image.put_pixel(x1 as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                        if x2 < image.width() as usize {
                            image.put_pixel(x2 as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                    }
                }
            }
            Tool::Arrow => {
                // 绘制箭头
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    let start_rel = Pos2::new(start.x - rel_rect.min.x, start.y - rel_rect.min.y);
                    let end_rel = Pos2::new(end.x - rel_rect.min.x, end.y - rel_rect.min.y);

                    // 绘制箭头线
                    let mut x = start_rel.x as usize;
                    let mut y = start_rel.y as usize;
                    let dx = (end_rel.x - start_rel.x) as f32;
                    let dy = (end_rel.y - start_rel.y) as f32;
                    let steps = (dx.hypot(dy)).ceil() as usize;

                    for _ in 0..steps {
                        if x < image.width() as usize && y < image.height() as usize {
                            image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                        x += dx.signum() as usize;
                        y += dy.signum() as usize;
                    }

                    // 绘制箭头头
                    let arrow_head_length = 5;

                    // 计算箭头头的位置
                    let arrow_head_x = end_rel.x as usize - (arrow_head_length as f32 * dx.signum()).round() as usize;
                    let arrow_head_y = end_rel.y as usize - (arrow_head_length as f32 * dy.signum()).round() as usize;

                    // 绘制箭头头
                    if arrow_head_x < image.width() as usize && arrow_head_y < image.height() as usize {
                        image.put_pixel(arrow_head_x as u32, arrow_head_y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }
            Tool::Text => {
                // 绘制文本
                if let Some(&pos) = annotation.points.first() {
                    let _pos_rel = Pos2::new(pos.x - rel_rect.min.x, pos.y - rel_rect.min.y);

                    if !annotation.text.is_empty() {
                        // 这里应添加文本绘制逻辑
                        // 由于文本绘制较复杂，这里省略具体实现
                    }
                }
            }
            _ => {}
        }
    }
}