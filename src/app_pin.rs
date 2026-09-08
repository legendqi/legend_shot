use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, Vec2};
use image::{GenericImageView, RgbaImage};

use crate::app_default::{AppLifecycle, DisplayTextureTile, MAX_TEXTURE_SIZE, ScreenshotApp};

#[derive(Default)]
pub(crate) struct PinnedImages {
    images: Vec<PinnedImage>,
    next_id: u64,
    save_request: Option<PinSaveRequest>,
}

struct PinSaveRequest {
    id: u64,
    hide_state: PinCaptureState,
}

struct PinnedImage {
    id: u64,
    image: RgbaImage,
    logical_size: Vec2,
    position: Pos2,
    zoom: f32,
    opacity: f32,
    textures: Vec<DisplayTextureTile>,
    positioned: bool,
    controls_open: bool,
    error: Option<String>,
    copied: bool,
}

impl PinnedImages {
    fn add(&mut self, image: RgbaImage, selection: Rect, bounds: Rect) -> u64 {
        self.next_id += 1;
        let logical_size = selection.size().max(Vec2::splat(1.0));
        let fit = (bounds.size() * 0.9 / logical_size).min_elem().min(1.0);
        let zoom = fit.min(4096.0 / logical_size.max_elem());
        let window_size = (logical_size * zoom).max(Vec2::splat(32.0));
        let position = selection
            .min
            .clamp(bounds.min, (bounds.max - window_size).max(bounds.min));
        self.images.push(PinnedImage {
            id: self.next_id,
            image,
            logical_size,
            position,
            zoom,
            opacity: 1.0,
            textures: Vec::new(),
            positioned: false,
            controls_open: false,
            error: None,
            copied: false,
        });
        self.next_id
    }

    fn remove(&mut self, id: u64) {
        self.images.retain(|pin| pin.id != id);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    pub(crate) fn is_saving(&self) -> bool {
        self.save_request.is_some()
    }

    fn take_ready_save(&mut self, now: Instant) -> Option<u64> {
        if self
            .save_request
            .as_mut()?
            .hide_state
            .after_hidden_frame(now)
        {
            self.save_request.take().map(|request| request.id)
        } else {
            None
        }
    }
}

impl PinnedImage {
    fn image_size(&self) -> Vec2 {
        self.logical_size * self.zoom
    }

    fn window_size(&self) -> Vec2 {
        self.image_size().max(Vec2::splat(32.0))
    }

    fn change_zoom(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            let max = (4096.0 / self.logical_size.max_elem()).min(8.0);
            self.zoom = (self.zoom * factor).clamp(0.1_f32.min(max), max);
        }
    }

    fn change_opacity(&mut self, delta: f32) {
        if delta.is_finite() {
            self.opacity = (self.opacity + delta).clamp(0.2, 1.0);
        }
    }

    fn viewport_id(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("legend-shot-pin", self.id))
    }

    fn controls_id(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("legend-shot-pin-controls", self.id))
    }

    fn builder(&self, visible: bool) -> egui::ViewportBuilder {
        let mut builder = egui::ViewportBuilder::default()
            .with_title("Legend Shot · 贴图")
            .with_inner_size(self.window_size())
            .with_min_inner_size(Vec2::splat(32.0))
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_active(false)
            .with_visible(visible);
        // Setting the original position on every frame would undo native dragging.
        if !self.positioned {
            builder = builder.with_position(self.position);
        }
        builder
    }

    fn ensure_textures(&mut self, ctx: &egui::Context) {
        if !self.textures.is_empty() {
            return;
        }
        for pixel_rect in
            crate::display::tile_rects(self.image.dimensions(), MAX_TEXTURE_SIZE as u32)
        {
            let tile = self
                .image
                .view(
                    pixel_rect.x,
                    pixel_rect.y,
                    pixel_rect.width,
                    pixel_rect.height,
                )
                .to_image();
            let texture = ctx.load_texture(
                format!("pin-{}-{}-{}", self.id, pixel_rect.x, pixel_rect.y),
                egui::ColorImage::from_rgba_unmultiplied(
                    [pixel_rect.width as usize, pixel_rect.height as usize],
                    tile.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            );
            self.textures.push(DisplayTextureTile {
                pixel_rect,
                texture,
            });
        }
    }

    fn draw_image(&mut self, ctx: &egui::Context) -> Option<PinAction> {
        self.ensure_textures(ctx);
        let mut action = None;
        if let Some(rect) = ctx.input(|input| input.viewport().outer_rect) {
            self.position = rect.min;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let window_rect = ui.max_rect();
                let image_rect = Rect::from_center_size(window_rect.center(), self.image_size());
                let scale = image_rect.size()
                    / Vec2::new(self.image.width() as f32, self.image.height() as f32);
                let tint = Color32::from_white_alpha((self.opacity * 255.0).round() as u8);
                for tile in &self.textures {
                    let pixels = tile.pixel_rect;
                    let rect = Rect::from_min_size(
                        image_rect.min + Vec2::new(pixels.x as f32, pixels.y as f32) * scale,
                        Vec2::new(pixels.width as f32, pixels.height as f32) * scale,
                    );
                    ui.painter().image(
                        tile.texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        tint,
                    );
                }
                // A border also gives very small or transparent screenshots a visible drag target.
                ui.painter().rect_stroke(
                    window_rect.shrink(0.5),
                    0.0,
                    (
                        1.0,
                        Color32::from_rgba_unmultiplied(
                            135,
                            110,
                            210,
                            (self.opacity * 220.0) as u8,
                        ),
                    ),
                    egui::StrokeKind::Inside,
                );
                let response = ui.interact(
                    window_rect,
                    ui.id().with("pin-image"),
                    egui::Sense::click_and_drag(),
                );
                if response.is_pointer_button_down_on()
                    && ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary))
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if response.secondary_clicked() {
                    self.controls_open = true;
                }
                if response.hovered() {
                    let wheel_events = ui.input(|input| {
                        input
                            .events
                            .iter()
                            .filter_map(|event| {
                                if let egui::Event::MouseWheel {
                                    unit,
                                    delta,
                                    modifiers,
                                } = event
                                {
                                    let scroll = delta.y
                                        * match unit {
                                            egui::MouseWheelUnit::Point => 1.0,
                                            egui::MouseWheelUnit::Line => 40.0,
                                            egui::MouseWheelUnit::Page => window_rect.height(),
                                        };
                                    (scroll != 0.0).then_some((scroll, *modifiers))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    });
                    for (scroll, modifiers) in wheel_events {
                        if modifiers.command || modifiers.ctrl {
                            self.change_opacity(scroll.signum() * 0.05);
                        } else {
                            self.change_zoom(1.1_f32.powf((scroll / 40.0).clamp(-10.0, 10.0)));
                        }
                    }
                }
            });
        if ctx.input(|input| input.viewport().close_requested()) {
            action = Some(PinAction::Close);
        } else if ctx.input(|input| input.focused) && !ctx.wants_keyboard_input() {
            ctx.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    action = Some(PinAction::Close);
                } else if input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy))
                    || input.consume_key(egui::Modifiers::COMMAND, egui::Key::C)
                {
                    action = Some(PinAction::Copy);
                } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::S) {
                    action = Some(PinAction::Save);
                }
            });
        }
        action
    }

    fn draw_controls(&mut self, ctx: &egui::Context) -> Option<PinAction> {
        let mut action = None;
        // Use a separate window so the controls remain usable even for a 1 × 1 screenshot.
        ctx.show_viewport_immediate(self.controls_id(), egui::ViewportBuilder::default()
            .with_title("Legend Shot · 贴图操作")
            .with_inner_size([280.0, 310.0])
            .with_resizable(false)
            .with_decorations(true)
            .with_transparent(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_visible(true), |controls_ctx, _| {
            if controls_ctx.input(|input| input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)) {
                self.controls_open = false;
            }
            egui::CentralPanel::default().show(controls_ctx, |ui| {
                ui.label(format!("原图 {} × {} 像素", self.image.width(), self.image.height()));
                ui.label(format!("显示缩放 {:.0}%", self.zoom * 100.0));
                ui.horizontal(|ui| {
                    if ui.button("缩小").clicked() { self.change_zoom(1.0 / 1.2); }
                    if ui.button("放大").clicked() { self.change_zoom(1.2); }
                    if ui.button("原始大小").clicked() { self.change_zoom(1.0 / self.zoom); }
                });
                ui.add(egui::Slider::new(&mut self.opacity, 0.2..=1.0).text("不透明度"));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("复制原图").clicked() { action = Some(PinAction::Copy); }
                    if ui.button("保存原图").clicked() { action = Some(PinAction::Save); }
                });
                if ui.button("关闭这张贴图").clicked() { action = Some(PinAction::Close); }
                ui.separator();
                ui.small("拖动移动 · 滚轮缩放\nCtrl/Cmd + 滚轮调整不透明度\nEsc 关闭贴图 · Ctrl/Cmd+C 复制\nCtrl/Cmd+S 保存 · 右键打开此面板");
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(210, 70, 60), error);
                } else if self.copied {
                    ui.label("已复制原图");
                }
            });
        });
        if !self.controls_open {
            ctx.send_viewport_cmd_to(self.controls_id(), egui::ViewportCommand::Close);
        }
        action
    }
}

/// Wait for the native compositor after submitting hide commands for existing pins.
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) enum PinCaptureState {
    #[default]
    Idle,
    Hiding,
    Settling(Instant),
}

impl PinCaptureState {
    pub(crate) fn after_hidden_frame(&mut self, now: Instant) -> bool {
        match *self {
            Self::Hiding => *self = Self::Settling(now + Duration::from_millis(80)),
            Self::Settling(deadline) if now >= deadline => {
                *self = Self::Idle;
                return true;
            }
            _ => {}
        }
        false
    }
}

impl ScreenshotApp {
    fn create_pin_from_selection(&mut self) -> Result<u64, String> {
        let selection = self
            .selection_rect
            .ok_or_else(|| "请选择要贴图的区域".to_owned())?;
        let image = self.compose_current_selection(&self.annotations_for_export())?;
        let session = self
            .capture_session
            .as_ref()
            .ok_or_else(|| "截图会话不存在".to_owned())?;
        let bounds = session
            .displays
            .iter()
            .max_by(|a, b| {
                a.geometry
                    .logical_bounds
                    .intersect(selection)
                    .area()
                    .max(0.0)
                    .total_cmp(
                        &b.geometry
                            .logical_bounds
                            .intersect(selection)
                            .area()
                            .max(0.0),
                    )
            })
            .map(|display| display.geometry.logical_bounds)
            .ok_or_else(|| "未检测到可显示贴图的显示器".to_owned())?;
        Ok(self.pins.add(image, selection, bounds))
    }

    pub(crate) fn pin_selection_and_finish(&mut self, ctx: &egui::Context) {
        match self.create_pin_from_selection() {
            Ok(_) => self.hide_capture_window(ctx),
            Err(error) => self.capture_error = Some(error),
        }
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    pub(crate) fn draw_pinned_images(&mut self, ctx: &egui::Context) {
        if self.lifecycle == AppLifecycle::Exiting {
            return;
        }
        let visible = self.lifecycle == AppLifecycle::TrayIdle && !self.pins.is_saving();
        let mut actions = Vec::new();
        for pin in &mut self.pins.images {
            let old_size = pin.window_size();
            let old_display = (pin.zoom, pin.opacity);
            let action = ctx.show_viewport_immediate(
                pin.viewport_id(),
                pin.builder(visible),
                |pin_ctx, _| {
                    if visible {
                        pin.draw_image(pin_ctx)
                    } else {
                        None
                    }
                },
            );
            pin.positioned = true;
            if !visible && pin.controls_open {
                ctx.send_viewport_cmd_to(pin.controls_id(), egui::ViewportCommand::Close);
                pin.controls_open = false;
            }
            if let Some(action) = action {
                actions.push((pin.id, action));
            } else if visible
                && pin.controls_open
                && let Some(action) = pin.draw_controls(ctx)
            {
                actions.push((pin.id, action));
            }
            if old_size != pin.window_size() {
                ctx.send_viewport_cmd_to(
                    pin.viewport_id(),
                    egui::ViewportCommand::InnerSize(pin.window_size()),
                );
            }
            if old_display != (pin.zoom, pin.opacity) {
                ctx.request_repaint_of(pin.viewport_id());
            }
        }
        // Process only requests that were present when this hidden frame started.
        // In particular, never open a modal dialog in the frame that clicked Save.
        if !visible && self.pins.is_saving() {
            if let Some(id) = self.pins.take_ready_save(Instant::now()) {
                self.save_pinned_image(ctx, id);
            } else {
                ctx.request_repaint_after(Duration::from_millis(20));
            }
        }
        for (id, action) in actions {
            self.handle_pin_action(ctx, id, action);
        }
    }

    fn handle_pin_action(&mut self, ctx: &egui::Context, id: u64, action: PinAction) {
        let Some(pin) = self.pins.images.iter().find(|pin| pin.id == id) else {
            return;
        };
        match action {
            PinAction::Close => {
                ctx.send_viewport_cmd_to(pin.viewport_id(), egui::ViewportCommand::Close);
                ctx.send_viewport_cmd_to(pin.controls_id(), egui::ViewportCommand::Close);
                self.pins.remove(id);
            }
            PinAction::Copy => {
                let result = self.set_to_clipboard(pin.image.clone());
                self.finish_pin_export(id, result, true);
            }
            PinAction::Save => {
                if self.pins.is_saving() {
                    return;
                }
                self.pins.save_request = Some(PinSaveRequest {
                    id,
                    hide_state: PinCaptureState::Hiding,
                });
                if let Some(runtime) = &self.hotkey_runtime {
                    runtime.set_capture_enabled(false);
                }
            }
        }
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    fn save_pinned_image(&mut self, ctx: &egui::Context, id: u64) {
        if let Some(image) = self
            .pins
            .images
            .iter()
            .find(|pin| pin.id == id)
            .map(|pin| pin.image.clone())
        {
            let result = self
                .choose_native_save_path()
                .map(|path| self.save_image_to_path(&image, &path))
                .unwrap_or(Ok(()));
            self.finish_pin_export(id, result, false);
        }
        if let Some(runtime) = &self.hotkey_runtime {
            runtime.set_capture_enabled(
                self.lifecycle == AppLifecycle::TrayIdle && !self.shortcut_settings.open,
            );
        }
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    fn finish_pin_export(&mut self, id: u64, result: Result<(), String>, copied: bool) {
        if let Some(pin) = self.pins.images.iter_mut().find(|pin| pin.id == id) {
            pin.copied = copied && result.is_ok();
            pin.error = result.err();
            if pin.error.is_some() {
                pin.controls_open = true;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PinAction {
    Copy,
    Save,
    Close,
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;
    use image::Rgba;

    use crate::app_default::{Annotation, AppLifecycle, TextInputState, Tool};
    use crate::display::{CaptureSession, CapturedDisplay, DisplayGeometry};

    fn bounds() -> Rect {
        Rect::from_min_size(Pos2::new(-400.0, 0.0), Vec2::new(400.0, 300.0))
    }

    fn capture_app() -> ScreenshotApp {
        let mut app = ScreenshotApp::default();
        let geometry = DisplayGeometry::new(0, bounds(), (800, 600)).unwrap();
        app.install_capture_session(
            CaptureSession::new(vec![
                CapturedDisplay::from_image(
                    geometry,
                    RgbaImage::from_pixel(800, 600, Rgba([240, 240, 240, 255])),
                    2048,
                )
                .unwrap(),
            ])
            .unwrap(),
        );
        app.selection_rect = Some(Rect::from_min_size(
            Pos2::new(-300.0, 20.0),
            Vec2::new(100.0, 80.0),
        ));
        app
    }

    #[test]
    fn pin_keeps_retina_pixels_and_annotations_after_capture_cleanup() {
        let mut app = capture_app();
        app.annotations.push(Annotation {
            tool: Tool::Rectangle,
            points: vec![Pos2::new(-280.0, 40.0), Pos2::new(-230.0, 80.0)],
            color: Color32::RED,
            stroke_width: 2.0,
            text: String::new(),
            number: None,
        });
        let expected = app.compose_current_selection(&app.annotations).unwrap();
        let id = app.create_pin_from_selection().unwrap();
        app.hide_capture_window(&egui::Context::default());

        assert_eq!(app.lifecycle, AppLifecycle::TrayIdle);
        assert!(app.capture_session.is_none());
        assert_eq!(app.pins.images.len(), 1);
        let pin = &app.pins.images[0];
        assert_eq!(pin.id, id);
        assert_eq!(pin.image, expected);
        assert_eq!(pin.image.dimensions(), (200, 160));
        assert_eq!(pin.image.get_pixel(40, 40).0, [255, 0, 0, 255]);
        assert_eq!(pin.image_size(), Vec2::new(100.0, 80.0));
    }

    #[test]
    fn pin_includes_text_currently_being_edited() {
        let mut app = capture_app();
        let annotation = Annotation {
            tool: Tool::Text,
            points: vec![Pos2::new(-290.0, 30.0)],
            color: Color32::RED,
            stroke_width: 3.0,
            text: String::new(),
            number: None,
        };
        app.current_annotation = Some(annotation.clone());
        app.text_input = Some(TextInputState {
            text: "Pin".to_owned(),
            ..TextInputState::new(Pos2::new(-290.0, 30.0))
        });
        let expected = app
            .compose_current_selection(&[Annotation {
                text: "Pin".to_owned(),
                ..annotation
            }])
            .unwrap();

        app.create_pin_from_selection().unwrap();

        assert_eq!(app.pins.images[0].image, expected);
        assert!(
            expected
                .pixels()
                .any(|pixel| pixel.0 != [240, 240, 240, 255])
        );
    }

    #[test]
    fn failed_pin_preserves_selection_and_existing_pins() {
        let mut app = capture_app();
        app.create_pin_from_selection().unwrap();
        app.capture_session = None;
        let selection = app.selection_rect;

        assert!(app.create_pin_from_selection().is_err());
        assert_eq!(app.pins.images.len(), 1);
        assert_eq!(app.selection_rect, selection);
    }

    #[test]
    fn closing_one_pin_does_not_close_or_reuse_other_pin_ids() {
        let mut app = capture_app();
        let first = app.create_pin_from_selection().unwrap();
        let second = app.create_pin_from_selection().unwrap();
        app.pins.remove(first);
        let third = app.create_pin_from_selection().unwrap();

        assert_eq!(
            app.pins.images.iter().map(|pin| pin.id).collect::<Vec<_>>(),
            vec![second, third]
        );
        assert_ne!(first, third);
    }

    #[test]
    fn zoom_and_opacity_do_not_change_export_pixels() {
        let mut app = capture_app();
        app.create_pin_from_selection().unwrap();
        let pin = &mut app.pins.images[0];
        let source = pin.image.clone();
        pin.change_zoom(2.0);
        pin.change_opacity(-0.5);

        assert_eq!(pin.image_size(), Vec2::new(200.0, 160.0));
        assert_eq!(pin.opacity, 0.5);
        assert_eq!(pin.image, source);
        pin.change_zoom(f32::INFINITY);
        pin.change_zoom(0.0);
        pin.change_zoom(1.0e20);
        pin.change_opacity(-100.0);
        assert!(pin.window_size().is_finite());
        assert!(pin.window_size().max_elem() <= 4096.0);
        assert!(pin.opacity >= 0.2);
    }

    #[test]
    fn initial_pin_fits_negative_origin_monitor_and_preserves_aspect_ratio() {
        let mut pins = PinnedImages::default();
        pins.add(
            RgbaImage::new(1200, 800),
            Rect::from_min_size(Pos2::new(-100.0, 250.0), Vec2::new(600.0, 400.0)),
            bounds(),
        );
        let pin = &pins.images[0];
        let rect = Rect::from_min_size(pin.position, pin.window_size());
        assert!(bounds().contains_rect(rect));
        assert!((pin.image_size().x / pin.image_size().y - 1.5).abs() < 0.001);
    }

    #[test]
    fn tiny_pin_has_a_draggable_window() {
        let mut pins = PinnedImages::default();
        pins.add(
            RgbaImage::new(1, 2),
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 2.0)),
            bounds(),
        );
        let pin = &pins.images[0];
        assert!(pin.window_size().min_elem() >= 32.0);
        assert_eq!(pin.image_size(), Vec2::new(1.0, 2.0));
    }

    fn frame_input(size: Vec2, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        }
    }

    fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn focused_pin_handles_copy_save_and_escape_without_touching_other_pins() {
        let mut app = capture_app();
        app.create_pin_from_selection().unwrap();
        let second = app.create_pin_from_selection().unwrap();
        let ctx = egui::Context::default();
        for (event, expected) in [
            (egui::Event::Copy, PinAction::Copy),
            (
                key_event(egui::Key::S, egui::Modifiers::COMMAND),
                PinAction::Save,
            ),
            (
                key_event(egui::Key::Escape, egui::Modifiers::NONE),
                PinAction::Close,
            ),
        ] {
            let pin = &mut app.pins.images[0];
            let mut action = None;
            let _ = ctx.run(frame_input(pin.window_size(), vec![event]), |ctx| {
                action = pin.draw_image(ctx)
            });
            assert!(action == Some(expected));
        }
        let first = app.pins.images[0].id;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.handle_pin_action(ctx, first, PinAction::Close)
        });
        assert_eq!(app.pins.images.len(), 1);
        assert_eq!(app.pins.images[0].id, second);
    }

    #[test]
    fn pointer_press_starts_native_drag_and_secondary_click_opens_controls() {
        let mut app = capture_app();
        app.create_pin_from_selection().unwrap();
        let pin = &mut app.pins.images[0];
        let ctx = egui::Context::default();
        let position = Pos2::new(15.0, 15.0);
        let pointer = |button, pressed| egui::Event::PointerButton {
            pos: position,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run(
            frame_input(pin.window_size(), vec![egui::Event::PointerMoved(position)]),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        let output = ctx.run(
            frame_input(
                pin.window_size(),
                vec![pointer(egui::PointerButton::Primary, true)],
            ),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|cmd| matches!(cmd, egui::ViewportCommand::StartDrag))
        );
        let _ = ctx.run(
            frame_input(
                pin.window_size(),
                vec![pointer(egui::PointerButton::Primary, false)],
            ),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        let _ = ctx.run(
            frame_input(
                pin.window_size(),
                vec![pointer(egui::PointerButton::Secondary, true)],
            ),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        let _ = ctx.run(
            frame_input(
                pin.window_size(),
                vec![pointer(egui::PointerButton::Secondary, false)],
            ),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        assert!(pin.controls_open);
    }

    #[test]
    fn oversized_image_uploads_tiled_textures_once_without_downsampling() {
        let mut pins = PinnedImages::default();
        let source = RgbaImage::from_pixel(4097, 2, Rgba([12, 23, 34, 255]));
        pins.add(source.clone(), bounds(), bounds());
        let pin = &mut pins.images[0];
        let ctx = egui::Context::default();
        pin.ensure_textures(&ctx);
        let ids = pin
            .textures
            .iter()
            .map(|tile| tile.texture.id())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 3);
        assert!(
            pin.textures
                .iter()
                .all(|tile| tile.texture.size()[0] <= MAX_TEXTURE_SIZE)
        );
        assert_eq!(
            pin.textures
                .iter()
                .map(|tile| tile.texture.size()[0])
                .sum::<usize>(),
            4097
        );
        pin.change_zoom(2.0);
        pin.ensure_textures(&ctx);
        assert_eq!(
            pin.textures
                .iter()
                .map(|tile| tile.texture.id())
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(pin.image, source);
    }

    #[test]
    fn wheel_uses_modifiers_at_event_time_for_opacity_instead_of_zoom() {
        let mut app = capture_app();
        app.create_pin_from_selection().unwrap();
        let pin = &mut app.pins.images[0];
        let ctx = egui::Context::default();
        let position = Pos2::new(15.0, 15.0);
        let _ = ctx.run(
            frame_input(pin.window_size(), vec![egui::Event::PointerMoved(position)]),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        let wheel = |modifiers| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -40.0),
            modifiers,
        };
        // Command was released before the frame, but was held for the wheel event.
        let _ = ctx.run(
            frame_input(pin.window_size(), vec![wheel(egui::Modifiers::COMMAND)]),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        assert_eq!(pin.zoom, 1.0);
        assert!(pin.opacity < 1.0);
        let opacity = pin.opacity;
        let _ = ctx.run(
            frame_input(pin.window_size(), vec![wheel(egui::Modifiers::NONE)]),
            |ctx| {
                pin.draw_image(ctx);
            },
        );
        assert!(pin.zoom < 1.0);
        assert_eq!(pin.opacity, opacity);
    }

    #[test]
    fn pin_windows_are_topmost_and_survive_capture_hide_and_restore() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut app = capture_app();
        app.create_pin_from_selection().unwrap();
        app.lifecycle = AppLifecycle::TrayIdle;
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let recorded = Rc::new(RefCell::new(Vec::new()));
        let recorder = recorded.clone();
        egui::Context::set_immediate_viewport_renderer(move |ctx, mut viewport| {
            recorder
                .borrow_mut()
                .push((viewport.ids.this, viewport.builder.clone()));
            let mut input = frame_input(viewport.builder.inner_size.unwrap(), vec![]);
            input.viewport_id = viewport.ids.this;
            input
                .viewports
                .insert(viewport.ids.this, egui::ViewportInfo::default());
            let _ = ctx.run(input, |ctx| (viewport.viewport_ui_cb)(ctx));
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.draw_pinned_images(ctx));
        app.lifecycle = AppLifecycle::Capturing;
        app.pin_capture_state = PinCaptureState::Hiding;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.draw_pinned_images(ctx));
        app.hide_capture_window(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.draw_pinned_images(ctx));

        let windows = recorded.borrow();
        assert_eq!(windows.len(), 3);
        assert!(windows.iter().all(|(id, builder)| *id == windows[0].0
            && builder.window_level == Some(egui::WindowLevel::AlwaysOnTop)));
        assert_eq!(
            windows
                .iter()
                .map(|(_, builder)| builder.visible)
                .collect::<Vec<_>>(),
            vec![Some(true), Some(false), Some(true)]
        );
        assert_eq!(windows[0].1.position, Some(Pos2::new(-300.0, 20.0)));
        assert_eq!(
            windows[2].1.position, None,
            "restoring must preserve the position after dragging"
        );
        assert_eq!(app.pin_capture_state, PinCaptureState::Idle);
        assert_eq!(app.pins.images.len(), 1);
        drop(windows);

        let id = app.pins.images[0].id;
        app.handle_pin_action(&ctx, id, PinAction::Save);
        assert!(app.pins.is_saving());
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.draw_pinned_images(ctx));
        assert_eq!(recorded.borrow().last().unwrap().1.visible, Some(false));
        assert_eq!(
            app.pins
                .take_ready_save(Instant::now() + Duration::from_secs(1)),
            Some(id)
        );
        // A cancelled native dialog returns Ok and must still restore the pin.
        app.finish_pin_export(id, Ok(()), false);
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.draw_pinned_images(ctx));
        assert_eq!(recorded.borrow().last().unwrap().1.visible, Some(true));
        assert_eq!(app.pins.images[0].id, id);
        app.finish_pin_export(id, Err("保存失败".to_owned()), false);
        assert_eq!(app.pins.images[0].error.as_deref(), Some("保存失败"));
        assert!(app.pins.images[0].controls_open);
        assert_eq!(app.pins.images[0].image.dimensions(), (200, 160));
    }

    #[test]
    fn capture_waits_for_hidden_frame_and_compositor_then_runs_once() {
        let now = Instant::now();
        let mut state = PinCaptureState::Hiding;
        assert!(!state.after_hidden_frame(now));
        assert!(!state.after_hidden_frame(now + Duration::from_millis(20)));
        assert!(state.after_hidden_frame(now + Duration::from_millis(100)));
        assert!(!state.after_hidden_frame(now + Duration::from_millis(200)));
        assert_eq!(state, PinCaptureState::Idle);
    }

    #[test]
    fn save_request_waits_for_hidden_frame_before_becoming_ready() {
        let mut pins = PinnedImages::default();
        let id = pins.add(RgbaImage::new(20, 20), bounds(), bounds());
        pins.save_request = Some(PinSaveRequest {
            id,
            hide_state: PinCaptureState::Hiding,
        });
        let now = Instant::now();
        assert!(pins.is_saving());
        assert_eq!(pins.take_ready_save(now), None);
        assert!(pins.is_saving());
        assert_eq!(pins.take_ready_save(now + Duration::from_millis(20)), None);
        assert_eq!(
            pins.take_ready_save(now + Duration::from_millis(100)),
            Some(id)
        );
        assert!(!pins.is_saving());
        assert_eq!(pins.take_ready_save(now + Duration::from_secs(1)), None);
        assert_eq!(pins.images.len(), 1);
    }

    #[test]
    fn shortcut_settings_cannot_open_while_pin_save_is_pending() {
        let mut app = capture_app();
        let id = app.create_pin_from_selection().unwrap();
        app.lifecycle = AppLifecycle::TrayIdle;
        let ctx = egui::Context::default();
        app.handle_pin_action(&ctx, id, PinAction::Save);
        app.open_shortcut_settings(&ctx);
        assert!(!app.shortcut_settings.open);
        assert!(app.pins.is_saving());
    }
}
