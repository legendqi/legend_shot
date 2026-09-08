use std::time::{Duration, Instant};

use crate::app_default::{AppLifecycle, AppView, ScreenshotApp};
use crate::ocr::OcrViewState;
use eframe::App;
use egui::Visuals;

use crate::display::CaptureSession;

const OCR_WINDOW_SAVE_DELAY: Duration = Duration::from_millis(300);
const OCR_WINDOW_MIN_SIZE: egui::Vec2 = egui::vec2(440.0, 320.0);
#[cfg(target_os = "linux")]
pub(crate) const WAYLAND_UNSUPPORTED: &str = "当前版本仅支持 Linux X11，暂不支持 Wayland 截图。";
pub(crate) const NO_MONITORS: &str = "未检测到可截图的显示器。";
pub(crate) const POINTER_UNAVAILABLE: &str = "无法读取系统鼠标位置；请检查辅助功能或输入权限。";
const CAPTURE_OVERLAY_TITLE: &str = "Legend Shot · Capture Overlay";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureOverlayLevel {
    AlwaysOnTop,
    AboveMainMenu,
}

fn capture_overlay_level_for(is_macos: bool) -> CaptureOverlayLevel {
    if is_macos {
        CaptureOverlayLevel::AboveMainMenu
    } else {
        CaptureOverlayLevel::AlwaysOnTop
    }
}

fn is_capture_overlay_title(title: &str) -> bool {
    title == CAPTURE_OVERLAY_TITLE
}

#[cfg(target_os = "macos")]
fn raise_capture_overlays_above_main_menu() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_core_graphics::kCGMainMenuWindowLevel;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let application = NSApplication::sharedApplication(mtm);
    let overlay_level = kCGMainMenuWindowLevel as isize + 1;
    for window in application.windows() {
        if is_capture_overlay_title(&window.title().to_string()) && window.level() != overlay_level
        {
            window.setLevel(overlay_level);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn raise_capture_overlays_above_main_menu() {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OverlayWindowSpec {
    pub display_index: usize,
    pub viewport_id: egui::ViewportId,
    pub position: egui::Pos2,
    pub size: egui::Vec2,
    pub decorated: bool,
    pub always_on_top: bool,
}

pub(crate) fn overlay_viewport_id(display_index: usize) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("capture-overlay", display_index))
}

pub(crate) fn overlay_specs(session: &CaptureSession) -> Vec<OverlayWindowSpec> {
    session
        .displays
        .iter()
        .enumerate()
        .map(|(display_index, display)| OverlayWindowSpec {
            display_index,
            viewport_id: overlay_viewport_id(display.geometry.session_index),
            position: display.geometry.logical_bounds.min,
            size: display.geometry.logical_bounds.size(),
            decorated: false,
            always_on_top: true,
        })
        .collect()
}

pub(crate) fn overlay_ids_to_close(session: &CaptureSession) -> Vec<egui::ViewportId> {
    overlay_specs(session)
        .into_iter()
        .map(|spec| spec.viewport_id)
        .collect()
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn linux_x11_session_supported(
    session_type: Option<&str>,
    display: Option<&str>,
) -> bool {
    if session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland")) {
        return false;
    }
    display.is_some_and(|value| !value.trim().is_empty())
}

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
    CaptureWindowStyle {
        accessory_application: is_macos,
        fullscreen: false,
        decorations: false,
        resizable: false,
        close_button: false,
        minimize_button: false,
        maximize_button: false,
    }
}

#[cfg(test)]
pub(crate) fn capture_window_geometry_commands(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> [egui::ViewportCommand; 3] {
    [
        egui::ViewportCommand::InnerSize(egui::vec2(width, height)),
        egui::ViewportCommand::OuterPosition(egui::pos2(x, y)),
        egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop),
    ]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionDisposition {
    Hide,
    Close,
}

pub(crate) fn completion_disposition_for(resident_platform: bool) -> CompletionDisposition {
    if resident_platform {
        CompletionDisposition::Hide
    } else {
        CompletionDisposition::Close
    }
}

impl App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        {
            self.poll_tray_commands(ctx);
            if let Some(runtime) = &self.hotkey_runtime {
                runtime.set_capture_enabled(
                    self.lifecycle == AppLifecycle::TrayIdle
                        && !self.shortcut_settings.open
                        && !self.pins.is_saving(),
                );
            }
            let triggered = self
                .hotkey_runtime
                .as_ref()
                .is_some_and(|runtime| runtime.take_triggered());
            if triggered && !self.shortcut_settings.open {
                self.begin_tray_capture(ctx);
            }
            self.draw_shortcut_settings(ctx);
            self.draw_pinned_images(ctx);
            if self.lifecycle != AppLifecycle::Exiting
                && ctx.input(|input| input.viewport().close_requested())
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.hide_capture_window(ctx);
                return;
            }
            if self.lifecycle != AppLifecycle::Capturing {
                return;
            }

            if self.pin_capture_state != crate::app_pin::PinCaptureState::Idle {
                if self.pin_capture_state.after_hidden_frame(Instant::now()) {
                    self.capture_prepared_screens(ctx);
                }
                ctx.request_repaint_after(Duration::from_millis(20));
                return;
            }

            if self.capture_reveal_state.take_reveal() {
                ctx.request_repaint();
            }
        }

        if self.process_native_save_dialog(ctx) {
            return;
        }

        self.poll_ocr(Instant::now());
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        self.window_rect = ctx.viewport_rect();

        match self.app_view {
            AppView::Capture => {
                if self.ocr_window_configured {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    self.ocr_window_configured = false;
                    self.capture_reveal_state.begin();
                }
                self.update_capture_view(ctx);
                if self.capture_reveal_state.finish_hidden_frame() {
                    ctx.request_repaint();
                }
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
    pub(crate) fn clear_capture_session(&mut self) {
        self.reset_capture_state();
    }

    pub(crate) fn finish_failed_capture(&mut self, resident_platform: bool) {
        self.clear_capture_session();
        if resident_platform {
            self.lifecycle.finish_capture();
            if let Some(runtime) = &self.hotkey_runtime {
                runtime.set_capture_enabled(!self.shortcut_settings.open && !self.pins.is_saving());
            }
        } else {
            self.lifecycle.exit();
        }
    }

    pub(crate) fn report_capture_start_failure(&mut self, message: &str, resident_platform: bool) {
        eprintln!("Legend Shot 截图启动失败: {message}");
        rfd::MessageDialog::new()
            .set_title("Legend Shot")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .show();
        self.finish_failed_capture(resident_platform);
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    pub(crate) fn install_tray(&mut self, tray_runtime: crate::tray::TrayRuntime) {
        self.tray_runtime = Some(tray_runtime);
        self.lifecycle = AppLifecycle::TrayIdle;
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
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
                crate::tray::TrayCommand::Settings => self.open_shortcut_settings(ctx),
                crate::tray::TrayCommand::Exit => self.exit_application(ctx),
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    fn begin_tray_capture(&mut self, ctx: &egui::Context) {
        if self.shortcut_settings.open || self.pins.is_saving() {
            return;
        }
        if !self.lifecycle.begin_capture() {
            return;
        }
        if let Some(runtime) = &self.hotkey_runtime {
            runtime.set_capture_enabled(false);
        }

        #[cfg(target_os = "macos")]
        if !crate::macos_screen_capture_is_ready() {
            self.lifecycle.finish_capture();
            if let Some(runtime) = &self.hotkey_runtime {
                runtime.set_capture_enabled(true);
            }
            return;
        }

        #[cfg(target_os = "linux")]
        if !linux_x11_session_supported(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("DISPLAY").ok().as_deref(),
        ) {
            self.report_capture_start_failure(WAYLAND_UNSUPPORTED, true);
            return;
        }

        if self.device_state.is_none() {
            self.device_state = device_query::DeviceState::checked_new();
        }
        if self.device_state.is_none() {
            self.report_capture_start_failure(POINTER_UNAVAILABLE, true);
            return;
        }

        self.reset_capture_state();
        if !self.pins.is_empty() {
            self.pin_capture_state = crate::app_pin::PinCaptureState::Hiding;
            ctx.request_repaint();
            return;
        }
        self.capture_prepared_screens(ctx);
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    fn capture_prepared_screens(&mut self, ctx: &egui::Context) {
        if let Err(error) = self.capture_screens() {
            let message = if error.contains("未检测到") {
                NO_MONITORS
            } else {
                &error
            };
            self.report_capture_start_failure(message, true);
            return;
        }
        self.app_view = AppView::Capture;
        self.ocr_window_configured = false;
        self.capture_reveal_state.begin();
        ctx.request_repaint();
    }

    fn reset_capture_state(&mut self) {
        self.pin_capture_state = crate::app_pin::PinCaptureState::Idle;
        self.capture_session = None;
        self.display_textures.clear();
        self.selection_rect = None;
        self.original_selection_rect = None;
        self.selection_start = egui::Pos2::ZERO;
        self.selection_end = egui::Pos2::ZERO;
        self.move_start = egui::Pos2::ZERO;
        self.current_tool = crate::app_default::Tool::Select;
        self.annotations.clear();
        self.edit_history = crate::app_history::EditHistory::default();
        self.capture_error = None;
        self.current_annotation = None;
        self.text_input = None;
        self.number_input = None;
        self.show_toolbar = false;
        self.toolbar_position = egui::Pos2::ZERO;
        self.toolbar_placement = None;
        self.toolbar_rect_global = None;
        self.tool_bar_focused = false;
        self.text_input_finalized = false;
        self.pointer_snapshot = None;
        self.last_primary_down = false;
        self.is_selecting = false;
        self.is_moving_box = false;
        self.resize_handle = None;
        self.pending_save_image = None;
        self.native_save_dialog_state = crate::app_default::NativeSaveDialogState::Idle;
        self.capture_reveal_state = crate::app_default::CaptureRevealState::Idle;
        self.last_click_time = 0.0;
        self.last_click_pos = egui::Pos2::ZERO;
        self.ocr_session.cancel();
        self.ocr_capture_snapshot = None;
    }

    pub(crate) fn hide_capture_window(&mut self, ctx: &egui::Context) {
        self.close_capture_viewports(ctx);
        match completion_disposition_for(cfg!(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows"
        ))) {
            CompletionDisposition::Hide => {
                self.clear_capture_session();
                self.lifecycle.finish_capture();
                if let Some(runtime) = &self.hotkey_runtime {
                    runtime.set_capture_enabled(
                        !self.shortcut_settings.open && !self.pins.is_saving(),
                    );
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
            CompletionDisposition::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    fn exit_application(&mut self, ctx: &egui::Context) {
        self.lifecycle.exit();
        if let Some(runtime) = &mut self.tray_runtime {
            runtime.shutdown();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn process_native_save_dialog(&mut self, ctx: &egui::Context) -> bool {
        let Some(restore_toolbar) = self.native_save_dialog_state.advance_frame() else {
            if self.native_save_dialog_state != crate::app_default::NativeSaveDialogState::Idle {
                ctx.request_repaint();
            }
            return false;
        };

        let Some(path) = self.choose_native_save_path() else {
            self.cancel_native_save_dialog(restore_toolbar);
            ctx.request_repaint();
            return false;
        };

        if let Some(image) = self.pending_save_image.take() {
            match self.save_image_to_path(&image, &path) {
                Ok(()) => {
                    self.hide_capture_window(ctx);
                    return true;
                }
                Err(error) => {
                    eprintln!("{error}");
                    self.capture_error = Some(error);
                    self.cancel_native_save_dialog(restore_toolbar);
                    ctx.request_repaint();
                    return false;
                }
            }
        }

        self.cancel_native_save_dialog(restore_toolbar);
        ctx.request_repaint();
        false
    }

    fn cancel_native_save_dialog(&mut self, restore_toolbar: bool) {
        self.pending_save_image = None;
        self.show_toolbar = restore_toolbar;
    }

    fn configure_ocr_window(&mut self, ctx: &egui::Context) {
        let state = self.config.ocr_window;
        self.close_capture_viewports(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
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

    fn update_capture_view(&mut self, ctx: &egui::Context) {
        self.update_capture_viewports(ctx);
    }

    fn update_capture_viewports(&mut self, ctx: &egui::Context) {
        self.ensure_display_textures(ctx);
        let Some(session) = &self.capture_session else {
            return;
        };
        let specs = overlay_specs(session);
        if let Some((snapshot, transition)) = self.poll_pointer() {
            self.update_global_pointer_interaction(snapshot, transition, ctx);
        }
        self.tool_bar_focused = false;
        let visible = self.capture_reveal_state == crate::app_default::CaptureRevealState::Idle;
        let mut close_requested = false;

        for spec in specs {
            let builder = egui::ViewportBuilder::default()
                .with_title(CAPTURE_OVERLAY_TITLE)
                .with_position(spec.position)
                .with_inner_size(spec.size)
                .with_decorations(spec.decorated)
                .with_resizable(false)
                .with_transparent(true)
                .with_has_shadow(false)
                .with_close_button(false)
                .with_minimize_button(false)
                .with_maximize_button(false)
                .with_window_level(egui::WindowLevel::AlwaysOnTop)
                .with_visible(visible);
            close_requested |= ctx.show_viewport_immediate(
                spec.viewport_id,
                builder,
                |viewport_ctx, _viewport_class| {
                    self.render_display_viewport(spec.display_index, viewport_ctx);
                    viewport_ctx.input(|input| input.viewport().close_requested())
                },
            );
        }
        if capture_overlay_level_for(cfg!(target_os = "macos"))
            == CaptureOverlayLevel::AboveMainMenu
        {
            raise_capture_overlays_above_main_menu();
        }

        if close_requested {
            self.hide_capture_window(ctx);
        } else {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn close_capture_viewports(&self, ctx: &egui::Context) {
        let Some(session) = &self.capture_session else {
            return;
        };
        for viewport_id in overlay_ids_to_close(session) {
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, Rect, Vec2};
    use image::RgbaImage;

    use crate::app_default::MAX_TEXTURE_SIZE;
    use crate::display::{CaptureSession, CapturedDisplay, DisplayGeometry};

    use super::{
        CAPTURE_OVERLAY_TITLE, CaptureOverlayLevel, CompletionDisposition, OCR_WINDOW_MIN_SIZE,
        capture_overlay_level_for, capture_window_geometry_commands, completion_disposition_for,
        is_capture_overlay_title, linux_x11_session_supported, overlay_ids_to_close, overlay_specs,
    };

    fn test_session_with_bounds(bounds: &[(f32, f32, f32, f32)]) -> CaptureSession {
        let displays = bounds
            .iter()
            .enumerate()
            .map(|(index, &(x, y, width, height))| {
                let pixels = (width as u32, height as u32);
                let geometry = DisplayGeometry::new(
                    index,
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
                    pixels,
                )
                .unwrap();
                CapturedDisplay::from_image(
                    geometry,
                    RgbaImage::new(pixels.0, pixels.1),
                    MAX_TEXTURE_SIZE as u32,
                )
                .unwrap()
            })
            .collect();
        CaptureSession::new(displays).unwrap()
    }

    #[test]
    fn capture_window_is_sized_before_it_moves_to_the_screen_origin() {
        let commands = capture_window_geometry_commands(0.0, 0.0, 1512.0, 982.0);

        assert!(matches!(commands[0], egui::ViewportCommand::InnerSize(_)));
        assert!(matches!(
            commands[1],
            egui::ViewportCommand::OuterPosition(_)
        ));
        assert!(matches!(
            commands[2],
            egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop)
        ));
    }

    #[test]
    fn capture_reset_restores_selection_interaction_defaults() {
        let mut app = crate::app_default::ScreenshotApp::default();
        let geometry = crate::display::DisplayGeometry::new(
            0,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(10.0, 10.0)),
            (10, 10),
        )
        .unwrap();
        let display = crate::display::CapturedDisplay::from_image(
            geometry,
            image::RgbaImage::new(10, 10),
            crate::app_default::MAX_TEXTURE_SIZE as u32,
        )
        .unwrap();
        app.install_capture_session(crate::display::CaptureSession::new(vec![display]).unwrap());
        app.current_tool = crate::app_default::Tool::Pen;
        app.tool_bar_focused = true;
        app.text_input_finalized = true;
        app.last_click_time = 42.0;
        app.last_primary_down = true;
        app.pointer_snapshot = Some(crate::app_default::PointerSnapshot {
            global_position: egui::pos2(1.0, 1.0),
            primary_down: true,
        });

        app.reset_capture_state();

        assert_eq!(app.current_tool, crate::app_default::Tool::Select);
        assert!(!app.tool_bar_focused);
        assert!(!app.text_input_finalized);
        assert_eq!(app.last_click_time, 0.0);
        assert!(!app.last_primary_down);
        assert!(app.pointer_snapshot.is_none());
        assert!(app.toolbar_rect_global.is_none());
        assert!(app.capture_session.is_none());
        assert!(app.display_textures.is_empty());
    }

    #[test]
    fn cancelling_native_save_restores_toolbar_and_keeps_capture_open() {
        let mut app = crate::app_default::ScreenshotApp {
            show_toolbar: false,
            pending_save_image: Some(image::RgbaImage::new(1, 1)),
            ..Default::default()
        };

        app.cancel_native_save_dialog(true);

        assert!(app.show_toolbar);
        assert!(app.pending_save_image.is_none());
        assert_eq!(app.lifecycle, crate::app_default::AppLifecycle::Capturing);
    }

    #[test]
    fn resident_platform_hides_after_capture() {
        assert_eq!(
            completion_disposition_for(true),
            CompletionDisposition::Hide
        );
    }

    #[test]
    fn non_resident_platform_closes_after_capture() {
        assert_eq!(
            completion_disposition_for(false),
            CompletionDisposition::Close
        );
    }

    #[test]
    fn ocr_window_minimum_size_supports_modern_layout() {
        assert_eq!(OCR_WINDOW_MIN_SIZE, egui::vec2(440.0, 320.0));
    }

    #[test]
    fn overlay_specs_preserve_negative_monitor_origins() {
        let session = test_session_with_bounds(&[
            (-1440.0, 0.0, 1440.0, 900.0),
            (0.0, -1080.0, 1920.0, 1080.0),
        ]);

        let specs = overlay_specs(&session);

        assert_eq!(specs[0].position, egui::pos2(-1440.0, 0.0));
        assert_eq!(specs[1].position, egui::pos2(0.0, -1080.0));
        assert_ne!(specs[0].viewport_id, specs[1].viewport_id);
        assert!(
            specs
                .iter()
                .all(|spec| spec.always_on_top && !spec.decorated)
        );
    }

    #[test]
    fn macos_capture_overlay_is_raised_above_the_system_menu_bar() {
        assert_eq!(
            capture_overlay_level_for(true),
            CaptureOverlayLevel::AboveMainMenu
        );
        assert_eq!(
            capture_overlay_level_for(false),
            CaptureOverlayLevel::AlwaysOnTop
        );
        assert!(is_capture_overlay_title(CAPTURE_OVERLAY_TITLE));
        assert!(!is_capture_overlay_title("Legend Shot · 快捷键设置"));
        assert!(!is_capture_overlay_title("Legend Shot · 贴图"));
    }

    #[test]
    fn linux_guard_rejects_wayland_and_requires_display() {
        assert!(linux_x11_session_supported(Some("x11"), Some(":0")));
        assert!(linux_x11_session_supported(None, Some(":0")));
        assert!(!linux_x11_session_supported(Some("wayland"), Some(":0")));
        assert!(!linux_x11_session_supported(None, None));
        assert!(!linux_x11_session_supported(Some("x11"), Some("")));
    }

    #[test]
    fn capture_failure_returns_resident_app_to_idle_without_partial_session() {
        let mut app = crate::app_default::ScreenshotApp {
            lifecycle: crate::app_default::AppLifecycle::Capturing,
            capture_session: None,
            ..Default::default()
        };

        app.finish_failed_capture(true);

        assert_eq!(app.lifecycle, crate::app_default::AppLifecycle::TrayIdle);
        assert!(app.capture_session.is_none());
        assert!(app.display_textures.is_empty());
    }

    #[test]
    fn closing_capture_clears_all_overlay_ids() {
        let mut app = crate::app_default::ScreenshotApp::default();
        app.install_capture_session(test_session_with_bounds(&[
            (0.0, 0.0, 100.0, 100.0),
            (100.0, 0.0, 100.0, 100.0),
        ]));
        let ids = overlay_ids_to_close(app.capture_session.as_ref().unwrap());
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], super::overlay_viewport_id(0));
        assert_eq!(ids[1], super::overlay_viewport_id(1));

        app.clear_capture_session();

        assert!(app.capture_session.is_none());
    }
}
