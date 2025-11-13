mod annotation;
mod app;
mod screenshot;

use arboard::Clipboard;
use eframe::emath::{Pos2, Rect};
use xcap::{Monitor, Window};
use crate::screenshot::ScreenshotApp;

fn get_combined_bounds(screens: &[Window]) -> Rect {
    if screens.is_empty() {
        return Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(100.0));
    }

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for screen in screens {
        min_x = min_x.min(screen.x().unwrap());
        min_y = min_y.min(screen.y().unwrap());
        max_x = max_x.max(screen.x().unwrap() + screen.width().unwrap() as i32);
        max_y = max_y.max(screen.y().unwrap() + screen.height().unwrap() as i32);
    }

    egui::Rect::from_min_max(
        egui::pos2(min_x as f32, min_y as f32),
        egui::pos2(max_x as f32, max_y as f32),
    )
}

/// 将 xcap::Image 复制到剪切板
fn copy_image_to_clipboard(image: &image::RgbaImage) -> Result<(), arboard::Error> {
    let mut clipboard = Clipboard::new()?;

    let width = image.width();
    let height = image.height();

    // 准备图像数据 (RGBA)
    let mut image_data = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            image_data.push(pixel.0[0]); // R
            image_data.push(pixel.0[1]); // G
            image_data.push(pixel.0[2]); // B
            image_data.push(pixel.0[3]); // A
        }
    }

    clipboard.set_image(arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: std::borrow::Cow::Borrowed(&image_data),
    })?;

    Ok(())
}

fn main() -> eframe::Result<()> {
    // let windows = Monitor::all().expect("获取屏幕列表失败");
    // let image = windows[0].capture_image().expect("捕获屏幕失败");
    // copy_image_to_clipboard(&image);
    // if windows.is_empty() {
    //     panic!("未检测到显示器");
    // }
    // let mut full_window_vec = vec![];
    // for window in windows.iter() {
    //     let image = window.capture_image().expect("捕获屏幕失败");
    //     let image_buffer = ImageBuffer::from_raw(
    //         window.width().unwrap(),
    //         window.height().unwrap(),
    //         image.into_raw(),
    //     )
    //     .expect("创建图片失败");
    //     full_window_vec.push(image_buffer);
    // }
    // let capture = windows.get(0).unwrap().capture_image().expect("捕获屏幕失败");
    // let width = capture.width() as usize;
    // let height = capture.height() as usize;
    // let rgba: Vec<u8> = capture.to_vec();

    // let mut options = eframe::NativeOptions::default();
    // options.viewport = egui::ViewportBuilder::default()
    //     .with_fullscreen(true)
    //     .with_maximized(true)
    //     .with_decorations(true)
    //     .with_always_on_top()
    //     .with_maximize_button(false)
    //     .with_minimize_button(false)
    //     .with_close_button(false)
    //     .with_transparent(true);
    
    
    
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_maximized(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_close_button(false)
            .with_transparent(true),
        // .with_position(egui::pos2(combined_bounds.min.x, combined_bounds.min.y))
        // .with_inner_size(egui::vec2(combined_bounds.width(), combined_bounds.height())),
        ..Default::default()
    };
    
    // let mut initial_pixels = Some(rgba);
    
    eframe::run_native(
        "",
        options,
        Box::new(|_cc| Ok(Box::new(ScreenshotApp::default()))),
    )?;

    Ok(())
}


