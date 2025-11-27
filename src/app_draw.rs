use device_query::{DeviceQuery, MousePosition};
use eframe::emath::{Pos2, Rect};
use eframe::epaint::{Color32, Shape, Stroke, StrokeKind};
use crate::app_default::{Annotation, MouseSelectionRect, ScreenshotApp, TextInputState, Tool};

#[cfg(target_os = "linux")]
use crate::ui::get_screen_rect;

impl ScreenshotApp {
    pub(crate) fn draw_screens(&self, ui: &mut egui::Ui) {
        for (_i, (screen, texture)) in self.screens.iter().zip(&self.display_textures).enumerate() {
            #[cfg(target_os = "linux")]
            {
                let screen_rect = get_screen_rect(screen);
                // 绘制屏幕截图
                ui.put(screen_rect, egui::Image::new(texture).fit_to_exact_size(screen_rect.size()));
            }

            #[cfg(target_os = "windows")]
            {
                let viewport_rect = ui.ctx().viewport_rect();
                // 计算缩放比例，保持宽高比
                let texture_size = texture.size_vec2();
                let scale = (viewport_rect.width() / texture_size.x).min(viewport_rect.height() / texture_size.y);
                let scaled_size = texture_size * scale;
                let rect = Rect::from_center_size(viewport_rect.center(), scaled_size);
                ui.put(rect, egui::Image::new(texture).shrink_to_fit());
            }

            #[cfg(target_os = "macos")]
            {
                let viewport_rect = ui.ctx().viewport_rect();
                // 计算缩放比例，保持宽高比
                let texture_size = texture.size_vec2();
                let scale = (viewport_rect.width() / texture_size.x).min(viewport_rect.height() / texture_size.y);
                let scaled_size = texture_size * scale;
                let rect = Rect::from_center_size(viewport_rect.center(), scaled_size);
                ui.put(rect, egui::Image::new(texture).shrink_to_fit());
            }
        }
    }

    pub(crate) fn draw_overlay(&mut self, ui: &mut egui::Ui) {
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


    pub(crate) fn handle_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pointer_pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO);
        let mouse_pos = self.device_state.get_mouse().coords;
        // 如果有活动的文本输入，优先处理文本输入
        if let Some(text_state) = &mut self.text_input && text_state.is_active {
            // 文本输入激活时，不处理其他工具
            self.handle_text_input(ui, ctx);
            // 如果文本输入已经完成，立即返回
            if self.text_input_finalized {
                self.finalize_text_input();
                return;
            }
        }

        // 鼠标按下开始选择
        if ui.input(|i| i.pointer.primary_pressed()) {
            if self.current_tool == Tool::Select && !self.show_toolbar {
                self.is_selecting = true;
                self.selection_start = pointer_pos;
                self.selection_end = pointer_pos;
                self.mouse_start = mouse_pos;
                self.mouse_end = mouse_pos;
                // !self.tool_bar_focused 要加载下面，不能放在前面判断然后return，不然会导致当前工具是文字标注，然后点击其他标注工具文字会跟随其他标注移动，也就是文字标注未结束
            } else if self.current_tool == Tool::MoveBox && self.selection_rect.is_some() && !self.tool_bar_focused {
                // 开始移动选择框
                if self.selection_rect.unwrap().contains(pointer_pos) {
                    self.is_moving_box = true;
                    self.show_toolbar = false;
                    self.move_start = pointer_pos;
                    self.mouse_move_start = mouse_pos;
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
                                self.start_annotation(pointer_pos, mouse_pos);
                            } else {
                                self.finalize_text_input();
                            }
                        } else if self.current_tool == Tool::Number {
                            if self.number_input.is_none() {
                                // 创建数字输入状态
                                self.number_input = Some(1);
                            } else {
                                self.number_input = Some(self.number_input.unwrap() + 1)
                            }
                            self.start_annotation(pointer_pos, mouse_pos);
                        }
                        else {
                            // 否则，开始标注
                            self.start_annotation(pointer_pos, mouse_pos);
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
        if let Some(text_state) = &mut self.text_input && text_state.is_active  {
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

        // 鼠标拖动
        if ui.input(|i| i.pointer.primary_down()) {
            if self.is_selecting {
                self.selection_end = pointer_pos;
                self.mouse_end = mouse_pos;

                self.update_selection_rect();
            } else if self.is_moving_box {
                // 移动选择框 - 基于原始位置计算偏移
                if let (Some(original_rect), Some(mouse_original_rect), Some(combined_bounds)) = (self.original_selection_rect, self.mouse_original_selection_rect, Some(self.get_combined_bounds())) {
                    let delta = pointer_pos - self.move_start;
                    let mouse_delta = (mouse_pos.0 - self.mouse_move_start.0, mouse_pos.1 - self.mouse_move_start.1);
                    // 应用偏移到原始位置
                    let mut new_rect = original_rect;
                    let mut new_mouse_rect: MouseSelectionRect = mouse_original_rect;
                    new_rect.min += delta;
                    new_rect.max += delta;
                    new_mouse_rect.start.0 += mouse_delta.0;
                    new_mouse_rect.end.0 += mouse_delta.0;
                    new_mouse_rect.start.1 += mouse_delta.1;
                    new_mouse_rect.end.1 += mouse_delta.1;

                    // 限制选择框在屏幕范围内
                    new_rect.min = new_rect.min.max(combined_bounds.min);
                    new_rect.max = new_rect.max.min(combined_bounds.max);

                    new_mouse_rect.start = new_mouse_rect.start.max((0, 0));
                    new_mouse_rect.end = new_mouse_rect.end.min((self.screen_with, self.screen_height));

                    // 确保选择框大小不变
                    let width = original_rect.width();
                    let height = original_rect.height();

                    let mouse_width = mouse_original_rect.end.0 - mouse_original_rect.start.0;
                    let mouse_height = mouse_original_rect.end.1 - mouse_original_rect.start.1;
                    // 限制选择框在屏幕范围内，考虑选择框的大小
                    let max_x = combined_bounds.max.x - width;
                    let max_y = combined_bounds.max.y - height;

                    let max_mouse_x = self.screen_with - mouse_width;
                    let max_mouse_y = self.screen_height - mouse_height;

                    new_rect.min.x = new_rect.min.x.clamp(combined_bounds.min.x, max_x);
                    new_rect.min.y = new_rect.min.y.clamp(combined_bounds.min.y, max_y);

                    new_mouse_rect.start.0 = new_mouse_rect.start.0.clamp(0, max_mouse_x);
                    new_mouse_rect.start.1 = new_mouse_rect.start.1.clamp(0, max_mouse_y);

                    // 根据调整后的min重新计算max
                    new_rect.max.x = new_rect.min.x + width;
                    new_rect.max.y = new_rect.min.y + height;

                    new_mouse_rect.end.0 = new_mouse_rect.start.0 + mouse_width;
                    new_mouse_rect.end.1 = new_mouse_rect.start.1 + mouse_height;

                    self.selection_rect = Some(new_rect);
                    self.mouse_selection_rect = Some(new_mouse_rect);

                    // ✅ 更新 selection_start 和 selection_end
                    self.selection_start = new_rect.min;
                    self.selection_end = new_rect.max;

                    self.mouse_start = new_mouse_rect.start;
                    self.mouse_end = new_mouse_rect.end;
                    // println!("mouse_start: {:?}, mouse_end: {:?}", self.mouse_start, self.mouse_end);

                    // 更新工具栏位置
                    self.update_toolbar_position(new_rect);
                }
            } else if let Some(ref mut annotation) = self.current_annotation {
                // 检查拖动点是否在选择区域内
                if let Some(selection_rect) = self.selection_rect {
                    if selection_rect.contains(pointer_pos) {
                        annotation.points.push(pointer_pos);
                        annotation.mouse_points.push(mouse_pos);
                    }
                }
            }
        }

        // 鼠标释放
        if ui.input(|i| i.pointer.primary_released()) {
            if self.is_selecting {
                self.is_selecting = false;
                self.selection_end = pointer_pos;
                self.mouse_end = mouse_pos;
                self.update_selection_rect();
                if let Some(rect) = self.selection_rect {
                    if rect.area() > 100.0 { // 最小区域阈值
                        self.show_toolbar = true;
                        self.update_toolbar_position(rect);
                    }
                }
            } else if self.is_moving_box && self.current_tool == Tool::MoveBox {
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
                self.current_tool = Tool::Select;
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

            // 处理键盘输入
            ctx.input(|input| {
                for event in &input.events {
                    if let egui::Event::Text(text) = event && !text.chars().next().map_or(false, |c| c.is_control())  {
                        text_state.text.push_str(text);
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
        if let (Some(text_state), Some(mut annotation)) = (self.text_input.take(), self.current_annotation.take()) {
            annotation.text = text_state.text;
            if !annotation.text.trim().is_empty() || annotation.points.len() > 1 {
                self.annotations.push(annotation);
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

        let mouse_min_x = self.mouse_start.0.min(self.mouse_end.0);
        let mouse_min_y = self.mouse_start.1.min(self.mouse_end.1);
        let mouse_max_x = self.mouse_start.0.max(self.mouse_end.0);
        let mouse_max_y = self.mouse_start.1.max(self.mouse_end.1);

        self.mouse_selection_rect = Some(MouseSelectionRect {
            start: (mouse_min_x, mouse_min_y),
            end: (mouse_max_x, mouse_max_y)
        });

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

    fn start_annotation(&mut self, pos: Pos2, mouse_pos: MousePosition) {
        self.current_annotation = Some(Annotation {
            tool: self.current_tool,
            points: vec![pos],
            mouse_points: vec![mouse_pos],
            color: self.annotation_color,
            stroke_width: self.brush_size,
            text: "".to_string(),
            number: self.number_input,
        });
    }
}