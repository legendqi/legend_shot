use eframe::App;
use eframe::epaint::StrokeKind;
use egui::{Color32, Id, Pos2, Rect, Stroke, Vec2};
use image::{GenericImageView, ImageBuffer, Rgba};
use xcap::{Monitor};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Brush,
    Rectangle,
    Arrow,
    Text,
}

#[derive(Clone)]
struct Annotation {
    tool: Tool,
    points: Vec<Pos2>,
    color: Color32,
    stroke_width: f32,
}

pub struct ScreenshotApp {
    screens: Vec<Monitor>,
    screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    display_textures: Vec<egui::TextureHandle>,

    // 选择状态
    selection_rect: Option<Rect>,
    is_selecting: bool,
    selection_start: Pos2,
    selection_end: Pos2,

    // 标注状态
    current_tool: Tool,
    annotations: Vec<Annotation>,
    current_annotation: Option<Annotation>,
    brush_size: f32,
    annotation_color: Color32,

    // UI 状态
    show_toolbar: bool,
    toolbar_position: Pos2,
    // 修复：窗口尺寸
    window_rect: Rect,
}

impl Default for ScreenshotApp {
    fn default() -> Self {
        Self {
            screens: Vec::new(),
            screenshots: Vec::new(),
            display_textures: Vec::new(),
            selection_rect: None,
            is_selecting: false,
            selection_start: Pos2::ZERO,
            selection_end: Pos2::ZERO,
            current_tool: Tool::Select,
            annotations: Vec::new(),
            current_annotation: None,
            brush_size: 5.0,
            annotation_color: Color32::RED,
            show_toolbar: false,
            toolbar_position: Pos2::ZERO,
            window_rect: Rect::NOTHING,
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

            // 转换为 image crate 的格式
            let img_buffer = ImageBuffer::from_raw(
                image.width() as u32,
                image.height() as u32,
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.window_rect = ctx.viewport_rect();
        // 首次运行截图
        if self.screenshots.is_empty() {
            if let Err(e) = self.capture_screens(ctx) {
                eprintln!("Failed to capture screens: {}", e);
            }
        }
        // 设置全屏窗口
        // frame.set_fullscreen(true);

        // 主界面
        egui::Area::new(Id::from("screenshot_area".to_string()))
            .order(egui::Order::Background)
            .fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            self.draw_screens(ui);
            self.draw_overlay(ui);        // 覆盖层和选择框
            self.draw_annotations(ui);    // 标注在覆盖层之上
            self.handle_input(ui, ctx);   // 输入处理，包括更新选择框和标注

            if self.show_toolbar {
                self.draw_toolbar(ui, ctx);
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
        for (i, (screen, texture)) in self.screens.iter().zip(&self.display_textures).enumerate() {
            let screen_rect = get_screen_rect(screen);

            // 绘制屏幕截图
            ui.put(screen_rect, egui::Image::new(texture).fit_to_exact_size(screen_rect.size()));
        }
    }

    fn draw_overlay(&mut self, ui: &mut egui::Ui) {
        let combined_bounds = self.get_combined_bounds();

        // 绘制半透明灰色覆盖层
        ui.painter().add(egui::Shape::rect_filled(
            combined_bounds,
            egui::CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(0, 0, 0, 100),
        ));

        // 如果有选择区域，移除该区域的覆盖层
        if let Some(selection) = self.selection_rect {
            let selection_min = selection.min.max(combined_bounds.min);
            let selection_max = selection.max.min(combined_bounds.max);
            let clipped_selection = Rect::from_min_max(selection_min, selection_max);

            if clipped_selection.area() > 0.0 {
                ui.painter().add(egui::Shape::rect_filled(
                    clipped_selection,
                    egui::CornerRadius::ZERO,
                    Color32::TRANSPARENT,
                ));

                // 绘制选择框边框
                ui.painter().add(egui::Shape::rect_stroke(
                    clipped_selection,
                    egui::CornerRadius::ZERO,
                    Stroke::new(2.0, Color32::WHITE),
                    StrokeKind::Middle
                ));
            }
        }
    }

    fn handle_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pointer_pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO);

        // 鼠标按下开始选择
        if ui.input(|i| i.pointer.primary_pressed()) {
            if self.current_tool == Tool::Select && !self.show_toolbar {
                self.is_selecting = true;
                self.selection_start = pointer_pos;
                self.selection_end = pointer_pos;
            } else if self.current_tool != Tool::Select {
                // 开始标注
                self.start_annotation(pointer_pos);
            }
        }

        // 鼠标拖动
        if ui.input(|i| i.pointer.primary_down()) {
            if self.is_selecting {
                self.selection_end = pointer_pos;
                self.update_selection_rect();
            } else if let Some(ref mut annotation) = self.current_annotation {
                annotation.points.push(pointer_pos);
            }
        }

        // 鼠标释放
        if ui.input(|i| i.pointer.primary_released()) {
            if self.is_selecting {
                self.is_selecting = false;
                self.selection_end = pointer_pos;
                self.update_selection_rect();

                if let Some(rect) = self.selection_rect {
                    if rect.area() > 100.0 { // 最小区域阈值
                        self.show_toolbar = true;
                        self.update_toolbar_position(rect);
                    }
                }
            } else if let Some(annotation) = self.current_annotation.take() {
                if annotation.points.len() > 1 {
                    self.annotations.push(annotation);
                }
            }
        }

        // ESC 键退出
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.show_toolbar {
                self.show_toolbar = false;
                self.selection_rect = None;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn update_selection_rect(&mut self) {
        let min_x = self.selection_start.x.min(self.selection_end.x);
        let min_y = self.selection_start.y.min(self.selection_end.y);
        let max_x = self.selection_start.x.max(self.selection_end.x);
        let max_y = self.selection_start.y.max(self.selection_end.y);

        self.selection_rect = Some(Rect::from_min_max(
            Pos2::new(min_x, min_y),
            Pos2::new(max_x, max_y),
        ));
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
        });
    }
}

impl ScreenshotApp {
    fn draw_toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(selection_rect) = self.selection_rect {
            let toolbar_size = Vec2::new(550.0, 60.0);
            let mut toolbar_pos = self.toolbar_position;

            // 确保工具栏在屏幕内
            let screen_size = ctx.viewport_rect().size();
            if toolbar_pos.x + toolbar_size.x > screen_size.x {
                toolbar_pos.x = selection_rect.min.x - toolbar_size.x;
            }
            if toolbar_pos.y + toolbar_size.y > screen_size.y {
                toolbar_pos.y = selection_rect.min.y - toolbar_size.y;
            }



            let toolbar_rect = Rect::from_min_size(toolbar_pos, toolbar_size);

            egui::Area::new(Id::from("annotation_toolbar".to_string()))
                .fixed_pos(toolbar_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::window(ui.style())
                        .stroke(Stroke::new(1.0, Color32::GRAY))
                        .corner_radius(egui::CornerRadius::same(5.0 as u8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // 工具选择
                                self.tool_button(ui, Tool::Select, "⬚");
                                self.tool_button(ui, Tool::Brush, "✎");
                                self.tool_button(ui, Tool::Rectangle, "□");
                                self.tool_button(ui, Tool::Arrow, "→");
                                self.tool_button(ui, Tool::Text, "T");

                                ui.separator();

                                // 颜色选择
                                ui.color_edit_button_srgba(&mut self.annotation_color);

                                // 画笔大小
                                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=20.0));

                                ui.separator();

                                // 操作按钮
                                if ui.button("save").clicked() {
                                    self.save_screenshot(ctx);
                                }

                                if ui.button("copy").clicked() {
                                    self.copy_to_clipboard(ctx);
                                }

                                if ui.button("cancle").clicked() {
                                    self.show_toolbar = false;
                                    self.selection_rect = None;
                                    self.annotations.clear();
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
                    painter.arrow(start, end - start, stroke);
                }
            }
            Tool::Text => {
                // 绘制文本（简化版）
                if let Some(&pos) = annotation.points.first() {
                    painter.text(
                        pos,
                        egui::Align2::LEFT_TOP,
                        "Text".to_string(),
                        egui::FontId::proportional(14.0),
                        annotation.color,
                    );
                }
            }
            _ => {}
        }
    }
}

impl ScreenshotApp {
    fn save_screenshot(&self, ctx: &egui::Context) {
        if let Some(selection_rect) = self.selection_rect {
            if let Some(cropped_image) = self.crop_selection(selection_rect) {
                // 使用文件对话框选择保存位置
                let task = rfd::AsyncFileDialog::new()
                    .set_title("save")
                    .add_filter("PNG", &["png".to_string()])
                    .add_filter("JPEG", &["jpg".to_string(), "jpeg".to_string()])
                    .save_file();

                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(file) = task.await {
                        let path = file.path();
                        if let Err(e) = cropped_image.save(path.to_path_buf()) {
                            eprintln!("Failed to save screenshot: {}", e);
                        }
                    }
                    ctx.request_repaint();
                });
            }
        }
    }

    fn copy_to_clipboard(&self, ctx: &egui::Context) {
        if let Some(selection_rect) = self.selection_rect {
            if let Some(cropped_image) = self.crop_selection(selection_rect) {
                // 转换为剪贴板格式
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
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

    fn crop_selection(&self, selection_rect: Rect) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let x = selection_rect.min.x as u32;
        let y = selection_rect.min.y as u32;
        let width = selection_rect.width() as u32;
        let height = selection_rect.height() as u32;

        // 查找包含选择区域的屏幕
        for (screen, screenshot) in self.screens.iter().zip(&self.screenshots) {
            let screen_rect = get_screen_rect(screen);

            if screen_rect.contains(selection_rect.center()) {
                // 计算在屏幕图像中的相对位置
                let rel_x = (x - screen.x().unwrap() as u32).max(0);
                let rel_y = (y - screen.y().unwrap() as u32).max(0);
                let crop_width = width.min(screen.width().unwrap() - rel_x);
                let crop_height = height.min(screen.height().unwrap() - rel_y);

                if crop_width > 0 && crop_height > 0 {
                    return Some(screenshot.view(rel_x, rel_y, crop_width, crop_height).to_image());
                }
            }
        }

        None
    }
}