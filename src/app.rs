use crate::app_default::{AppSignal, ScreenshotApp};
use eframe::App;
use egui::Visuals;

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 首次运行截图（仅 macOS 需要先截图再全屏）
        #[cfg(target_os = "macos")]
        {
            if self.is_first {
                let _ = self.capture_screens();
                self.is_first = false;
            }
            if self.display_textures_split.is_empty() {
                self.screen_to_texture(ctx);
            }
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            // Windows/Linux: 首次运行时截图
            if self.screenshots.is_empty() {
                self.capture_screens().expect("截图失败");
            }
        }

        self.window_rect = ctx.viewport_rect();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                #[cfg(target_os = "macos")]
                {
                    self.draw_screens(ui);
                }
                self.draw_overlay(ui);
                self.draw_annotations(ui);
                self.handle_input(ui, ctx);
                self.draw_text_input(ui);
                if self.show_toolbar {
                    self.draw_toolbar(ctx);
                }
                let mut pending_action: Option<AppSignal> = None;
                if let Some(signal_receiver) = self.signal_receiver.as_ref() {
                    if let Ok(signal) = signal_receiver.lock().unwrap().try_recv() {
                        match signal {
                            AppSignal::Save => {
                                pending_action = Some(AppSignal::Save);
                            }
                            AppSignal::Copy => {
                                pending_action = Some(AppSignal::Copy);
                            }
                        }
                    }
                    if let Some(action) = pending_action.take() {
                        match action {
                            AppSignal::Save => {
                                self.trigger_save_dialog();
                            }
                            AppSignal::Copy => {
                                let _ = self.copy_to_clipboard();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                }
            });

        self.save_dialog.update(ctx);

        if let Some(path) = self.save_dialog.take_picked() {
            if let Some(image) = self.pending_save_image.take() {
                self.save_image_to_path(&image, &path);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    // 不设置背景图片和背景色，必须添加此方法，不然会出现黑色背景
    fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
