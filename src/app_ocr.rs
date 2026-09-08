use std::time::Instant;

use image::RgbaImage;

use crate::app_default::{AppView, CaptureSnapshot, ScreenshotApp};
use crate::ocr::{OcrRequest, OcrViewState};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OcrResultAction {
    Ignore,
    CloseViewport,
}

pub(crate) fn ocr_result_escape_action(state: &OcrViewState) -> OcrResultAction {
    if matches!(state, OcrViewState::Recognizing) {
        OcrResultAction::Ignore
    } else {
        OcrResultAction::CloseViewport
    }
}

impl ScreenshotApp {
    pub fn crop_selection_for_ocr(&self) -> Option<RgbaImage> {
        self.compose_current_selection(&[]).ok()
    }

    pub fn submit_ocr_for_current_selection(&mut self, now: Instant) -> Result<(), String> {
        let image = self
            .crop_selection_for_ocr()
            .ok_or_else(|| "请选择要识别的图片".to_string())?;
        self.submit_ocr_image(image, now)
    }

    fn submit_ocr_image(&mut self, image: RgbaImage, now: Instant) -> Result<(), String> {
        let worker = self
            .ocr_worker
            .as_ref()
            .ok_or_else(|| "OCR 服务未启动".to_string())?;
        let request_id = self.ocr_session.submit(now);

        if worker
            .request_tx
            .send(OcrRequest { request_id, image })
            .is_err()
        {
            self.ocr_session.apply_response(crate::ocr::OcrResponse {
                request_id,
                result: Err(crate::ocr::OcrErrorKind::Recognition),
            });
            self.app_view = AppView::OcrResult;
            return Err("OCR 服务不可用".to_string());
        }

        self.app_view = AppView::OcrResult;
        Ok(())
    }

    pub fn poll_ocr(&mut self, now: Instant) -> bool {
        let mut changed = false;
        if let Some(worker) = &self.ocr_worker {
            while let Ok(response) = worker.response_rx.try_recv() {
                changed |= self.ocr_session.apply_response(response);
            }
        }
        changed | self.ocr_session.expire_if_needed(now)
    }

    pub fn begin_ocr_recapture(&mut self) -> bool {
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            return false;
        }

        self.ocr_capture_snapshot = Some(CaptureSnapshot {
            selection_rect: self.selection_rect,
            annotations: self.annotations.clone(),
            edit_history: self.edit_history.clone(),
            number_input: self.number_input,
        });
        self.ocr_session.begin_capture();
        self.app_view = AppView::Capture;
        self.selection_rect = None;
        self.annotations.clear();
        self.edit_history = crate::app_history::EditHistory::default();
        self.number_input = None;
        self.current_tool = crate::app_default::Tool::Select;
        self.text_input = None;
        self.current_annotation = None;
        self.show_toolbar = false;
        true
    }

    pub fn cancel_ocr_recapture(&mut self) -> bool {
        if !matches!(self.ocr_session.state, OcrViewState::Capturing)
            || self.ocr_capture_snapshot.is_none()
        {
            return false;
        }

        if let Some(snapshot) = self.ocr_capture_snapshot.take() {
            self.selection_rect = snapshot.selection_rect;
            self.annotations = snapshot.annotations;
            self.edit_history = snapshot.edit_history;
            self.number_input = snapshot.number_input;
        }
        self.ocr_session.cancel_capture();
        self.app_view = AppView::OcrResult;
        true
    }

    pub fn finish_ocr_recapture(&mut self, now: Instant) -> Result<(), String> {
        if !matches!(self.ocr_session.state, OcrViewState::Capturing) {
            return Err("当前不在 OCR 重新截图状态".to_string());
        }

        let result = self.submit_ocr_for_current_selection(now);
        if result.is_ok() {
            self.ocr_capture_snapshot = None;
        } else {
            if let Some(snapshot) = self.ocr_capture_snapshot.take() {
                self.selection_rect = snapshot.selection_rect;
                self.annotations = snapshot.annotations;
                self.edit_history = snapshot.edit_history;
                self.number_input = snapshot.number_input;
            }
            self.ocr_session.cancel_capture();
            self.app_view = AppView::OcrResult;
        }
        result
    }

    pub fn copy_ocr_text(&self, ctx: &egui::Context) -> Result<(), String> {
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            return Err("OCR 识别中，请稍候".to_string());
        }
        if self.ocr_session.text.is_empty() {
            return Err("没有可复制的识别文本".to_string());
        }

        ctx.copy_text(self.ocr_session.text.clone());
        Ok(())
    }

    pub fn close_ocr_result(&mut self) -> bool {
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            return false;
        }

        self.ocr_session.cancel();
        self.ocr_capture_snapshot = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use image::RgbaImage;
    use std::sync::mpsc;

    use std::time::Instant;

    use crate::app_default::{AppView, CaptureSnapshot, ScreenshotApp};
    use crate::ocr::{OCR_TIMEOUT, OcrErrorKind, OcrResponse, OcrViewState, OcrWorker};

    use super::{OcrResultAction, ocr_result_escape_action};

    #[test]
    fn recognizing_defensively_rejects_recapture_and_close() {
        let now = Instant::now();
        let mut app = ScreenshotApp::default();
        let request_id = app.ocr_session.submit(now);
        app.app_view = AppView::OcrResult;

        assert!(!app.begin_ocr_recapture());
        assert!(!app.close_ocr_result());

        assert!(matches!(app.ocr_session.state, OcrViewState::Recognizing));
        assert_eq!(app.ocr_session.active_request_id, Some(request_id));
        assert_eq!(app.ocr_session.deadline, Some(now + OCR_TIMEOUT));
        assert_eq!(app.app_view, AppView::OcrResult);
        assert!(app.ocr_capture_snapshot.is_none());
    }

    #[test]
    fn close_result_cleans_state_without_returning_to_capture() {
        let mut app = ScreenshotApp::default();
        app.ocr_session.text = "识别文本".to_string();
        app.ocr_session.state = OcrViewState::Result;
        app.app_view = AppView::OcrResult;
        app.ocr_capture_snapshot = Some(CaptureSnapshot {
            edit_history: Default::default(),
            number_input: None,
            selection_rect: None,
            annotations: Vec::new(),
        });

        assert!(app.close_ocr_result());

        assert!(matches!(app.ocr_session.state, OcrViewState::Cancelled));
        assert_eq!(app.app_view, AppView::OcrResult);
        assert!(app.ocr_capture_snapshot.is_none());
    }

    #[test]
    fn ocr_result_escape_is_ignored_while_recognizing() {
        assert_eq!(
            ocr_result_escape_action(&OcrViewState::Recognizing),
            OcrResultAction::Ignore
        );
    }

    #[test]
    fn ocr_result_escape_closes_viewport_when_not_recognizing() {
        assert_eq!(
            ocr_result_escape_action(&OcrViewState::Result),
            OcrResultAction::CloseViewport
        );
        assert_eq!(
            ocr_result_escape_action(&OcrViewState::Failed(crate::ocr::OcrErrorKind::Recognition)),
            OcrResultAction::CloseViewport
        );
    }

    #[test]
    fn cancel_recapture_restores_snapshot_and_old_result() {
        let mut app = ScreenshotApp::default();
        app.ocr_session.text = "旧文本".to_string();
        app.ocr_session.state = OcrViewState::Capturing;
        app.app_view = AppView::Capture;
        app.ocr_capture_snapshot = Some(CaptureSnapshot {
            edit_history: Default::default(),
            number_input: None,
            selection_rect: Some(egui::Rect::from_min_max(
                egui::pos2(10.0, 20.0),
                egui::pos2(30.0, 40.0),
            )),
            annotations: Vec::new(),
        });

        assert!(app.cancel_ocr_recapture());

        assert!(matches!(app.ocr_session.state, OcrViewState::Result));
        assert_eq!(app.ocr_session.text, "旧文本");
        assert_eq!(app.app_view, AppView::OcrResult);
        assert!(app.selection_rect.is_some());
        assert!(app.ocr_capture_snapshot.is_none());
    }

    #[test]
    fn queued_current_response_wins_over_deadline_timeout() {
        let (_request_tx, request_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();
        let mut app = ScreenshotApp {
            ocr_worker: Some(OcrWorker {
                request_tx: _request_tx,
                response_rx,
            }),
            ..Default::default()
        };
        app.ocr_session.text = "旧文本".to_string();
        let now = Instant::now();
        let request_id = app.ocr_session.submit(now);
        response_tx
            .send(OcrResponse {
                request_id,
                result: Ok("边界响应".to_string()),
            })
            .unwrap();

        assert!(app.poll_ocr(now + OCR_TIMEOUT));

        assert!(matches!(app.ocr_session.state, OcrViewState::Result));
        assert_eq!(app.ocr_session.text, "边界响应");
        assert_eq!(app.ocr_session.active_request_id, None);
        drop(request_rx);
    }

    #[test]
    fn disconnected_request_channel_shows_recognition_failure_and_keeps_old_text() {
        let (request_tx, request_rx) = mpsc::channel();
        drop(request_rx);
        let (_response_tx, response_rx) = mpsc::channel();
        let mut app = ScreenshotApp {
            ocr_worker: Some(OcrWorker {
                request_tx,
                response_rx,
            }),
            ..Default::default()
        };
        app.ocr_session.text = "旧文本".to_string();

        let result = app.submit_ocr_image(RgbaImage::new(1, 1), Instant::now());

        assert!(result.is_err());
        assert!(matches!(
            app.ocr_session.state,
            OcrViewState::Failed(OcrErrorKind::Recognition)
        ));
        assert_eq!(app.ocr_session.text, "旧文本");
        assert_eq!(app.app_view, AppView::OcrResult);
        assert_eq!(app.ocr_session.active_request_id, None);
        assert_eq!(app.ocr_session.deadline, None);
    }

    #[test]
    fn failed_recapture_submission_restores_snapshot_and_old_result() {
        let mut app = ScreenshotApp::default();
        let old_selection =
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(30.0, 40.0));
        let old_annotation = crate::app_default::Annotation {
            tool: crate::app_default::Tool::Pen,
            points: vec![egui::pos2(12.0, 22.0), egui::pos2(18.0, 28.0)],
            color: egui::Color32::RED,
            stroke_width: 3.0,
            text: String::new(),
            number: None,
        };
        app.ocr_session.text = "旧文本".to_string();
        app.ocr_session.state = OcrViewState::Capturing;
        app.app_view = AppView::Capture;
        app.ocr_capture_snapshot = Some(CaptureSnapshot {
            edit_history: Default::default(),
            number_input: None,
            selection_rect: Some(old_selection),
            annotations: vec![old_annotation],
        });

        assert!(app.finish_ocr_recapture(Instant::now()).is_err());

        assert!(matches!(app.ocr_session.state, OcrViewState::Result));
        assert_eq!(app.ocr_session.text, "旧文本");
        assert_eq!(app.app_view, AppView::OcrResult);
        assert_eq!(app.selection_rect, Some(old_selection));
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].tool, crate::app_default::Tool::Pen);
        assert!(app.ocr_capture_snapshot.is_none());
    }
}
