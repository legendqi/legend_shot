use crate::app_default::{Annotation, AppSignal, ScreenshotApp, Tool};
use crate::display::place_toolbar;
use crate::ui::{
    ARROW_ICON, COPY_ICON, EXIT_ICON, MOSAIC_ICON, MOVE_ICON, NUMBER_ICON, PEN_ICON,
    RECTANGLE_ICON, SAVE_ICON, UNDO_ICON, WORD_ICON, load_texture_from_png, ocr_button,
};
use eframe::emath::{Pos2, Rect, Vec2};
use eframe::epaint::{Color32, Hsva, Shape, Stroke, StrokeKind};
use egui::color_picker::color_picker_hsva_2d;
use egui::{Button, Id, Popup, PopupCloseBehavior, Response, Ui, ViewportId, color_picker};
use std::io::Write;
use std::time::Instant;

fn is_persistent_toolbar_tool(tool: Tool) -> bool {
    tool.is_annotation_tool() || matches!(tool, Tool::MoveBox | Tool::ColorPicker)
}

fn toolbar_width(item_spacing: f32) -> f32 {
    const ITEM_COUNT: f32 = 13.0;
    const CONTENT_WIDTH: f32 = 7.0 * 30.0 + 30.0 + 30.0 + 30.0 + 3.0 * 30.0;
    const HORIZONTAL_MARGIN: f32 = 20.0;

    CONTENT_WIDTH + (ITEM_COUNT - 1.0) * item_spacing + HORIZONTAL_MARGIN
}

fn draw_annotation_text_editor(
    ui: &mut Ui,
    text_state: &mut crate::app_default::TextInputState,
    desired_width: f32,
    color: Color32,
) -> Response {
    let viewport_focused = ui.input(|input| input.focused);
    if !viewport_focused {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        ui.ctx().request_repaint();
    }
    ui.style_mut().visuals.text_cursor.stroke.color = color;
    let response = ui.add(
        egui::TextEdit::multiline(&mut text_state.text)
            .font(egui::FontId::proportional(16.0))
            .desired_width(desired_width)
            .desired_rows(1)
            .min_size(Vec2::ZERO)
            .frame(false)
            .text_color(color)
            .hint_text("")
            .id(text_state.widget_id.with("editor")),
    );
    if !response.has_focus() {
        response.request_focus();
    }
    text_state.has_focus = response.has_focus() || response.gained_focus();
    response
}

impl ScreenshotApp {
    pub(crate) fn update_toolbar_placement(&mut self, endpoint: Pos2, ctx: &egui::Context) {
        let Some(session) = &self.capture_session else {
            self.toolbar_placement = None;
            return;
        };
        let Some(selection) = self.selection_rect else {
            self.toolbar_placement = None;
            return;
        };
        let geometries = session
            .displays
            .iter()
            .map(|display| display.geometry.clone())
            .collect::<Vec<_>>();
        let toolbar_size = Vec2::new(toolbar_width(ctx.style().spacing.item_spacing.x), 40.0);
        self.toolbar_placement =
            place_toolbar(&geometries, selection, endpoint, toolbar_size, 5.0).ok();
        if let Some(placement) = self.toolbar_placement {
            self.toolbar_position = placement.global_position;
        }
    }

    pub(crate) fn draw_annotations_for_display(&self, display_index: usize, ui: &mut Ui) {
        let Some(display) = self
            .capture_session
            .as_ref()
            .and_then(|session| session.displays.get(display_index))
        else {
            return;
        };
        let origin = display.geometry.logical_bounds.min.to_vec2();
        let painter = ui.painter();
        for annotation in self
            .annotations
            .iter()
            .chain(self.current_annotation.iter())
        {
            let mut local = annotation.clone();
            for point in &mut local.points {
                *point -= origin;
            }
            if local.tool == Tool::Mosaic {
                Self::draw_mosaic_annotation(
                    painter,
                    &local,
                    &display.original_image,
                    display.geometry.pixel_scale,
                );
            } else {
                self.draw_single_annotation(painter, &local);
            }
        }
    }

    fn draw_mosaic_annotation(
        painter: &egui::Painter,
        annotation: &Annotation,
        image: &image::RgbaImage,
        pixel_scale: Vec2,
    ) {
        let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last())
        else {
            return;
        };
        let rect = Rect::from_two_pos(start, end);
        let block_size = 4.0;
        let cols = (rect.width() / block_size).ceil() as usize;
        let rows = (rect.height() / block_size).ceil() as usize;
        for row in 0..rows {
            for col in 0..cols {
                let block_min = Pos2::new(
                    rect.min.x + col as f32 * block_size,
                    rect.min.y + row as f32 * block_size,
                );
                let sample_x = (block_min.x * pixel_scale.x).floor().max(0.0) as u32;
                let sample_y = (block_min.y * pixel_scale.y).floor().max(0.0) as u32;
                if sample_x < image.width() && sample_y < image.height() {
                    let pixel = image.get_pixel(sample_x, sample_y);
                    painter.rect_filled(
                        Rect::from_min_size(block_min, Vec2::splat(block_size)),
                        egui::CornerRadius::ZERO,
                        Color32::from_rgb(pixel[0], pixel[1], pixel[2]),
                    );
                }
            }
        }
    }

    pub(crate) fn draw_text_input_for_display(&mut self, display_index: usize, ui: &mut Ui) {
        let Some(display_bounds) = self
            .capture_session
            .as_ref()
            .and_then(|session| session.displays.get(display_index))
            .map(|display| display.geometry.logical_bounds)
        else {
            return;
        };
        let selection_max_x = self
            .selection_rect
            .map_or(display_bounds.max.x, |rect| rect.max.x);
        let color = self.annotation_color;
        let Some(text_state) = &mut self.text_input else {
            return;
        };
        if !text_state.is_active || !display_bounds.contains(text_state.position) {
            return;
        }
        let local_position = text_state.position - display_bounds.min.to_vec2();
        let desired_width = (selection_max_x.min(display_bounds.max.x) - text_state.position.x)
            .abs()
            .max(10.0);
        egui::Area::new(text_state.widget_id)
            .fixed_pos(local_position)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE.show(ui, |ui| {
                    draw_annotation_text_editor(ui, text_state, desired_width, color)
                });
            });
    }

    pub(crate) fn draw_toolbar_for_display(&mut self, display_index: usize, ctx: &egui::Context) {
        if !self.show_toolbar {
            return;
        }
        if let Some(placement) = self.toolbar_placement
            && self.selection_rect.is_some()
            && placement.display_index == display_index
        {
            let Some(display_origin) = self
                .capture_session
                .as_ref()
                .and_then(|session| session.displays.get(display_index))
                .map(|display| display.geometry.logical_bounds.min.to_vec2())
            else {
                return;
            };
            let toolbar_pos = placement.global_position - display_origin;
            self.toolbar_position = placement.global_position;
            let toolbar_id = Id::new("annotation_toolbar");
            egui::Area::new(toolbar_id)
                .fixed_pos(toolbar_pos)
                .order(egui::Order::Foreground)
                .interactable(true)
                .show(ctx, |ui| {
                    egui::Frame::NONE.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // 工具选择
                            self.purple_icon_button(ui, Tool::MoveBox, ctx, MOVE_ICON, "move");
                            self.purple_icon_button(ui, Tool::Pen, ctx, PEN_ICON, "pen");
                            self.purple_icon_button(
                                ui,
                                Tool::Rectangle,
                                ctx,
                                RECTANGLE_ICON,
                                "rectangle",
                            );
                            self.purple_icon_button(ui, Tool::Arrow, ctx, ARROW_ICON, "arrow");
                            self.purple_icon_button(ui, Tool::Text, ctx, WORD_ICON, "word");
                            self.purple_icon_button(ui, Tool::Mosaic, ctx, MOSAIC_ICON, "mosaic");
                            self.purple_icon_button(ui, Tool::Number, ctx, NUMBER_ICON, "number");

                            // 颜色选择
                            self.custom_color_picker(ui, ctx);

                            let undo_icon = load_texture_from_png(ctx, UNDO_ICON, "undo").unwrap();
                            let undo_button =
                                Button::new("").min_size(Vec2::new(30.0, 30.0)).frame(false);
                            let undo_response = ui.add_sized(Vec2::new(30.0, 30.0), undo_button);
                            let undo_hovered = undo_response.hovered() || undo_response.has_focus();
                            if undo_hovered {
                                self.tool_bar_focused = true;
                                ui.painter().circle_filled(
                                    undo_response.rect.center(),
                                    undo_response.rect.width() / 2.0,
                                    Color32::BLUE,
                                );
                            } else {
                                ui.painter().circle_filled(
                                    undo_response.rect.center(),
                                    undo_response.rect.width() / 2.0,
                                    Color32::from_rgb(0, 100, 255),
                                );
                            }
                            let undo_icon_size = Vec2::new(20.0, 20.0);
                            let undo_icon_rect =
                                Rect::from_center_size(undo_response.rect.center(), undo_icon_size);
                            ui.painter().image(
                                undo_icon,
                                undo_icon_rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                            if undo_response.clicked() {
                                if let Some(popped) = self.annotations.pop() {
                                    if popped.tool == Tool::Number {
                                        self.number_input = match self.number_input {
                                            Some(n) if n > 1 => Some(n - 1),
                                            _ => None,
                                        };
                                    }
                                }
                            }

                            // 操作： OCR，复制，保存，退出
                            let ocr_response = ocr_button(ui, ctx);
                            if ocr_response.hovered() || ocr_response.has_focus() {
                                self.tool_bar_focused = true;
                            }
                            if ocr_response.clicked() {
                                if let Err(error) =
                                    self.submit_ocr_for_current_selection(Instant::now())
                                {
                                    eprintln!("OCR 提交失败: {error}");
                                }
                            }

                            if self
                                .purple_icon_button(ui, Tool::Copy, ctx, COPY_ICON, "copy")
                                .clicked()
                            {
                                if !self.selection_rect.unwrap().contains(self.toolbar_position) {
                                    let _ = self.copy_to_clipboard();
                                    self.hide_capture_window(ctx);
                                } else {
                                    self.show_toolbar = false;
                                    ctx.request_repaint_of(ViewportId(toolbar_id));
                                    let sender_clone = self.signal_sender.clone();
                                    std::thread::spawn(move || {
                                        // 保存截图逻辑...
                                        // 保存完成后可能需要再次重绘
                                        if let Some(signal_sender) = sender_clone {
                                            // 发送信号给主窗口
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                20,
                                            ));
                                            signal_sender
                                                .lock()
                                                .unwrap()
                                                .send(AppSignal::Copy)
                                                .ok();
                                        }
                                    });
                                }
                            };
                            let save_response =
                                self.purple_icon_button(ui, Tool::Save, ctx, SAVE_ICON, "save");
                            if save_response.clicked() {
                                self.trigger_save_dialog(ctx);
                            }

                            self.purple_icon_button(ui, Tool::Exit, ctx, EXIT_ICON, "exit")
                                .clicked()
                                .then(|| {
                                    self.hide_capture_window(ctx);
                                    std::io::stdout().write_all("cancel".as_bytes()).unwrap();
                                    std::io::stdout().flush().unwrap();
                                });
                        });
                    });
                });
        }
    }

    fn custom_color_picker(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let button_size = Vec2::new(30.0, 30.0);

        // 创建自定义按钮
        let button = Button::new("").min_size(button_size).frame(false);
        let color_pick_response = ui.add(button);

        let is_hovered_or_focused =
            color_pick_response.hovered() || color_pick_response.has_focus();

        // 颜色选择
        let inner_radius = color_pick_response.rect.width() / 2.0;
        // 画边框（紫色，半径为 inner_radius + 2.0）
        if is_hovered_or_focused || self.current_tool == Tool::ColorPicker {
            self.tool_bar_focused = true;
            ui.painter().circle_filled(
                color_pick_response.rect.center(),
                inner_radius,
                Color32::from_rgb(128, 0, 128),
            );
        } else {
            ui.painter().circle_filled(
                color_pick_response.rect.center(),
                inner_radius,
                Color32::from_rgb(128, 80, 128),
            );
        }

        ui.painter().circle_filled(
            color_pick_response.rect.center(),
            inner_radius - 4.0,
            self.annotation_color,
        );
        // 👉 关键修复：生成ID放在点击逻辑外面（但确保在同一个UI帧）
        let popup_id = ui.auto_id_with("color_popup");
        let mut hsva = Hsva::from(self.annotation_color);
        Popup::menu(&color_pick_response)
            .id(popup_id)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.add_space(10.0); // 必须加，否则不会显示
                ui.spacing_mut().slider_width = 275.0;
                if color_picker_hsva_2d(ui, &mut hsva, color_picker::Alpha::BlendOrAdditive) {
                    self.annotation_color = Color32::from(hsva);
                }
            });
        // 处理点击：用 if 而不是 then！
        if color_pick_response.clicked() {
            self.current_tool = Tool::ColorPicker;
            Popup::open_id(ctx, popup_id);
        }
    }

    pub fn purple_icon_button(
        &mut self,
        ui: &mut Ui,
        tool: Tool,
        ctx: &egui::Context,
        icon_bytes: &[u8],
        icon_id: &str,
    ) -> Response {
        let icon = load_texture_from_png(ctx, icon_bytes, icon_id).unwrap();
        let selected = self.current_tool == tool;
        let button_size = Vec2::new(30.0, 30.0);

        // 创建自定义按钮
        let button = Button::new("").min_size(button_size).frame(false);
        // 根据状态设置按钮颜色
        let response = ui.add_sized(button_size, button);
        let is_hovered_or_focused = response.hovered() || response.has_focus();

        // 设置按钮填充颜色
        if selected && is_persistent_toolbar_tool(tool) {
            ui.painter().circle_filled(
                response.rect.center(),
                response.rect.width() / 2.0, // 圆角为0
                Color32::BLUE,
                // Color32::from_rgb(128, 0, 128), // 选中或悬停时为蓝色
            );
        } else if is_hovered_or_focused {
            self.tool_bar_focused = true;
            ui.painter().circle_filled(
                response.rect.center(),
                response.rect.width() / 2.0, // 圆角为0
                Color32::BLUE,               // 选中或悬停时为蓝色
            );
        } else {
            ui.painter().circle_filled(
                response.rect.center(),
                response.rect.width() / 2.0,    // 圆角为0
                Color32::from_rgb(0, 100, 255), // 初始状态为淡紫色
            );
        }
        // 绘制图标
        let icon_size = Vec2::new(20.0, 20.0);
        let icon_rect = Rect::from_center_size(response.rect.center(), icon_size);
        ui.painter().image(
            icon,
            icon_rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if response.clicked() {
            self.current_tool = tool;
        }
        response
    }

    pub(crate) fn draw_annotations(&self, ui: &mut Ui) {
        let painter = ui.painter();

        for annotation in &self.annotations {
            if annotation.tool == Tool::Mosaic {
                self.draw_single_annotation(painter, annotation);
            }
        }
        for annotation in &self.annotations {
            if annotation.tool != Tool::Mosaic {
                self.draw_single_annotation(painter, annotation);
            }
        }

        if let Some(annotation) = &self.current_annotation {
            self.draw_single_annotation(painter, annotation);
        }
    }

    // 文本输入
    pub(crate) fn draw_text_input(&mut self, ui: &mut Ui) {
        if let Some(text_state) = &mut self.text_input
            && text_state.is_active
        {
            let max_x = self.selection_end.x;
            let current_x = text_state.position.x;
            let desired_width = (max_x - current_x).abs().max(10.0);

            // 创建文本输入区域
            egui::Area::new(text_state.widget_id)
                .fixed_pos(text_state.position)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::NONE.show(ui, |ui| {
                        draw_annotation_text_editor(
                            ui,
                            text_state,
                            desired_width,
                            self.annotation_color,
                        )
                    });
                });
        }
    }

    fn draw_single_annotation(&self, painter: &egui::Painter, annotation: &Annotation) {
        let min_points = if annotation.tool == Tool::Mosaic {
            1
        } else {
            2
        };
        if annotation.points.len() < min_points {
            return;
        }

        let stroke = Stroke::new(annotation.stroke_width, annotation.color);
        match annotation.tool {
            Tool::Pen => {
                // 绘制自由画笔
                for window in annotation.points.windows(2) {
                    if let [start, end] = window {
                        painter.line_segment([*start, *end], stroke);
                    }
                }
            }
            Tool::Rectangle => {
                // 绘制矩形
                if let (Some(&start), Some(&end)) =
                    (annotation.points.first(), annotation.points.last())
                {
                    let rect = Rect::from_two_pos(start, end);
                    painter.rect_stroke(rect, egui::CornerRadius::ZERO, stroke, StrokeKind::Middle);
                }
            }
            Tool::Arrow => {
                // 绘制箭头
                if let (Some(&start), Some(&end)) =
                    (annotation.points.first(), annotation.points.last())
                {
                    // 1️⃣ 先画箭杆（线，和原来一样）

                    painter.line_segment([start, end], stroke);

                    // 2️⃣ 计算箭头头的三个点（实心三角形！）
                    let dir = end - start;
                    let dir_len = dir.length();
                    if dir_len < 1.0 {
                        return;
                    } // 防止太短

                    let dir_norm = dir / dir_len; // 方向单位向量

                    // ✅ 修正：手动计算垂直向量（egui中没有perp()方法）
                    let perp = Vec2::new(-dir_norm.y, dir_norm.x); // 旋转90度

                    // 箭头尺寸（可调，我设了10x5，你按需改）
                    let arrow_length = 15.0;
                    let arrow_width = 8.0;
                    // 将箭头尖端向前延伸，使其不与线条末端重合
                    let tip = end + dir_norm * 5.0; // 箭头尖端向前延伸5个像素
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
                if let Some(&pos) = annotation.points.first()
                    && !annotation.text.is_empty()
                {
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
                }
            }
            Tool::Mosaic => {
                if let (Some(&start), Some(&end)) =
                    (annotation.points.first(), annotation.points.last())
                {
                    let rect = Rect::from_two_pos(start, end);

                    let block_size = 4.0;

                    let width = rect.width();
                    let height = rect.height();
                    let cols = (width / block_size).ceil() as usize;
                    let rows = (height / block_size).ceil() as usize;
                    let tex = &self.screenshots[0];
                    for row in 0..rows {
                        for col in 0..cols {
                            let block_rect = Rect::from_min_size(
                                Pos2::new(
                                    rect.min.x + col as f32 * block_size,
                                    rect.min.y + row as f32 * block_size,
                                ),
                                Vec2::new(block_size, block_size),
                            );
                            let sample_x =
                                ((rect.min.x + col as f32 * block_size) * self.screen_scale) as u32;
                            let sample_y =
                                ((rect.min.y + row as f32 * block_size) * self.screen_scale) as u32;
                            if sample_x < tex.width() && sample_y < tex.height() {
                                let pixel = tex.get_pixel(sample_x, sample_y);
                                let current_color = Color32::from_rgb(pixel[0], pixel[1], pixel[2]);
                                painter.rect_filled(
                                    block_rect,
                                    egui::CornerRadius::ZERO,
                                    current_color,
                                );
                            }
                        }
                    }
                }
            }
            Tool::Number => {
                if let (Some(number), Some(&pos)) = (annotation.number, annotation.points.first()) {
                    let number_str = number.to_string();
                    let font_size = 16.0;
                    let circle_radius = 12.0; // 圆圈半径（比文字大点更舒服）

                    // 1️⃣ 先画圆圈背景（半透明黑，避免遮挡）
                    painter.circle_filled(pos, circle_radius, annotation.color);

                    // 2️⃣ 再画白色序号（居中）
                    painter.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        number_str,
                        egui::FontId::proportional(font_size),
                        Color32::WHITE, // 白色文字
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{draw_annotation_text_editor, is_persistent_toolbar_tool, toolbar_width};
    use crate::app_default::{TextInputState, Tool};

    #[test]
    fn unfocused_annotation_editor_requests_window_focus() {
        let context = egui::Context::default();
        let mut state = TextInputState::new(egui::Pos2::ZERO);
        let mut input = egui::RawInput::default();
        input.focused = false;
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(300.0, 100.0),
        ));

        let output = context.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw_annotation_text_editor(ui, &mut state, 280.0, egui::Color32::WHITE);
            });
        });

        let commands = &output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("root viewport output should exist")
            .commands;
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Focus)),
            "starting an annotation editor from an accessory window must request keyboard focus"
        );
    }

    #[test]
    fn annotation_text_editor_inside_area_accepts_text() {
        let context = egui::Context::default();
        let mut state = TextInputState::new(egui::pos2(20.0, 20.0));
        let run_frame = |events: Vec<egui::Event>, state: &mut TextInputState| {
            let mut input = egui::RawInput::default();
            input.focused = true;
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 100.0),
            ));
            input.events = events;
            let _ = context.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {
                    egui::Area::new(state.widget_id)
                        .fixed_pos(state.position)
                        .show(ctx, |ui| {
                            draw_annotation_text_editor(ui, state, 260.0, egui::Color32::WHITE);
                        });
                });
            });
        };

        run_frame(Vec::new(), &mut state);
        run_frame(vec![egui::Event::Text("a".to_string())], &mut state);

        assert_eq!(state.text, "a");
    }

    #[test]
    fn annotation_text_editor_accepts_ime_committed_text() {
        let context = egui::Context::default();
        let mut state = TextInputState::new(egui::Pos2::ZERO);
        let run_frame = |events: Vec<egui::Event>, state: &mut TextInputState| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 100.0),
            ));
            input.events = events;
            let _ = context.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {
                    egui::Area::new(state.widget_id)
                        .fixed_pos(state.position)
                        .show(ctx, |ui| {
                            draw_annotation_text_editor(ui, state, 280.0, egui::Color32::WHITE);
                        });
                });
            });
        };

        run_frame(Vec::new(), &mut state);
        run_frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)], &mut state);
        run_frame(
            vec![egui::Event::Ime(egui::ImeEvent::Preedit(
                "中文".to_string(),
            ))],
            &mut state,
        );
        run_frame(
            vec![egui::Event::Ime(egui::ImeEvent::Commit("中文".to_string()))],
            &mut state,
        );

        assert_eq!(state.text, "中文");
    }

    #[test]
    fn toolbar_width_covers_all_items_spacing_and_margin() {
        let default_item_spacing = egui::Style::default().spacing.item_spacing.x;
        let item_count = 13.0;
        let content_width = 7.0 * 30.0 + 30.0 + 30.0 + 30.0 + 3.0 * 30.0;
        let required_width = content_width + (item_count - 1.0) * default_item_spacing + 20.0;

        assert!(toolbar_width(default_item_spacing) >= required_width);
    }

    #[test]
    fn annotation_and_move_tools_are_persistent_toolbar_tools() {
        assert!(is_persistent_toolbar_tool(Tool::Pen));
        assert!(is_persistent_toolbar_tool(Tool::MoveBox));
        assert!(is_persistent_toolbar_tool(Tool::ColorPicker));
    }

    #[test]
    fn ocr_and_other_actions_are_not_persistent_toolbar_tools() {
        assert!(!is_persistent_toolbar_tool(Tool::Ocr));
        assert!(!is_persistent_toolbar_tool(Tool::Copy));
        assert!(!is_persistent_toolbar_tool(Tool::Save));
        assert!(!is_persistent_toolbar_tool(Tool::Exit));
    }
}
