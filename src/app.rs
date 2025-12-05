use eframe::App;
use crate::app_default::{AppSignal, ScreenshotApp};

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.window_rect = ctx.viewport_rect();
        // 首次运行截图
        if self.display_textures_split.is_empty() {
            self.screen_to_texture(ctx);
        }
        // 主界面
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
            self.draw_screens(ui);
            self.draw_overlay(ui);        // 覆盖层和选择框
            self.draw_annotations(ui);    // 标注在覆盖层之上
            self.handle_input(ui, ctx);   // 输入处理，包括更新选择框和标注
            self.draw_text_input(ui);     // 添加文本输入UI
            if self.show_toolbar {
                self.draw_toolbar(ctx);
            }
            let mut pending_action: Option<AppSignal> = None;
            if let(Some(signal_receiver)) = self.signal_receiver.as_ref() {
                if let Ok(signal) = signal_receiver.lock().unwrap().try_recv() {
                    match signal {
                    AppSignal::Save => {
                        // 执行复制逻辑
                        pending_action = Some(AppSignal::Save);
                    },
                    AppSignal::Copy => {
                        pending_action = Some(AppSignal::Copy);
                    }
                    // 处理其他信号...
                }

                }
                if let Some(action) = pending_action.take() {
                    match action {
                        AppSignal::Save => {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            // 执行复制逻辑
                            let _ = self.save_screenshot();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        },
                        AppSignal::Copy => {
                            let _ = self.copy_to_clipboard();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }
        });
    }
}