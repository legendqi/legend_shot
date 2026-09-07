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
                self.draw_text_input_for_display(display_index, ui);
                self.draw_toolbar_for_display(display_index, ctx);
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
        match transition {
            PrimaryButtonTransition::Pressed => {
                if self.show_toolbar
                    && self
                        .toolbar_rect_global
                        .is_some_and(|rect| rect.contains(position))
                {
                    return;
                }
                let current_time = ctx.input(|input| input.time);
                if self
                    .selection_rect
                    .is_some_and(|rect| rect.contains(position))
                    && !self.tool_bar_focused
                    && current_time - self.last_click_time < 0.5
                    && (position - self.last_click_pos).length() < 10.0
                {
                    if self.copy_to_clipboard().is_ok() {
                        self.hide_capture_window(ctx);
                    }
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
                    self.is_moving_box = true;
                    self.show_toolbar = false;
                    self.move_start = position;
                    self.original_selection_rect = self.selection_rect;
                } else if self.current_tool.is_annotation_tool() {
                    self.begin_global_annotation(position);
                }
            }
            PrimaryButtonTransition::Held => {
                if self.is_selecting {
                    self.update_global_selection(position);
                } else if self.is_moving_box {
                    if let (Some(original), Some(desktop)) = (
                        self.original_selection_rect,
                        self.capture_session
                            .as_ref()
                            .map(|session| session.desktop_bounds),
                    ) {
                        let moved =
                            clamp_translated_rect(original, position - self.move_start, desktop);
                        self.selection_rect = Some(moved);
                        self.selection_start = moved.min;
                        self.selection_end = moved.max;
                    }
                } else if let Some(annotation) = &mut self.current_annotation
                    && self
                        .selection_rect
                        .is_some_and(|rect| rect.contains(position))
                {
                    annotation.points.push(position);
                }
            }
            PrimaryButtonTransition::Released => {
                if self.is_selecting {
                    self.update_global_selection(position);
                    self.finish_global_selection();
                    if let Some(rect) = self.selection_rect
                        && rect.area() > 100.0
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
                } else if self.current_tool != Tool::Text {
                    let should_commit =
                        self.current_annotation.as_ref().is_some_and(|annotation| {
                            annotation.points.len()
                                >= if annotation.tool == Tool::Mosaic {
                                    1
                                } else {
                                    2
                                }
                        });
                    if should_commit {
                        if let Some(annotation) = self.current_annotation.take() {
                            self.annotations.push(annotation);
                        }
                    } else {
                        self.current_annotation = None;
                    }
                }
            }
            PrimaryButtonTransition::Idle => {}
        }
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
            self.text_input = Some(TextInputState::new(position));
            self.text_input_finalized = false;
        } else if self.current_tool == Tool::Number {
            self.number_input = Some(self.number_input.unwrap_or(0) + 1);
        }
        self.start_annotation(position);
    }

    fn handle_viewport_keyboard(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| {
            input.key_pressed(egui::Key::Enter) && (input.modifiers.ctrl || input.modifiers.command)
        }) {
            self.finalize_text_input();
        }
        if !ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            return;
        }
        if matches!(self.ocr_session.state, OcrViewState::Capturing)
            && self.ocr_capture_snapshot.is_some()
        {
            self.cancel_ocr_recapture();
        } else if self.text_input.is_some() {
            self.cancel_text_input();
        } else if self.show_toolbar {
            self.show_toolbar = false;
            self.selection_rect = None;
            self.current_tool = Tool::Select;
        } else {
            self.hide_capture_window(ctx);
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
            }
        }
        self.text_input_finalized = false;
    }

    fn cancel_text_input(&mut self) {
        self.text_input = None;
        self.current_annotation = None;
        self.text_input_finalized = false;
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
