use eframe::App;
use eframe::emath::Pos2;
use egui::Id;
use crate::app_default::ScreenshotApp;

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.window_rect = ctx.viewport_rect();
        // 首次运行截图
        if self.screenshots.is_empty() {
            if let Err(e) = self.capture_screens(ctx) {
                eprintln!("Failed to capture screens: {}", e);
            }
        }
        // let mut style = ctx.style();
        // style.visuals.text_cursor.stroke.color = self.annotation_color;
        // ctx.set_style(style);
        // 主界面
        egui::Area::new(Id::from("screenshot_area".to_string()))
            .order(egui::Order::Background)
            .fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            self.draw_screens(ui);
            self.draw_overlay(ui);        // 覆盖层和选择框
            self.draw_annotations(ui);    // 标注在覆盖层之上
            self.handle_input(ui, ctx);   // 输入处理，包括更新选择框和标注
            self.draw_text_input(ui);     // 添加文本输入UI

            if self.show_toolbar {
                self.draw_toolbar(ctx);
            }
        });
    }
}