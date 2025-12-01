// // deepseek
// use image::{ImageBuffer, RgbaImage};
//
// fn split_screenshot(image: &RgbaImage, max_tile_size: u32) -> Vec<(usize, usize, RgbaImage)> {
//     let (width, height) = image.dimensions();
//     let mut tiles = Vec::new();

//     for y in (0..height).step_by(max_tile_size as usize) {
//         for x in (0..width).step_by(max_tile_size as usize) {
//             let tile_width = (width - x).min(max_tile_size);
//             let tile_height = (height - y).min(max_tile_size);

//             let tile = image.view(x, y, tile_width, tile_height).to_image();
//             tiles.push((x as usize, y as usize, tile));
//         }
//     }
//     tiles
// }
//
// use egui::{ColorImage, TextureHandle};
//
// fn create_texture_tiles(
//     ctx: &egui::Context,
//     tiles: Vec<(usize, usize, RgbaImage)>,
// ) -> Vec<(usize, usize, TextureHandle)> {
//     tiles
//         .into_iter()
//         .enumerate()
//         .map(|(i, (x, y, tile))| {
//             let size = [tile.width() as usize, tile.height() as usize];
//             let pixels = tile.into_raw();
//             // 注意：这里假设 image crate 返回的是 RGBA 字节，与 egui 的 ColorImage 匹配
//             let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
//             let texture_handle = ctx.load_texture(format!("tile_{}", i), color_image, Default::default());
//             (x, y, texture_handle)
//         })
//         .collect()
// }
//
// fn draw_texture_tiles(ui: &mut egui::Ui, texture_tiles: &[(usize, usize, TextureHandle)]) {
//     for (x, y, texture_handle) in texture_tiles {
//         let tile_size = texture_handle.size();
//         // 将瓦片绘制到对应的位置
//         ui.put(
//             egui::Rect::from_min_size(
//                 egui::pos2(*x as f32, *y as f32),
//                 egui::vec2(tile_size[0] as f32, tile_size[1] as f32),
//             ),
//             egui::Image::new(texture_handle, tile_size),
//         );
//     }
// }
//
// struct MyApp {
//     texture_tiles: Vec<(usize, usize, TextureHandle)>,
// }
//
// impl MyApp {
//     fn new(cc: &eframe::CreationContext<'_>) -> Self {
//         // 1. 截取屏幕（此处需要你具体的截图逻辑，假设已得到 RgbaImage）
//         let screenshot: RgbaImage = capture_screen();
//
//         // 2. 分割图像，例如按 2048x2048 的瓦片
//         let tiles = split_screenshot(&screenshot, 2048);
//
//         // 3. 创建纹理瓦片
//         let texture_tiles = create_texture_tiles(&cc.egui_ctx, tiles);
//
//         Self { texture_tiles }
//     }
// }
//
// impl eframe::App for MyApp {
//     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//         egui::CentralPanel::default().show(ctx, |ui| {
//             // 4. 绘制所有截图瓦片
//             draw_texture_tiles(ui, &self.texture_tiles);
//
//             // 5. 绘制蒙层
//             draw_overlay(ui);
//         });
//     }
// }
//
//
// // doubao
//
// use image::{DynamicImage, GenericImageView};
// use std::collections::HashMap;
//
// struct Tile {
//     texture: egui::TextureHandle,
//     position: (usize, usize), // (x, y) 瓦片在原始图像中的位置
//     size: (usize, usize),     // (width, height) 瓦片的尺寸
// }
//
// impl MyApp {
//     fn load_large_image(&mut self, image_path: &str, tile_size: (usize, usize)) {
//         let image = image::open(image_path).expect("Failed to open image");
//         let (width, height) = image.dimensions();
//
//         // 计算需要的瓦片数量
//         let tiles_x = (width + tile_size.0 - 1) / tile_size.0;
//         let tiles_y = (height + tile_size.1 - 1) / tile_size.1;
//
//         self.tiles = HashMap::new();
//
//         for y in 0..tiles_y {
//             for x in 0..tiles_x {
//                 // 计算当前瓦片的位置和尺寸
//                 let x_start = x * tile_size.0;
//                 let y_start = y * tile_size.1;
//                 let x_end = std::cmp::min(x_start + tile_size.0, width);
//                 let y_end = std::cmp::min(y_start + tile_size.1, height);
//
//                 // 裁剪瓦片
//                 let tile_image = image.crop(x_start, y_start, x_end - x_start, y_end - y_start);
//
//                 // 将瓦片转换为纹理
//                 let rgba_image = tile_image.to_rgba8();
//                 let (w, h) = tile_image.dimensions();
//
//                 // 创建纹理
//                 let texture = self.ctx.load_texture(
//                     &format!("tile_{}_{}", x, y),
//                     egui::ColorImage::from_rgba_unmultiplied(
//                         [w as _, h as _],
//                         rgba_image.into_raw(),
//                     ),
//                 );
//
//                 self.tiles.insert(
//                     (x, y),
//                     Tile {
//                         texture,
//                         position: (x_start, y_start),
//                         size: (x_end - x_start, y_end - y_start),
//                     },
//                 );
//             }
//         }
//     }
//
//     fn paint_large_image(&self, ui: &mut egui::Ui) {
//         for y in 0..self.tiles_y {
//             for x in 0..self.tiles_x {
//                 if let Some(tile) = self.tiles.get(&(x, y)) {
//                     // 计算瓦片在UI中的位置
//                     let x_pos = (x * tile_size.0) as f32;
//                     let y_pos = (y * tile_size.1) as f32;
//
//                     // 绘制瓦片
//                     ui.painter().image(
//                         tile.texture.id(),
//                         [x_pos, y_pos, x_pos + tile.size.0 as f32, y_pos + tile.size.1 as f32],
//                         [0.0, 0.0, 1.0, 1.0],
//                         egui::Color32::WHITE,
//                     );
//                 }
//             }
//         }
//     }
// }