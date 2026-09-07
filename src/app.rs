use std::time::{Duration, Instant};

use crate::app_default::{AppLifecycle, AppSignal, AppView, ScreenshotApp};
use crate::ocr::OcrViewState;
use eframe::App;
use egui::Visuals;

const OCR_WINDOW_SAVE_DELAY: Duration = Duration::from_millis(300);
const OCR_WINDOW_MIN_SIZE: egui::Vec2 = egui::vec2(440.0, 320.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CaptureWindowStyle {
    pub accessory_application: bool,
    pub fullscreen: bool,
    pub decorations: bool,
    pub resizable: bool,
    pub close_button: bool,
    pub minimize_button: bool,
    pub maximize_button: bool,
}

pub(crate) fn capture_window_style() -> CaptureWindowStyle {
    capture_window_style_for(cfg!(target_os = "macos"))
}

pub(crate) fn capture_window_style_for(is_macos: bool) -> CaptureWindowStyle {
    if is_macos {
        CaptureWindowStyle {
            accessory_application: true,
            fullscreen: false,
            decorations: false,
            resizable: false,
            close_button: false,
            minimize_button: false,
            maximize_button: false,
        }
    } else {
        CaptureWindowStyle {
            accessory_application: false,
            fullscreen: true,
            decorations: true,
            resizable: true,
            close_button: true,
            minimize_button: true,
            maximize_button: true,
        }
    }
}

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
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            self.poll_tray_commands(ctx);
            if self.lifecycle != AppLifecycle::Capturing {
                return;
            }
        }

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
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn install_tray(&mut self, tray_runtime: crate::tray::TrayRuntime) {
        self.tray_runtime = Some(tray_runtime);
        self.lifecycle = AppLifecycle::TrayIdle;
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn poll_tray_commands(&mut self, ctx: &egui::Context) {
        let mut commands = Vec::new();
        if let Some(runtime) = &self.tray_runtime {
            while let Some(command) = runtime.try_recv() {
                commands.push(command);
            }
        }

        for command in commands {
            match command {
                crate::tray::TrayCommand::Capture => self.begin_tray_capture(ctx),
                crate::tray::TrayCommand::Exit => self.exit_application(ctx),
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn begin_tray_capture(&mut self, ctx: &egui::Context) {
        if !self.lifecycle.begin_capture() {
            return;
        }

        #[cfg(target_os = "macos")]
        if !crate::macos_screen_capture_is_ready() {
            self.lifecycle.finish_capture();
            return;
        }

        if self.device_state.is_none() {
            self.device_state = device_query::DeviceState::checked_new();
        }
        if self.device_state.is_none() {
            eprintln!("无法访问系统指针，请授予辅助功能权限后重试截图");
            self.lifecycle.finish_capture();
            return;
        }

        self.reset_capture_state();
        if let Err(error) = self.capture_screens() {
            eprintln!("从托盘启动截图失败: {error}");
            self.lifecycle.finish_capture();
            return;
        }
        self.app_view = AppView::Capture;
        self.ocr_window_configured = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn reset_capture_state(&mut self) {
        self.screens.clear();
        self.screenshots.clear();
        self.original_screenshots.clear();
        self.screenshots_positions.clear();
        self.display_textures_split.clear();
        self.selection_rect = None;
        self.mouse_selection_rect = None;
        self.original_selection_rect = None;
        self.mouse_original_selection_rect = None;
        self.annotations.clear();
        self.current_annotation = None;
        self.text_input = None;
        self.number_input = None;
        self.show_toolbar = false;
        self.is_selecting = false;
        self.is_moving_box = false;
    }

    pub(crate) fn hide_capture_window(&mut self, ctx: &egui::Context) {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            self.reset_capture_state();
            self.lifecycle.finish_capture();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn exit_application(&mut self, ctx: &egui::Context) {
        self.lifecycle.exit();
        if let Some(runtime) = &mut self.tray_runtime {
            runtime.shutdown();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

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
        #[cfg(target_os = "macos")]
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::Normal,
        ));

        let fullscreen = ctx.input(|input| input.viewport().fullscreen);
        if !ocr_window_geometry_can_be_applied(fullscreen) {
            ctx.request_repaint();
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            state.width.max(OCR_WINDOW_MIN_SIZE.x),
            state.height.max(OCR_WINDOW_MIN_SIZE.y),
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(OCR_WINDOW_MIN_SIZE));
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
            && next.width >= OCR_WINDOW_MIN_SIZE.x
            && next.height >= OCR_WINDOW_MIN_SIZE.y
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
        let style = capture_window_style();
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(style.fullscreen));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(style.decorations));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(style.resizable));
        ctx.send_viewport_cmd(egui::ViewportCommand::EnableButtons {
            close: style.close_button,
            minimized: style.minimize_button,
            maximize: style.maximize_button,
        });

        #[cfg(target_os = "macos")]
        if let Some(screen) = self
            .screens
            .iter()
            .find(|screen| screen.is_primary().unwrap_or(false))
            .or_else(|| self.screens.first())
        {
            if let (Ok(x), Ok(y), Ok(width), Ok(height)) =
                (screen.x(), screen.y(), screen.width(), screen.height())
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    x as f32, y as f32,
                )));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    width as f32,
                    height as f32,
                )));
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                    egui::WindowLevel::AlwaysOnTop,
                ));
            }
        }

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

#[cfg(test)]
mod tests {
    use super::OCR_WINDOW_MIN_SIZE;

    #[test]
    fn ocr_window_minimum_size_supports_modern_layout() {
        assert_eq!(OCR_WINDOW_MIN_SIZE, egui::vec2(440.0, 320.0));
    }
}
