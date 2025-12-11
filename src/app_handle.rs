use std::env::temp_dir;
use std::io::Write;
use arboard::Clipboard;
use device_query::MousePosition;
use eframe::emath::{Pos2, Rect};
use egui::{Color32};
use image::{EncodableLayout, ImageBuffer, Rgba};
use crate::app_default::{Annotation, MouseSelectionRect, ScreenshotApp, Tool, MAX_TEXTURE_SIZE};
use crate::ui::{draw_simple_char, get_compress_image, get_screen_rect};

impl ScreenshotApp {

    pub fn xcap_capture_region(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
        let monitors = xcap::Monitor::all().unwrap();
        let mut window_image = monitors[0].capture_image().map_err(|e| e.to_string())?;
        let (width, height) = window_image.dimensions();
        if width > MAX_TEXTURE_SIZE as u32 || height > MAX_TEXTURE_SIZE as u32 {
            window_image = get_compress_image(width, height, window_image);;
        }
        let x = ((self.mouse_start.0.min(self.mouse_end.0) as f32) * self.image_scale) as i32;
        let y = ((self.mouse_start.1.min(self.mouse_end.1) as f32) * self.image_scale) as i32;
        let width = ((self.mouse_end.0 - self.mouse_start.0).abs() as f32 * self.image_scale) as i32;
        let height = ((self.mouse_end.1 - self.mouse_start.1).abs() as f32 * self.image_scale) as i32;

        let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width as u32, height as u32);
        // 复制原始截图内容
        for src_y in y..(y + height) {
            for src_x in x..(x + width) {
                let dst_x = src_x - x;
                let dst_y = src_y - y;

                let pixel = window_image.get_pixel(src_x as u32, src_y as u32);
                cropped_image.put_pixel(dst_x.try_into().unwrap(), dst_y.try_into().unwrap(), pixel.clone());
            }
        }
        Ok(cropped_image)
    }

    pub fn handle_file_dialog(&self, image: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<(), String> {
        let now = chrono::Local::now();
        let filename = now.format("screenshot_%Y-%m-%d_%H-%M-%S.png").to_string();
        let file_save_dialog = rfd::FileDialog::new();
        let save_path = file_save_dialog.set_file_name(filename.as_str())
            .add_filter("PNG Image", &["png"])
            .add_filter("JPEG Image", &["jpg", "jpeg"])
            .save_file();
        if let Some(path) = save_path {
            // 根据文件扩展名自动选择保存格式
            let format = match path.as_path().extension().and_then(|e| e.to_str()) {
                Some("jpg") | Some("jpeg") => image::ImageFormat::Jpeg,
                _ => image::ImageFormat::Png,
            };
            // 执行实际保存操作
            if let Err(_e) = image.save_with_format(&path, format) {
                return Err("保存失败".to_string());
            } else {
                println!("截图已成功保存至: {:?}", path.as_path());
            }
        }
        Ok(())
    }

    pub fn set_to_clipboard(&self, image: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<(), String> {
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        let img_data = arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: std::borrow::Cow::Borrowed(image.as_raw()),
        };
        clipboard.set_image(img_data).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_screenshot(&mut self) -> Result<(), String> {
        if self.selection_rect.is_none() {
            return Err("请选择要保存的图片".to_string());
        }
        let result_image = self.xcap_capture_region().map_err(|e| e.to_string())?;
        self.handle_file_dialog(result_image)?;
        // if let Some(cropped_image) = self.crop_selection(self.selection_rect.unwrap(), &self.annotations) {
        //     self.handle_file_dialog(cropped_image)?;
        // }
        Ok(())
    }

    pub fn copy_to_clipboard(&mut self) -> Result<(), String> {
        if self.selection_rect.is_none() {
            return Err("请选择要复制的图片".to_string());
        }
        let mut annotations = self.annotations.clone();
        if let (Some(text_state), Some(annotation)) = (&self.text_input, &self.current_annotation) {
            let mut new_annotation = annotation.clone();
            new_annotation.text = text_state.text.clone();
            annotations.push(new_annotation);
        }
        let result_image = self.xcap_capture_region().map_err(|e| e.to_string())?;
        let temp_dir_path = temp_dir();
        let now = chrono::Local::now();
        let filename = now.format("screenshot_%Y-%m-%d_%H-%M-%S.png").to_string();
        let temp_file_path = temp_dir_path.join(filename);
        result_image.save(temp_file_path.clone()).map_err(|e| e.to_string())?;
        let temp_file_path_str = temp_file_path.to_str().unwrap().to_string();
        std::io::stdout().write_all(temp_file_path_str.into_bytes().as_bytes()).unwrap();
        std::io::stdout().flush().unwrap();  // 确保立即输出
        self.set_to_clipboard(result_image).map_err(|e| e.to_string())?;
        // if let Some(cropped_image) = self.crop_selection(self.selection_rect.unwrap(), &annotations) {
        //     // 转换为剪贴板格式
        //     self.set_to_clipboard(cropped_image).map_err(|e| e.to_string())?;
        // }
        Ok(())
    }

    fn crop_selection(&self, selection_rect: Rect, annotations: &Vec<Annotation>) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let x = ((self.mouse_start.0.min(self.mouse_end.0) as f32) * self.image_scale) as i32;
        let y = ((self.mouse_start.1.min(self.mouse_end.1) as f32) * self.image_scale) as i32;
        let width = ((self.mouse_end.0 - self.mouse_start.0).abs() as f32 * self.image_scale) as i32;
        let height = ((self.mouse_end.1 - self.mouse_start.1).abs() as f32 * self.image_scale) as i32;
        // 查找包含选择区域的屏幕
        for (screen, screenshot) in self.screens.iter().zip(&self.screenshots) {
            let screen_rect = get_screen_rect(screen);

            if screen_rect.contains(selection_rect.center()) {
                // 计算在屏幕图像中的相对位置

                if width > 0 && height > 0 {
                    // 创建新的图像缓冲区
                    let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width as u32, height as u32);

                    // 复制原始截图内容
                    for src_y in y..(y + height) {
                        for src_x in x..(x + width) {
                            let dst_x = src_x - x;
                            let dst_y = src_y - y;

                            let pixel = screenshot.get_pixel(src_x as u32, src_y as u32);
                            cropped_image.put_pixel(dst_x.try_into().unwrap(), dst_y.try_into().unwrap(), pixel.clone());
                        }
                    }

                    // 添加标注内容
                    self.add_annotations_to_image(&mut cropped_image, annotations);

                    return Some(cropped_image);
                }
            }
        }
        None
    }

    fn add_annotations_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotations: &Vec<Annotation>) {
        for annotation in annotations {
            self.draw_single_annotation_to_image(image, annotation);
        }
    }

    fn draw_single_annotation_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotation: &Annotation) {
        if annotation.points.len() < 2 {
            return;
        }
        if self.mouse_selection_rect.is_none() {
            return;
        }
        let mouse_selection_rect = self.mouse_selection_rect.unwrap();
        let color = annotation.color;
        match annotation.tool {
            Tool::Pen => {
                // 修复画笔断断续续问题：使用更平滑的Bresenham算法
                for window in annotation.mouse_points.windows(2) {
                    if let [start, end] = window {
                        // let start_rel: MousePosition = (start.0 - mouse_selection_rect.start.0, start.1 - mouse_selection_rect.start.1);
                        // let end_rel: MousePosition = (end.0 - mouse_selection_rect.start.0, end.1 - mouse_selection_rect.start.1);
                        let start_rel: MousePosition = (((start.0 - mouse_selection_rect.start.0) as f32 * self.image_scale) as i32, ((start.1 - mouse_selection_rect.start.1) as f32 * self.image_scale) as i32);
                        let end_rel: MousePosition = (((end.0 - mouse_selection_rect.start.0) as f32 * self.image_scale) as i32, ((end.1 - mouse_selection_rect.start.1) as f32 * self.image_scale) as i32);
                        // 修复：使用浮点坐标转换确保连续
                        self.draw_smooth_line(image, start_rel, end_rel, color, annotation);
                    }
                }
            }
            Tool::Rectangle => {
                // 绘制矩形
                self.draw_rectangle(image, annotation, mouse_selection_rect, color)
            }
            Tool::Arrow => {
                // 修复：实心箭头绘制
                if let (Some(&start), Some(&end)) = (annotation.mouse_points.first(), annotation.mouse_points.last()) {
                    let start_rel: MousePosition = (((start.0 - mouse_selection_rect.start.0) as f32 * self.image_scale) as i32, ((start.1 - mouse_selection_rect.start.1) as f32 * self.image_scale) as i32);
                    let end_rel: MousePosition = (((end.0 - mouse_selection_rect.start.0) as f32 * self.image_scale) as i32, ((end.1 - mouse_selection_rect.start.1) as f32 * self.image_scale) as i32);

                    // 绘制箭头线
                    self.draw_smooth_line(image, start_rel, end_rel, color, annotation);
                    let end_pos = Pos2::new(end_rel.0 as f32, end_rel.1 as f32);
                    let start_pos = Pos2::new(start_rel.0 as f32, start_rel.1 as f32);
                    // 绘制实心箭头头
                    self.draw_filled_arrow_head(image, end_pos, start_pos, color);
                }
            }
            Tool::Text => {
                // 改进的文本绘制
                if let Some(&pos) = annotation.mouse_points.first() {
                    let pos_rel = Pos2::new(
                        (pos.0 - mouse_selection_rect.start.0).max(0) as f32 * self.image_scale,
                        (pos.1 - mouse_selection_rect.start.1).max(0) as f32 * self.image_scale
                    );

                    if !annotation.text.is_empty() {
                        self.draw_text(image, pos_rel, &annotation.text, color);
                    }
                }
            }
            Tool::Mosaic => {
                // 马赛克绘制
                if let (Some(&start), Some(&end)) = (annotation.mouse_points.first(), annotation.mouse_points.last()) {
                    let rect_rel = Rect::from_min_max(
                        Pos2::new(
                            (start.0  - mouse_selection_rect.start.0).max(0) as f32 * self.image_scale,
                            (start.1  - mouse_selection_rect.start.1).max(0) as f32 * self.image_scale
                        ),
                        Pos2::new(
                            (end.0 - mouse_selection_rect.start.0).min(image.width() as i32) as f32 * self.image_scale,
                            (end.1 - mouse_selection_rect.start.1).min(image.height() as i32) as f32 * self.image_scale
                        )
                    );

                    self.draw_mosaic(image, rect_rel, 4);
                }
            }
            Tool::Number => {
                // 序号绘制
                if let Some(&pos) = annotation.mouse_points.first() {
                    let pos: MousePosition = (((pos.0 - mouse_selection_rect.start.0).max(0) as f32 * self.image_scale) as i32, ((pos.1 - mouse_selection_rect.start.1).max(0) as f32 * self.image_scale) as i32);
                    self.draw_number(image, pos, &annotation.number.unwrap().to_string(), color);
                }
            }
            _ => {}
        }
    }

    fn draw_smooth_line(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, start: MousePosition, end: MousePosition, color: Color32, annotation: &Annotation) {
        let mut x0 = start.0;
        let mut y0 = start.1;
        let x1 = end.0;
        let y1 = end.1;

        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        // 计算直线长度，用于归一化距离
        let line_length = ((x1 - start.0) as f32).powi(2) + ((y1 - start.1) as f32).powi(2);
        let line_length = line_length.sqrt();

        // 如果线长为0，直接返回
        if line_length <= 0.0 {
            return;
        }

        loop {
            // 计算当前像素(x0, y0)到直线的距离
            let dist = ((x1 - start.0) * (start.1 - y0) - (start.0 - x0) * (y1 - start.1)).abs() as f32;
            let normalized_dist = (dist / line_length).min(1.0); // 确保在[0,1]范围内

            // 计算alpha值，距离直线越近，alpha越大
            let alpha = (255.0 * (1.0 - normalized_dist)).clamp(0.0, 255.0) as u8;

            // 确保坐标在图像范围内
            for dx in 0..annotation.stroke_width as i32 {
                for dy in 0..annotation.stroke_width as i32 {
                    let x = x0 + dx;
                    let y = y0 + dy;
                    if x >= 0 && x < image.width() as i32 && y >= 0 && y < image.height() as i32 {
                        // 获取当前像素的RGBA值
                        let current_pixel = image.get_pixel(x as u32, y as u32);
                        let (_r, _g, _b, a) = (current_pixel[0], current_pixel[1], current_pixel[2], current_pixel[3]);

                        // 计算新的alpha值（考虑当前像素的alpha）
                        let alpha_ratio = alpha as f32 / 255.0;
                        let new_alpha = (a as f32 * (1.0 - alpha_ratio) + alpha as f32).clamp(0.0, 255.0) as u8;

                        // 设置新像素值
                        image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), new_alpha]));
                    }
                }
            }

            if x0 == x1 && y0 == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x0 += sx;
            }
            if e2 < dx {
                err += dx;
                y0 += sy;
            }
        }
    }
    // 画矩形框
    fn draw_rectangle(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotation: &Annotation, mouse_selection_rect: MouseSelectionRect, color: Color32) {
        if let (Some(&start), Some(&end)) = (annotation.mouse_points.first(), annotation.mouse_points.last()) {
            let start_rel: MousePosition = (start.0 - mouse_selection_rect.start.0, start.1 - mouse_selection_rect.start.1);
            let end_rel: MousePosition = (end.0 - mouse_selection_rect.start.0, end.1 - mouse_selection_rect.start.1);
            let rect_rel = Rect::from_min_max(
                Pos2::new(start_rel.0 as f32, start_rel.1 as f32),
                Pos2::new(end_rel.0 as f32, end_rel.1 as f32)
            );


            // 顶边
            for y in (rect_rel.min.y as usize)..((rect_rel.min.y + annotation.stroke_width) as usize) {
                for x in (rect_rel.min.x as usize)..((rect_rel.max.x) as usize) {
                    if x < image.width() as usize && y < image.height() as usize {
                        image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }

            // 低边
            for y in (rect_rel.max.y as usize)..((rect_rel.max.y + annotation.stroke_width) as usize) {
                for x in (rect_rel.min.x as usize)..((rect_rel.max.x + annotation.stroke_width) as usize) {
                    if x < image.width() as usize && y < image.height() as usize {
                        image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }

            // 左边
            for x in (rect_rel.min.x as usize)..((rect_rel.min.x + annotation.stroke_width) as usize) {
                for y in (rect_rel.min.y as usize)..((rect_rel.max.y) as usize) {
                    if x < image.width() as usize && y < image.height() as usize {
                        image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }

            // 右边
            for x in (rect_rel.max.x as usize)..((rect_rel.max.x + annotation.stroke_width) as usize) {
                for y in (rect_rel.min.y as usize)..((rect_rel.max.y + annotation.stroke_width) as usize) {
                    if x < image.width() as usize && y < image.height() as usize {
                        image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }
        }
    }

    // 修复箭头实心问题：绘制实心箭头
    fn draw_filled_arrow_head(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, tip: Pos2, from: Pos2, color: Color32) {
        let arrow_length = 15.0;
        let arrow_angle = std::f32::consts::PI / 6.0; // 30度

        let dx = tip.x - from.x;
        let dy = tip.y - from.y;
        let length = (dx * dx + dy * dy).sqrt();

        if length < f32::EPSILON {
            return;
        }

        // 方向向量
        let dir_x = dx / length;
        let dir_y = dy / length;

        // 计算箭头两侧的点
        let left_x = tip.x - arrow_length * (dir_x * arrow_angle.cos() - dir_y * arrow_angle.sin());
        let left_y = tip.y - arrow_length * (dir_x * arrow_angle.sin() + dir_y * arrow_angle.cos());

        let right_x = tip.x - arrow_length * (dir_x * arrow_angle.cos() + dir_y * arrow_angle.sin());
        let right_y = tip.y - arrow_length * (-dir_x * arrow_angle.sin() + dir_y * arrow_angle.cos());

        // 创建三角形点
        let points = [
            (tip.x, tip.y),
            (left_x, left_y),
            (right_x, right_y)
        ];

        // 用Bresenham绘制三角形
        self.draw_filled_triangle(image, points, color);
    }

    // 绘制实心三角形（填充）
    fn draw_filled_triangle(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, points: [(f32, f32); 3], color: Color32) {
        // 计算三角形边界
        let x_min = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let x_max = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let y_min = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let y_max = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);

        // 遍历边界框内的每个像素
        for y in (y_min as i32)..=(y_max as i32) {
            for x in (x_min as i32)..=(x_max as i32) {
                if self.point_in_triangle(x as f32, y as f32, points) {
                    // 确保在图像范围内
                    if x >= 0 && x < image.width() as i32 && y >= 0 && y < image.height() as i32 {
                        image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }
        }
    }

    // 判断点是否在三角形内（射线法）
    fn point_in_triangle(&self, x: f32, y: f32, points: [(f32, f32); 3]) -> bool {
        let (a, b, c) = (points[0], points[1], points[2]);

        // 检查射线与三角形边的交点
        let mut intersections = 0;

        // 检查边ab
        if self.intersects_ray(x, y, a, b) {
            intersections += 1;
        }

        // 检查边bc
        if self.intersects_ray(x, y, b, c) {
            intersections += 1;
        }

        // 检查边ca
        if self.intersects_ray(x, y, c, a) {
            intersections += 1;
        }

        intersections % 2 == 1
    }

    // 检查射线是否与线段相交
    fn intersects_ray(&self, x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> bool {
        // 检查线段是否与射线相交
        if (a.1 <= y && b.1 > y) || (a.1 > y && b.1 <= y) {
            // 计算交点x坐标
            let t = (y - a.1) / (b.1 - a.1);
            let intersect_x = a.0 + t * (b.0 - a.0);

            // 检查交点是否在射线方向（x >= 当前点x）
            intersect_x > x
        } else {
            false
        }
    }

    // 简单的文本绘制（使用位图字体或简单图形）
    fn draw_text(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, pos: Pos2, text: &str, color: Color32) {
        // 这里可以使用位图字体库，或者简单的字符绘制
        // 示例：绘制简单的矩形文字背景和文字轮廓
        let x = pos.x as i32;
        let y = pos.y as i32;

        // 简单绘制文字边框（实际项目中应该使用字体渲染）
        for (i, ch) in text.chars().enumerate() {
            let char_x = x + i as i32 * 8;
            draw_simple_char(image, char_x, y, ch, color);
        }
    }

    // 绘制序号（带圆圈的数字）
    fn draw_number(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, pos: MousePosition, number: &str, color: Color32) {
        let radius = 12;

        // 绘制圆形背景
        self.draw_circle(image, pos.0, pos.1, radius, color);
        if let Some(first_char) = number.chars().next() && number.len() == 1 {
            draw_simple_char(image, pos.0, pos.1, first_char, Color32::WHITE);
        } else {
            for (index, char) in number.chars().enumerate() {
                if index == 0 {
                    draw_simple_char(image, pos.0 - 4, pos.1, char, Color32::WHITE);
                } else {
                    draw_simple_char(image, pos.0 + 4, pos.1, char, Color32::WHITE);
                }

            }
        }
    }

    // 绘制圆形
    // 绘制实心圆且带抗锯齿效果
    fn draw_circle(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, center_x: i32, center_y: i32, radius: i32, color: Color32) {
        let radius = radius as f32;
        let center_x = center_x as f32;
        let center_y = center_y as f32;

        // 扩大计算范围，包含完整的抗锯齿区域
        let min_y = (center_y - radius - 1.0) as i32;
        let max_y = (center_y + radius + 1.0) as i32;

        for y in min_y..=max_y {
            if y < 0 || y >= image.height() as i32 {
                continue;
            }

            for x in (center_x - radius - 1.0) as i32..=(center_x + radius + 1.0) as i32 {
                if x < 0 || x >= image.width() as i32 {
                    continue;
                }

                // 计算像素中心到圆心的距离
                let dx = (x as f32 + 0.5) - center_x;
                let dy = (y as f32 + 0.5) - center_y;
                let d = (dx * dx + dy * dy).sqrt();

                // 改进的抗锯齿计算：使用更平滑的过渡
                let alpha = if d <= radius - 0.8 {
                    1.0 // 完全在圆内
                } else if d >= radius + 0.8 {
                    0.0 // 完全在圆外
                } else {
                    // 在边缘区域使用平滑过渡（1.6像素宽度）
                    let t = (d - (radius - 0.8)) / 1.6;
                    // 使用平滑函数替代线性插值
                    1.0 - t * t * (3.0 - 2.0 * t) // 平滑步进函数
                };

                if alpha <= 0.0 {
                    continue;
                }

                let new_alpha = (alpha * color.a() as f32) as u8;
                let new_color = Rgba([color.r(), color.g(), color.b(), new_alpha]);

                // 使用alpha混合而不是直接覆盖
                if new_alpha == 255 {
                    image.put_pixel(x as u32, y as u32, new_color);
                } else {
                    let pixel = image.get_pixel_mut(x as u32, y as u32);
                    self.blend_pixels(pixel, &new_color);
                }
            }
        }
    }

    // Alpha混合函数
    fn blend_pixels(&self, background: &mut Rgba<u8>, foreground: &Rgba<u8>) {
        let alpha_fg = foreground[3] as f32 / 255.0;
        let alpha_bg = background[3] as f32 / 255.0;
        let alpha_out = alpha_fg + alpha_bg * (1.0 - alpha_fg);

        if alpha_out > 0.0 {
            for i in 0..3 {
                background[i] = ((foreground[i] as f32 * alpha_fg +
                    background[i] as f32 * alpha_bg * (1.0 - alpha_fg)) / alpha_out) as u8;
            }
            background[3] = (alpha_out * 255.0) as u8;
        }
    }

    // 马赛克效果
    fn draw_mosaic(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, rect: Rect, block_size: u32) {
        let x1 = rect.min.x as u32;
        let y1 = rect.min.y as u32;
        let x2 = rect.max.x as u32;
        let y2 = rect.max.y as u32;

        for block_y in (y1..y2).step_by(block_size as usize) {
            for block_x in (x1..x2).step_by(block_size as usize) {
                let block_end_x = (block_x + block_size).min(x2);
                let block_end_y = (block_y + block_size).min(y2);

                // 计算块内的平均颜色
                let (mut r_sum, mut g_sum, mut b_sum, mut count) = (0u32, 0u32, 0u32, 0u32);

                for y in block_y..block_end_y {
                    for x in block_x..block_end_x {
                        if x < image.width() && y < image.height() {
                            let pixel = image.get_pixel(x, y);
                            r_sum += pixel[0] as u32;
                            g_sum += pixel[1] as u32;
                            b_sum += pixel[2] as u32;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    let avg_r = (r_sum / count) as u8;
                    let avg_g = (g_sum / count) as u8;
                    let avg_b = (b_sum / count) as u8;

                    // 用平均颜色填充整个块
                    for y in block_y..block_end_y {
                        for x in block_x..block_end_x {
                            if x < image.width() && y < image.height() {
                                image.put_pixel(x, y, Rgba([avg_r, avg_g, avg_b, 255]));
                            }
                        }
                    }
                }
            }
        }
    }

}