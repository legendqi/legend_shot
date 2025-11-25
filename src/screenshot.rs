// use arboard::Clipboard;
// use device_query::{DeviceQuery, DeviceState, MousePosition};
// use eframe::App;
// use eframe::emath::{Pos2, Rect, Vec2};
// use eframe::epaint::{Color32, Hsva, Shape, Stroke, StrokeKind};
// use egui::{color_picker, text_selection, Button, Id, Popup, PopupCloseBehavior, Response, Ui};
// use egui::color_picker::color_picker_hsva_2d;
// use image::{ImageBuffer, Rgba};
// use xcap::Monitor;
// use crate::ui::{get_screen_rect, load_texture_from_png};
// 
// #[derive(Clone, Copy, PartialEq, Debug)]
// pub enum Tool {
//     Select,
//     Pen,
//     Rectangle,
//     Arrow,
//     Text,
//     MoveBox,
//     Number,
//     Mosaic,
//     ColorPicker,
//     Button
// }
// 
// #[derive(Clone)]
// pub struct Annotation {
//     pub tool: Tool,
//     pub points: Vec<Pos2>,
//     pub color: Color32,
//     pub stroke_width: f32,
//     pub text: String,
//     pub number: Option<i32>,
// }
// 
// #[derive(Clone)]
// pub struct TextInputState {
//     pub position: Pos2,
//     pub text: String,
//     pub is_active: bool,
//     pub widget_id: Id, // 添加widget_id用于焦点管理
//     pub has_focus: bool, // 新增：跟踪焦点状态
//     pub last_interaction_time: f64,
// }
// 
// // 在创建TextInputState时初始化widget_id
// impl TextInputState {
//     pub fn new(position: Pos2) -> Self {
//         Self {
//             position,
//             text: String::new(),
//             is_active: true,
//             widget_id: Id::new(format!("text_input_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())), // 使用固定ID或生成唯一ID
//             has_focus: false,
//             last_interaction_time: 0.0,
//         }
//     }
// }
// 
// #[derive(Clone, Copy)]
// pub struct MouseSelectionRect {
//     pub start: MousePosition,
//     pub end: MousePosition,
// }
// 
// 
// pub struct ScreenshotApp {
//     pub screens: Vec<Monitor>,
//     pub screenshots: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>,
//     pub display_textures: Vec<egui::TextureHandle>,
//     pub original_selection_rect: Option<Rect>,
//     pub mouse_original_selection_rect: Option<MouseSelectionRect>,
// 
// 
//     // 选择状态
//     pub selection_rect: Option<Rect>,
//     pub mouse_selection_rect: Option<MouseSelectionRect>,
//     pub is_selecting: bool,
//     pub selection_start: Pos2,
//     pub selection_end: Pos2,
//     pub mouse_start: MousePosition, // 添加鼠标位置变量 窗口中鼠标位置和屏幕中鼠标位置的坐标不一样，导致最后截图不对，故添加此参数
//     pub mouse_end: MousePosition, // 添加鼠标位置变量 窗口中鼠标位置和屏幕中鼠标位置的坐标不一样，导致最后截图不对，故添加此参数
//     pub is_moving_box: bool,
//     pub move_start: Pos2,
//     pub mouse_move_start: MousePosition,
// 
//     // 标注状态
//     pub current_tool: Tool,
//     pub annotations: Vec<Annotation>,
//     pub current_annotation: Option<Annotation>,
//     pub  brush_size: f32,
//     pub annotation_color: Color32,
//     pub text_input: Option<TextInputState>,
//     pub number_input: Option<i32>,
//     pub tool_bar_focused: bool, // 添加工具栏焦点状态，主要是为了处理框选全屏时，工具栏在选框内部，工具栏无法点击的问题
// 
//     // UI 状态
//     pub show_toolbar: bool,
//     pub toolbar_position: Pos2,
//     // 修复：窗口尺寸
//     pub window_rect: Rect,
// 
//     // 新增：文本输入完成标记
//     pub text_input_finalized: bool,
//     pub device_state: DeviceState,
// 
//     pub screen_with: i32, // 屏幕宽度
//     pub screen_height: i32, // 屏幕高度
// }
// 
// impl Default for ScreenshotApp {
//     fn default() -> Self {
//         Self {
//             screens: Vec::new(),
//             screenshots: Vec::new(),
//             display_textures: Vec::new(),
//             original_selection_rect: None,
//             mouse_original_selection_rect: None,
//             selection_rect: None,
//             mouse_selection_rect: None,
//             is_selecting: false,
//             selection_start: Pos2::ZERO,
//             selection_end: Pos2::ZERO,
//             mouse_start: (0, 0),
//             mouse_end: (0, 0),
//             is_moving_box: false,
//             move_start: Pos2::ZERO,
//             mouse_move_start: (0, 0),
//             current_tool: Tool::Select,
//             annotations: Vec::new(),
//             current_annotation: None,
//             brush_size: 3.0,
//             annotation_color: Color32::RED,
//             text_input: None,
//             number_input: None,
//             tool_bar_focused: false,
//             show_toolbar: false,
//             toolbar_position: Pos2::ZERO,
//             window_rect: Rect::NOTHING,
//             text_input_finalized: false,
//             device_state: DeviceState::new(),
//             screen_with: 0,
//             screen_height: 0,
//         }
//     }
// }
// 
// impl ScreenshotApp {
//     pub(crate) fn capture_screens(&mut self, ctx: &egui::Context) -> Result<(), Box<dyn std::error::Error>> {
//         self.screens = Monitor::all()?;
//         self.screenshots.clear();
//         self.display_textures.clear();
// 
//         for screen in &self.screens {
//             let image = screen.capture_image()?;
//             self.screen_with = image.width() as i32;
//             self.screen_height = image.height() as i32;
//             // 转换为 image crate 的格式
//             let img_buffer = ImageBuffer::from_raw(
//                 image.width(),
//                 image.height(),
//                 image.to_vec(),
//             ).ok_or("Failed to create image buffer")?;
//             self.screenshots.push(img_buffer);
// 
//             // 创建 egui 纹理
//             let texture = ctx.load_texture(
//                 format!("screen_{}", self.display_textures.len()),
//                 egui::ColorImage::from_rgba_unmultiplied(
//                     [image.width() as usize, image.height() as usize],
//                     &image.to_vec(),
//                 ),
//                 egui::TextureOptions::LINEAR,
//             );
// 
//             self.display_textures.push(texture);
//         }
// 
//         Ok(())
//     }
// 
//     pub fn get_combined_bounds(&self) -> Rect {
//         if self.screens.is_empty() {
//             return Rect::NOTHING;
//         }
// 
//         let mut min_x = i32::MAX;
//         let mut min_y = i32::MAX;
//         let mut max_x = i32::MIN;
//         let mut max_y = i32::MIN;
// 
//         for screen in &self.screens {
//             min_x = min_x.min(screen.x().unwrap());
//             min_y = min_y.min(screen.y().unwrap());
//             max_x = max_x.max(screen.x().unwrap() + screen.width().unwrap() as i32);
//             max_y = max_y.max(screen.y().unwrap() + screen.height().unwrap() as i32);
//         }
// 
//         Rect::from_min_max(
//             Pos2::new(min_x as f32, min_y as f32),
//             Pos2::new(max_x as f32, max_y as f32),
//         )
//     }
// }
// 
// impl App for ScreenshotApp {
//     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//         self.window_rect = ctx.viewport_rect();
//         // 首次运行截图
//         if self.screenshots.is_empty() {
//             if let Err(e) = self.capture_screens(ctx) {
//                 eprintln!("Failed to capture screens: {}", e);
//             }
//         }
//         // let mut style = ctx.style();
//         // style.visuals.text_cursor.stroke.color = self.annotation_color;
//         // ctx.set_style(style);
//         // 主界面
//         egui::Area::new(Id::from("screenshot_area".to_string()))
//             .order(egui::Order::Background)
//             .fixed_pos(Pos2::ZERO).show(ctx, |ui| {
//             self.draw_screens(ui);
//             self.draw_overlay(ui);        // 覆盖层和选择框
//             self.draw_annotations(ui);    // 标注在覆盖层之上
//             self.handle_input(ui, ctx);   // 输入处理，包括更新选择框和标注
//             self.draw_text_input(ui);     // 添加文本输入UI
// 
//             if self.show_toolbar {
//                 self.draw_toolbar(ctx);
//             }
//         });
//     }
// }
// 
// impl ScreenshotApp {
//     pub(crate) fn draw_screens(&self, ui: &mut egui::Ui) {
//         for (_i, (screen, texture)) in self.screens.iter().zip(&self.display_textures).enumerate() {
//             let screen_rect = get_screen_rect(screen);
// 
//             // 绘制屏幕截图
//             ui.put(screen_rect, egui::Image::new(texture).fit_to_exact_size(screen_rect.size()));
//         }
//     }
// 
//     pub(crate) fn draw_overlay(&mut self, ui: &mut egui::Ui) {
//         let combined_bounds = self.get_combined_bounds();
// 
//         // 绘制半透明灰色覆盖层，但排除选择区域
//         if let Some(selection) = self.selection_rect {
//             let selection_min = selection.min.max(combined_bounds.min);
//             let selection_max = selection.max.min(combined_bounds.max);
//             let clipped_selection = Rect::from_min_max(selection_min, selection_max);
// 
//             if clipped_selection.area() > 0.0 {
//                 let overlay_color = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
// 
//                 // 将覆盖层分割成4个矩形区域（选择区域周围的区域）
//                 // 上方区域
//                 if combined_bounds.min.y < clipped_selection.min.y {
//                     let top_rect = Rect::from_min_max(
//                         combined_bounds.min,
//                         Pos2::new(combined_bounds.max.x, clipped_selection.min.y)
//                     );
//                     ui.painter().add(Shape::rect_filled(
//                         top_rect,
//                         egui::CornerRadius::ZERO,
//                         overlay_color,
//                     ));
//                 }
// 
//                 // 下方区域
//                 if combined_bounds.max.y > clipped_selection.max.y {
//                     let bottom_rect = Rect::from_min_max(
//                         Pos2::new(combined_bounds.min.x, clipped_selection.max.y),
//                         combined_bounds.max
//                     );
//                     ui.painter().add(Shape::rect_filled(
//                         bottom_rect,
//                         egui::CornerRadius::ZERO,
//                         overlay_color,
//                     ));
//                 }
// 
//                 // 左方区域
//                 if combined_bounds.min.x < clipped_selection.min.x {
//                     let left_rect = Rect::from_min_max(
//                         Pos2::new(combined_bounds.min.x, clipped_selection.min.y),
//                         Pos2::new(clipped_selection.min.x, clipped_selection.max.y)
//                     );
//                     ui.painter().add(Shape::rect_filled(
//                         left_rect,
//                         egui::CornerRadius::ZERO,
//                         overlay_color,
//                     ));
//                 }
// 
//                 // 右方区域
//                 if combined_bounds.max.x > clipped_selection.max.x {
//                     let right_rect = Rect::from_min_max(
//                         Pos2::new(clipped_selection.max.x, clipped_selection.min.y),
//                         Pos2::new(combined_bounds.max.x, clipped_selection.max.y)
//                     );
//                     ui.painter().add(Shape::rect_filled(
//                         right_rect,
//                         egui::CornerRadius::ZERO,
//                         overlay_color,
//                     ));
//                 }
// 
//                 // 绘制选择框边框
//                 ui.painter().add(Shape::rect_stroke(
//                     clipped_selection,
//                     egui::CornerRadius::ZERO,
//                     Stroke::new(2.0, Color32::RED),
//                     StrokeKind::Inside
//                 ));
//             } else {
//                 // 选择区域完全在边界外，绘制完整覆盖层
//                 ui.painter().add(Shape::rect_filled(
//                     combined_bounds,
//                     egui::CornerRadius::ZERO,
//                     Color32::from_rgba_unmultiplied(0, 0, 0, 100),
//                 ));
//             }
//         } else {
//             // 没有选择时绘制完整覆盖层
//             ui.painter().add(Shape::rect_filled(
//                 combined_bounds,
//                 egui::CornerRadius::ZERO,
//                 Color32::from_rgba_unmultiplied(0, 0, 0, 100),
//             ));
//         }
//     }
// 
// 
//     pub(crate) fn handle_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
//         let pointer_pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO);
//         let mouse_pos = self.device_state.get_mouse().coords;
//         // 如果有活动的文本输入，优先处理文本输入
//         if let Some(text_state) = &mut self.text_input && text_state.is_active {
//             // 文本输入激活时，不处理其他工具
//             self.handle_text_input(ui, ctx);
//             // 如果文本输入已经完成，立即返回
//             if self.text_input_finalized {
//                 self.finalize_text_input();
//                 return;
//             }
//         }
// 
//         // 鼠标按下开始选择
//         if ui.input(|i| i.pointer.primary_pressed()) {
//             if self.current_tool == Tool::Select && !self.show_toolbar {
//                 self.is_selecting = true;
//                 self.selection_start = pointer_pos;
//                 self.selection_end = pointer_pos;
//                 self.mouse_start = mouse_pos;
//                 self.mouse_end = mouse_pos;
//                 // !self.tool_bar_focused 要加载下面，不能放在前面判断然后return，不然会导致当前工具是文字标注，然后点击其他标注工具文字会跟随其他标注移动，也就是文字标注未结束
//             } else if self.current_tool == Tool::MoveBox && self.selection_rect.is_some() && !self.tool_bar_focused {
//                 // 开始移动选择框
//                 if self.selection_rect.unwrap().contains(pointer_pos) {
//                     self.is_moving_box = true;
//                     self.show_toolbar = false;
//                     self.move_start = pointer_pos;
//                     self.mouse_move_start = mouse_pos;
//                     // 保存选择框的原始位置
//                     self.original_selection_rect = self.selection_rect;
//                     self.mouse_original_selection_rect = self.mouse_selection_rect;
//                 }
//             } else if self.current_tool != Tool::Select && self.current_tool != Tool::MoveBox {
//                 // 检查是否在选择区域内才允许开始标注
//                 if let Some(selection_rect) = self.selection_rect {
//                     if selection_rect.contains(pointer_pos) {
//                         // 如果是文本工具，开始文本输入
//                         if self.current_tool == Tool::Text {
//                             if self.text_input.is_none() {
//                                 // 创建文本输入状态
//                                 let text_state = TextInputState::new(pointer_pos);
//                                 self.text_input = Some(text_state);
//                                 self.text_input_finalized = false;
//                                 self.start_annotation(pointer_pos);
//                             } else {
//                                 self.finalize_text_input();
//                             }
//                         } else if self.current_tool == Tool::Number {
//                             if self.number_input.is_none() {
//                                 // 创建数字输入状态
//                                 self.number_input = Some(1);
//                             } else {
//                                 self.number_input = Some(self.number_input.unwrap() + 1)
//                             }
//                             self.start_annotation(pointer_pos);
//                         }
//                         else {
//                             // 否则，开始标注
//                             self.start_annotation(pointer_pos);
//                         }
//                     } else {
//                         // 点击区域外的地方, 取消文本输入
//                         if self.text_input.is_some() || self.current_tool == Tool::Text {
//                             // 创建文本输入状态
//                             self.finalize_text_input();
//                         }
//                     }
//                 }
//             }
//         }
// 
//         // 处理键盘输入（仅在文本输入激活时）
//         if let Some(text_state) = &mut self.text_input && text_state.is_active  {
//             ctx.input(|input| {
//                 // 处理字符输入
//                 for event in &input.events {
//                     match event {
//                         egui::Event::Text(text) => {
//                             // 过滤控制字符，只添加可打印字符
//                             if !text.chars().next().map_or(false, |c| c.is_control()) {
//                                 text_state.text.push_str(text);
//                             }
//                         }
//                         _ => {}
//                     }
//                 }
// 
//                 // 处理特殊键
//                 if input.key_pressed(egui::Key::Enter) {
//                     text_state.text.push('\n');
//                 }
// 
//                 if input.key_pressed(egui::Key::Backspace) {
//                     text_state.text.pop();
//                 }
//             });
// 
//             // ESC 键取消文本输入
//             if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
//                 self.text_input = None;
//                 self.current_annotation = None; // 同时取消当前标注
//             }
//         }
// 
//         // 鼠标拖动
//         if ui.input(|i| i.pointer.primary_down()) {
//             if self.is_selecting {
//                 self.selection_end = pointer_pos;
//                 self.mouse_end = mouse_pos;
// 
//                 self.update_selection_rect();
//             } else if self.is_moving_box {
//                 // 移动选择框 - 基于原始位置计算偏移
//                 if let (Some(original_rect), Some(mouse_original_rect), Some(combined_bounds)) = (self.original_selection_rect, self.mouse_original_selection_rect, Some(self.get_combined_bounds())) {
//                     let delta = pointer_pos - self.move_start;
//                     let mouse_delta = (mouse_pos.0 - self.mouse_move_start.0, mouse_pos.1 - self.mouse_move_start.1);
//                     // 应用偏移到原始位置
//                     let mut new_rect = original_rect;
//                     let mut new_mouse_rect: MouseSelectionRect = mouse_original_rect;
//                     new_rect.min += delta;
//                     new_rect.max += delta;
//                     new_mouse_rect.start.0 += mouse_delta.0;
//                     new_mouse_rect.end.0 += mouse_delta.0;
//                     new_mouse_rect.start.1 += mouse_delta.1;
//                     new_mouse_rect.end.1 += mouse_delta.1;
// 
//                     // 限制选择框在屏幕范围内
//                     new_rect.min = new_rect.min.max(combined_bounds.min);
//                     new_rect.max = new_rect.max.min(combined_bounds.max);
// 
//                     new_mouse_rect.start = new_mouse_rect.start.max((0, 0));
//                     new_mouse_rect.end = new_mouse_rect.end.min((self.screen_with, self.screen_height));
// 
//                     // 确保选择框大小不变
//                     let width = original_rect.width();
//                     let height = original_rect.height();
// 
//                     let mouse_width = mouse_original_rect.end.0 - mouse_original_rect.start.0;
//                     let mouse_height = mouse_original_rect.end.1 - mouse_original_rect.start.1;
//                     // 限制选择框在屏幕范围内，考虑选择框的大小
//                     let max_x = combined_bounds.max.x - width;
//                     let max_y = combined_bounds.max.y - height;
// 
//                     let max_mouse_x = self.screen_with - mouse_width;
//                     let max_mouse_y = self.screen_height - mouse_height;
// 
//                     new_rect.min.x = new_rect.min.x.clamp(combined_bounds.min.x, max_x);
//                     new_rect.min.y = new_rect.min.y.clamp(combined_bounds.min.y, max_y);
// 
//                     new_mouse_rect.start.0 = new_mouse_rect.start.0.clamp(0, max_mouse_x);
//                     new_mouse_rect.start.1 = new_mouse_rect.start.1.clamp(0, max_mouse_y);
// 
//                     // 根据调整后的min重新计算max
//                     new_rect.max.x = new_rect.min.x + width;
//                     new_rect.max.y = new_rect.min.y + height;
// 
//                     new_mouse_rect.end.0 = new_mouse_rect.start.0 + mouse_width;
//                     new_mouse_rect.end.1 = new_mouse_rect.start.1 + mouse_height;
// 
//                     self.selection_rect = Some(new_rect);
//                     self.mouse_selection_rect = Some(new_mouse_rect);
// 
//                     // ✅ 更新 selection_start 和 selection_end
//                     self.selection_start = new_rect.min;
//                     self.selection_end = new_rect.max;
// 
//                     self.mouse_start = new_mouse_rect.start;
//                     self.mouse_end = new_mouse_rect.end;
//                     // println!("mouse_start: {:?}, mouse_end: {:?}", self.mouse_start, self.mouse_end);
// 
//                     // 更新工具栏位置
//                     self.update_toolbar_position(new_rect);
//                 }
//             } else if let Some(ref mut annotation) = self.current_annotation {
//                 // 检查拖动点是否在选择区域内
//                 if let Some(selection_rect) = self.selection_rect {
//                     if selection_rect.contains(pointer_pos) {
//                         annotation.points.push(pointer_pos);
//                     }
//                 }
//             }
//         }
// 
//         // 鼠标释放
//         if ui.input(|i| i.pointer.primary_released()) {
//             if self.is_selecting {
//                 self.is_selecting = false;
//                 self.selection_end = pointer_pos;
//                 self.mouse_end = mouse_pos;
//                 self.update_selection_rect();
//                 if let Some(rect) = self.selection_rect {
//                     if rect.area() > 100.0 { // 最小区域阈值
//                         self.show_toolbar = true;
//                         self.update_toolbar_position(rect);
//                     }
//                 }
//             } else if self.is_moving_box && self.current_tool == Tool::MoveBox {
//                 self.is_moving_box = false;
//                 self.show_toolbar = true;
//                 self.original_selection_rect = None;
//                 self.mouse_original_selection_rect = None;
//             } else if let Some(annotation) = &self.current_annotation {
//                 // 对于非文本工具，直接完成标注
//                 if self.current_tool != Tool::Text {
//                     if annotation.points.len() > 1 {
//                         self.annotations.push(annotation.clone());
//                     }
//                     self.current_annotation = None;
//                 }
//                 // 文本工具的完成由文本输入处理
//             }
//         }
// 
//         // ESC 键退出
//         if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
//             if let Some(text_state) = &mut self.text_input {
//                 if text_state.is_active {
//                     self.text_input = None;
//                     return;
//                 }
//             }
//             if self.show_toolbar {
//                 self.show_toolbar = false;
//                 self.selection_rect = None;
//                 self.mouse_selection_rect = None;
//                 self.current_tool = Tool::Select;
//             } else {
//                 ctx.send_viewport_cmd(egui::ViewportCommand::Close);
//             }
//         }
//     }
// 
//     fn handle_text_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
//         if let Some(text_state) = &mut self.text_input.clone() {
//             if !text_state.is_active {
//                 return;
//             }
// 
//             // 确保焦点
//             if !text_state.has_focus {
//                 ui.memory_mut(|mem| mem.request_focus(text_state.widget_id));
//                 text_state.has_focus = true;
//             }
// 
//             // 处理键盘输入
//             ctx.input(|input| {
//                 for event in &input.events {
//                     if let egui::Event::Text(text) = event && !text.chars().next().map_or(false, |c| c.is_control())  {
//                         text_state.text.push_str(text);
//                     }
//                 }
// 
//                 if input.key_pressed(egui::Key::Enter) {
//                     if input.modifiers.ctrl {
//                         // Ctrl+Enter 完成输入
//                         self.finalize_text_input();
//                     } else {
//                         // 普通回车换行
//                         text_state.text.push('\n');
//                     }
//                 }
// 
//                 if input.key_pressed(egui::Key::Backspace) {
//                     text_state.text.pop();
//                 }
//             });
// 
//             // ESC 键取消
//             if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
//                 self.cancel_text_input();
//             }
//         }
//     }
// 
//     fn finalize_text_input(&mut self) {
//         if let (Some(text_state), Some(mut annotation)) = (self.text_input.take(), self.current_annotation.take()) {
//             annotation.text = text_state.text;
//             if !annotation.text.trim().is_empty() || annotation.points.len() > 1 {
//                 self.annotations.push(annotation);
//             }
//         }
//         self.text_input_finalized = false;
//     }
// 
//     fn cancel_text_input(&mut self) {
//         self.text_input = None;
//         self.current_annotation = None;
//         self.text_input_finalized = false;
//     }
// 
//     fn update_selection_rect(&mut self) {
//         let min_x = self.selection_start.x.min(self.selection_end.x);
//         let min_y = self.selection_start.y.min(self.selection_end.y);
//         let max_x = self.selection_start.x.max(self.selection_end.x);
//         let max_y = self.selection_start.y.max(self.selection_end.y);
// 
//         let mouse_min_x = self.mouse_start.0.min(self.mouse_end.0);
//         let mouse_min_y = self.mouse_start.1.min(self.mouse_end.1);
//         let mouse_max_x = self.mouse_start.0.max(self.mouse_end.0);
//         let mouse_max_y = self.mouse_start.1.max(self.mouse_end.1);
// 
//         self.mouse_selection_rect = Some(MouseSelectionRect {
//             start: (mouse_min_x, mouse_min_y),
//             end: (mouse_max_x, mouse_max_y)
//         });
// 
//         self.selection_rect = Some(Rect::from_min_max(
//             Pos2::new(min_x, min_y),
//             Pos2::new(max_x, max_y),
//         ));
//         // 框选完成后，设置默认工具为 MoveBox
//         if self.selection_rect.is_some() && self.current_tool != Tool::MoveBox {
//             self.current_tool = Tool::MoveBox;
//         }
//     }
// 
//     fn update_toolbar_position(&mut self, selection_rect: Rect) {
//         // 工具栏显示在选择框右下角
//         self.toolbar_position = Pos2::new(
//             selection_rect.max.x,
//             selection_rect.max.y,
//         );
//     }
// 
//     fn start_annotation(&mut self, pos: Pos2) {
//         self.current_annotation = Some(Annotation {
//             tool: self.current_tool,
//             points: vec![pos],
//             color: self.annotation_color,
//             stroke_width: self.brush_size,
//             text: "".to_string(),
//             number: self.number_input,
//         });
//     }
// }
// 
// impl ScreenshotApp {
// 
//     pub fn save_screenshot(&self) {
//         if self.selection_rect.is_none() {
//             return;
//         }
//         if let Some(cropped_image) = self.crop_selection(self.selection_rect.unwrap(), &self.annotations) {
//             let now = chrono::Local::now();
//             let filename = now.format("screenshot_%Y-%m-%d_%H-%M-%S.png").to_string();
//             let file_save_dialog = rfd::FileDialog::new();
//             let save_path = file_save_dialog.set_file_name(filename.as_str())
//                 .add_filter("PNG Image", &["png"])
//                 .add_filter("JPEG Image", &["jpg", "jpeg"])
//                 .save_file();
//             if let Some(path) = save_path {
//                 // 根据文件扩展名自动选择保存格式
//                 let format = match path.as_path().extension().and_then(|e| e.to_str()) {
//                     Some("jpg") | Some("jpeg") => image::ImageFormat::Jpeg,
//                     _ => image::ImageFormat::Png,
//                 };
//                 // 执行实际保存操作
//                 if let Err(e) = cropped_image.save_with_format(&path, format) {
//                     eprintln!("保存失败: {:?}", e);
//                 } else {
//                     println!("截图已成功保存至: {:?}", path.as_path());
//                 }
//             }
//         }
//     }
// 
//     pub fn copy_to_clipboard_agina(&self) {
//         if let Some(selection_rect) = self.selection_rect {
// 
//             // 确保当前文本输入完成
//             let mut annotations = self.annotations.clone();
//             if let (Some(text_state), Some(annotation)) = (&self.text_input, &self.current_annotation) {
//                 let mut new_annotation = annotation.clone();
//                 new_annotation.text = text_state.text.clone();
//                 annotations.push(new_annotation);
//             }
// 
//             if let Some(cropped_image) = self.crop_selection(selection_rect, &annotations) {
//                 // 转换为剪贴板格式
//                 if let Ok(mut clipboard) = Clipboard::new() {
//                     let image_data = arboard::ImageData {
//                         width: cropped_image.width() as usize,
//                         height: cropped_image.height() as usize,
//                         bytes: std::borrow::Cow::Borrowed(&cropped_image.as_raw()),
//                     };
// 
//                     if let Err(e) = clipboard.set_image(image_data) {
//                         eprintln!("Failed to copy to clipboard: {}", e);
//                     }
//                 }
//             }
//         }
//     }
// 
//     pub fn copy_to_clipboard(&self) -> Result<(), String> {
//         if self.selection_rect.is_none() {
//             return Err("请选择要复制的图片".to_string());
//         }
//         let x = self.selection_start.x.min(self.selection_end.x).min((self.screens[0].width().unwrap() - 1) as f32);
//         let y = self.selection_start.y.min(self.selection_end.y).min((self.screens[0].height().unwrap() - 1) as f32);
//         let width = (self.selection_start.x - self.selection_end.x).abs().min((self.screens[0].width().unwrap() - 1) as f32);
//         let height = (self.selection_start.y - self.selection_end.y).abs().min((self.screens[0].height().unwrap() - 1) as f32);
//         let image = self.screens[0].capture_region(x as u32, y as u32, width as u32, height as u32).map_err(|e| e.to_string())?;
//         let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
//         let img_data = arboard::ImageData {
//             width: image.width() as usize,
//             height: image.height() as usize,
//             bytes: std::borrow::Cow::Borrowed(image.as_raw()),
//         };
//         clipboard.set_image(img_data).map_err(|e| e.to_string())?;
//         Ok(())
//     }
// 
//     fn crop_selection(&self, selection_rect: Rect, annotations: &Vec<Annotation>) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
//         let x = self.mouse_start.0.min(self.mouse_end.0);
//         let y = self.mouse_start.1.min(self.mouse_end.1);
//         let width = (self.mouse_end.0 - self.mouse_start.0).abs();
//         let height = (self.mouse_end.1 - self.mouse_start.1).abs();
// 
//         // 创建新的图像缓冲区 - 直接使用选择框的大小
//         let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width as u32, height as u32);
//         // 用白色背景填充
//         for pixel in cropped_image.pixels_mut() {
//             *pixel = Rgba([255, 255, 255, 255]);
//         }
// 
//         // 查找包含选择区域的屏幕
//         for (screen, screenshot) in self.screens.iter().zip(&self.screenshots) {
//             let screen_rect = get_screen_rect(screen);
// 
//             if screen_rect.contains(selection_rect.center()) {
//                 // 计算在屏幕图像中的相对位置
// 
//                 if width > 0 && height > 0 {
//                     // 创建新的图像缓冲区
//                     let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width as u32, height as u32);
// 
//                     // 复制原始截图内容
//                     for src_y in y..(y + height) {
//                         for src_x in x..(x + width) {
//                             let dst_x = src_x - x;
//                             let dst_y = src_y - y;
// 
//                             let pixel = screenshot.get_pixel(src_x as u32, src_y as u32);
//                             cropped_image.put_pixel(dst_x.try_into().unwrap(), dst_y.try_into().unwrap(), pixel.clone());
//                         }
//                     }
// 
//                     // 添加标注内容
//                     self.add_annotations_to_image(&mut cropped_image, selection_rect, annotations);
// 
//                     return Some(cropped_image);
//                 }
//             }
//         }
//         None
//     }
// 
//     fn add_annotations_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, selection_rect: Rect, annotations: &Vec<Annotation>) {
//         for annotation in annotations {
//             self.draw_single_annotation_to_image(image, annotation, selection_rect);
//         }
//     }
// 
//     fn draw_single_annotation_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotation: &Annotation, selection_rect: Rect) {
//         if annotation.points.len() < 2 {
//             return;
//         }
// 
//         // 转换为相对于裁剪图像的坐标
//         let rel_rect = Rect::from_min_size(
//             Pos2::new(selection_rect.min.x, selection_rect.min.y),
//             Vec2::new(selection_rect.width(), selection_rect.height())
//         );
// 
//         let color = annotation.color;
// 
//         match annotation.tool {
//             Tool::Pen => {
//                 // 绘制自由画笔
//                 // 修复画笔断断续续问题：使用更平滑的Bresenham算法
//                 for window in annotation.points.windows(2) {
//                     if let [start, end] = window {
//                         let start_rel = Pos2::new(start.x - rel_rect.min.x, start.y - rel_rect.min.y);
//                         let end_rel = Pos2::new(end.x - rel_rect.min.x, end.y - rel_rect.min.y);
// 
//                         // 修复：使用浮点坐标转换确保连续
//                         self.draw_smooth_line(image, start_rel, end_rel, color);
//                     }
//                 }
//             }
//             Tool::Rectangle => {
//                 // 绘制矩形
//                 if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
//                     let rect = Rect::from_two_pos(start, end);
//                     let rect_rel = Rect::from_min_max(
//                         Pos2::new(rect.min.x - rel_rect.min.x, rect.min.y - rel_rect.min.y),
//                         Pos2::new(rect.max.x - rel_rect.min.x, rect.max.y - rel_rect.min.y)
//                     );
// 
//                     // 绘制矩形边框
//                     let x1 = rect_rel.min.x as usize;
//                     let y1 = rect_rel.min.y as usize;
//                     let x2 = rect_rel.max.x as usize;
//                     let y2 = rect_rel.max.y as usize;
// 
//                     // 顶部和底部边
//                     for x in x1..=x2 {
//                         if y1 < image.height() as usize {
//                             image.put_pixel(x as u32, y1 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//                         }
//                         if y2 < image.height() as usize {
//                             image.put_pixel(x as u32, y2 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//                         }
//                     }
// 
//                     // 左侧和右侧边
//                     for y in y1..=y2 {
//                         if x1 < image.width() as usize {
//                             image.put_pixel(x1 as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//                         }
//                         if x2 < image.width() as usize {
//                             image.put_pixel(x2 as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//                         }
//                     }
//                 }
//             }
//             Tool::Arrow => {
//                 // 修复：实心箭头绘制
//                 if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
//                     let start_rel = Pos2::new(
//                         (start.x - rel_rect.min.x).max(0.0),
//                         (start.y - rel_rect.min.y).max(0.0)
//                     );
//                     let end_rel = Pos2::new(
//                         (end.x - rel_rect.min.x).max(0.0),
//                         (end.y - rel_rect.min.y).max(0.0)
//                     );
// 
//                     // 绘制箭头线
//                     self.draw_smooth_line(image, start_rel, end_rel, color);
// 
//                     // 绘制实心箭头头
//                     self.draw_filled_arrow_head(image, end_rel, start_rel, color);
//                 }
//             }
//             Tool::Text => {
//                 // 改进的文本绘制
//                 if let Some(&pos) = annotation.points.first() {
//                     let pos_rel = Pos2::new(
//                         (pos.x - rel_rect.min.x).max(0.0),
//                         (pos.y - rel_rect.min.y).max(0.0)
//                     );
// 
//                     if !annotation.text.is_empty() {
//                         self.draw_text(image, pos_rel, &annotation.text, color);
//                     }
//                 }
//             }
//             Tool::Mosaic => {
//                 // 马赛克绘制
//                 if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
//                     let rect = Rect::from_two_pos(start, end);
//                     let rect_rel = Rect::from_min_max(
//                         Pos2::new(
//                             (rect.min.x - rel_rect.min.x).max(0.0),
//                             (rect.min.y - rel_rect.min.y).max(0.0)
//                         ),
//                         Pos2::new(
//                             (rect.max.x - rel_rect.min.x).min(image.width() as f32),
//                             (rect.max.y - rel_rect.min.y).min(image.height() as f32)
//                         )
//                     );
// 
//                     self.draw_mosaic(image, rect_rel, 8); // 8x8像素的马赛克块
//                 }
//             }
//             Tool::Number => {
//                 // 序号绘制
//                 if let Some(&pos) = annotation.points.first() {
//                     let pos_rel = Pos2::new(
//                         (pos.x - rel_rect.min.x).max(0.0),
//                         (pos.y - rel_rect.min.y).max(0.0)
//                     );
// 
//                     if !annotation.text.is_empty() {
//                         self.draw_number(image, pos_rel, &annotation.text, color);
//                     }
//                 }
//             }
//             _ => {}
//         }
//     }
// 
//     // 修复画笔断断续续问题：使用更平滑的Bresenham算法
//     fn draw_smooth_line(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, start: Pos2, end: Pos2, color: Color32) {
//         let mut x0 = start.x as i32;
//         let mut y0 = start.y as i32;
//         let x1 = end.x as i32;
//         let y1 = end.y as i32;
// 
//         let dx = (x1 - x0).abs();
//         let dy = (y1 - y0).abs();
//         let sx = if x0 < x1 { 1 } else { -1 };
//         let sy = if y0 < y1 { 1 } else { -1 };
//         let mut err = dx - dy;
// 
//         loop {
//             // 确保坐标在图像范围内
//             if x0 >= 0 && x0 < image.width() as i32 && y0 >= 0 && y0 < image.height() as i32 {
//                 image.put_pixel(x0 as u32, y0 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//             }
// 
//             if x0 == x1 && y0 == y1 {
//                 break;
//             }
// 
//             let e2 = 2 * err;
//             if e2 > -dy {
//                 err -= dy;
//                 x0 += sx;
//             }
//             if e2 < dx {
//                 err += dx;
//                 y0 += sy;
//             }
//         }
//     }
// 
//     // 修复箭头实心问题：绘制实心箭头
//     fn draw_filled_arrow_head(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, tip: Pos2, from: Pos2, color: Color32) {
//         let arrow_length = 15.0;
//         let arrow_angle = std::f32::consts::PI / 6.0; // 30度
// 
//         let dx = tip.x - from.x;
//         let dy = tip.y - from.y;
//         let length = (dx * dx + dy * dy).sqrt();
// 
//         if length < f32::EPSILON {
//             return;
//         }
// 
//         // 方向向量
//         let dir_x = dx / length;
//         let dir_y = dy / length;
// 
//         // 计算箭头两侧的点
//         let left_x = tip.x - arrow_length * (dir_x * arrow_angle.cos() - dir_y * arrow_angle.sin());
//         let left_y = tip.y - arrow_length * (dir_x * arrow_angle.sin() + dir_y * arrow_angle.cos());
// 
//         let right_x = tip.x - arrow_length * (dir_x * arrow_angle.cos() + dir_y * arrow_angle.sin());
//         let right_y = tip.y - arrow_length * (-dir_x * arrow_angle.sin() + dir_y * arrow_angle.cos());
// 
//         // 创建三角形点
//         let points = [
//             (tip.x, tip.y),
//             (left_x, left_y),
//             (right_x, right_y)
//         ];
// 
//         // 用Bresenham绘制三角形
//         self.draw_filled_triangle(image, points, color);
//     }
// 
//     // 绘制实心三角形（填充）
//     fn draw_filled_triangle(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, points: [(f32, f32); 3], color: Color32) {
//         // 计算三角形边界
//         let x_min = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
//         let x_max = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
//         let y_min = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
//         let y_max = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
// 
//         // 遍历边界框内的每个像素
//         for y in (y_min as i32)..=(y_max as i32) {
//             for x in (x_min as i32)..=(x_max as i32) {
//                 if self.point_in_triangle(x as f32, y as f32, points) {
//                     // 确保在图像范围内
//                     if x >= 0 && x < image.width() as i32 && y >= 0 && y < image.height() as i32 {
//                         image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//                     }
//                 }
//             }
//         }
//     }
// 
//     // 判断点是否在三角形内（射线法）
//     fn point_in_triangle(&self, x: f32, y: f32, points: [(f32, f32); 3]) -> bool {
//         let (a, b, c) = (points[0], points[1], points[2]);
// 
//         // 检查射线与三角形边的交点
//         let mut intersections = 0;
// 
//         // 检查边ab
//         if self.intersects_ray(x, y, a, b) {
//             intersections += 1;
//         }
// 
//         // 检查边bc
//         if self.intersects_ray(x, y, b, c) {
//             intersections += 1;
//         }
// 
//         // 检查边ca
//         if self.intersects_ray(x, y, c, a) {
//             intersections += 1;
//         }
// 
//         intersections % 2 == 1
//     }
// 
//     // 检查射线是否与线段相交
//     fn intersects_ray(&self, x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> bool {
//         // 检查线段是否与射线相交
//         if (a.1 <= y && b.1 > y) || (a.1 > y && b.1 <= y) {
//             // 计算交点x坐标
//             let t = (y - a.1) / (b.1 - a.1);
//             let intersect_x = a.0 + t * (b.0 - a.0);
// 
//             // 检查交点是否在射线方向（x >= 当前点x）
//             intersect_x > x
//         } else {
//             false
//         }
//     }
// 
//     // Bresenham直线绘制算法
//     fn draw_line_bresenham(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, start: Pos2, end: Pos2, color: Color32) {
//         let mut x0 = start.x as i32;
//         let mut y0 = start.y as i32;
//         let x1 = end.x as i32;
//         let y1 = end.y as i32;
// 
//         let dx = (x1 - x0).abs();
//         let dy = -(y1 - y0).abs();
//         let sx = if x0 < x1 { 1 } else { -1 };
//         let sy = if y0 < y1 { 1 } else { -1 };
//         let mut err = dx + dy;
// 
//         loop {
//             if x0 >= 0 && x0 < image.width() as i32 && y0 >= 0 && y0 < image.height() as i32 {
//                 image.put_pixel(x0 as u32, y0 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//             }
// 
//             if x0 == x1 && y0 == y1 {
//                 break;
//             }
// 
//             let e2 = 2 * err;
//             if e2 >= dy {
//                 err += dy;
//                 x0 += sx;
//             }
//             if e2 <= dx {
//                 err += dx;
//                 y0 += sy;
//             }
//         }
//     }
// 
//     // 绘制箭头头部
//     fn draw_arrow_head(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, tip: Pos2, from: Pos2, color: Color32) {
//         let arrow_length = 15.0;
//         let arrow_angle = std::f32::consts::PI / 6.0; // 30度
// 
//         let dx = tip.x - from.x;
//         let dy = tip.y - from.y;
//         let length = (dx * dx + dy * dy).sqrt();
// 
//         if length < f32::EPSILON {
//             return;
//         }
// 
//         // 方向向量
//         let dir_x = dx / length;
//         let dir_y = dy / length;
// 
//         // 计算箭头两侧的点
//         let left_x = tip.x - arrow_length * (dir_x * arrow_angle.cos() - dir_y * arrow_angle.sin());
//         let left_y = tip.y - arrow_length * (dir_x * arrow_angle.sin() + dir_y * arrow_angle.cos());
// 
//         let right_x = tip.x - arrow_length * (dir_x * arrow_angle.cos() + dir_y * arrow_angle.sin());
//         let right_y = tip.y - arrow_length * (-dir_x * arrow_angle.sin() + dir_y * arrow_angle.cos());
// 
//         // 绘制箭头两侧的线
//         self.draw_line_bresenham(image, tip, Pos2::new(left_x, left_y), color);
//         self.draw_line_bresenham(image, tip, Pos2::new(right_x, right_y), color);
//     }
// 
//     // 简单的文本绘制（使用位图字体或简单图形）
//     fn draw_text(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, pos: Pos2, text: &str, color: Color32) {
//         // 这里可以使用位图字体库，或者简单的字符绘制
//         // 示例：绘制简单的矩形文字背景和文字轮廓
//         let x = pos.x as i32;
//         let y = pos.y as i32;
// 
//         // 简单绘制文字边框（实际项目中应该使用字体渲染）
//         for (i, ch) in text.chars().enumerate() {
//             let char_x = x + i as i32 * 8;
//             self.draw_simple_char(image, char_x, y, ch, color);
//         }
//     }
// 
//     // 绘制序号（带圆圈的数字）
//     fn draw_number(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, pos: Pos2, number: &str, color: Color32) {
//         let x = pos.x as i32;
//         let y = pos.y as i32;
//         let radius = 10;
// 
//         // 绘制圆形背景
//         self.draw_circle(image, x, y, radius, color);
// 
//         // 绘制数字（居中）
//         if let Some(first_char) = number.chars().next() {
//             let char_x = x - 3;
//             let char_y = y - 3;
//             self.draw_simple_char(image, char_x, char_y, first_char, Color32::WHITE);
//         }
//     }
// 
//     // 绘制圆形
//     fn draw_circle(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, center_x: i32, center_y: i32, radius: i32, color: Color32) {
//         let mut x = radius;
//         let mut y = 0;
//         let mut err = 0;
// 
//         while x >= y {
//             self.draw_circle_points(image, center_x, center_y, x, y, color);
// 
//             y += 1;
//             err += 1 + 2 * y;
//             if 2 * (err - x) + 1 > 0 {
//                 x -= 1;
//                 err += 1 - 2 * x;
//             }
//         }
//     }
// 
//     // 绘制序号
//     fn draw_circle_points(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, cx: i32, cy: i32, x: i32, y: i32, color: Color32) {
//         let points = [
//             (cx + x, cy + y), (cx - x, cy + y),
//             (cx + x, cy - y), (cx - x, cy - y),
//             (cx + y, cy + x), (cx - y, cy + x),
//             (cx + y, cy - x), (cx - y, cy - x),
//         ];
// 
//         for (px, py) in points.iter() {
//             if *px >= 0 && *px < image.width() as i32 && *py >= 0 && *py < image.height() as i32 {
//                 image.put_pixel(*px as u32, *py as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//             }
//         }
//     }
// 
//     // 马赛克效果
//     fn draw_mosaic(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, rect: Rect, block_size: u32) {
//         let x1 = rect.min.x as u32;
//         let y1 = rect.min.y as u32;
//         let x2 = rect.max.x as u32;
//         let y2 = rect.max.y as u32;
// 
//         for block_y in (y1..y2).step_by(block_size as usize) {
//             for block_x in (x1..x2).step_by(block_size as usize) {
//                 let block_end_x = (block_x + block_size).min(x2);
//                 let block_end_y = (block_y + block_size).min(y2);
// 
//                 // 计算块内的平均颜色
//                 let (mut r_sum, mut g_sum, mut b_sum, mut count) = (0u32, 0u32, 0u32, 0u32);
// 
//                 for y in block_y..block_end_y {
//                     for x in block_x..block_end_x {
//                         if x < image.width() && y < image.height() {
//                             let pixel = image.get_pixel(x, y);
//                             r_sum += pixel[0] as u32;
//                             g_sum += pixel[1] as u32;
//                             b_sum += pixel[2] as u32;
//                             count += 1;
//                         }
//                     }
//                 }
// 
//                 if count > 0 {
//                     let avg_r = (r_sum / count) as u8;
//                     let avg_g = (g_sum / count) as u8;
//                     let avg_b = (b_sum / count) as u8;
// 
//                     // 用平均颜色填充整个块
//                     for y in block_y..block_end_y {
//                         for x in block_x..block_end_x {
//                             if x < image.width() && y < image.height() {
//                                 image.put_pixel(x, y, Rgba([avg_r, avg_g, avg_b, 255]));
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//     }
// 
//     // 简单字符绘制（用于文本和数字）
//     fn draw_simple_char(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: i32, y: i32, ch: char, color: Color32) {
//         // 这里可以实现简单的位图字体
//         // 示例：为数字0-9和部分字母提供简单绘制
//         match ch {
//             '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | 'A'..='Z' | 'a'..='z' => {
//                 // 简单实现：绘制字符的边界框（实际应该使用位图字体）
//                 for i in 0..6 {
//                     for j in 0..8 {
//                         let px = x + i;
//                         let py = y + j;
//                         if px >= 0 && px < image.width() as i32 && py >= 0 && py < image.height() as i32 {
//                             // 简单模式：在边界绘制像素
//                             if i == 0 || i == 5 || j == 0 || j == 7 {
//                                 image.put_pixel(px as u32, py as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
//                             }
//                         }
//                     }
//                 }
//             }
//             _ => {}
//         }
//     }
// }
// 
// impl ScreenshotApp {
//     pub(crate) fn draw_toolbar(&mut self, ctx: &egui::Context) {
//         self.tool_bar_focused = false;
//         if let Some(selection_rect) = self.selection_rect {
//             let toolbar_size = Vec2::new(50.0, 40.0);
//             // 计算工具栏位置：在选择框右下角，并与选择框右对齐
//             let mut toolbar_pos = Pos2::new(
//                 selection_rect.min.x, // 左对齐：工具栏左侧与选择框左侧对齐
//                 selection_rect.max.y + 5.0,  // 在选择框下方，留 5.0 的间距
//             );
//             // 确保工具栏在屏幕内
//             let screen_rect = ctx.viewport_rect();
// 
//             // 如果工具栏超出右边界，向左调整
//             if toolbar_pos.x + toolbar_size.x > screen_rect.max.x {
//                 toolbar_pos.x = screen_rect.max.x - toolbar_size.x;
//             }
// 
//             // 如果工具栏超出左边界，确保至少显示一部分
//             if toolbar_pos.x < screen_rect.min.x {
//                 toolbar_pos.x = screen_rect.min.x + 10.0;
//             }
// 
//             // 如果工具栏超出下边界，显示在选择框上方
//             if toolbar_pos.y + toolbar_size.y > screen_rect.max.y {
//                 toolbar_pos.y = selection_rect.min.y - toolbar_size.y;
//             }
// 
//             // 如果工具栏超出上边界，确保至少显示一部分
//             if toolbar_pos.y < screen_rect.min.y {
//                 toolbar_pos.y = screen_rect.min.y + 10.0;
//             }
// 
// 
//             egui::Area::new(Id::from("annotation_toolbar".to_string()))
//                 .fixed_pos(toolbar_pos)
//                 .order(egui::Order::Foreground)
//                 .interactable(true)
//                 .show(ctx, |ui| {
//                     egui::Frame::NONE
//                         .show(ui, |ui| {
//                             ui.horizontal(|ui| {
//                                 // 工具选择
//                                 self.purple_icon_button(ui, Tool::MoveBox, ctx, "src/icon/move.png");
//                                 self.purple_icon_button(ui, Tool::Pen, ctx, "src/icon/pen.png");
//                                 self.purple_icon_button(ui, Tool::Rectangle, ctx, "src/icon/rectangle.png");
//                                 self.purple_icon_button(ui, Tool::Arrow, ctx, "src/icon/arrow.png");
//                                 self.purple_icon_button(ui, Tool::Text, ctx, "src/icon/word.png");
//                                 self.purple_icon_button(ui, Tool::Mosaic, ctx, "src/icon/mosaic.png");
//                                 self.purple_icon_button(ui, Tool::Number, ctx, "src/icon/number.png");
// 
//                                 // 颜色选择
//                                 self.custom_color_picker(ui, ctx);
//                                 // 画笔大小
//                                 // ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=20.0));
// 
//                                 // 操作： 复制，保存，退出
//                                 self.purple_icon_button(ui, Tool::Button, ctx, "src/icon/copy.png").clicked().then(|| {
//                                     // match self.copy_to_clipboard() {
//                                     //     Ok(_) => {
//                                     //         println!("复制成功");
//                                     //     },
//                                     //     Err(e) => {
//                                     //         println!("复制失败： {:?}", e);
//                                     //     }
//                                     // }
//                                     self.copy_to_clipboard_agina();
//                                     ctx.send_viewport_cmd(egui::ViewportCommand::Close);
//                                 });
// 
//                                 self.purple_icon_button(ui, Tool::Button, ctx, "src/icon/save.png").clicked().then(|| {
//                                     ctx.send_viewport_cmd(egui::ViewportCommand::Close);
//                                     self.save_screenshot();
//                                 });
// 
//                                 self.purple_icon_button(ui, Tool::Button, ctx, "src/icon/exit.png").clicked().then(|| {
//                                     ctx.send_viewport_cmd(egui::ViewportCommand::Close);
//                                 });
//                             });
//                         });
//                 });
// 
//         }
//     }
// 
//     fn custom_color_picker(&mut self, ui: &mut Ui, ctx: &egui::Context) {
//         let button_size = Vec2::new(30.0, 30.0);
// 
//         // 创建自定义按钮
//         let button = Button::new("")
//             .min_size(button_size)
//             .frame(false);
//         let color_pick_response = ui.add(button);
// 
//         let is_hovered_or_focused = color_pick_response.hovered() || color_pick_response.has_focus();
// 
//         // 颜色选择
//         let inner_radius = color_pick_response.rect.width() / 2.0;
//         // 画边框（紫色，半径为 inner_radius + 2.0）
//         if is_hovered_or_focused || self.current_tool == Tool::ColorPicker {
//             self.tool_bar_focused = true;
//             ui.painter().circle_filled(
//                 color_pick_response.rect.center(),
//                 inner_radius,
//                 Color32::from_rgb(128, 0, 128),
//             );
//         } else {
//             ui.painter().circle_filled(
//                 color_pick_response.rect.center(),
//                 inner_radius,
//                 Color32::from_rgb(128, 80, 128),
//             );
//         }
// 
//         ui.painter().circle_filled(
//             color_pick_response.rect.center(),
//             inner_radius - 4.0,
//             self.annotation_color,
//         );
//         // 👉 关键修复：生成ID放在点击逻辑外面（但确保在同一个UI帧）
//         let popup_id = ui.auto_id_with("color_popup");
//         let mut hsva = Hsva::from(self.annotation_color);
//         Popup::menu(&color_pick_response)
//             .id(popup_id)
//             .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
//             .show(|ui| {
//                 ui.add_space(10.0); // 必须加，否则不会显示
//                 ui.spacing_mut().slider_width = 275.0;
//                 if color_picker_hsva_2d(ui, & mut hsva, color_picker::Alpha::BlendOrAdditive) {
//                     self.annotation_color = Color32::from(hsva);
//                 }
//             });
//         // 处理点击：用 if 而不是 then！
//         if color_pick_response.clicked() {
//             self.current_tool = Tool::ColorPicker;
//             Popup::open_id(ctx, popup_id);
//         }
//     }
// 
//     pub fn purple_icon_button(&mut self, ui: &mut Ui, tool: Tool, ctx: &egui::Context, icon_path: &str) -> Response {
//         let icon = load_texture_from_png(ctx, icon_path).unwrap();
//         let selected = self.current_tool == tool;
//         let button_size = Vec2::new(30.0, 30.0);
// 
//         // 创建自定义按钮
//         let button = Button::new("")
//             .min_size(button_size)
//             .frame(false);
//         // 根据状态设置按钮颜色
//         let response = ui.add_sized(button_size, button);
//         let is_hovered_or_focused = response.hovered() || response.has_focus();
// 
//         // 设置按钮填充颜色
//         if selected && tool != Tool::Button {
//             ui.painter().circle_filled(
//                 response.rect.center(),
//                 response.rect.width() / 2.0, // 圆角为0
//                 Color32::from_rgb(128, 0, 128), // 选中或悬停时为紫色
//             );
//         } else if is_hovered_or_focused {
//             self.tool_bar_focused = true;
//             ui.painter().circle_filled(
//                 response.rect.center(),
//                 response.rect.width() / 2.0, // 圆角为0
//                 Color32::from_rgb(128, 0, 128), // 选中或悬停时为紫色
//             );
//         }
//         else {
//             ui.painter().circle_filled(
//                 response.rect.center(),
//                 response.rect.width() / 2.0, // 圆角为0
//                 Color32::from_rgb(128, 80, 128), // 初始状态为淡紫色
//             );
//         }
//         // 绘制图标
//         let icon_size = Vec2::new(20.0, 20.0);
//         let icon_rect = Rect::from_center_size(response.rect.center(), icon_size);
//         ui.painter().image(
//             icon,
//             icon_rect,
//             Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
//             Color32::WHITE,
//         );
//         if response.clicked() {
//             self.current_tool = tool;
//         }
//         response
//     }
// 
//     pub(crate) fn draw_annotations(&self, ui: &mut Ui) {
//         let painter = ui.painter();
// 
//         for annotation in &self.annotations {
//             self.draw_single_annotation(painter, annotation);
//         }
// 
//         if let Some(annotation) = &self.current_annotation {
//             self.draw_single_annotation(painter, annotation);
//         }
//     }
// 
//     // 文本输入
//     pub(crate) fn draw_text_input(&mut self, ui: &mut Ui) {
//         if let Some(text_state) = &mut self.text_input && text_state.is_active {
//             let max_x = self.selection_end.x;
//             let current_x = text_state.position.x;
//             let desired_width = (max_x - current_x).abs().max(10.0);
// 
//             // 获取当前时间用于光标闪烁
//             let now = ui.ctx().input(|i| i.time);
// 
//             // 创建文本输入区域
//             let _text_response = egui::Area::new(text_state.widget_id)
//                 .fixed_pos(text_state.position)
//                 .order(egui::Order::Foreground)
//                 .show(ui.ctx(), |ui| {
//                     egui::Frame::NONE
//                         .show(ui, |ui| {
//                             ui.style_mut().visuals.text_cursor.stroke.color = self.annotation_color; // 设置为红色光标
//                             let text_edit = egui::TextEdit::multiline(&mut text_state.text)
//                                 .font(egui::FontId::proportional(16.0))
//                                 .desired_width(desired_width)
//                                 .desired_rows(1)
//                                 .min_size(Vec2::ZERO)
//                                 .frame(false)
//                                 .text_color(self.annotation_color)
//                                 .hint_text("")
//                                 .id(text_state.widget_id);
//                             let response = ui.add(text_edit);
// 
//                             // 更新焦点状态和交互时间
//                             text_state.has_focus = response.has_focus();
//                             if response.changed() || response.lost_focus() || response.gained_focus() {
//                                 text_state.last_interaction_time = now;
//                             }
//                             response
//                         }).inner
//                 }).response;
//             // 手动绘制光标
//             ui.visuals_mut().text_cursor.stroke.color = self.annotation_color;
//             let painter = ui.painter();
// 
//             // 计算光标位置（这里需要根据文本内容计算准确的光标位置）
//             // 这是一个简化的实现，实际可能需要更复杂的光标位置计算
//             let cursor_rect = {
//                 let galley = ui.fonts_mut(|f| f.layout_no_wrap(
//                     text_state.text.clone(),
//                     egui::FontId::proportional(16.0),
//                     self.annotation_color,
//                 ));
// 
//                 let cursor_x = text_state.position.x + galley.size().x + 2.0; // 在文本末尾
//                 let cursor_y = text_state.position.y;
//                 let cursor_height = 16.0; // 字体高度
// 
//                 Rect::from_min_size(
//                     egui::pos2(cursor_x, cursor_y),
//                     egui::vec2(20.0, cursor_height), // 光标宽度为2像素
//                 )
//             };
//             // 绘制光标
//             text_selection::visuals::paint_text_cursor(
//                 ui,
//                 &painter,
//                 cursor_rect,
//                 now - text_state.last_interaction_time,
//             );
//         }
// 
//     }
// 
//     fn draw_single_annotation(&self, painter: &egui::Painter, annotation: &Annotation) {
//         if annotation.points.len() < 2 {
//             return;
//         }
// 
//         let stroke = Stroke::new(annotation.stroke_width, annotation.color);
//         match annotation.tool {
//             Tool::Pen => {
//                 // 绘制自由画笔
//                 for window in annotation.points.windows(2) {
//                     if let [start, end] = window {
//                         painter.line_segment([*start, *end], stroke);
//                     }
//                 }
//             }
//             Tool::Rectangle => {
//                 // 绘制矩形
//                 if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
//                     let rect = Rect::from_two_pos(start, end);
//                     painter.rect_stroke(rect, egui::CornerRadius::ZERO, stroke, StrokeKind::Middle);
//                 }
//             }
//             Tool::Arrow => {
//                 // 绘制箭头
//                 if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
//                     // 1️⃣ 先画箭杆（线，和原来一样）
//                     painter.line_segment([start, end], stroke);
// 
//                     // 2️⃣ 计算箭头头的三个点（实心三角形！）
//                     let dir = end - start;
//                     let dir_len = dir.length();
//                     if dir_len < 1.0 { return; } // 防止太短
// 
//                     let dir_norm = dir / dir_len; // 方向单位向量
// 
//                     // ✅ 修正：手动计算垂直向量（egui中没有perp()方法）
//                     let perp = Vec2::new(-dir_norm.y, dir_norm.x); // 旋转90度
// 
//                     // 箭头尺寸（可调，我设了10x5，你按需改）
//                     let arrow_length = 15.0;
//                     let arrow_width = 8.0;
// 
//                     let tip = end; // 箭头尖端
//                     let left = end - dir_norm * arrow_length + perp * arrow_width;
//                     let right = end - dir_norm * arrow_length - perp * arrow_width;
// 
//                     // 3️⃣ 用凸多边形实心填充箭头头
//                     painter.add(Shape::convex_polygon(
//                         vec![tip, left, right],
//                         annotation.color,
//                         Stroke::NONE,
//                     ));
//                 }
//             }
//             Tool::Text => {
//                 // 显示已保存的文本（仅在文本输入不活动时）
//                 if let Some(&pos) = annotation.points.first() && !annotation.text.is_empty() {
//                     // 按换行符分割文本
//                     let lines: Vec<&str> = annotation.text.lines().collect();
//                     let line_height = 16.0; // 与字体大小一致
//                     // 逐行绘制
//                     for (i, line) in lines.iter().enumerate() {
//                         painter.text(
//                             Pos2::new(
//                                 pos.x + 4f32,
//                                 pos.y + (i as f32) * line_height + 2f32, // 逐行下移
//                             ),
//                             // x和y加的4和2为为了避免文本向左上角移动
//                             egui::Align2::LEFT_TOP,
//                             line.to_string(),
//                             egui::FontId::proportional(16.0),
//                             annotation.color,
//                         );
//                     }
//                 }
//             }
//             Tool::Mosaic => {
//                 // 绘制马赛克效果
//                 if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
//                     let rect = Rect::from_two_pos(start, end);
// 
//                     // 马赛克块大小（可调整）
//                     let block_size = 4.0;
// 
//                     // 计算马赛克网格
//                     let width = rect.width();
//                     let height = rect.height();
//                     let cols = (width / block_size).ceil() as usize;
//                     let rows = (height / block_size).ceil() as usize;
// 
//                     // 绘制马赛克网格
//                     for row in 0..rows {
//                         for col in 0..cols {
//                             let block_rect = Rect::from_min_size(
//                                 Pos2::new(
//                                     rect.min.x + col as f32 * block_size,
//                                     rect.min.y + row as f32 * block_size
//                                 ),
//                                 Vec2::new(block_size, block_size)
//                             );
//                             let pixel = self.screenshots[0].get_pixel(block_rect.min.x as u32, block_rect.min.y as u32);
//                             let current_color = Color32::from_rgb(pixel[0], pixel[1], pixel[2]);
//                             painter.rect_filled(block_rect, egui::CornerRadius::ZERO, current_color);
//                         }
//                     }
// 
//                     // // 可选：绘制马赛克区域的边框
//                     // painter.rect_stroke(rect, egui::CornerRadius::ZERO, stroke, StrokeKind::Middle);
//                 }
//             }
//             Tool::Number => {
//                 if let (Some(number), Some(&pos)) = (annotation.number, annotation.points.first()) {
//                     let number_str = number.to_string();
//                     let font_size = 16.0;
//                     let circle_radius = 12.0; // 圆圈半径（比文字大点更舒服）
// 
//                     // 1️⃣ 先画圆圈背景（半透明黑，避免遮挡）
//                     painter.circle_filled(
//                         pos,
//                         circle_radius,
//                         annotation.color
//                     );
// 
//                     // 2️⃣ 再画白色序号（居中）
//                     painter.text(
//                         pos,
//                         egui::Align2::CENTER_CENTER,
//                         number_str,
//                         egui::FontId::proportional(font_size),
//                         Color32::WHITE, // 白色文字
//                     );
//                 }
//             }
//             _ => {}
//         }
//     }
// }