use eframe::emath::{Pos2, Rect, Vec2};
use eframe::epaint::{Color32, Hsva, Shape, Stroke, StrokeKind};
use egui::{color_picker, text_selection, Button, Id, Popup, PopupCloseBehavior, Response, Ui};
use egui::color_picker::color_picker_hsva_2d;
use crate::app_default::{Annotation, ScreenshotApp, Tool};
use crate::ui::load_texture_from_png;

impl ScreenshotApp {
    pub(crate) fn draw_toolbar(&mut self, ctx: &egui::Context) {
        self.tool_bar_focused = false;
        if let Some(selection_rect) = self.selection_rect {
            let toolbar_size = Vec2::new(50.0, 40.0);
            // 计算工具栏位置：在选择框右下角，并与选择框右对齐
            let mut toolbar_pos = Pos2::new(
                selection_rect.min.x, // 左对齐：工具栏左侧与选择框左侧对齐
                selection_rect.max.y + 5.0,  // 在选择框下方，留 5.0 的间距
            );
            // 确保工具栏在屏幕内
            let screen_rect = ctx.viewport_rect();

            // 如果工具栏超出右边界，向左调整
            if toolbar_pos.x + toolbar_size.x > screen_rect.max.x {
                toolbar_pos.x = screen_rect.max.x - toolbar_size.x;
            }

            // 如果工具栏超出左边界，确保至少显示一部分
            if toolbar_pos.x < screen_rect.min.x {
                toolbar_pos.x = screen_rect.min.x + 10.0;
            }

            // 如果工具栏超出下边界，显示在选择框上方
            if toolbar_pos.y + toolbar_size.y > screen_rect.max.y {
                toolbar_pos.y = selection_rect.min.y - toolbar_size.y;
            }

            // 如果工具栏超出上边界，确保至少显示一部分
            if toolbar_pos.y < screen_rect.min.y {
                toolbar_pos.y = screen_rect.min.y + 10.0;
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
                                self.purple_icon_button(ui, Tool::MoveBox, ctx, "src/icon/move.png");
                                self.purple_icon_button(ui, Tool::Pen, ctx, "src/icon/pen.png");
                                self.purple_icon_button(ui, Tool::Rectangle, ctx, "src/icon/rectangle.png");
                                self.purple_icon_button(ui, Tool::Arrow, ctx, "src/icon/arrow.png");
                                self.purple_icon_button(ui, Tool::Text, ctx, "src/icon/word.png");
                                self.purple_icon_button(ui, Tool::Mosaic, ctx, "src/icon/mosaic.png");
                                self.purple_icon_button(ui, Tool::Number, ctx, "src/icon/number.png");

                                // 颜色选择
                                self.custom_color_picker(ui, ctx);
                                // 画笔大小
                                // ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=20.0));

                                // 操作： 复制，保存，退出
                                self.purple_icon_button(ui, Tool::Button, ctx, "src/icon/copy.png").clicked().then(|| {
                                    self.copy_to_clipboard();
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                });

                                self.purple_icon_button(ui, Tool::Button, ctx, "src/icon/save.png").clicked().then(|| {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    self.save_screenshot();
                                });

                                self.purple_icon_button(ui, Tool::Button, ctx, "src/icon/exit.png").clicked().then(|| {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                });
                            });
                        });
                });

        }
    }

    fn custom_color_picker(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let button_size = Vec2::new(30.0, 30.0);

        // 创建自定义按钮
        let button = Button::new("")
            .min_size(button_size)
            .frame(false);
        let color_pick_response = ui.add(button);

        let is_hovered_or_focused = color_pick_response.hovered() || color_pick_response.has_focus();

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
                if color_picker_hsva_2d(ui, & mut hsva, color_picker::Alpha::BlendOrAdditive) {
                    self.annotation_color = Color32::from(hsva);
                }
            });
        // 处理点击：用 if 而不是 then！
        if color_pick_response.clicked() {
            self.current_tool = Tool::ColorPicker;
            Popup::open_id(ctx, popup_id);
        }
    }

    pub fn purple_icon_button(&mut self, ui: &mut Ui, tool: Tool, ctx: &egui::Context, icon_path: &str) -> Response {
        let icon = load_texture_from_png(ctx, icon_path).unwrap();
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
            self.tool_bar_focused = true;
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
                Color32::from_rgb(128, 80, 128), // 初始状态为淡紫色
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
            self.draw_single_annotation(painter, annotation);
        }

        if let Some(annotation) = &self.current_annotation {
            self.draw_single_annotation(painter, annotation);
        }
    }

    // 文本输入
    pub(crate) fn draw_text_input(&mut self, ui: &mut Ui) {
        if let Some(text_state) = &mut self.text_input && text_state.is_active {
            let max_x = self.selection_end.x;
            let current_x = text_state.position.x;
            let desired_width = (max_x - current_x).abs().max(10.0);

            // 获取当前时间用于光标闪烁
            let now = ui.ctx().input(|i| i.time);

            // 创建文本输入区域
            let _text_response = egui::Area::new(text_state.widget_id)
                .fixed_pos(text_state.position)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::NONE
                        .show(ui, |ui| {
                            ui.style_mut().visuals.text_cursor.stroke.color = self.annotation_color; // 设置为红色光标
                            let text_edit = egui::TextEdit::multiline(&mut text_state.text)
                                .font(egui::FontId::proportional(16.0))
                                .desired_width(desired_width)
                                .desired_rows(1)
                                .min_size(Vec2::ZERO)
                                .frame(false)
                                .text_color(self.annotation_color)
                                .hint_text("")
                                .id(text_state.widget_id);
                            let response = ui.add(text_edit);

                            // 更新焦点状态和交互时间
                            text_state.has_focus = response.has_focus();
                            if response.changed() || response.lost_focus() || response.gained_focus() {
                                text_state.last_interaction_time = now;
                            }
                            response
                        }).inner
                }).response;
            // 手动绘制光标
            ui.visuals_mut().text_cursor.stroke.color = self.annotation_color;
            let painter = ui.painter();

            // 计算光标位置（这里需要根据文本内容计算准确的光标位置）
            // 这是一个简化的实现，实际可能需要更复杂的光标位置计算
            let cursor_rect = {
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(
                    text_state.text.clone(),
                    egui::FontId::proportional(16.0),
                    self.annotation_color,
                ));

                let cursor_x = text_state.position.x + galley.size().x + 2.0; // 在文本末尾
                let cursor_y = text_state.position.y;
                let cursor_height = 16.0; // 字体高度

                Rect::from_min_size(
                    egui::pos2(cursor_x, cursor_y),
                    egui::vec2(20.0, cursor_height), // 光标宽度为2像素
                )
            };
            // 绘制光标
            text_selection::visuals::paint_text_cursor(
                ui,
                &painter,
                cursor_rect,
                now - text_state.last_interaction_time,
            );
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
                    let arrow_width = 8.0;

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
                if let Some(&pos) = annotation.points.first() && !annotation.text.is_empty() {
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
                // 绘制马赛克效果
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    let rect = Rect::from_two_pos(start, end);

                    // 马赛克块大小（可调整）
                    let block_size = 4.0;

                    // 计算马赛克网格
                    let width = rect.width();
                    let height = rect.height();
                    let cols = (width / block_size).ceil() as usize;
                    let rows = (height / block_size).ceil() as usize;

                    // 绘制马赛克网格
                    for row in 0..rows {
                        for col in 0..cols {
                            let block_rect = Rect::from_min_size(
                                Pos2::new(
                                    rect.min.x + col as f32 * block_size,
                                    rect.min.y + row as f32 * block_size
                                ),
                                Vec2::new(block_size, block_size)
                            );
                            let pixel = self.screenshots[0].get_pixel(block_rect.min.x as u32, block_rect.min.y as u32);
                            let current_color = Color32::from_rgb(pixel[0], pixel[1], pixel[2]);
                            painter.rect_filled(block_rect, egui::CornerRadius::ZERO, current_color);
                        }
                    }

                    // // 可选：绘制马赛克区域的边框
                    // painter.rect_stroke(rect, egui::CornerRadius::ZERO, stroke, StrokeKind::Middle);
                }
            }
            Tool::Number => {
                if let (Some(number), Some(&pos)) = (annotation.number, annotation.points.first()) {
                    let number_str = number.to_string();
                    let font_size = 16.0;
                    let circle_radius = 12.0; // 圆圈半径（比文字大点更舒服）

                    // 1️⃣ 先画圆圈背景（半透明黑，避免遮挡）
                    painter.circle_filled(
                        pos,
                        circle_radius,
                        annotation.color
                    );

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