use eframe::emath::{Pos2, Rect};
use eframe::epaint::textures::TextureOptions;
use egui::{Vec2};
use xcap::Monitor;

pub fn get_screen_rect(screen: &Monitor) -> Rect {
    Rect::from_min_size(
        Pos2::new(screen.x().unwrap() as f32, screen.y().unwrap() as f32),
        Vec2::new(screen.width().unwrap() as f32, screen.height().unwrap() as f32),
    )
}


pub fn load_texture_from_png(ctx: &egui::Context, path: &str) -> Option<egui::TextureId> {
    // 使用 image crate 加载图片
    let image_bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&image_bytes).ok()?;
    let image_buffer = image.to_rgba8();
    let size = [image_buffer.width() as _, image_buffer.height() as _];
    let pixels = image_buffer.into_raw();

    let image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    Some(ctx.load_texture(path, image, TextureOptions::LINEAR).id())
}