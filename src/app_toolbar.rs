use std::fs;
use eframe::emath::{Pos2, Rect, Vec2};
use eframe::epaint::{Color32, ColorImage, Hsva, Shape, Stroke, StrokeKind};
use egui::{color_picker, AtomExt, Button, Id, Image, ImageSource, Key, Popup, PopupCloseBehavior, Response, Sense, Ui};
use egui::color_picker::color_picker_hsva_2d;
use crate::app_default::{Annotation, ScreenshotApp, Tool};
use crate::ui::load_texture_from_png;

impl ScreenshotApp {
    pub(crate) fn draw_toolbar(&mut self, ctx: &egui::Context) {
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
                .interactable(true)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // 工具选择
                                let pen_icon = load_texture_from_png(ctx, "src/icon/pen.png").unwrap();
                                let arrow_icon = load_texture_from_png(ctx, "src/icon/arrow.png").unwrap();
                                let copy_icon = load_texture_from_png(ctx, "src/icon/copy.png").unwrap();
                                let mosaic_icon = load_texture_from_png(ctx, "src/icon/mosaic.png").unwrap();
                                let move_icon = load_texture_from_png(ctx, "src/icon/move.png").unwrap();
                                let number_icon = load_texture_from_png(ctx, "src/icon/number.png").unwrap();
                                let rectangle_icon = load_texture_from_png(ctx, "src/icon/rectangle.png").unwrap();
                                let save_icon = load_texture_from_png(ctx, "src/icon/save.png").unwrap();
                                let word_icon = load_texture_from_png(ctx, "src/icon/word.png").unwrap();
                                let exit_icon = load_texture_from_png(ctx, "src/icon/exit.png").unwrap();
                                self.purple_icon_button(ui, Tool::MoveBox, move_icon);
                                self.purple_icon_button(ui, Tool::Pen, pen_icon);
                                self.purple_icon_button(ui, Tool::Rectangle, rectangle_icon);
                                self.purple_icon_button(ui, Tool::Arrow, arrow_icon);
                                self.purple_icon_button(ui, Tool::Text, word_icon).hovered();
                                self.purple_icon_button(ui, Tool::Mosaic, mosaic_icon).hovered();
                                self.purple_icon_button(ui, Tool::Number, number_icon).hovered();

                                ui.separator();

                                let button_size = Vec2::new(30.0, 30.0);

                                // 创建自定义按钮
                                let button = Button::new("")
                                    .min_size(button_size)
                                    .frame(false);
                                let mut color_pick_response = ui.add(button);


                                // 颜色选择
                                // let mut color_pick_response = ui.color_edit_button_srgba(&mut self.annotation_color);
                                ui.painter().circle_filled(
                                    color_pick_response.rect.center(),
                                    color_pick_response.rect.width() / 2.0,
                                    self.annotation_color,
                                );
                                // 使用状态来跟踪弹出窗口是否打开
                                let popup_id = ui.auto_id_with("color_popup");
                                // let mut is_popup_open = ui.memory(|mem| mem.is_popup_open(popup_id));

                                // if is_popup_open {
                                //     let mut hsva = Hsva::from(self.annotation_color);
                                //     Popup::menu(&color_pick_response)
                                //         .id(popup_id)
                                //         .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                                //         .show(|ui| {
                                //             ui.spacing_mut().slider_width = 275.0;
                                //             if color_picker_hsva_2d(ui, & mut hsva, color_picker::Alpha::BlendOrAdditive) {
                                //                 self.annotation_color = Color32::from(hsva);
                                //             }
                                //             if ui.button("关闭").clicked() {
                                //                 ui.memory_mut(|mem| mem.close_popup(popup_id));
                                //                 is_popup_open = false;
                                //             }
                                //         });
                                // }

                                // 点击按钮时打开弹出窗口
                                // if color_pick_response.clicked() {
                                //     is_popup_open = true;
                                //     ui.memory_mut(|mem| mem.open_popup(popup_id));
                                // }
                                color_pick_response.clicked().then(|| {
                                    let mut hsva = Hsva::from(self.annotation_color);
                                    Popup::menu(&color_pick_response)
                                        .id(popup_id)
                                        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                                        .show(|ui| {
                                            ui.spacing_mut().slider_width = 275.0;
                                            if color_picker_hsva_2d(ui, & mut hsva, color_picker::Alpha::BlendOrAdditive) {
                                                self.annotation_color = Color32::from(hsva);
                                            }
                                            if ui.button("关闭").clicked() {
                                                ui.memory_mut(|mem| mem.close_popup(popup_id));
                                            }
                                        });
                                    ui.memory_mut(|mem| mem.open_popup(popup_id));

                                });

                                // 画笔大小
                                // ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=20.0));

                                ui.separator();

                                self.purple_icon_button(ui, Tool::Button, copy_icon).clicked().then(|| {
                                    self.copy_to_clipboard();
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                });

                                self.purple_icon_button(ui, Tool::Button, save_icon).clicked().then(|| {
                                    self.copy_to_clipboard();
                                });

                                self.purple_icon_button(ui, Tool::Button, exit_icon).clicked().then(|| {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                });
                            });
                        });
                });
        }
    }

    fn tool_button(&mut self, ui: &mut Ui, tool: Tool, icon: &str) -> Response {
        let is_selected = self.current_tool == tool;
        let response = ui.selectable_label(is_selected, icon);
        if response.clicked() {
            self.current_tool = tool;
        }

        response
    }

    pub fn purple_icon_button(&mut self, ui: &mut Ui, tool: Tool, icon_texture_id: egui::TextureId) -> Response {
        let selected = self.current_tool == tool;
        let button_size = Vec2::new(30.0, 30.0);

        // 创建自定义按钮
        let button = Button::new("")
            .min_size(button_size)
            .frame(false);
        // 根据状态设置按钮颜色
        let response = ui.add_sized(button_size, button);
        let is_hovered_or_focused = response.hovered() || response.has_focus();

        // 设置按钮填充颜色
        if selected && tool != Tool::Button {
            ui.painter().circle_filled(
                response.rect.center(),
                response.rect.width() / 2.0, // 圆角为0
                Color32::from_rgb(128, 0, 128), // 选中或悬停时为紫色
            );
        } else if is_hovered_or_focused {
            ui.painter().circle_filled(
                response.rect.center(),
                response.rect.width() / 2.0, // 圆角为0
                Color32::from_rgb(128, 0, 128), // 选中或悬停时为紫色
            );
        }
        else {
            ui.painter().circle_filled(
                response.rect.center(),
                response.rect.width() / 2.0, // 圆角为0
                Color32::from_rgb(128, 100, 128), // 初始状态为淡紫色
            );
        }
        // 绘制图标
        let icon_size = Vec2::new(20.0, 20.0);
        let icon_rect = Rect::from_center_size(response.rect.center(), icon_size);
        ui.painter().image(
            icon_texture_id,
            icon_rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if response.clicked() {
            self.current_tool = tool;
        }
        response
    }

    pub(crate) fn draw_annotations(&self, ui: &mut egui::Ui) {
        let painter = ui.painter();

        for annotation in &self.annotations {
            self.draw_single_annotation(painter, annotation);
        }

        if let Some(annotation) = &self.current_annotation {
            self.draw_single_annotation(painter, annotation);
        }
    }

    // 文本输入
    pub(crate) fn draw_text_input(&mut self, ui: &mut egui::Ui) {
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