use crate::app_default::{Annotation, ScreenshotApp, Tool};
use crate::ui::{draw_annotation_text, draw_simple_char, load_cjk_font};
#[cfg(not(target_os = "linux"))]
use arboard::Clipboard;
use device_query::MousePosition;
use eframe::emath::{Pos2, Rect};
use egui::Color32;
use image::{ImageBuffer, Rgba};
use imageproc::point::Point;

fn requires_mosaic_background(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|annotation| annotation.tool == Tool::Mosaic)
}

fn alpha_bounds(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        bounds = Some(match bounds {
            Some((min_x, min_y, max_x, max_y)) => {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            }
            None => (x, y, x, y),
        });
    }
    bounds
}

#[allow(dead_code)]
impl ScreenshotApp {
    pub(crate) fn annotations_for_export(&self) -> Vec<Annotation> {
        let mut annotations = self.annotations.clone();
        if let (Some(text_state), Some(annotation)) = (&self.text_input, &self.current_annotation) {
            let mut annotation = annotation.clone();
            annotation.text = text_state.text.clone();
            annotations.push(annotation);
        }
        annotations
    }

    pub fn compose_current_selection(
        &self,
        annotations: &[Annotation],
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
        let selection = self
            .selection_rect
            .ok_or_else(|| "请选择截图区域".to_string())?;
        let session = self
            .capture_session
            .as_ref()
            .ok_or_else(|| "截图会话不存在".to_string())?;
        let mut composed = crate::display::compose_selection(&session.displays, selection)?;
        self.draw_annotations_on_composed(&mut composed.image, composed.transform, annotations);
        Ok(composed.image)
    }

    fn draw_annotations_on_composed(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        transform: crate::display::OutputTransform,
        annotations: &[Annotation],
    ) {
        let background = requires_mosaic_background(annotations).then(|| image.clone());
        let visual_scale = transform.scale.x.min(transform.scale.y).max(1.0);
        let draw = |annotation: &Annotation, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>| {
            let mut output_annotation = annotation.clone();
            output_annotation.points = annotation
                .points
                .iter()
                .map(|point| transform.global_to_output(*point))
                .collect();
            output_annotation.stroke_width *= visual_scale;
            self.draw_single_annotation_to_image(
                image,
                &output_annotation,
                background.as_ref(),
                visual_scale,
            );
        };
        for annotation in annotations
            .iter()
            .filter(|annotation| annotation.tool == Tool::Mosaic)
        {
            draw(annotation, image);
        }
        for annotation in annotations
            .iter()
            .filter(|annotation| annotation.tool != Tool::Mosaic)
        {
            draw(annotation, image);
        }
    }

    pub fn set_to_clipboard(&self, image: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<(), String> {
        // 在 Linux 上使用 xclip 来保持剪贴板数据
        #[cfg(target_os = "linux")]
        {
            // 保存到临时文件
            let temp_dir = std::env::temp_dir();
            let temp_file = temp_dir.join("legend_shot_clipboard.png");

            image
                .save(&temp_file)
                .map_err(|e| format!("保存临时文件失败: {}", e))?;

            // 使用 xclip 设置剪贴板
            let status = std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/png", "-i"])
                .arg(&temp_file)
                .status()
                .map_err(|e| format!("执行 xclip 失败: {}。请确保已安装 xclip", e))?;

            if !status.success() {
                return Err("xclip 设置剪贴板失败".to_string());
            }
        }

        // 在其他平台上使用 arboard
        #[cfg(not(target_os = "linux"))]
        {
            let mut clipboard = Clipboard::new().map_err(|e| format!("无法访问剪贴板: {}", e))?;

            let img_data = arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: std::borrow::Cow::Owned(image.into_raw()),
            };

            clipboard
                .set_image(img_data)
                .map_err(|e| format!("设置剪贴板图片失败: {}", e))?;
        }

        Ok(())
    }

    pub fn trigger_save_dialog(&mut self, ctx: &egui::Context) {
        if self.selection_rect.is_none() {
            return;
        }

        let mut annotations = self.annotations.clone();
        if let (Some(text_state), Some(annotation)) = (&self.text_input, &self.current_annotation) {
            let mut new_annotation = annotation.clone();
            new_annotation.text = text_state.text.clone();
            annotations.push(new_annotation);
        }

        let cropped_image = match self.compose_current_selection(&annotations) {
            Ok(image) => image,
            Err(error) => {
                self.capture_error = Some(error);
                ctx.request_repaint();
                return;
            }
        };
        let restore_toolbar = self.show_toolbar;
        if !self.native_save_dialog_state.begin(restore_toolbar) {
            return;
        }
        self.capture_error = None;
        self.pending_save_image = Some(cropped_image);
        self.show_toolbar = false;
        ctx.request_repaint();
    }

    pub(crate) fn choose_native_save_path(&self) -> Option<std::path::PathBuf> {
        let filename = format!(
            "screenshot_{}.png",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        let mut dialog = rfd::FileDialog::new()
            .set_title("保存截图")
            .set_file_name(filename)
            .add_filter("PNG 图片", &["png"])
            .add_filter("JPEG 图片", &["jpg", "jpeg"]);
        if let Some(directory) = &self.config.last_save_dir {
            dialog = dialog.set_directory(directory);
        }
        dialog.save_file()
    }

    pub fn save_image_to_path(
        &mut self,
        image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some("jpg") | Some("jpeg") => image::ImageFormat::Jpeg,
            _ => image::ImageFormat::Png,
        };

        image
            .save_with_format(path, format)
            .map_err(|e| format!("保存失败: {e}"))?;

        if let Some(parent) = path.parent() {
            self.config.last_save_dir = Some(parent.to_path_buf());
            self.save_config();
        }

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

        self.copy_current_selection_with(&annotations, |image| self.set_to_clipboard(image))?;
        eprintln!("=== GUI 模式: 复制到剪贴板成功 ===");
        Ok(())
    }

    pub(crate) fn copy_selection_and_finish(&mut self, ctx: &egui::Context) {
        match self.copy_to_clipboard() {
            Ok(()) => self.hide_capture_window(ctx),
            Err(error) => {
                self.capture_error = Some(error);
                self.show_toolbar = self.selection_rect.is_some();
                ctx.request_repaint();
            }
        }
    }

    fn copy_current_selection_with<F>(
        &self,
        annotations: &[Annotation],
        deliver: F,
    ) -> Result<(), String>
    where
        F: FnOnce(ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<(), String>,
    {
        deliver(self.compose_current_selection(annotations)?)
    }

    /// 测试模式专用的裁剪方法，不需要标注
    pub fn crop_selection_for_test(&self) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        self.compose_current_selection(&[]).ok()
    }

    fn draw_single_annotation_to_image(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        annotation: &Annotation,
        mosaic_source: Option<&ImageBuffer<Rgba<u8>, Vec<u8>>>,
        visual_scale: f32,
    ) {
        let points = annotation
            .points
            .iter()
            .map(|point| (point.x.round() as i32, point.y.round() as i32))
            .collect::<Vec<_>>();
        if points.is_empty() {
            return;
        }
        let color = annotation.color;
        match annotation.tool {
            Tool::Pen => {
                for window in points.windows(2) {
                    self.draw_smooth_line(image, window[0], window[1], color, annotation);
                }
            }
            Tool::Rectangle => {
                if let (Some(&start), Some(&end)) = (points.first(), points.last()) {
                    let corners = [
                        (start.0, start.1),
                        (end.0, start.1),
                        (end.0, end.1),
                        (start.0, end.1),
                        (start.0, start.1),
                    ];
                    for edge in corners.windows(2) {
                        self.draw_smooth_line(image, edge[0], edge[1], color, annotation);
                    }
                }
            }
            Tool::Arrow => {
                if let (Some(&start), Some(&end)) = (points.first(), points.last()) {
                    self.draw_filled_arrow(
                        image,
                        Pos2::new(start.0 as f32, start.1 as f32),
                        Pos2::new(end.0 as f32, end.1 as f32),
                        color,
                        annotation.stroke_width,
                        visual_scale,
                    );
                }
            }
            Tool::Text => {
                if let Some(&pos) = points.first()
                    && !annotation.text.is_empty()
                {
                    self.draw_text(
                        image,
                        Pos2::new(pos.0.max(0) as f32, pos.1.max(0) as f32),
                        &annotation.text,
                        color,
                        visual_scale,
                    );
                }
            }
            Tool::Mosaic => {
                if let (Some(&start), Some(&end), Some(mosaic_source)) =
                    (points.first(), points.last(), mosaic_source)
                {
                    self.draw_mosaic(
                        image,
                        mosaic_source,
                        Rect::from_two_pos(
                            Pos2::new(start.0 as f32, start.1 as f32),
                            Pos2::new(end.0 as f32, end.1 as f32),
                        ),
                        (4.0 * visual_scale).round().max(1.0) as u32,
                    );
                }
            }
            Tool::Number => {
                if let (Some(&pos), Some(number)) = (points.first(), annotation.number) {
                    self.draw_number(
                        image,
                        (pos.0.max(0), pos.1.max(0)),
                        &number.to_string(),
                        color,
                        visual_scale,
                    );
                }
            }
            _ => {}
        }
    }

    fn draw_smooth_line(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        start: MousePosition,
        end: MousePosition,
        color: Color32,
        annotation: &Annotation,
    ) {
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
            let dist =
                ((x1 - start.0) * (start.1 - y0) - (start.0 - x0) * (y1 - start.1)).abs() as f32;
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
                        let (_r, _g, _b, a) = (
                            current_pixel[0],
                            current_pixel[1],
                            current_pixel[2],
                            current_pixel[3],
                        );

                        // 计算新的alpha值（考虑当前像素的alpha）
                        let alpha_ratio = alpha as f32 / 255.0;
                        let new_alpha =
                            (a as f32 * (1.0 - alpha_ratio) + alpha as f32).clamp(0.0, 255.0) as u8;

                        // 设置新像素值
                        image.put_pixel(
                            x as u32,
                            y as u32,
                            Rgba([color.r(), color.g(), color.b(), new_alpha]),
                        );
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
    fn draw_filled_arrow(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        start: Pos2,
        tip: Pos2,
        color: Color32,
        stroke_width: f32,
        visual_scale: f32,
    ) {
        let direction = tip - start;
        let length = direction.length();
        if length < 1.0 {
            return;
        }

        let direction = direction / length;
        let perpendicular = eframe::emath::Vec2::new(-direction.y, direction.x);
        let head_length = (15.0 * visual_scale).min(length * 0.75);
        let head_half_width = (8.0 * visual_scale).min(length * 0.4);
        let shaft_half_width = (stroke_width / 2.0).max(0.5);
        let neck = tip - direction * head_length;
        let polygon = [
            start + perpendicular * shaft_half_width,
            neck + perpendicular * shaft_half_width,
            neck + perpendicular * head_half_width,
            tip,
            neck - perpendicular * head_half_width,
            neck - perpendicular * shaft_half_width,
            start - perpendicular * shaft_half_width,
        ]
        .map(|point| Point::new(point.x.round() as i32, point.y.round() as i32));

        imageproc::drawing::draw_antialiased_polygon_mut(
            image,
            &polygon,
            Rgba([color.r(), color.g(), color.b(), color.a()]),
            imageproc::pixelops::interpolate,
        );
    }

    // 简单的文本绘制（使用位图字体或简单图形）
    fn draw_text(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        pos: Pos2,
        text: &str,
        color: Color32,
        visual_scale: f32,
    ) {
        // 这里可以使用位图字体库，或者简单的字符绘制
        // 示例：绘制简单的矩形文字背景和文字轮廓
        let x = pos.x as i32;
        let y = pos.y as i32;

        if draw_annotation_text(image, x, y, text, color, visual_scale) {
            return;
        }

        // 简单绘制文字边框（实际项目中应该使用字体渲染）
        let bitmap_scale = visual_scale.round().max(1.0) as u32;
        for (i, ch) in text.chars().enumerate() {
            let char_x = x + i as i32 * 8 * bitmap_scale as i32;
            draw_simple_char(image, char_x, y, ch, color, bitmap_scale);
        }
    }

    // 绘制序号（带圆圈的数字）
    fn draw_number(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        pos: MousePosition,
        number: &str,
        color: Color32,
        visual_scale: f32,
    ) {
        let radius = (12.0 * visual_scale).round() as i32;

        // 绘制圆形背景
        self.draw_circle(image, pos.0, pos.1, radius, color);
        if self.draw_centered_number_text(image, pos, number, visual_scale) {
            return;
        }

        let bitmap_scale = visual_scale.round().max(1.0) as u32;
        let count = number.chars().count() as i32;
        for (index, character) in number.chars().enumerate() {
            let offset = (index as i32 * 8 - (count - 1) * 4) * bitmap_scale as i32;
            draw_simple_char(
                image,
                pos.0 + offset,
                pos.1,
                character,
                Color32::WHITE,
                bitmap_scale,
            );
        }
    }

    fn draw_centered_number_text(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        pos: MousePosition,
        number: &str,
        visual_scale: f32,
    ) -> bool {
        let Some(font_data) = load_cjk_font() else {
            return false;
        };
        let Ok(font) = ab_glyph::FontArc::try_from_vec(font_data.as_ref().clone()) else {
            return false;
        };

        let mut font_size = 16.0 * visual_scale;
        let (initial_width, _) = imageproc::drawing::text_size(font_size, &font, number);
        let available_width = 19.0 * visual_scale;
        if initial_width as f32 > available_width {
            font_size *= available_width / initial_width as f32;
        }
        let (text_width, text_height) = imageproc::drawing::text_size(font_size, &font, number);
        let padding = (4.0 * visual_scale).ceil() as u32;
        let mut glyph = ImageBuffer::from_pixel(
            text_width + padding * 2,
            text_height + padding * 2,
            Rgba([0u8, 0, 0, 0]),
        );
        imageproc::drawing::draw_text_mut(
            &mut glyph,
            Rgba([255, 255, 255, 255]),
            padding as i32,
            padding as i32,
            font_size,
            &font,
            number,
        );

        let Some((min_x, min_y, max_x, max_y)) = alpha_bounds(&glyph) else {
            return false;
        };
        let glyph_width = max_x - min_x + 1;
        let glyph_height = max_y - min_y + 1;
        let target_x = pos.0 - glyph_width as i32 / 2;
        let target_y = pos.1 - glyph_height as i32 / 2;
        for source_y in min_y..=max_y {
            for source_x in min_x..=max_x {
                let destination_x = target_x + (source_x - min_x) as i32;
                let destination_y = target_y + (source_y - min_y) as i32;
                if destination_x < 0
                    || destination_y < 0
                    || destination_x >= image.width() as i32
                    || destination_y >= image.height() as i32
                {
                    continue;
                }
                let foreground = glyph.get_pixel(source_x, source_y);
                if foreground[3] != 0 {
                    let background =
                        image.get_pixel_mut(destination_x as u32, destination_y as u32);
                    self.blend_pixels(background, foreground);
                }
            }
        }
        true
    }

    // 绘制圆形
    // 绘制实心圆且带抗锯齿效果
    fn draw_circle(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        center_x: i32,
        center_y: i32,
        radius: i32,
        color: Color32,
    ) {
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
                background[i] = ((foreground[i] as f32 * alpha_fg
                    + background[i] as f32 * alpha_bg * (1.0 - alpha_fg))
                    / alpha_out) as u8;
            }
            background[3] = (alpha_out * 255.0) as u8;
        }
    }

    // 马赛克效果
    fn draw_mosaic(
        &self,
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        source: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        rect: Rect,
        block_size: u32,
    ) {
        let x1 = rect.min.x.max(0.0) as u32;
        let y1 = rect.min.y.max(0.0) as u32;
        let x2 = rect.max.x.max(0.0) as u32;
        let y2 = rect.max.y.max(0.0) as u32;

        for block_y in (y1..y2).step_by(block_size as usize) {
            for block_x in (x1..x2).step_by(block_size as usize) {
                let block_end_x = (block_x + block_size).min(x2);
                let block_end_y = (block_y + block_size).min(y2);

                // 计算块内的平均颜色
                let (mut r_sum, mut g_sum, mut b_sum, mut count) = (0u32, 0u32, 0u32, 0u32);

                for y in block_y..block_end_y {
                    for x in block_x..block_end_x {
                        if x < source.width() && y < source.height() {
                            let pixel = source.get_pixel(x, y);
                            r_sum += pixel[0] as u32;
                            g_sum += pixel[1] as u32;
                            b_sum += pixel[2] as u32;
                            count += 1;
                        }
                    }
                }

                let (Some(avg_r), Some(avg_g), Some(avg_b)) = (
                    r_sum.checked_div(count),
                    g_sum.checked_div(count),
                    b_sum.checked_div(count),
                ) else {
                    continue;
                };

                // 用平均颜色填充整个块
                for y in block_y..block_end_y {
                    for x in block_x..block_end_x {
                        if x < image.width() && y < image.height() {
                            image.put_pixel(
                                x,
                                y,
                                Rgba([avg_r as u8, avg_g as u8, avg_b as u8, 255]),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn copy_action_failure_preserves_selection_and_shows_error() {
        let selection = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20.0, 20.0));
        let mut app = ScreenshotApp {
            selection_rect: Some(selection),
            ..Default::default()
        };
        app.copy_selection_and_finish(&egui::Context::default());
        assert_eq!(app.selection_rect, Some(selection));
        assert!(app.show_toolbar);
        assert!(app.capture_error.is_some());
        assert_eq!(app.lifecycle, crate::app_default::AppLifecycle::Capturing);
    }

    #[test]
    fn save_composition_failure_preserves_selection_and_shows_error() {
        let selection = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20.0, 20.0));
        let mut app = ScreenshotApp {
            selection_rect: Some(selection),
            show_toolbar: true,
            ..Default::default()
        };
        app.trigger_save_dialog(&egui::Context::default());
        assert_eq!(app.selection_rect, Some(selection));
        assert!(app.show_toolbar);
        assert!(app.capture_error.is_some());
        assert!(app.pending_save_image.is_none());
    }

    use eframe::emath::{Pos2, Rect, Vec2};
    use egui::Color32;
    use image::{Rgba, RgbaImage};

    use crate::app_default::{Annotation, ScreenshotApp, Tool};
    use crate::display::{CaptureSession, CapturedDisplay, DisplayGeometry};

    fn solid_display(
        index: usize,
        origin_x: f32,
        logical_width: f32,
        pixels: (u32, u32),
        color: [u8; 4],
    ) -> CapturedDisplay {
        let geometry = DisplayGeometry::new(
            index,
            Rect::from_min_size(Pos2::new(origin_x, 0.0), Vec2::new(logical_width, 50.0)),
            pixels,
        )
        .unwrap();
        CapturedDisplay::from_image(
            geometry,
            RgbaImage::from_pixel(pixels.0, pixels.1, Rgba(color)),
            2048,
        )
        .unwrap()
    }

    fn app_with_two_solid_displays(retina_second: bool) -> ScreenshotApp {
        let mut app = ScreenshotApp::default();
        let second_pixels = if retina_second { (200, 100) } else { (100, 50) };
        app.install_capture_session(
            CaptureSession::new(vec![
                solid_display(0, 0.0, 100.0, (100, 50), [255, 0, 0, 255]),
                solid_display(1, 100.0, 100.0, second_pixels, [0, 0, 255, 255]),
            ])
            .unwrap(),
        );
        app
    }

    #[test]
    fn current_selection_composes_both_displays_for_every_consumer() {
        let mut app = app_with_two_solid_displays(false);
        app.selection_rect = Some(Rect::from_min_size(
            Pos2::new(50.0, 0.0),
            Vec2::new(100.0, 50.0),
        ));

        let image = app.compose_current_selection(&[]).unwrap();

        assert_eq!(image.dimensions(), (100, 50));
        assert_eq!(image.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(90, 10).0, [0, 0, 255, 255]);
    }

    #[test]
    fn clipboard_delivery_uses_composed_image_directly() {
        let mut app = app_with_two_solid_displays(false);
        app.selection_rect = Some(Rect::from_min_size(
            Pos2::new(50.0, 0.0),
            Vec2::new(100.0, 50.0),
        ));
        let mut delivered = None;

        app.copy_current_selection_with(&[], |image| {
            delivered = Some(image);
            Ok(())
        })
        .unwrap();

        let image = delivered.unwrap();
        assert_eq!(image.dimensions(), (100, 50));
        assert_eq!(image.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(90, 10).0, [0, 0, 255, 255]);
    }

    #[test]
    fn background_copy_is_only_required_for_mosaic_annotations() {
        let rectangle = Annotation {
            tool: Tool::Rectangle,
            points: vec![],
            color: Color32::RED,
            stroke_width: 1.0,
            text: String::new(),
            number: None,
        };
        let mosaic = Annotation {
            tool: Tool::Mosaic,
            ..rectangle.clone()
        };

        assert!(!super::requires_mosaic_background(&[]));
        assert!(!super::requires_mosaic_background(&[rectangle]));
        assert!(super::requires_mosaic_background(&[mosaic]));
    }

    #[test]
    fn mosaic_annotation_still_renders_from_the_unmodified_background() {
        let source =
            RgbaImage::from_fn(8, 8, |x, y| Rgba([(x * 16) as u8, (y * 16) as u8, 0, 255]));
        let geometry = DisplayGeometry::new(
            0,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(8.0, 8.0)),
            (8, 8),
        )
        .unwrap();
        let mut app = ScreenshotApp::default();
        app.install_capture_session(
            CaptureSession::new(vec![
                CapturedDisplay::from_image(geometry, source, 2048).unwrap(),
            ])
            .unwrap(),
        );
        app.selection_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(8.0, 8.0)));
        let mosaic = Annotation {
            tool: Tool::Mosaic,
            points: vec![Pos2::ZERO, Pos2::new(8.0, 8.0)],
            color: Color32::TRANSPARENT,
            stroke_width: 1.0,
            text: String::new(),
            number: None,
        };

        let image = app.compose_current_selection(&[mosaic]).unwrap();

        assert_eq!(image.get_pixel(0, 0).0, [24, 24, 0, 255]);
        assert_eq!(image.get_pixel(4, 4).0, [88, 88, 0, 255]);
    }

    #[test]
    fn annotation_global_point_uses_composed_output_transform() {
        let mut app = app_with_two_solid_displays(true);
        app.selection_rect = Some(Rect::from_min_size(
            Pos2::new(50.0, 0.0),
            Vec2::new(100.0, 50.0),
        ));
        let annotation = Annotation {
            tool: Tool::Rectangle,
            points: vec![Pos2::new(75.0, 10.0), Pos2::new(125.0, 40.0)],
            color: Color32::RED,
            stroke_width: 1.0,
            text: String::new(),
            number: None,
        };

        let image = app.compose_current_selection(&[annotation]).unwrap();

        assert_eq!(image.dimensions(), (200, 100));
        assert_eq!(image.get_pixel(50, 20).0[..3], [255, 0, 0]);
    }

    #[test]
    fn exported_arrow_shaft_is_centered_on_its_direction_axis() {
        let mut app = app_with_two_solid_displays(false);
        app.selection_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 50.0)));
        let arrow = Annotation {
            tool: Tool::Arrow,
            points: vec![Pos2::new(80.0, 25.0), Pos2::new(20.0, 25.0)],
            color: Color32::GREEN,
            stroke_width: 4.0,
            text: String::new(),
            number: None,
        };

        let image = app.compose_current_selection(&[arrow]).unwrap();

        assert_eq!(image.get_pixel(50, 24).0[..3], [0, 255, 0]);
        assert_eq!(image.get_pixel(50, 26).0[..3], [0, 255, 0]);
    }

    #[test]
    fn exported_number_glyph_has_antialiased_edges() {
        let mut app = app_with_two_solid_displays(false);
        app.selection_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 50.0)));
        let number = Annotation {
            tool: Tool::Number,
            points: vec![Pos2::new(50.0, 25.0)],
            color: Color32::RED,
            stroke_width: 1.0,
            text: String::new(),
            number: Some(8),
        };

        let image = app.compose_current_selection(&[number]).unwrap();
        let has_smoothed_glyph_pixel = (17..=33).any(|y| {
            (42..=58).any(|x| {
                let pixel = image.get_pixel(x, y).0;
                pixel[0] == 255 && (1..=254).contains(&pixel[1]) && (1..=254).contains(&pixel[2])
            })
        });

        assert!(has_smoothed_glyph_pixel);
    }

    #[test]
    fn save_image_to_path_reports_write_failure() {
        let mut app = crate::app_default::ScreenshotApp::default();
        let image = image::RgbaImage::new(1, 1);
        let directory_instead_of_file = std::env::temp_dir();

        let result = app.save_image_to_path(&image, &directory_instead_of_file);

        assert!(result.is_err());
    }
}
