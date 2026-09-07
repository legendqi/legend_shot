use std::time::{Duration, Instant};

use crate::app_default::ScreenshotApp;
use crate::app_ocr::{OcrResultAction, ocr_result_escape_action};
use crate::ocr::{OcrSession, OcrViewState};

const COPY_FEEDBACK_DURATION: Duration = Duration::from_secs(2);

pub struct OcrViewModel {
    text: String,
    recognizing: bool,
    failed: bool,
    error_message: Option<&'static str>,
    status_text: String,
    is_empty: bool,
    copy_enabled: bool,
    recapture_enabled: bool,
    close_enabled: bool,
}

impl OcrViewModel {
    pub fn from_session(session: &OcrSession) -> Self {
        let recognizing = matches!(session.state, OcrViewState::Recognizing);
        let failed = matches!(session.state, OcrViewState::Failed(_));
        let error_message = match &session.state {
            OcrViewState::Failed(kind) => Some(kind.message()),
            _ => None,
        };
        let character_count = session.text.chars().count();
        let is_empty = session.text.is_empty();
        let status_text = if recognizing {
            "正在识别图片中的文字…".to_string()
        } else if failed {
            "识别失败，可重新截图后再试".to_string()
        } else if is_empty {
            "未识别到文字".to_string()
        } else {
            format!("识别完成 · {character_count} 个字符")
        };

        Self {
            text: session.text.clone(),
            recognizing,
            failed,
            error_message,
            status_text,
            is_empty,
            copy_enabled: !recognizing && !session.text.is_empty(),
            recapture_enabled: !recognizing,
            close_enabled: !recognizing,
        }
    }
}

fn copy_button_label(now: Instant, copied_until: Option<Instant>) -> &'static str {
    if copied_until.is_some_and(|deadline| now < deadline) {
        "已复制"
    } else {
        "复制文本"
    }
}

impl ScreenshotApp {
    pub(crate) fn draw_ocr_result(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut model = OcrViewModel::from_session(&self.ocr_session);
        let now = Instant::now();
        if let Some(deadline) = self.ocr_copied_until.filter(|deadline| now < *deadline) {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        } else {
            self.ocr_copied_until = None;
        }

        egui::Frame::NONE.inner_margin(20.0).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            draw_header(ui, &model);
            ui.add_space(16.0);

            let actions_height = 36.0;
            let card_height =
                (ui.available_height() - actions_height - 16.0 - ui.spacing().item_spacing.y)
                    .max(120.0);
            draw_result_card(ui, &mut model, card_height);
            ui.add_space(16.0);

            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), actions_height),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let label = copy_button_label(now, self.ocr_copied_until);
                    let selection = ui.visuals().selection;
                    let copy_button =
                        egui::Button::new(egui::RichText::new(label).color(selection.stroke.color))
                            .fill(selection.bg_fill)
                            .corner_radius(8.0)
                            .min_size(egui::vec2(104.0, 34.0));
                    if ui.add_enabled(model.copy_enabled, copy_button).clicked()
                        && self.copy_ocr_text(ctx).is_ok()
                    {
                        self.ocr_copied_until = Some(now + COPY_FEEDBACK_DURATION);
                        ctx.request_repaint();
                    }

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let secondary = |text| {
                            egui::Button::new(text)
                                .corner_radius(8.0)
                                .min_size(egui::vec2(88.0, 34.0))
                        };
                        if ui
                            .add_enabled(model.recapture_enabled, secondary("重新截图"))
                            .clicked()
                        {
                            self.ocr_copied_until = None;
                            self.begin_ocr_recapture();
                        }
                        if ui
                            .add_enabled(model.close_enabled, secondary("关闭"))
                            .clicked()
                            && self.close_ocr_result()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                },
            );
        });

        if ui.input(|input| input.key_pressed(egui::Key::Escape))
            && matches!(
                ocr_result_escape_action(&self.ocr_session.state),
                OcrResultAction::CloseViewport
            )
            && self.close_ocr_result()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn draw_header(ui: &mut egui::Ui, model: &OcrViewModel) {
    ui.label(egui::RichText::new("OCR 识别结果").heading().strong());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if model.recognizing {
            ui.add(egui::Spinner::new().size(14.0));
        }
        let color = if model.failed {
            ui.visuals().error_fg_color
        } else {
            ui.visuals().weak_text_color()
        };
        ui.label(
            egui::RichText::new(&model.status_text)
                .color(color)
                .size(13.0),
        );
    });
}

fn draw_result_card(ui: &mut egui::Ui, model: &mut OcrViewModel, height: f32) {
    let visuals = ui.visuals();
    let fill = visuals.extreme_bg_color;
    let stroke = visuals.widgets.noninteractive.bg_stroke;

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(10.0)
                .inner_margin(16.0)
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());

                    if model.recognizing {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(16.0));
                            ui.weak("正在识别，请稍候");
                        });
                        if !model.is_empty {
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(8.0);
                        }
                    }

                    if let Some(message) = model.error_message {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                        if !model.is_empty {
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(8.0);
                        }
                    }

                    if model.is_empty {
                        ui.centered_and_justified(|ui| {
                            ui.weak(if model.recognizing {
                                "识别结果将在完成后显示"
                            } else {
                                "识别结果将在这里显示"
                            });
                        });
                        return;
                    }

                    let mut text = model.text.as_str();

                    egui::ScrollArea::vertical()
                        .id_salt("ocr_result_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_sized(
                                ui.available_size(),
                                egui::TextEdit::multiline(&mut text)
                                    .id(egui::Id::new("ocr_result_text"))
                                    .desired_width(f32::INFINITY)
                                    .frame(false),
                            );
                        });
                });
        },
    );
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::ocr::{OcrErrorKind, OcrSession, OcrViewState};

    use super::{OcrViewModel, copy_button_label, draw_result_card};

    #[test]
    fn long_result_card_does_not_consume_reserved_action_space() {
        let context = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(500.0, 500.0),
        ));
        let mut used_height = 0.0;

        let _ = context.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut session = OcrSession::new();
                session.state = OcrViewState::Result;
                session.text = "很长的识别结果\n".repeat(200);
                let mut model = OcrViewModel::from_session(&session);
                let start_y = ui.next_widget_position().y;

                draw_result_card(ui, &mut model, 200.0);

                used_height = ui.next_widget_position().y - start_y;
            });
        });

        assert!(
            used_height <= 204.0,
            "result card used {used_height} points instead of the requested 200"
        );
    }

    #[test]
    fn recognizing_keeps_text_and_disables_all_actions() {
        let mut session = OcrSession::new();
        session.text = "旧文本".to_string();
        session.state = OcrViewState::Recognizing;

        let model = OcrViewModel::from_session(&session);

        assert_eq!(model.text, "旧文本");
        assert!(model.recognizing);
        assert!(!model.copy_enabled);
        assert!(!model.recapture_enabled);
        assert!(!model.close_enabled);
        assert_eq!(model.error_message, None);
    }

    #[test]
    fn failed_keeps_old_text_and_shows_classified_message() {
        let mut session = OcrSession::new();
        session.text = "旧文本".to_string();
        session.state = OcrViewState::Failed(OcrErrorKind::Model);

        let model = OcrViewModel::from_session(&session);

        assert_eq!(model.text, "旧文本");
        assert!(!model.recognizing);
        assert!(model.copy_enabled);
        assert!(model.recapture_enabled);
        assert!(model.close_enabled);
        assert_eq!(
            model.error_message,
            Some("OCR 模型下载或加载失败，请检查网络后重试")
        );
    }

    #[test]
    fn result_reports_character_count_and_status() {
        let mut session = OcrSession::new();
        session.text = "识别文本".to_string();
        session.state = OcrViewState::Result;

        let model = OcrViewModel::from_session(&session);

        assert_eq!(model.status_text, "识别完成 · 4 个字符");
        assert!(!model.is_empty);
    }

    #[test]
    fn empty_result_reports_empty_status() {
        let mut session = OcrSession::new();
        session.state = OcrViewState::Result;

        let model = OcrViewModel::from_session(&session);

        assert_eq!(model.status_text, "未识别到文字");
        assert!(model.is_empty);
    }

    #[test]
    fn copy_button_feedback_expires_at_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(2);

        assert_eq!(copy_button_label(now, Some(deadline)), "已复制");
        assert_eq!(copy_button_label(deadline, Some(deadline)), "复制文本");
        assert_eq!(copy_button_label(now, None), "复制文本");
    }
}
