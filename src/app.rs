use std::time::{Duration, Instant};

use crate::app_default::{AppSignal, AppView, ScreenshotApp};
use crate::ocr::OcrViewState;
use eframe::App;
use egui::Visuals;

const OCR_WINDOW_SAVE_DELAY: Duration = Duration::from_millis(300);

pub(crate) fn saved_position_is_visible(
    state: crate::app_default::OcrWindowState,
    monitor_size: egui::Vec2,
) -> bool {
    let (Some(x), Some(y)) = (state.x, state.y) else {
        return false;
    };
    let window = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(state.width, state.height));
    window.intersects(egui::Rect::from_min_size(egui::Pos2::ZERO, monitor_size))
}

pub(crate) fn ocr_window_geometry_can_be_applied(fullscreen: Option<bool>) -> bool {
    fullscreen == Some(false)
}

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_ocr(Instant::now());
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        self.window_rect = ctx.viewport_rect();

        match self.app_view {
            AppView::Capture => {
                if self.ocr_window_configured {
                    self.restore_capture_window(ctx);
                }
                self.update_capture_view(ctx);
            }
            AppView::OcrResult => {
                if !self.ocr_window_configured {
                    self.configure_ocr_window(ctx);
                } else {
                    self.remember_ocr_window(ctx);
                }
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
    fn configure_ocr_window(&mut self, ctx: &egui::Context) {
        let state = self.config.ocr_window;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::EnableButtons {
            close: true,
            minimized: true,
            maximize: true,
        });

        let fullscreen = ctx.input(|input| input.viewport().fullscreen);
        if !ocr_window_geometry_can_be_applied(fullscreen) {
            ctx.request_repaint();
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            state.width.max(200.0),
            state.height.max(200.0),
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            200.0, 200.0,
        )));
        let monitor_size = ctx.input(|input| input.viewport().monitor_size);
        if let (Some(x), Some(y)) = (state.x, state.y)
            && monitor_size.is_none_or(|size| saved_position_is_visible(state, size))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        } else if let Some(command) = egui::ViewportCommand::center_on_screen(ctx) {
            ctx.send_viewport_cmd(command);
        }
        self.ocr_window_configured = true;
    }

    fn remember_ocr_window(&mut self, ctx: &egui::Context) {
        let next = ctx.input(|input| {
            let viewport = input.viewport();
            let inner = viewport.inner_rect?;
            let outer = viewport.outer_rect?;
            Some(crate::app_default::OcrWindowState {
                x: Some(outer.min.x),
                y: Some(outer.min.y),
                width: inner.width(),
                height: inner.height(),
            })
        });

        if let Some(next) = next
            && next.width >= 200.0
            && next.height >= 200.0
            && next != self.config.ocr_window
            && self
                .pending_ocr_window
                .is_none_or(|(pending, _)| pending != next)
        {
            self.pending_ocr_window = Some((next, Instant::now()));
        }

        if let Some((pending, changed_at)) = self.pending_ocr_window
            && Instant::now().duration_since(changed_at) >= OCR_WINDOW_SAVE_DELAY
        {
            self.config.ocr_window = pending;
            self.pending_ocr_window = None;
            self.save_config();
        }
    }

    fn restore_capture_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::EnableButtons {
            close: false,
            minimized: false,
            maximize: false,
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        self.ocr_window_configured = false;
    }

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
