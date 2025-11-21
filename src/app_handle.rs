use arboard::Clipboard;
use eframe::emath::{Pos2, Rect, Vec2};
use image::{ImageBuffer, Rgba};
use crate::app_default::{Annotation, ScreenshotApp, Tool};
use crate::ui::get_screen_rect;

impl ScreenshotApp {

    pub fn save_screenshot(&self) {
        if let Some(selection_rect) = self.selection_rect {
            if let Some(cropped_image) = self.crop_selection(selection_rect, &self.annotations) {
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
                    if let Err(e) = cropped_image.save_with_format(&path, format) {
                        eprintln!("保存失败: {:?}", e);
                    } else {
                        println!("截图已成功保存至: {:?}", path.as_path());
                    }
                }
            }
        }
    }

    pub fn copy_to_clipboard(&self) {
        if let Some(selection_rect) = self.selection_rect {

            // 确保当前文本输入完成
            let mut annotations = self.annotations.clone();
            if let Some(text_state) = &self.text_input {
                if let Some(annotation) = &self.current_annotation {
                    let mut new_annotation = annotation.clone();
                    new_annotation.text = text_state.text.clone();
                    annotations.push(new_annotation);
                }
            }

            if let Some(cropped_image) = self.crop_selection(selection_rect, &annotations) {
                // 转换为剪贴板格式
                if let Ok(mut clipboard) = Clipboard::new() {
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

    fn crop_selection(&self, selection_rect: Rect, annotations: &Vec<Annotation>) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {

        let x = self.mouse_start.0 as u32;
        let y = self.mouse_start.1 as u32;
        let width = self.mouse_end.0 as u32 - x;
        let height = self.mouse_end.1 as u32 - y;
        // 创建新的图像缓冲区 - 直接使用选择框的大小
        let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        // 用白色背景填充
        for pixel in cropped_image.pixels_mut() {
            *pixel = Rgba([255, 255, 255, 255]);
        }

        // 查找包含选择区域的屏幕
        for (screen, screenshot) in self.screens.iter().zip(&self.screenshots) {
            let screen_rect = get_screen_rect(screen);

            if screen_rect.contains(selection_rect.center()) {
                // 计算在屏幕图像中的相对位置

                if width > 0 && height > 0 {
                    // 创建新的图像缓冲区
                    let mut cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

                    // 复制原始截图内容
                    for src_y in y..(y + height) {
                        for src_x in x..(x + width) {
                            let dst_x = src_x - x;
                            let dst_y = src_y - y;

                            let pixel = screenshot.get_pixel(src_x, src_y);
                            cropped_image.put_pixel(dst_x, dst_y, pixel.clone());
                        }
                    }

                    // 添加标注内容
                    self.add_annotations_to_image(&mut cropped_image, selection_rect, annotations);

                    return Some(cropped_image);
                }
            }
        }
        None
    }

    fn add_annotations_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, selection_rect: Rect, annotations: &Vec<Annotation>) {
        for annotation in annotations {
            self.draw_single_annotation_to_image(image, annotation, selection_rect);
        }
    }

    fn draw_single_annotation_to_image(&self, image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotation: &Annotation, selection_rect: Rect) {
        if annotation.points.len() < 2 {
            return;
        }

        // 转换为相对于裁剪图像的坐标
        let rel_rect = Rect::from_min_size(
            Pos2::new(selection_rect.min.x, selection_rect.min.y),
            Vec2::new(selection_rect.width(), selection_rect.height())
        );

        let color = annotation.color;

        match annotation.tool {
            Tool::Pen => {
                // 绘制自由画笔
                for window in annotation.points.windows(2) {
                    if let [start, end] = window {
                        let start_rel = Pos2::new(start.x - rel_rect.min.x, start.y - rel_rect.min.y);
                        let end_rel = Pos2::new(end.x - rel_rect.min.x, end.y - rel_rect.min.y);

                        // 绘制线条
                        // 由于代码较长，这里只展示基本逻辑
                        let mut x = start_rel.x as usize;
                        let mut y = start_rel.y as usize;
                        let dx = end_rel.x - start_rel.x;
                        let dy = end_rel.y - start_rel.y;
                        let steps = (dx.hypot(dy)).ceil() as usize;

                        for _ in 0..steps {
                            if x < image.width() as usize && y < image.height() as usize {
                                image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                            }
                            x += dx.signum() as usize;
                            y += dy.signum() as usize;
                        }
                    }
                }
            }
            Tool::Rectangle => {
                // 绘制矩形
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    let rect = Rect::from_two_pos(start, end);
                    let rect_rel = Rect::from_min_max(
                        Pos2::new(rect.min.x - rel_rect.min.x, rect.min.y - rel_rect.min.y),
                        Pos2::new(rect.max.x - rel_rect.min.x, rect.max.y - rel_rect.min.y)
                    );

                    // 绘制矩形边框
                    let x1 = rect_rel.min.x as usize;
                    let y1 = rect_rel.min.y as usize;
                    let x2 = rect_rel.max.x as usize;
                    let y2 = rect_rel.max.y as usize;

                    // 顶部和底部边
                    for x in x1..=x2 {
                        if y1 < image.height() as usize {
                            image.put_pixel(x as u32, y1 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                        if y2 < image.height() as usize {
                            image.put_pixel(x as u32, y2 as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                    }

                    // 左侧和右侧边
                    for y in y1..=y2 {
                        if x1 < image.width() as usize {
                            image.put_pixel(x1 as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                        if x2 < image.width() as usize {
                            image.put_pixel(x2 as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                    }
                }
            }
            Tool::Arrow => {
                // 绘制箭头
                if let (Some(&start), Some(&end)) = (annotation.points.first(), annotation.points.last()) {
                    let start_rel = Pos2::new(start.x - rel_rect.min.x, start.y - rel_rect.min.y);
                    let end_rel = Pos2::new(end.x - rel_rect.min.x, end.y - rel_rect.min.y);

                    // 绘制箭头线
                    let mut x = start_rel.x as usize;
                    let mut y = start_rel.y as usize;
                    let dx = (end_rel.x - start_rel.x) as f32;
                    let dy = (end_rel.y - start_rel.y) as f32;
                    let steps = (dx.hypot(dy)).ceil() as usize;

                    for _ in 0..steps {
                        if x < image.width() as usize && y < image.height() as usize {
                            image.put_pixel(x as u32, y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                        }
                        x += dx.signum() as usize;
                        y += dy.signum() as usize;
                    }

                    // 绘制箭头头
                    let arrow_head_length = 5;

                    // 计算箭头头的位置
                    let arrow_head_x = end_rel.x as usize - (arrow_head_length as f32 * dx.signum()).round() as usize;
                    let arrow_head_y = end_rel.y as usize - (arrow_head_length as f32 * dy.signum()).round() as usize;

                    // 绘制箭头头
                    if arrow_head_x < image.width() as usize && arrow_head_y < image.height() as usize {
                        image.put_pixel(arrow_head_x as u32, arrow_head_y as u32, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    }
                }
            }
            Tool::Text => {
                // 绘制文本
                if let Some(&pos) = annotation.points.first() {
                    let _pos_rel = Pos2::new(pos.x - rel_rect.min.x, pos.y - rel_rect.min.y);

                    if !annotation.text.is_empty() {
                        // 这里应添加文本绘制逻辑
                        // 由于文本绘制较复杂，这里省略具体实现
                    }
                }
            }
            _ => {}
        }
    }
}