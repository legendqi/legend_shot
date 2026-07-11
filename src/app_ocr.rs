use std::time::Instant;

use image::{GenericImageView, RgbaImage};

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

pub fn crop_rgba_region(
    source: &RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    if width == 0 || height == 0 {
        return None;
    }

    let end_x = x.checked_add(width)?;
    let end_y = y.checked_add(height)?;
    if end_x > source.width() || end_y > source.height() {
        return None;
    }

    Some(source.view(x, y, width, height).to_image())
}

pub fn crop_region_for_global_selection(
    monitor: (i32, i32, u32, u32),
    selection_start: (i32, i32),
    selection_end: (i32, i32),
) -> Option<(u32, u32, u32, u32)> {
    let global_x = selection_start.0.min(selection_end.0);
    let global_y = selection_start.1.min(selection_end.1);
    let width = (selection_end.0 - selection_start.0).unsigned_abs();
    let height = (selection_end.1 - selection_start.1).unsigned_abs();
    if width == 0 || height == 0 {
        return None;
    }

    let local_x = global_x.checked_sub(monitor.0)?;
    let local_y = global_y.checked_sub(monitor.1)?;
    let local_x = u32::try_from(local_x).ok()?;
    let local_y = u32::try_from(local_y).ok()?;
    let end_x = local_x.checked_add(width)?;
    let end_y = local_y.checked_add(height)?;
    if end_x > monitor.2 || end_y > monitor.3 {
        return None;
    }

    Some((local_x, local_y, width, height))
}

impl ScreenshotApp {
    pub fn crop_selection_for_ocr(&self) -> Option<RgbaImage> {
        let mouse_selection = self.mouse_selection_rect?;

        self.screens
            .iter()
            .zip(&self.original_screenshots)
            .find_map(|(screen, screenshot)| {
                let crop = crop_region_for_global_selection(
                    (
                        screen.x().ok()?,
                        screen.y().ok()?,
                        screen.width().ok()?,
                        screen.height().ok()?,
                    ),
                    mouse_selection.start,
                    mouse_selection.end,
                )?;
                crop_rgba_region(screenshot, crop.0, crop.1, crop.2, crop.3)
            })
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
            mouse_selection_rect: self.mouse_selection_rect,
            annotations: self.annotations.clone(),
        });
        self.ocr_session.begin_capture();
        self.app_view = AppView::Capture;
        self.selection_rect = None;
        self.mouse_selection_rect = None;
        self.annotations.clear();
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
            self.mouse_selection_rect = snapshot.mouse_selection_rect;
            self.annotations = snapshot.annotations;
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
                self.mouse_selection_rect = snapshot.mouse_selection_rect;
                self.annotations = snapshot.annotations;
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
    use image::{ImageBuffer, Rgba, RgbaImage};
    use std::sync::mpsc;

    use std::time::Instant;

    use crate::app_default::{AppView, CaptureSnapshot, ScreenshotApp};
    use crate::ocr::{
        OCR_TIMEOUT, OcrErrorKind, OcrResponse, OcrViewState, OcrWorker,
    };

    use super::{
        OcrResultAction, crop_region_for_global_selection, crop_rgba_region,
        ocr_result_escape_action,
    };

    #[test]
    fn global_selection_uses_negative_monitor_origin_for_local_crop() {
        let crop = crop_region_for_global_selection(
            (-1920, -200, 1920, 1080),
            (-1820, -150),
            (-1780, -120),
        )
        .expect("selection is inside the negative-origin monitor");

        assert_eq!(crop, (100, 50, 40, 30));
    }

    #[test]
    fn global_selection_ignores_hidpi_egui_coordinates_when_locating_monitor() {
        let monitors = [(0, 0, 2560, 1440), (2560, 0, 3840, 2160)];
        let selection = ((3000, 300), (3200, 500));

        let located = monitors.iter().enumerate().find_map(|(index, &monitor)| {
            crop_region_for_global_selection(monitor, selection.0, selection.1)
                .map(|crop| (index, crop))
        });

        assert_eq!(located, Some((1, (440, 300, 200, 200))));
    }

    #[test]
    fn crop_rgba_region_crops_expected_pixels() {
        let source = ImageBuffer::from_fn(3, 2, |x, y| {
            Rgba([(y * 3 + x) as u8, x as u8, y as u8, 255])
        });

        let cropped = crop_rgba_region(&source, 1, 0, 2, 2).expect("valid crop");

        assert_eq!(cropped.dimensions(), (2, 2));
        assert_eq!(cropped.get_pixel(0, 0), source.get_pixel(1, 0));
        assert_eq!(cropped.get_pixel(1, 0), source.get_pixel(2, 0));
        assert_eq!(cropped.get_pixel(0, 1), source.get_pixel(1, 1));
        assert_eq!(cropped.get_pixel(1, 1), source.get_pixel(2, 1));
    }

    #[test]
    fn crop_rgba_region_rejects_empty_region() {
        let source = ImageBuffer::from_pixel(2, 2, Rgba([1, 2, 3, 255]));

        assert!(crop_rgba_region(&source, 0, 0, 0, 1).is_none());
        assert!(crop_rgba_region(&source, 0, 0, 1, 0).is_none());
    }

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
            selection_rect: None,
            mouse_selection_rect: None,
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
            selection_rect: Some(egui::Rect::from_min_max(
                egui::pos2(10.0, 20.0),
                egui::pos2(30.0, 40.0),
            )),
            mouse_selection_rect: None,
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
        let mut app = ScreenshotApp::default();
        app.ocr_worker = Some(OcrWorker {
            request_tx: _request_tx,
            response_rx,
        });
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
        let mut app = ScreenshotApp::default();
        app.ocr_worker = Some(OcrWorker {
            request_tx,
            response_rx,
        });
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
        let old_selection = egui::Rect::from_min_max(
            egui::pos2(10.0, 20.0),
            egui::pos2(30.0, 40.0),
        );
        let old_mouse_selection = crate::app_default::MouseSelectionRect {
            start: (10, 20),
            end: (30, 40),
        };
        let old_annotation = crate::app_default::Annotation {
            tool: crate::app_default::Tool::Pen,
            points: vec![egui::pos2(12.0, 22.0), egui::pos2(18.0, 28.0)],
            mouse_points: vec![(12, 22), (18, 28)],
            color: egui::Color32::RED,
            stroke_width: 3.0,
            text: String::new(),
            number: None,
        };
        app.ocr_session.text = "旧文本".to_string();
        app.ocr_session.state = OcrViewState::Capturing;
        app.app_view = AppView::Capture;
        app.ocr_capture_snapshot = Some(CaptureSnapshot {
            selection_rect: Some(old_selection),
            mouse_selection_rect: Some(old_mouse_selection),
            annotations: vec![old_annotation],
        });

        assert!(app.finish_ocr_recapture(Instant::now()).is_err());

        assert!(matches!(app.ocr_session.state, OcrViewState::Result));
        assert_eq!(app.ocr_session.text, "旧文本");
        assert_eq!(app.app_view, AppView::OcrResult);
        assert_eq!(app.selection_rect, Some(old_selection));
        let restored_mouse_selection = app.mouse_selection_rect.expect("mouse selection restored");
        assert_eq!(restored_mouse_selection.start, old_mouse_selection.start);
        assert_eq!(restored_mouse_selection.end, old_mouse_selection.end);
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].tool, crate::app_default::Tool::Pen);
        assert!(app.ocr_capture_snapshot.is_none());
    }
}
