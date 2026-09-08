use crate::app_default::{
    Annotation, PointerSnapshot, PrimaryButtonTransition, ScreenshotApp, TextInputState, Tool,
};
use crate::ocr::OcrViewState;
use eframe::emath::{Pos2, Rect};
use eframe::epaint::{Color32, Stroke, StrokeKind};

pub(crate) fn global_rect_to_display_local(
    display_bounds: Rect,
    global_rect: Rect,
) -> Option<Rect> {
    let intersection = display_bounds.intersect(global_rect);
    if intersection.area() <= 0.0 {
        return None;
    }
    Some(intersection.translate(-display_bounds.min.to_vec2()))
}

pub(crate) fn clamp_translated_rect(
    original: Rect,
    delta: egui::Vec2,
    desktop_bounds: Rect,
) -> Rect {
    let size = original.size();
    let max_min = desktop_bounds.max - size;
    let translated_min = original.min + delta;
    let min = Pos2::new(
        translated_min.x.clamp(desktop_bounds.min.x, max_min.x),
        translated_min.y.clamp(desktop_bounds.min.y, max_min.y),
    );
    Rect::from_min_size(min, size)
}

impl ScreenshotApp {
    pub(crate) fn render_display_viewport(&mut self, display_index: usize, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.draw_display(display_index, ui);
                self.draw_overlay_for_display(display_index, ui);
                self.draw_annotations_for_display(display_index, ui);
                crate::selection::draw_selection_aids(self, display_index, ui);
                self.draw_text_input_for_display(display_index, ui);
                self.draw_toolbar_for_display(display_index, ctx);
                self.draw_capture_error(display_index, ui);
            });
        self.handle_viewport_keyboard(ctx);
    }

    fn draw_overlay_for_display(&self, display_index: usize, ui: &mut egui::Ui) {
        let Some(display) = self
            .capture_session
            .as_ref()
            .and_then(|session| session.displays.get(display_index))
        else {
            return;
        };
        let display_rect = Rect::from_min_size(Pos2::ZERO, display.geometry.logical_bounds.size());
        let overlay_color = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
        let Some(selection) = self
            .selection_rect
            .and_then(|rect| global_rect_to_display_local(display.geometry.logical_bounds, rect))
        else {
            ui.painter()
                .rect_filled(display_rect, egui::CornerRadius::ZERO, overlay_color);
            return;
        };

        for rect in [
            Rect::from_min_max(
                display_rect.min,
                Pos2::new(display_rect.max.x, selection.min.y),
            ),
            Rect::from_min_max(
                Pos2::new(display_rect.min.x, selection.max.y),
                display_rect.max,
            ),
            Rect::from_min_max(
                Pos2::new(display_rect.min.x, selection.min.y),
                Pos2::new(selection.min.x, selection.max.y),
            ),
            Rect::from_min_max(
                Pos2::new(selection.max.x, selection.min.y),
                Pos2::new(display_rect.max.x, selection.max.y),
            ),
        ] {
            if rect.area() > 0.0 {
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::ZERO, overlay_color);
            }
        }
        ui.painter().rect_stroke(
            selection,
            egui::CornerRadius::ZERO,
            Stroke::new(1.0, Color32::BLUE),
            StrokeKind::Inside,
        );
    }

    pub(crate) fn update_global_pointer_interaction(
        &mut self,
        snapshot: PointerSnapshot,
        transition: PrimaryButtonTransition,
        ctx: &egui::Context,
    ) {
        let position = snapshot.global_position;
        if self.text_input.is_none()
            && !egui::Popup::is_any_open(ctx)
            && matches!(self.current_tool, Tool::Select | Tool::MoveBox)
            && let Some(rect) = self.selection_rect
        {
            let handle = self
                .resize_handle
                .or_else(|| crate::selection::hit_test_handle(rect, position, 6.0));
            if let Some(handle) = handle {
                ctx.set_cursor_icon(handle.cursor());
            }
        }
        match transition {
            PrimaryButtonTransition::Pressed => {
                if egui::Popup::is_any_open(ctx) {
                    return;
                }
                if self.show_toolbar
                    && self
                        .toolbar_rect_global
                        .is_some_and(|rect| rect.contains(position))
                {
                    return;
                }
                if self.text_input.is_none()
                    && self.show_toolbar
                    && matches!(self.current_tool, Tool::Select | Tool::MoveBox)
                    && let Some(rect) = self.selection_rect
                    && let Some(handle) = crate::selection::hit_test_handle(rect, position, 6.0)
                {
                    self.begin_edit();
                    self.resize_handle = Some(handle);
                    self.original_selection_rect = Some(rect);
                    self.move_start = position;
                    self.show_toolbar = false;
                    self.last_click_time = f64::NEG_INFINITY;
                    return;
                }
                let current_time = ctx.input(|input| input.time);
                if self
                    .selection_rect
                    .is_some_and(|rect| rect.contains(position))
                    && !self.tool_bar_focused
                    && matches!(self.current_tool, Tool::Select | Tool::MoveBox)
                    && current_time - self.last_click_time < 0.5
                    && (position - self.last_click_pos).length() < 10.0
                {
                    self.copy_selection_and_finish(ctx);
                    return;
                }
                self.last_click_time = current_time;
                self.last_click_pos = position;

                if self.current_tool == Tool::Select && !self.show_toolbar {
                    self.begin_global_selection(position);
                } else if self.current_tool == Tool::MoveBox
                    && !self.tool_bar_focused
                    && self
                        .selection_rect
                        .is_some_and(|rect| rect.contains(position))
                {
                    self.begin_edit();
                    self.is_moving_box = true;
                    self.show_toolbar = false;
                    self.move_start = position;
                    self.original_selection_rect = self.selection_rect;
                } else if self.current_tool.is_annotation_tool() {
                    self.begin_global_annotation(position);
                }
            }
            PrimaryButtonTransition::Held => {
                if self.update_selection_drag(position) {
                } else if self.is_selecting {
                    self.update_global_selection(position);
                } else if let Some(annotation) = &mut self.current_annotation
                    && self
                        .selection_rect
                        .is_some_and(|rect| rect.contains(position))
                {
                    annotation.points.push(position);
                }
            }
            PrimaryButtonTransition::Released => {
                self.update_selection_drag(position);
                if self.resize_handle.take().is_some() {
                    self.show_toolbar = true;
                    self.original_selection_rect = None;
                    self.commit_edit();
                    self.update_toolbar_placement(position, ctx);
                } else if self.is_selecting {
                    self.update_global_selection(position);
                    self.finish_global_selection();
                    if let Some(rect) = self.selection_rect
                        && rect.width() >= 1.0
                        && rect.height() >= 1.0
                    {
                        if matches!(self.ocr_session.state, OcrViewState::Capturing)
                            && self.ocr_capture_snapshot.is_some()
                        {
                            if let Err(error) = self.finish_ocr_recapture(std::time::Instant::now())
                            {
                                eprintln!("OCR 重新截图提交失败: {error}");
                            }
                        } else {
                            self.show_toolbar = true;
                            self.update_toolbar_placement(position, ctx);
                        }
                    }
                } else if self.is_moving_box {
                    self.is_moving_box = false;
                    self.show_toolbar = true;
                    self.update_toolbar_placement(position, ctx);
                    self.original_selection_rect = None;
                    self.commit_edit();
                } else if self.current_tool != Tool::Text {
                    let should_commit =
                        self.current_annotation.as_ref().is_some_and(|annotation| {
                            annotation.points.len()
                                >= if matches!(annotation.tool, Tool::Mosaic | Tool::Number) {
                                    1
                                } else {
                                    2
                                }
                        });
                    if should_commit {
                        if let Some(annotation) = self.current_annotation.take() {
                            self.annotations.push(annotation);
                        }
                        self.commit_edit();
                    } else {
                        self.current_annotation = None;
                        self.cancel_edit();
                    }
                }
            }
            PrimaryButtonTransition::Idle => {}
        }
    }

    fn update_selection_drag(&mut self, position: Pos2) -> bool {
        let (Some(original), Some(session)) = (self.original_selection_rect, &self.capture_session)
        else {
            return false;
        };
        let next = if let Some(handle) = self.resize_handle {
            crate::selection::resize_selection(
                original,
                handle,
                position - self.move_start,
                session.desktop_bounds,
            )
        } else if self.is_moving_box {
            clamp_translated_rect(original, position - self.move_start, session.desktop_bounds)
        } else {
            return false;
        };
        self.selection_rect = Some(next);
        self.selection_start = next.min;
        self.selection_end = next.max;
        true
    }

    fn begin_global_annotation(&mut self, position: Pos2) {
        if !self
            .selection_rect
            .is_some_and(|rect| rect.contains(position))
        {
            if self.text_input.is_some() || self.current_tool == Tool::Text {
                self.finalize_text_input();
            }
            return;
        }
        if self.current_tool == Tool::Text {
            if self.text_input.is_some() {
                self.finalize_text_input();
                return;
            }
            self.begin_edit();
            self.text_input = Some(TextInputState::new(position));
            self.text_input_finalized = false;
        } else {
            self.begin_edit();
            if self.current_tool == Tool::Number {
                self.number_input = Some(self.number_input.unwrap_or(0) + 1);
            }
        }
        self.start_annotation(position);
    }

    fn handle_viewport_keyboard(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.focused) {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
            self.finalize_text_input();
        }
        if egui::Popup::is_any_open(ctx) {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if matches!(self.ocr_session.state, OcrViewState::Capturing)
                && self.ocr_capture_snapshot.is_some()
            {
                self.cancel_ocr_recapture();
            } else if self.text_input.is_some() {
                self.cancel_text_input();
            } else if self.is_moving_box || self.resize_handle.is_some() {
                self.cancel_edit();
            } else if self.show_toolbar {
                self.clear_selection_edits();
            } else {
                self.hide_capture_window(ctx);
            }
            return;
        }
        if self.text_input.is_some() {
            return;
        }
        let command = egui::Modifiers::COMMAND;
        let redo = ctx.input_mut(|input| {
            input.consume_key(command | egui::Modifiers::SHIFT, egui::Key::Z)
                || (!cfg!(target_os = "macos") && input.consume_key(command, egui::Key::Y))
        });
        if redo {
            self.redo_edit();
            self.refresh_edit_toolbar(ctx);
        } else if ctx.input_mut(|input| input.consume_key(command, egui::Key::Z)) {
            self.undo_edit();
            self.refresh_edit_toolbar(ctx);
        }
        // winit translates Cmd/Ctrl+C into Event::Copy on some platforms.
        let copy = ctx.input_mut(|input| {
            let copy_event = input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Copy));
            input
                .events
                .retain(|event| !matches!(event, egui::Event::Copy));
            input.consume_key(command, egui::Key::C) || copy_event
        });
        if copy && self.selection_rect.is_some() {
            self.copy_selection_and_finish(ctx);
            return;
        }
        if ctx.input_mut(|input| input.consume_key(command, egui::Key::S)) {
            self.trigger_save_dialog(ctx);
            return;
        }
        if !self.show_toolbar
            || self.is_selecting
            || self.is_moving_box
            || self.resize_handle.is_some()
            || self.current_annotation.is_some()
        {
            return;
        }
        let nudges = ctx.input_mut(|input| {
            let mut nudges = Vec::new();
            input.events.retain(|event| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                if modifiers.command || modifiers.ctrl || modifiers.mac_cmd {
                    return true;
                }
                let direction = match key {
                    egui::Key::ArrowLeft => egui::vec2(-1.0, 0.0),
                    egui::Key::ArrowRight => egui::vec2(1.0, 0.0),
                    egui::Key::ArrowUp => egui::vec2(0.0, -1.0),
                    egui::Key::ArrowDown => egui::vec2(0.0, 1.0),
                    _ => return true,
                };
                let step = if modifiers.shift { 10.0 } else { 1.0 };
                nudges.push((direction * step, modifiers.alt));
                false
            });
            nudges
        });
        for (delta, resize) in nudges {
            if let (Some(rect), Some(session)) = (self.selection_rect, &self.capture_session) {
                let updated =
                    crate::selection::nudge_selection(rect, delta, resize, session.desktop_bounds);
                self.begin_edit();
                self.selection_rect = Some(updated);
                self.selection_start = updated.min;
                self.selection_end = updated.max;
                self.commit_edit();
                self.refresh_edit_toolbar(ctx);
            }
        }
    }

    pub(crate) fn draw_display(&self, display_index: usize, ui: &mut egui::Ui) {
        let Some(session) = &self.capture_session else {
            return;
        };
        let Some(display) = session.displays.get(display_index) else {
            return;
        };
        let Some(textures) = self.display_textures.get(display_index) else {
            return;
        };

        for tile in textures {
            let rect = Rect::from_min_size(
                Pos2::new(
                    tile.pixel_rect.x as f32 / display.geometry.pixel_scale.x,
                    tile.pixel_rect.y as f32 / display.geometry.pixel_scale.y,
                ),
                egui::vec2(
                    tile.pixel_rect.width as f32 / display.geometry.pixel_scale.x,
                    tile.pixel_rect.height as f32 / display.geometry.pixel_scale.y,
                ),
            );
            ui.put(
                rect,
                egui::Image::new(&tile.texture).fit_to_exact_size(rect.size()),
            );
        }
    }

    fn finalize_text_input(&mut self) {
        if let (Some(text_state), Some(mut annotation)) =
            (self.text_input.take(), self.current_annotation.take())
        {
            annotation.text = text_state.text;
            if !annotation.text.trim().is_empty() || annotation.points.len() > 1 {
                self.annotations.push(annotation);
                self.commit_edit();
            } else {
                self.cancel_edit();
            }
        }
        self.text_input_finalized = false;
    }

    fn cancel_text_input(&mut self) {
        self.cancel_edit();
        self.text_input = None;
        self.current_annotation = None;
        self.text_input_finalized = false;
    }

    pub(crate) fn refresh_edit_toolbar(&mut self, ctx: &egui::Context) {
        if let Some(rect) = self.selection_rect {
            let endpoint = self
                .pointer_snapshot
                .map_or(rect.max, |pointer| pointer.global_position);
            self.update_toolbar_placement(endpoint, ctx);
        }
        ctx.request_repaint();
    }

    fn draw_capture_error(&self, display_index: usize, ui: &mut egui::Ui) {
        let Some(error) = &self.capture_error else {
            return;
        };
        if self
            .toolbar_placement
            .is_some_and(|placement| placement.display_index != display_index)
        {
            return;
        }
        let rect = ui.max_rect();
        let text = ui.painter().layout(
            error.clone(),
            egui::FontId::proportional(15.0),
            Color32::WHITE,
            (rect.width() - 48.0).max(1.0),
        );
        let position = egui::pos2(rect.center().x - text.size().x / 2.0, rect.top() + 24.0);
        ui.painter().rect_filled(
            Rect::from_min_size(position, text.size()).expand(10.0),
            6.0,
            Color32::from_rgb(155, 35, 35),
        );
        ui.painter().galley(position, text, Color32::WHITE);
    }

    fn start_annotation(&mut self, pos: Pos2) {
        self.current_annotation = Some(Annotation {
            tool: self.current_tool,
            points: vec![pos],
            color: self.annotation_color,
            stroke_width: self.brush_size,
            text: String::new(),
            number: self.number_input,
        });
    }
}

#[cfg(test)]
mod tests {
    fn app_with_selection() -> ScreenshotApp {
        let mut app = ScreenshotApp::default();
        let geometry = crate::display::DisplayGeometry::new(
            0,
            Rect::from_min_size(Pos2::new(-100.0, -100.0), Vec2::new(300.0, 300.0)),
            (300, 300),
        )
        .unwrap();
        let display = crate::display::CapturedDisplay::from_image(
            geometry,
            image::RgbaImage::new(300, 300),
            2048,
        )
        .unwrap();
        app.install_capture_session(crate::display::CaptureSession::new(vec![display]).unwrap());
        app.selection_rect = Some(Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)));
        app.show_toolbar = true;
        app.current_tool = Tool::MoveBox;
        app
    }

    #[test]
    fn resize_release_uses_final_pointer_and_undo_restores_original() {
        let mut app = app_with_selection();
        let ctx = egui::Context::default();
        let original = app.selection_rect;
        app.update_global_pointer_interaction(
            PointerSnapshot {
                global_position: Pos2::new(100.0, 100.0),
                primary_down: true,
            },
            PrimaryButtonTransition::Pressed,
            &ctx,
        );
        app.update_global_pointer_interaction(
            PointerSnapshot {
                global_position: Pos2::new(120.0, 130.0),
                primary_down: false,
            },
            PrimaryButtonTransition::Released,
            &ctx,
        );
        assert_eq!(app.selection_rect.unwrap().max, Pos2::new(120.0, 130.0));
        app.undo_edit();
        assert_eq!(app.selection_rect, original);
    }

    #[test]
    fn keyboard_nudge_resize_and_undo_use_logical_coordinates() {
        let mut app = app_with_selection();
        let ctx = egui::Context::default();
        let modifiers = egui::Modifiers::ALT | egui::Modifiers::SHIFT;
        let _ = ctx.run(
            egui::RawInput {
                modifiers,
                events: vec![key_event(egui::Key::ArrowRight, modifiers)],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert_eq!(app.selection_rect.unwrap().max.x, 110.0);
        app.undo_edit();
        assert_eq!(app.selection_rect.unwrap().max.x, 100.0);
    }

    #[test]
    fn arrow_keys_do_not_commit_an_unfinished_annotation() {
        let mut app = app_with_selection();
        app.current_tool = Tool::Pen;
        app.begin_global_annotation(Pos2::new(20.0, 20.0));
        app.current_annotation
            .as_mut()
            .unwrap()
            .points
            .push(Pos2::new(30.0, 30.0));
        let selection = app.selection_rect;
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![key_event(egui::Key::ArrowRight, egui::Modifiers::NONE)],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert_eq!(app.selection_rect, selection);
        app.update_global_pointer_interaction(
            PointerSnapshot {
                global_position: Pos2::new(30.0, 30.0),
                primary_down: false,
            },
            PrimaryButtonTransition::Released,
            &ctx,
        );
        assert_eq!(app.annotations.len(), 1);
        app.undo_edit();
        assert!(app.annotations.is_empty());
        assert_eq!(app.selection_rect, selection);
    }

    #[test]
    fn nudge_uses_each_key_events_modifiers_even_if_released_before_frame() {
        let mut app = app_with_selection();
        let ctx = egui::Context::default();
        let modifiers = egui::Modifiers::ALT | egui::Modifiers::SHIFT;
        let _ = ctx.run(
            egui::RawInput {
                modifiers: egui::Modifiers::NONE,
                events: vec![
                    key_event(egui::Key::ArrowRight, modifiers),
                    key_event(egui::Key::ArrowRight, modifiers),
                ],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert_eq!(app.selection_rect.unwrap().min, Pos2::ZERO);
        assert_eq!(app.selection_rect.unwrap().max, Pos2::new(120.0, 100.0));
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
    fn keyboard_undo_redo_changes_capture_history() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.selection_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 30.0)));
        app.commit_edit();
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                modifiers: egui::Modifiers::COMMAND,
                events: vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert!(app.selection_rect.is_none());
        let redo = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let _ = ctx.run(
            egui::RawInput {
                modifiers: redo,
                events: vec![key_event(egui::Key::Z, redo)],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert_eq!(app.selection_rect.unwrap().size(), Vec2::new(40.0, 30.0));
    }

    #[test]
    fn text_editing_keeps_undo_for_the_text_editor() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.selection_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 30.0)));
        app.commit_edit();
        app.text_input = Some(crate::app_default::TextInputState::new(Pos2::ZERO));
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                modifiers: egui::Modifiers::COMMAND,
                events: vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert!(app.selection_rect.is_some());
        assert!(app.can_undo());
    }

    #[test]
    fn escape_selection_clears_annotations_and_history() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.selection_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 30.0)));
        app.commit_edit();
        app.show_toolbar = true;
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
                ..Default::default()
            },
            |ctx| app.handle_viewport_keyboard(ctx),
        );
        assert!(app.selection_rect.is_none());
        assert!(!app.can_undo());
    }

    use eframe::emath::{Pos2, Rect, Vec2};

    use crate::app_default::{PointerSnapshot, PrimaryButtonTransition, ScreenshotApp, Tool};

    use super::{clamp_translated_rect, global_rect_to_display_local};

    #[test]
    fn selection_is_clipped_and_translated_for_each_display() {
        let display = Rect::from_min_size(Pos2::new(-1920.0, 0.0), Vec2::new(1920.0, 1080.0));
        let selection = Rect::from_min_max(Pos2::new(-200.0, 100.0), Pos2::new(300.0, 400.0));

        let local = global_rect_to_display_local(display, selection).unwrap();

        assert_eq!(local.min, Pos2::new(1720.0, 100.0));
        assert_eq!(local.max, Pos2::new(1920.0, 400.0));
    }

    #[test]
    fn moving_selection_preserves_size_at_virtual_desktop_edge() {
        let desktop = Rect::from_min_max(Pos2::new(-1920.0, 0.0), Pos2::new(2560.0, 1440.0));
        let original = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(600.0, 300.0));

        let moved = clamp_translated_rect(original, Vec2::new(3000.0, 2000.0), desktop);

        assert_eq!(moved.size(), original.size());
        assert_eq!(moved.max, desktop.max);
    }

    #[test]
    fn toolbar_press_does_not_start_selection_interaction() {
        let mut app = ScreenshotApp {
            current_tool: Tool::MoveBox,
            selection_rect: Some(Rect::from_min_max(
                Pos2::new(0.0, 0.0),
                Pos2::new(500.0, 500.0),
            )),
            show_toolbar: true,
            toolbar_rect_global: Some(Rect::from_min_size(
                Pos2::new(100.0, 100.0),
                Vec2::new(300.0, 40.0),
            )),
            ..Default::default()
        };

        app.update_global_pointer_interaction(
            PointerSnapshot {
                global_position: Pos2::new(120.0, 120.0),
                primary_down: true,
            },
            PrimaryButtonTransition::Pressed,
            &egui::Context::default(),
        );

        assert!(!app.is_moving_box);
        assert!(!app.is_selecting);
        assert!(app.show_toolbar);
    }
}
