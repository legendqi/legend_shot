use crate::app_default::ScreenshotApp;
use crate::app_ocr::{OcrResultAction, ocr_result_escape_action};
use crate::ocr::{OcrSession, OcrViewState};

pub struct OcrViewModel {
    text: String,
    recognizing: bool,
    error_message: Option<&'static str>,
    copy_enabled: bool,
    recapture_enabled: bool,
    close_enabled: bool,
}

impl OcrViewModel {
    pub fn from_session(session: &OcrSession) -> Self {
        let recognizing = matches!(session.state, OcrViewState::Recognizing);
        let error_message = match &session.state {
            OcrViewState::Failed(kind) => Some(kind.message()),
            _ => None,
        };

        Self {
            text: session.text.clone(),
            recognizing,
            error_message,
            copy_enabled: !recognizing && !session.text.is_empty(),
            recapture_enabled: !recognizing,
            close_enabled: !recognizing,
        }
    }
}

impl ScreenshotApp {
    pub(crate) fn draw_ocr_result(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut model = OcrViewModel::from_session(&self.ocr_session);

        ui.vertical_centered(|ui| {
            ui.heading("OCR 识别结果");
            if model.recognizing {
                ui.add(egui::Spinner::new());
                ui.label("正在识别…");
            }
            if let Some(message) = model.error_message {
                ui.colored_label(egui::Color32::RED, message);
            }
        });

        ui.add_space(12.0);
        let actions_height = ui.spacing().interact_size.y + ui.spacing().item_spacing.y + 12.0;
        egui::ScrollArea::vertical()
            .id_salt("ocr_result_scroll")
            .max_height((ui.available_height() - actions_height).max(0.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut model.text)
                        .id(egui::Id::new("ocr_result_text"))
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(model.copy_enabled, egui::Button::new("复制文本"))
                .clicked()
            {
                let _ = self.copy_ocr_text(ctx);
            }
            if ui
                .add_enabled(model.recapture_enabled, egui::Button::new("重新截图"))
                .clicked()
            {
                self.begin_ocr_recapture();
            }
            if ui
                .add_enabled(model.close_enabled, egui::Button::new("关闭"))
                .clicked()
            {
                if self.close_ocr_result() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
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

#[cfg(test)]
mod tests {
    use crate::ocr::{OcrErrorKind, OcrSession, OcrViewState};

    use super::OcrViewModel;

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
}
