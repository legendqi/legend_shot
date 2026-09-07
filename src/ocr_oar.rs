use image::RgbaImage;
use oar_ocr::oarocr::{OAROCR, OAROCRBuilder, OAROCRResult};

use crate::ocr::{OcrBackend, OcrBackendFactory, OcrErrorKind};

const DETECTION_MODEL: &str = "pp-ocrv5_mobile_det.onnx";
const RECOGNITION_MODEL: &str = "pp-ocrv5_mobile_rec.onnx";
const CHARACTER_DICT: &str = "ppocrv5_dict.txt";

#[derive(Default)]
pub struct OarOcrFactory;

impl OarOcrFactory {
    pub fn new() -> Self {
        Self
    }
}

impl OcrBackendFactory for OarOcrFactory {
    fn create(&mut self) -> Result<Box<dyn OcrBackend>, OcrErrorKind> {
        let ocr = OAROCRBuilder::new(DETECTION_MODEL, RECOGNITION_MODEL, CHARACTER_DICT)
            .build()
            .map_err(|error| {
                eprintln!("OCR 模型下载或加载失败: {error}");
                OcrErrorKind::Model
            })?;

        Ok(Box::new(OarOcrBackend { ocr }))
    }
}

pub struct OarOcrBackend {
    ocr: OAROCR,
}

impl OcrBackend for OarOcrBackend {
    fn recognize(&mut self, image: RgbaImage) -> Result<String, OcrErrorKind> {
        let image = image::DynamicImage::ImageRgba8(image).into_rgb8();
        let results = self.ocr.predict(vec![image]).map_err(|error| {
            eprintln!("OCR 识别失败: {error}");
            OcrErrorKind::Recognition
        })?;

        let result = results.first().ok_or_else(|| {
            eprintln!("OCR 识别失败: oar-ocr 未返回图片结果");
            OcrErrorKind::Recognition
        })?;

        detected_text(result).ok_or_else(|| {
            eprintln!("OCR 识别失败: oar-ocr 返回的文本为空");
            OcrErrorKind::Recognition
        })
    }
}

fn detected_text(result: &OAROCRResult) -> Option<String> {
    join_detected_text(
        result
            .text_regions
            .iter()
            .filter_map(|region| region.text.as_deref()),
    )
}

fn normalize_detected_text(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn join_detected_text<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let text = texts
        .into_iter()
        .filter_map(normalize_detected_text)
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::{join_detected_text, normalize_detected_text};

    #[test]
    fn normalize_detected_text_trims_surrounding_whitespace() {
        assert_eq!(
            normalize_detected_text("  召唤师峡谷  \n"),
            Some("召唤师峡谷".to_string())
        );
    }

    #[test]
    fn normalize_detected_text_discards_whitespace_only_text() {
        assert_eq!(normalize_detected_text(" \t\r\n "), None);
    }

    #[test]
    fn join_detected_text_joins_non_empty_regions_with_newlines() {
        let regions = ["  第一行  ", "\t", "第二行\n"];

        assert_eq!(
            join_detected_text(regions),
            Some("第一行\n第二行".to_string())
        );
    }

    #[test]
    fn join_detected_text_returns_none_when_all_regions_are_empty() {
        assert_eq!(join_detected_text(["", "  ", "\r\n"]), None);
    }
}
