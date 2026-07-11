use std::time::{Duration, Instant};

pub type OcrRequestId = u64;
pub const OCR_TIMEOUT: Duration = Duration::from_secs(15);

pub enum OcrErrorKind {
    Model,
    Timeout,
    Recognition,
}

impl OcrErrorKind {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Model => "OCR 模型下载或加载失败，请检查网络后重试",
            Self::Timeout => "识别超时，请重新截图后重试",
            Self::Recognition => "识别失败，请稍后重试",
        }
    }
}

pub enum OcrViewState {
    Capturing,
    Result,
    Recognizing,
    Failed(OcrErrorKind),
    Cancelled,
}

pub struct OcrResponse {
    pub request_id: OcrRequestId,
    pub result: Result<String, OcrErrorKind>,
}

pub struct OcrSession {
    pub state: OcrViewState,
    pub text: String,
    pub active_request_id: Option<OcrRequestId>,
    pub deadline: Option<Instant>,
    pub next_request_id: OcrRequestId,
}

impl OcrSession {
    pub fn new() -> Self {
        Self {
            state: OcrViewState::Capturing,
            text: String::new(),
            active_request_id: None,
            deadline: None,
            next_request_id: 0,
        }
    }

    pub fn begin_capture(&mut self) {
        self.invalidate_request();
        self.state = OcrViewState::Capturing;
    }

    pub fn cancel_capture(&mut self) {
        self.state = if self.text.is_empty() {
            OcrViewState::Cancelled
        } else {
            OcrViewState::Result
        };
    }

    pub fn submit(&mut self, now: Instant) -> OcrRequestId {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.state = OcrViewState::Recognizing;
        self.active_request_id = Some(request_id);
        self.deadline = Some(now + OCR_TIMEOUT);
        request_id
    }

    pub fn apply_response(&mut self, response: OcrResponse) -> bool {
        if self.active_request_id != Some(response.request_id) {
            return false;
        }

        self.invalidate_request();
        match response.result {
            Ok(text) => {
                self.text = text;
                self.state = OcrViewState::Result;
            }
            Err(kind) => self.state = OcrViewState::Failed(kind),
        }
        true
    }

    pub fn expire_if_needed(&mut self, now: Instant) -> bool {
        if !matches!(self.state, OcrViewState::Recognizing)
            || !self.deadline.is_some_and(|deadline| now >= deadline)
        {
            return false;
        }

        self.invalidate_request();
        self.state = OcrViewState::Failed(OcrErrorKind::Timeout);
        true
    }

    pub fn cancel(&mut self) {
        self.invalidate_request();
        self.state = OcrViewState::Cancelled;
    }

    fn invalidate_request(&mut self) {
        self.active_request_id = None;
        self.deadline = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{OCR_TIMEOUT, OcrErrorKind, OcrResponse, OcrSession, OcrViewState};

    #[test]
    fn error_messages_are_fixed() {
        assert_eq!(
            OcrErrorKind::Model.message(),
            "OCR 模型下载或加载失败，请检查网络后重试"
        );
        assert_eq!(
            OcrErrorKind::Timeout.message(),
            "识别超时，请重新截图后重试"
        );
        assert_eq!(OcrErrorKind::Recognition.message(), "识别失败，请稍后重试");
    }

    #[test]
    fn submit_enters_recognizing_and_sets_deadline() {
        let now = Instant::now();
        let mut session = OcrSession::new();

        let request_id = session.submit(now);

        assert!(matches!(session.state, OcrViewState::Recognizing));
        assert_eq!(session.active_request_id, Some(request_id));
        assert_eq!(session.deadline, Some(now + OCR_TIMEOUT));
    }

    #[test]
    fn current_success_replaces_text() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let request_id = session.submit(now);

        assert!(session.apply_response(OcrResponse {
            request_id,
            result: Ok("新文本".to_string()),
        }));
        assert!(matches!(session.state, OcrViewState::Result));
        assert_eq!(session.text, "新文本");
        assert_eq!(session.active_request_id, None);
        assert_eq!(session.deadline, None);
    }

    #[test]
    fn current_failure_keeps_old_text() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let request_id = session.submit(now);

        assert!(session.apply_response(OcrResponse {
            request_id,
            result: Err(OcrErrorKind::Recognition),
        }));
        assert!(matches!(
            session.state,
            OcrViewState::Failed(OcrErrorKind::Recognition)
        ));
        assert_eq!(session.text, "旧文本");
        assert_eq!(session.active_request_id, None);
        assert_eq!(session.deadline, None);
    }

    #[test]
    fn request_expires_at_fifteen_seconds() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        session.submit(now);

        assert!(!session.expire_if_needed(now + OCR_TIMEOUT - Duration::from_nanos(1)));
        assert!(session.expire_if_needed(now + OCR_TIMEOUT));
        assert!(matches!(
            session.state,
            OcrViewState::Failed(OcrErrorKind::Timeout)
        ));
        assert_eq!(session.text, "旧文本");
        assert_eq!(session.active_request_id, None);
        assert_eq!(session.deadline, None);
    }

    #[test]
    fn late_response_is_discarded() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let request_id = session.submit(now);
        session.expire_if_needed(now + OCR_TIMEOUT);

        assert!(!session.apply_response(OcrResponse {
            request_id,
            result: Ok("迟到文本".to_string()),
        }));
        assert!(matches!(
            session.state,
            OcrViewState::Failed(OcrErrorKind::Timeout)
        ));
        assert_eq!(session.text, "旧文本");
    }

    #[test]
    fn only_latest_request_is_effective() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let old_request_id = session.submit(now);
        let latest_request_id = session.submit(now + Duration::from_secs(1));

        assert!(!session.apply_response(OcrResponse {
            request_id: old_request_id,
            result: Ok("旧请求文本".to_string()),
        }));
        assert!(matches!(session.state, OcrViewState::Recognizing));
        assert_eq!(session.text, "旧文本");
        assert_eq!(session.active_request_id, Some(latest_request_id));
        assert_eq!(
            session.deadline,
            Some(now + Duration::from_secs(1) + OCR_TIMEOUT)
        );

        assert!(session.apply_response(OcrResponse {
            request_id: latest_request_id,
            result: Ok("最新文本".to_string()),
        }));
        assert_eq!(session.text, "最新文本");
    }

    #[test]
    fn recapture_cancel_restores_result_and_keeps_old_text() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let request_id = session.submit(now);

        session.begin_capture();
        assert!(matches!(session.state, OcrViewState::Capturing));
        assert_eq!(session.active_request_id, None);
        assert_eq!(session.deadline, None);
        assert_eq!(session.text, "旧文本");

        session.cancel_capture();
        assert!(matches!(session.state, OcrViewState::Result));
        assert_eq!(session.text, "旧文本");
        assert!(!session.apply_response(OcrResponse {
            request_id,
            result: Ok("已失效文本".to_string()),
        }));
    }

    #[test]
    fn cancel_invalidates_request_and_keeps_text() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let request_id = session.submit(now);

        session.cancel();

        assert!(matches!(session.state, OcrViewState::Cancelled));
        assert_eq!(session.text, "旧文本");
        assert_eq!(session.active_request_id, None);
        assert_eq!(session.deadline, None);
        assert!(!session.apply_response(OcrResponse {
            request_id,
            result: Ok("已取消文本".to_string()),
        }));
    }

    fn session_with_old_text() -> OcrSession {
        let mut session = OcrSession::new();
        session.text = "旧文本".to_string();
        session.state = OcrViewState::Result;
        session
    }
}
