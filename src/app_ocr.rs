use std::time::Instant;

use image::{GenericImageView, RgbaImage};

use crate::app_default::{AppView, CaptureSnapshot, ScreenshotApp};
use crate::ocr::{OcrRequest, OcrViewState};
use crate::ui::get_screen_rect;

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

impl ScreenshotApp {
    pub fn crop_selection_for_ocr(&self) -> Option<RgbaImage> {
        let selection_rect = self.selection_rect?;
        let mouse_selection = self.mouse_selection_rect?;
        let global_x = mouse_selection.start.0.min(mouse_selection.end.0);
        let global_y = mouse_selection.start.1.min(mouse_selection.end.1);
        let width = (mouse_selection.end.0 - mouse_selection.start.0).unsigned_abs();
        let height = (mouse_selection.end.1 - mouse_selection.start.1).unsigned_abs();

        self.screens
            .iter()
            .zip(&self.original_screenshots)
            .find_map(|(screen, screenshot)| {
                let screen_rect = get_screen_rect(screen);
                if !screen_rect.contains(selection_rect.center()) {
                    return None;
                }

                let local_x = global_x.checked_sub(screen.x().ok()?)? as u32;
                let local_y = global_y.checked_sub(screen.y().ok()?)? as u32;
                crop_rgba_region(screenshot, local_x, local_y, width, height)
            })
    }

    pub fn submit_ocr_for_current_selection(&mut self, now: Instant) -> Result<(), String> {
        let image = self
            .crop_selection_for_ocr()
            .ok_or_else(|| "请选择要识别的图片".to_string())?;
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
            self.ocr_session.cancel();
            return Err("OCR 服务不可用".to_string());
        }

        self.app_view = AppView::OcrResult;
        Ok(())
    }

    pub fn poll_ocr(&mut self, now: Instant) -> bool {
        let mut changed = self.ocr_session.expire_if_needed(now);
        if let Some(worker) = &self.ocr_worker {
            while let Ok(response) = worker.response_rx.try_recv() {
                changed |= self.ocr_session.apply_response(response);
            }
        }
        changed
    }

    pub fn begin_ocr_recapture(&mut self) {
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
    }

    pub fn cancel_ocr_recapture(&mut self) {
        if let Some(snapshot) = self.ocr_capture_snapshot.take() {
            self.selection_rect = snapshot.selection_rect;
            self.mouse_selection_rect = snapshot.mouse_selection_rect;
            self.annotations = snapshot.annotations;
        }
        self.ocr_session.cancel_capture();
        self.app_view = AppView::OcrResult;
    }

    pub fn finish_ocr_recapture(&mut self, now: Instant) -> Result<(), String> {
        self.submit_ocr_for_current_selection(now)
    }

    pub fn copy_ocr_text(&self) -> Result<(), String> {
        if matches!(self.ocr_session.state, OcrViewState::Recognizing) {
            return Err("OCR 识别中，请稍候".to_string());
        }
        if self.ocr_session.text.is_empty() {
            return Err("没有可复制的识别文本".to_string());
        }

        let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        clipboard
            .set_text(self.ocr_session.text.clone())
            .map_err(|error| error.to_string())
    }

    pub fn close_ocr_result(&mut self) {
        self.ocr_session.cancel();
        self.ocr_capture_snapshot = None;
        self.app_view = AppView::Capture;
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::crop_rgba_region;

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
}
