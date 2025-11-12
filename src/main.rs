mod annotation;
mod app;

use std::borrow::Cow;

use arboard::{Clipboard, ImageData};
use eframe::{
    App, CreationContext,
    egui::{
        self, Align2, Color32, CursorIcon, Key, Pos2, Rect, Sense, Shape, Stroke, Vec2,
        ViewportCommand, pos2, vec2,
    },
};
use image::{ImageBuffer, Rgba};
use imageproc::drawing::{draw_hollow_rect_mut, draw_line_segment_mut};
use screenshots::Screen;
use rfd::FileDialog;

fn main() -> eframe::Result<()> {
    let screens = Screen::all().expect("获取屏幕列表失败");
    let screen = screens
        .into_iter()
        .max_by_key(|s| {
            let info = s.display_info;
            (info.width as u64) * (info.height as u64)
        })
        .expect("未找到显示器");

    let capture = screen.capture().expect("捕获屏幕失败");
    let width = capture.width() as usize;
    let height = capture.height() as usize;
    let rgba: Vec<u8> = capture.into();

    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_fullscreen(true)
        .with_maximized(true)
        .with_inner_size(Vec2::new(width as f32, height as f32))
        .with_decorations(false)
        .with_always_on_top()
        .with_maximize_button(false)
        .with_minimize_button(false)
        .with_close_button(false)
        .with_transparent(false);
    options.renderer = eframe::Renderer::Glow;

    let mut initial_pixels = Some(rgba);

    eframe::run_native(
        "Rust 截图工具",
        options,
        Box::new(move |cc| {
            let pixels = initial_pixels.take().expect("应用创建函数应当只被调用一次");
            Box::new(ScreenshotApp::new(cc, pixels, width, height))
        }),
    )?;

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Selecting,
    Annotating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrawingTool {
    Pen,
    Rectangle,
}

#[derive(Clone)]
struct Annotation {
    tool: DrawingTool,
    points: Vec<Pos2>,
    stroke: Stroke,
}

impl Annotation {
    fn new(tool: DrawingTool, stroke: Stroke) -> Self {
        Self {
            tool,
            points: Vec::new(),
            stroke,
        }
    }
}

struct ScreenshotApp {
    mode: Mode,
    base_rgba: Vec<u8>,
    full_texture: egui::TextureHandle,
    crop_texture: Option<egui::TextureHandle>,
    crop_rgba: Option<(Vec<u8>, usize, usize)>,
    selection_start: Option<Pos2>,
    selection_rect: Option<Rect>,
    annotations: Vec<Annotation>,
    drawing_tool: DrawingTool,
    drawing_color: Color32,
    drawing_width: f32,
    current_annotation: Option<Annotation>,
    screen_width: usize,
    screen_height: usize,
    status_message: Option<String>,
}

impl ScreenshotApp {
    fn new(
        cc: &CreationContext<'_>,
        base_rgba: Vec<u8>,
        screen_width: usize,
        screen_height: usize,
    ) -> Self {
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([screen_width, screen_height], &base_rgba);
        let texture = cc.egui_ctx.load_texture(
            "full_screenshot",
            color_image,
            egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
            },
        );

        cc.egui_ctx.set_pixels_per_point(1.0);

        Self {
            mode: Mode::Selecting,
            base_rgba,
            full_texture: texture,
            crop_texture: None,
            crop_rgba: None,
            selection_start: None,
            selection_rect: None,
            annotations: Vec::new(),
            drawing_tool: DrawingTool::Pen,
            drawing_color: Color32::from_rgb(255, 0, 0),
            drawing_width: 3.0,
            current_annotation: None,
            screen_width,
            screen_height,
            status_message: None,
        }
    }

    fn reset_annotations(&mut self) {
        self.annotations.clear();
        self.current_annotation = None;
    }

    fn selection_phase(&mut self, ctx: &egui::Context) {
        let screen_size = Vec2::new(self.screen_width as f32, self.screen_height as f32);

        let frame = egui::Frame::none().fill(Color32::BLACK);
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.set_min_size(screen_size);
            let rect = ui.max_rect();
            let response = ui.allocate_rect(rect, Sense::click_and_drag());
            let painter = ui.painter_at(rect);

            painter.image(
                self.full_texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            if response.hovered() {
                ctx.output_mut(|o| o.cursor_icon = CursorIcon::Crosshair);
            }

            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    self.selection_start = Some(pos);
                }
            }

            if response.dragged() {
                if let (Some(start), Some(current)) =
                    (self.selection_start, response.interact_pointer_pos())
                {
                    self.selection_rect = Some(Rect::from_two_pos(start, current));
                }
            }

            if response.drag_released() {
                if let Some(rect) = self.selection_rect {
                    let min = Pos2::new(rect.min.x.min(rect.max.x), rect.min.y.min(rect.max.y));
                    let max = Pos2::new(rect.min.x.max(rect.max.x), rect.min.y.max(rect.max.y));
                    let clipped = Rect::from_min_max(min, max);
                    if clipped.width() >= 4.0 && clipped.height() >= 4.0 {
                        let crop = self.crop_rgba_from_rect(&clipped);
                        if let Some((crop_rgba, crop_w, crop_h)) = crop {
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [crop_w, crop_h],
                                &crop_rgba,
                            );
                            self.crop_texture = Some(ctx.load_texture(
                                "selection",
                                color_image,
                                egui::TextureOptions::LINEAR,
                            ));
                            self.crop_rgba = Some((crop_rgba, crop_w, crop_h));
                            self.mode = Mode::Annotating;
                            self.reset_annotations();
                            ctx.send_viewport_cmd(ViewportCommand::Decorations(false));
                            ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
                                crop_w as f32 + 400.0,
                                crop_h as f32 + 200.0,
                            )));
                            return;
                        }
                    }
                }
                self.selection_start = None;
                self.selection_rect = None;
            }

            if let Some(rect) = self.selection_rect {
                painter.rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::from_rgb(0, 120, 215)));
                painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(0, 120, 215, 40));
            }
        });
    }

    fn crop_rgba_from_rect(&self, rect: &Rect) -> Option<(Vec<u8>, usize, usize)> {
        let min = rect.min;
        let max = rect.max;
        let width = (max.x - min.x).round().max(1.0) as usize;
        let height = (max.y - min.y).round().max(1.0) as usize;
        let full_width = self.screen_width;
        let full_height = self.screen_height;

        if min.x < 0.0 || min.y < 0.0 || max.x > full_width as f32 || max.y > full_height as f32 {
            return None;
        }

        let start_x = min.x.round() as usize;
        let start_y = min.y.round() as usize;

        let mut cropped = vec![0u8; width * height * 4];

        for y in 0..height {
            let src_y = start_y + y;
            let src_row_start = (src_y * full_width + start_x) * 4;
            let dst_row_start = y * width * 4;
            let src = &self.base_rgba[src_row_start..src_row_start + width * 4];
            let dst = &mut cropped[dst_row_start..dst_row_start + width * 4];
            dst.copy_from_slice(src);
        }

        Some((cropped, width, height))
    }

    fn annotate_phase(&mut self, ctx: &egui::Context) {
        let Some((ref crop_rgba, crop_w, crop_h)) = self.crop_rgba else {
            return;
        };
        if self.crop_texture.is_none() {
            let color_image = egui::ColorImage::from_rgba_unmultiplied([crop_w, crop_h], crop_rgba);
            self.crop_texture =
                Some(ctx.load_texture("selection", color_image, egui::TextureOptions::LINEAR));
        }
        let texture = self
            .crop_texture
            .as_ref()
            .expect("裁剪纹理应当已初始化")
            .clone();
        let toolbar_width = 200.0;
        let toolbar_height = 300.0;
        egui::Window::new("")
            .anchor(Align2::RIGHT_BOTTOM, egui::Vec2::new(-10.0, -10.0))
            .resizable(false)
            .title_bar(false)
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(30, 30, 30, 200)))  // 半透明背景
            .show(ctx, |ui| {
                ui.set_min_size(Vec2::new(toolbar_width, toolbar_height));
                ui.set_max_size(Vec2::new(toolbar_width, toolbar_height));
                ui.horizontal(|ui| {
                    ui.label("tools：");
                    ui.selectable_value(&mut self.drawing_tool, DrawingTool::Pen, "✏");
                    ui.selectable_value(&mut self.drawing_tool, DrawingTool::Rectangle, "⬜");
                    ui.separator();
                    if ui.button("save as PNG").clicked() {
                        if let Err(err) = self.save_to_png() {
                            self.status_message = Some(format!("保存失败：{err}"));
                        } else {
                            self.status_message = Some("已保存为 PNG".to_string());
                        }
                    }
                    if ui.button("Copy to clipboard").clicked() {
                        if let Err(err) = self.copy_to_clipboard() {
                            self.status_message = Some(format!("复制失败：{err}"));
                        } else {
                            self.status_message = Some("截图已复制到剪贴板".to_string());
                        }
                    }
                    if ui.button("完成（复制并退出）").clicked() {
                        match self.copy_to_clipboard() {
                            Ok(_) => ctx.send_viewport_cmd(ViewportCommand::Close),
                            Err(err) => self.status_message = Some(format!("复制失败：{err}")),
                        }
                    }
                    ui.separator();
                    ui.label("color：");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut self.drawing_color,
                        egui::color_picker::Alpha::Opaque,
                    );
                    ui.separator();
                    ui.label("Line width：");
                    ui.add(egui::Slider::new(&mut self.drawing_width, 1.0..=12.0));
                    ui.separator();
                    if ui.add_enabled(self.crop_rgba.is_some(), egui::Button::new("重选")).clicked() {
                        self.mode = Mode::Selecting;
                        self.selection_rect = None;
                        self.selection_start = None;
                        self.crop_texture = None;
                        self.crop_rgba = None;
                        self.reset_annotations();
                        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
                        ctx.send_viewport_cmd(ViewportCommand::Decorations(false));
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(crop_w as f32, crop_h as f32),
                    Sense::click_and_drag(),
                );
                let painter = ui.painter_at(rect);
                let mut shapes: Vec<Shape> = Vec::new();

                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                if response.hovered() {
                    ctx.output_mut(|o| {
                        o.cursor_icon = match self.drawing_tool {
                            DrawingTool::Pen => CursorIcon::Crosshair,
                            DrawingTool::Rectangle => CursorIcon::Move,
                        };
                    });
                }

                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let local = pos - rect.min;
                        let mut ann = Annotation::new(
                            self.drawing_tool,
                            Stroke::new(self.drawing_width, self.drawing_color),
                        );
                        ann.points.push(pos2(local.x, local.y));
                        self.current_annotation = Some(ann);
                    }
                }

                if response.dragged() {
                    if let (Some(mut ann), Some(pos)) = (
                        self.current_annotation.take(),
                        response.interact_pointer_pos(),
                    ) {
                        let local = pos - rect.min;
                        match ann.tool {
                            DrawingTool::Pen => {
                                ann.points.push(pos2(local.x, local.y));
                            }
                            DrawingTool::Rectangle => {
                                if ann.points.len() == 1 {
                                    ann.points.push(pos2(local.x, local.y));
                                } else {
                                    *ann.points.last_mut().unwrap() = pos2(local.x, local.y);
                                }
                            }
                        }
                        self.current_annotation = Some(ann);
                    }
                }

                if response.drag_released() {
                    if let Some(ann) = self.current_annotation.take() {
                        if ann.points.len() >= 2 {
                            self.annotations.push(ann);
                        }
                    }
                }

                for ann in &self.annotations {
                    shapes.extend(self.annotation_shapes(ann, rect.min));
                }

                if let Some(ann) = &self.current_annotation {
                    shapes.extend(self.annotation_shapes(ann, rect.min));
                }

                painter.extend(shapes);
            });
        });

        if let Some(message) = self.status_message.clone() {
            egui::Window::new("操作反馈")
                .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
                .auto_sized()
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(message);
                    if ui.button("关闭").clicked() {
                        self.status_message = None;
                    }
                });
        }

        egui::TopBottomPanel::bottom("status")
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "分辨率：{} x {} | 标注数：{}",
                        crop_w,
                        crop_h,
                        self.annotations.len() + usize::from(self.current_annotation.is_some())
                    ));
                    ui.separator();
                    ui.label("快捷键：Ctrl+C 复制 | Ctrl+S 保存 | Enter 完成 | Esc 返回/退出");
                });
            });
    }

    fn annotation_shapes(&self, ann: &Annotation, origin: Pos2) -> Vec<Shape> {
        let mut shapes = Vec::new();
        match ann.tool {
            DrawingTool::Pen => {
                if ann.points.len() < 2 {
                    return shapes;
                }
                for segment in ann.points.windows(2) {
                    let p0 = origin + vec2(segment[0].x, segment[0].y);
                    let p1 = origin + vec2(segment[1].x, segment[1].y);
                    shapes.push(Shape::line_segment([p0, p1], ann.stroke));
                }
            }
            DrawingTool::Rectangle => {
                if ann.points.len() < 2 {
                    return shapes;
                }
                let top_left = origin + vec2(ann.points[0].x, ann.points[0].y);
                let bottom_right = origin + vec2(ann.points[1].x, ann.points[1].y);
                let rect = Rect::from_two_pos(top_left, bottom_right);
                shapes.push(Shape::rect_stroke(rect, 0.0, ann.stroke));
            }
        }
        shapes
    }

    fn copy_to_clipboard(&self) -> Result<(), String> {
        let Some((ref crop_rgba, crop_w, crop_h)) = self.crop_rgba else {
            return Err("尚未选择区域".into());
        };

        let mut buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
            crop_w as u32,
            crop_h as u32,
            crop_rgba.clone(),
        )
        .ok_or_else(|| "无法构建图像缓冲区".to_string())?;

        let annotations = self
            .annotations
            .iter()
            .chain(self.current_annotation.iter());

        for ann in annotations {
            match ann.tool {
                DrawingTool::Pen => {
                    for window in ann.points.windows(2) {
                        let p0 = window[0];
                        let p1 = window[1];
                        draw_line_segment_mut(
                            &mut buffer,
                            (p0.x, p0.y),
                            (p1.x, p1.y),
                            Rgba([
                                ann.stroke.color.r(),
                                ann.stroke.color.g(),
                                ann.stroke.color.b(),
                                ann.stroke.color.a(),
                            ]),
                        );
                    }
                }
                DrawingTool::Rectangle => {
                    if ann.points.len() >= 2 {
                        let min = Pos2::new(
                            ann.points[0].x.min(ann.points[1].x),
                            ann.points[0].y.min(ann.points[1].y),
                        );
                        let max = Pos2::new(
                            ann.points[0].x.max(ann.points[1].x),
                            ann.points[0].y.max(ann.points[1].y),
                        );
                        let width = (max.x - min.x).abs() as u32;
                        let height = (max.y - min.y).abs() as u32;
                        if width > 0 && height > 0 {
                            let rect = imageproc::rect::Rect::at(min.x as i32, min.y as i32)
                                .of_size(width, height);
                            let color = Rgba([
                                ann.stroke.color.r(),
                                ann.stroke.color.g(),
                                ann.stroke.color.b(),
                                ann.stroke.color.a(),
                            ]);
                            draw_hollow_rect_mut(&mut buffer, rect, color);
                        }
                    }
                }
            }
        }

        let bytes = buffer.clone().into_raw();
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        clipboard
            .set_image(ImageData {
                width: crop_w,
                height: crop_h,
                bytes: Cow::Owned(bytes),
            })
            .map_err(|e| e.to_string())
    }

    fn save_to_png(&self) -> Result<(), String> {
        let Some((_, crop_w, crop_h)) = self.crop_rgba.as_ref() else {
            return Err("尚未选择区域".into());
        };
        // 复用绘制后的图像
        let mut buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
            *crop_w as u32,
            *crop_h as u32,
            self.crop_rgba.as_ref().unwrap().0.clone(),
        )
        .ok_or_else(|| "无法构建图像缓冲区".to_string())?;

        let annotations = self
            .annotations
            .iter()
            .chain(self.current_annotation.iter());

        for ann in annotations {
            match ann.tool {
                DrawingTool::Pen => {
                    for window in ann.points.windows(2) {
                        let p0 = window[0];
                        let p1 = window[1];
                        draw_line_segment_mut(
                            &mut buffer,
                            (p0.x, p0.y),
                            (p1.x, p1.y),
                            Rgba([
                                ann.stroke.color.r(),
                                ann.stroke.color.g(),
                                ann.stroke.color.b(),
                                ann.stroke.color.a(),
                            ]),
                        );
                    }
                }
                DrawingTool::Rectangle => {
                    if ann.points.len() >= 2 {
                        let min = Pos2::new(
                            ann.points[0].x.min(ann.points[1].x),
                            ann.points[0].y.min(ann.points[1].y),
                        );
                        let max = Pos2::new(
                            ann.points[0].x.max(ann.points[1].x),
                            ann.points[0].y.max(ann.points[1].y),
                        );
                        let width = (max.x - min.x).abs() as u32;
                        let height = (max.y - min.y).abs() as u32;
                        if width > 0 && height > 0 {
                            let rect = imageproc::rect::Rect::at(min.x as i32, min.y as i32)
                                .of_size(width, height);
                            let color = Rgba([
                                ann.stroke.color.r(),
                                ann.stroke.color.g(),
                                ann.stroke.color.b(),
                                ann.stroke.color.a(),
                            ]);
                            draw_hollow_rect_mut(&mut buffer, rect, color);
                        }
                    }
                }
            }
        }

        let path = FileDialog::new()
            .set_title("保存截图为 PNG")
            .add_filter("PNG 图片", &["png"])
            .set_file_name("screenshot.png")
            .save_file()
            .ok_or_else(|| "用户取消".to_string())?;

        image::save_buffer(
            &path,
            &buffer.into_raw(),
            *crop_w as u32,
            *crop_h as u32,
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
    }
}

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // 快捷键
        let ctrl = ctx.input(|i| i.modifiers.ctrl);
        if ctrl && ctx.input(|i| i.key_pressed(Key::C)) {
            if let Err(err) = self.copy_to_clipboard() {
                self.status_message = Some(format!("复制失败：{err}"));
            } else {
                self.status_message = Some("截图已复制到剪贴板".to_string());
            }
        }
        if ctrl && ctx.input(|i| i.key_pressed(Key::S)) {
            if let Err(err) = self.save_to_png() {
                self.status_message = Some(format!("保存失败：{err}"));
            } else {
                self.status_message = Some("已保存为 PNG".to_string());
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            match self.copy_to_clipboard() {
                Ok(_) => ctx.send_viewport_cmd(ViewportCommand::Close),
                Err(err) => self.status_message = Some(format!("复制失败：{err}")),
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            match self.mode {
                Mode::Annotating => {
                    self.mode = Mode::Selecting;
                    self.selection_rect = None;
                    self.selection_start = None;
                    self.crop_texture = None;
                    self.crop_rgba = None;
                    self.reset_annotations();
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
                    ctx.send_viewport_cmd(ViewportCommand::Decorations(false));
                    self.status_message = None;
                }
                Mode::Selecting => {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }

        match self.mode {
            Mode::Selecting => self.selection_phase(ctx),
            Mode::Annotating => self.annotate_phase(ctx),
        }
    }
}
