use crate::app_default::{Annotation, ScreenshotApp, Tool};
use crate::display::place_toolbar;
use crate::ui::{
    ARROW_ICON, COPY_ICON, EXIT_ICON, MOSAIC_ICON, MOVE_ICON, NUMBER_ICON, PEN_ICON,
    RECTANGLE_ICON, SAVE_ICON, WORD_ICON, load_texture_from_png, ocr_button,
};
use eframe::emath::{Pos2, Rect, Vec2};
use eframe::epaint::{Color32, Hsva, Shape, Stroke, StrokeKind};
use egui::color_picker::color_picker_hsva_2d;
use egui::{Button, Id, Popup, PopupCloseBehavior, Response, Ui, color_picker};
use std::io::Write;
use std::time::Instant;

fn is_persistent_toolbar_tool(tool: Tool) -> bool {
    tool.is_annotation_tool() || matches!(tool, Tool::MoveBox | Tool::ColorPicker)
}

fn toolbar_width(item_spacing: f32) -> f32 {
    const ITEM_COUNT: f32 = 15.0;
    const CONTENT_WIDTH: f32 = 15.0 * 30.0;
    const HORIZONTAL_MARGIN: f32 = 20.0;

    CONTENT_WIDTH + (ITEM_COUNT - 1.0) * item_spacing + HORIZONTAL_MARGIN
}

#[derive(Clone, Copy)]
enum ToolbarActionIcon {
    Undo,
    Redo,
    Pin,
}

fn toolbar_action_colors(enabled: bool, highlighted: bool) -> (Color32, Color32) {
    if !enabled {
        return (
            Color32::from_rgba_unmultiplied(0, 100, 255, 115),
            Color32::from_white_alpha(115),
        );
    }

    let background = if highlighted {
        Color32::from_rgb(30, 120, 255)
    } else {
        Color32::from_rgb(0, 100, 255)
    };
    (background, Color32::WHITE)
}

fn toolbar_action_button(
    ui: &mut Ui,
    enabled: bool,
    icon: ToolbarActionIcon,
    tooltip: &str,
) -> Response {
    let size = Vec2::splat(30.0);
    let response = ui.add_enabled(enabled, Button::new("").min_size(size).frame(false));
    let (background, foreground) =
        toolbar_action_colors(enabled, response.hovered() || response.has_focus());
    ui.painter().circle_filled(
        response.rect.center(),
        response.rect.width() / 2.0,
        background,
    );

    match icon {
        ToolbarActionIcon::Undo | ToolbarActionIcon::Redo => {
            draw_history_action_icon(
                ui.painter(),
                response.rect.center(),
                matches!(icon, ToolbarActionIcon::Redo),
                foreground,
            );
        }
        ToolbarActionIcon::Pin => {
            let center = response.rect.center();
            let stroke = Stroke::new(2.0, foreground);
            ui.painter().add(Shape::line(
                vec![
                    center + egui::vec2(-5.0, -7.0),
                    center + egui::vec2(5.0, -7.0),
                    center + egui::vec2(3.0, -2.0),
                    center + egui::vec2(6.0, 1.0),
                    center + egui::vec2(-6.0, 1.0),
                    center + egui::vec2(-3.0, -2.0),
                    center + egui::vec2(-5.0, -7.0),
                ],
                stroke,
            ));
            ui.painter().line_segment(
                [center + egui::vec2(0.0, 1.0), center + egui::vec2(0.0, 8.0)],
                stroke,
            );
        }
    }

    response.on_hover_text(tooltip)
}

fn history_action_icon_geometry(redo: bool) -> Vec<Pos2> {
    let mirror = if redo { -1.0 } else { 1.0 };
    let point = |x: f32, y: f32| Pos2::new(x * mirror, y);
    let mut points = vec![
        point(-1.5, -7.0),
        point(-7.0, -2.5),
        point(-1.0, 2.0),
        point(-6.0, -2.5),
        point(1.0, -2.5),
    ];

    let mut append_cubic = |start: Pos2, control_1: Pos2, control_2: Pos2, end: Pos2| {
        for step in 1..=8 {
            let t = step as f32 / 8.0;
            let inverse = 1.0 - t;
            points.push(Pos2::new(
                inverse.powi(3) * start.x
                    + 3.0 * inverse.powi(2) * t * control_1.x
                    + 3.0 * inverse * t.powi(2) * control_2.x
                    + t.powi(3) * end.x,
                inverse.powi(3) * start.y
                    + 3.0 * inverse.powi(2) * t * control_1.y
                    + 3.0 * inverse * t.powi(2) * control_2.y
                    + t.powi(3) * end.y,
            ));
        }
    };
    let turn = point(6.5, 2.5);
    append_cubic(point(1.0, -2.5), point(4.5, -2.5), point(6.5, -0.5), turn);
    append_cubic(turn, point(6.5, 5.0), point(4.8, 6.5), point(2.0, 6.5));
    points
}

fn draw_history_action_icon(painter: &egui::Painter, center: Pos2, redo: bool, color: Color32) {
    let points = history_action_icon_geometry(redo)
        .into_iter()
        .map(|point| center + point.to_vec2())
        .collect::<Vec<_>>();
    let stroke = Stroke::new(2.0, color);
    painter.add(Shape::line(points[..3].to_vec(), stroke));
    painter.add(Shape::line(points[3..].to_vec(), stroke));
}

#[cfg(target_os = "linux")]
fn backport_linux_ime_commit_fix(ui: &mut Ui) {
    ui.input_mut(|input| {
        for event in &mut input.events {
            if let egui::Event::Ime(egui::ImeEvent::Commit(text)) = event {
                *event = egui::Event::Text(std::mem::take(text));
            }
        }
    });
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
    // egui 0.33 rejects later IME commits when the cursor moved from its
    // initial position. Treat a Linux commit as the equivalent text event;
    // macOS and Windows keep egui's native path unchanged.
    #[cfg(target_os = "linux")]
    backport_linux_ime_commit_fix(ui);
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
            self.toolbar_rect_global =
                Some(Rect::from_min_size(placement.global_position, toolbar_size));
        } else {
            self.toolbar_rect_global = None;
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

                            let undo = toolbar_action_button(
                                ui,
                                self.can_undo(),
                                ToolbarActionIcon::Undo,
                                "撤销（Ctrl/Cmd+Z）",
                            );
                            if undo.hovered() || undo.has_focus() {
                                self.tool_bar_focused = true;
                            }
                            if undo.clicked() {
                                self.undo_edit();
                                self.refresh_edit_toolbar(ctx);
                            }
                            let redo = toolbar_action_button(
                                ui,
                                self.can_redo(),
                                ToolbarActionIcon::Redo,
                                "重做（Ctrl/Cmd+Shift+Z）",
                            );
                            if redo.hovered() || redo.has_focus() {
                                self.tool_bar_focused = true;
                            }
                            if redo.clicked() {
                                self.redo_edit();
                                self.refresh_edit_toolbar(ctx);
                            }

                            // 操作：OCR，贴图，复制，保存，退出
                            let ocr_response = ocr_button(ui, ctx);
                            if ocr_response.hovered() || ocr_response.has_focus() {
                                self.tool_bar_focused = true;
                            }
                            if ocr_response.clicked()
                                && let Err(error) =
                                    self.submit_ocr_for_current_selection(Instant::now())
                            {
                                eprintln!("OCR 提交失败: {error}");
                            }

                            let pin_response = toolbar_action_button(
                                ui,
                                true,
                                ToolbarActionIcon::Pin,
                                "将选区贴到屏幕并置顶",
                            );
                            if pin_response.hovered() || pin_response.has_focus() {
                                self.tool_bar_focused = true;
                            }
                            if pin_response.clicked() {
                                self.pin_selection_and_finish(ctx);
                            }

                            if self
                                .purple_icon_button(ui, Tool::Copy, ctx, COPY_ICON, "copy")
                                .clicked()
                            {
                                self.copy_selection_and_finish(ctx);
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
        response.on_hover_text(match tool {
            Tool::MoveBox => "移动选区；拖动边角调整大小",
            Tool::Pen => "画笔",
            Tool::Rectangle => "矩形",
            Tool::Arrow => "箭头",
            Tool::Text => "文字（Ctrl/Cmd+Enter 完成）",
            Tool::Number => "序号",
            Tool::Mosaic => "马赛克",
            Tool::Copy => "复制（Ctrl/Cmd+C）",
            Tool::Save => "保存（Ctrl/Cmd+S）",
            Tool::Exit => "取消（Esc）",
            _ => "选择",
        })
    }

    fn draw_single_annotation(&self, painter: &egui::Painter, annotation: &Annotation) {
        let min_points = if matches!(annotation.tool, Tool::Mosaic | Tool::Text | Tool::Number) {
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
    use super::{
        ToolbarActionIcon, draw_annotation_text_editor, history_action_icon_geometry,
        is_persistent_toolbar_tool, toolbar_action_button, toolbar_action_colors, toolbar_width,
    };
    use crate::app_default::{TextInputState, Tool};

    #[test]
    fn unfocused_annotation_editor_requests_window_focus() {
        let context = egui::Context::default();
        let mut state = TextInputState::new(egui::Pos2::ZERO);
        let input = egui::RawInput {
            focused: false,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 100.0),
            )),
            ..Default::default()
        };

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
            let input = egui::RawInput {
                focused: true,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 100.0),
                )),
                events,
                ..Default::default()
            };
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
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 100.0),
                )),
                events,
                ..Default::default()
            };
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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_annotation_editor_accepts_consecutive_ime_commits() {
        let context = egui::Context::default();
        let mut state = TextInputState::new(egui::Pos2::ZERO);
        let run_frame = |events: Vec<egui::Event>, state: &mut TextInputState| {
            let input = egui::RawInput {
                focused: true,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 100.0),
                )),
                events,
                ..Default::default()
            };
            let _ = context.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    draw_annotation_text_editor(ui, state, 280.0, egui::Color32::WHITE);
                });
            });
        };

        run_frame(Vec::new(), &mut state);
        run_frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)], &mut state);
        run_frame(
            vec![egui::Event::Ime(egui::ImeEvent::Commit("中文".to_string()))],
            &mut state,
        );
        run_frame(
            vec![egui::Event::Ime(egui::ImeEvent::Commit("测试".to_string()))],
            &mut state,
        );

        assert_eq!(state.text, "中文测试");
    }

    #[test]
    fn toolbar_width_covers_all_items_spacing_and_margin() {
        let default_item_spacing = egui::Style::default().spacing.item_spacing.x;
        let item_count = 15.0;
        let content_width = 15.0 * 30.0;
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
        assert!(!is_persistent_toolbar_tool(Tool::Copy));
        assert!(!is_persistent_toolbar_tool(Tool::Save));
        assert!(!is_persistent_toolbar_tool(Tool::Exit));
    }

    #[test]
    fn disabled_history_buttons_keep_the_toolbar_blue_hue() {
        let (enabled_background, _) = toolbar_action_colors(true, false);
        let (disabled_background, _) = toolbar_action_colors(false, false);
        let [enabled_r, enabled_g, enabled_b, enabled_a] =
            enabled_background.to_srgba_unmultiplied();
        let [disabled_r, disabled_g, disabled_b, disabled_a] =
            disabled_background.to_srgba_unmultiplied();

        assert_eq!(
            (disabled_r, disabled_g, disabled_b),
            (enabled_r, enabled_g, enabled_b)
        );
        assert!(disabled_a < enabled_a);
    }

    #[test]
    fn undo_and_redo_arrow_geometry_are_exact_mirrors() {
        let undo = history_action_icon_geometry(false);
        let redo = history_action_icon_geometry(true);

        assert_eq!(undo.len(), redo.len());
        for (undo_point, redo_point) in undo.iter().zip(&redo) {
            assert_eq!(undo_point.x, -redo_point.x);
            assert_eq!(undo_point.y, redo_point.y);
        }
    }

    #[test]
    fn toolbar_action_icons_are_vector_paths_without_font_or_bitmap_assets() {
        let context = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(180.0, 60.0),
            )),
            ..Default::default()
        };

        let output = context.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    toolbar_action_button(ui, true, ToolbarActionIcon::Undo, "撤销");
                    toolbar_action_button(ui, true, ToolbarActionIcon::Redo, "重做");
                    toolbar_action_button(ui, true, ToolbarActionIcon::Pin, "贴图");
                });
            });
        });

        let mut has_circle = false;
        let mut has_bitmap = false;
        let mut path_count = 0;
        let mut has_text = false;
        for shape in &output.shapes {
            match &shape.shape {
                egui::Shape::Circle(_) => has_circle = true,
                egui::Shape::Mesh(_) => has_bitmap = true,
                egui::Shape::Path(_) => path_count += 1,
                egui::Shape::Text(text) if !text.galley.text().is_empty() => has_text = true,
                _ => {}
            }
        }

        assert!(
            has_circle,
            "action buttons need the same circular background as the toolbar"
        );
        assert!(!has_bitmap, "action icons must not load bitmap artwork");
        assert!(path_count >= 3, "every action icon needs a vector path");
        assert!(
            !has_text,
            "action icons must not depend on the current font"
        );
    }
}
