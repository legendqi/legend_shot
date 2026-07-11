use std::time::{Duration, Instant};

use crate::app_default::{AppSignal, AppView, ScreenshotApp};
use crate::ocr::OcrViewState;
use eframe::App;
use egui::Visuals;

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_ocr(Instant::now());
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        self.window_rect = ctx.viewport_rect();

        match self.app_view {
            AppView::Capture => self.update_capture_view(ctx),
            AppView::OcrResult => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.draw_ocr_result(ui, ctx);
                });
            }
        }
    }

    // 不设置背景图片和背景色，必须添加此方法，不然会出现黑色背景
    fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

impl ScreenshotApp {
    fn update_capture_view(&mut self, ctx: &egui::Context) {
        if self.display_textures_split.is_empty() {
            self.screen_to_texture(ctx);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.draw_screens(ui);
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
}
