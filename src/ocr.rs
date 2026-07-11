use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use image::RgbaImage;

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

pub struct OcrRequest {
    pub request_id: OcrRequestId,
    pub image: RgbaImage,
}

pub struct OcrResponse {
    pub request_id: OcrRequestId,
    pub result: Result<String, OcrErrorKind>,
}

pub trait OcrBackend: Send + 'static {
    fn recognize(&mut self, image: RgbaImage) -> Result<String, OcrErrorKind>;
}

pub trait OcrBackendFactory: Send + 'static {
    fn create(&mut self) -> Result<Box<dyn OcrBackend>, OcrErrorKind>;
}

pub struct OcrWorker {
    pub request_tx: mpsc::Sender<OcrRequest>,
    pub response_rx: mpsc::Receiver<OcrResponse>,
}

pub fn spawn_ocr_worker<F>(mut factory: F) -> OcrWorker
where
    F: OcrBackendFactory,
{
    let (request_tx, request_rx) = mpsc::channel::<OcrRequest>();
    let (response_tx, response_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut backend: Option<Box<dyn OcrBackend>> = None;

        while let Ok(request) = request_rx.recv() {
            let result = match backend.as_mut() {
                Some(backend) => backend.recognize(request.image),
                None => match factory.create() {
                    Ok(mut created_backend) => {
                        let result = created_backend.recognize(request.image);
                        backend = Some(created_backend);
                        result
                    }
                    Err(kind) => Err(kind),
                },
            };

            if response_tx
                .send(OcrResponse {
                    request_id: request.request_id,
                    result,
                })
                .is_err()
            {
                break;
            }
        }
    });

    OcrWorker {
        request_tx,
        response_rx,
    }
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
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use image::RgbaImage;

    use super::{
        OCR_TIMEOUT, OcrBackend, OcrBackendFactory, OcrErrorKind, OcrRequest, OcrResponse,
        OcrSession, OcrViewState, spawn_ocr_worker,
    };

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
    fn recapture_submission_uses_new_id_and_discards_previous_response() {
        let now = Instant::now();
        let mut session = session_with_old_text();
        let old_request_id = session.submit(now);

        session.begin_capture();
        let new_request_id = session.submit(now + Duration::from_secs(1));

        assert_ne!(new_request_id, old_request_id);
        assert!(!session.apply_response(OcrResponse {
            request_id: old_request_id,
            result: Ok("迟到旧文本".to_string()),
        }));
        assert_eq!(session.text, "旧文本");
        assert!(matches!(session.state, OcrViewState::Recognizing));
        assert_eq!(session.active_request_id, Some(new_request_id));
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

    #[test]
    fn worker_reuses_backend_for_multiple_requests() {
        let create_count = Arc::new(Mutex::new(0));
        let recognize_count = Arc::new(Mutex::new(0));
        let worker = spawn_ocr_worker(FakeFactory::new(
            Arc::clone(&create_count),
            Arc::clone(&recognize_count),
            VecDeque::from([Ok(())]),
            VecDeque::from([Ok("第一次"), Ok("第二次")]),
        ));

        send_request(&worker, 10);
        send_request(&worker, 11);

        assert_response(&worker, 10, Ok("第一次"));
        assert_response(&worker, 11, Ok("第二次"));
        assert_eq!(*create_count.lock().unwrap(), 1);
        assert_eq!(*recognize_count.lock().unwrap(), 2);
    }

    #[test]
    fn worker_retries_initialization_after_model_failure() {
        let create_count = Arc::new(Mutex::new(0));
        let recognize_count = Arc::new(Mutex::new(0));
        let worker = spawn_ocr_worker(FakeFactory::new(
            Arc::clone(&create_count),
            Arc::clone(&recognize_count),
            VecDeque::from([Err(OcrErrorKind::Model), Ok(())]),
            VecDeque::from([Ok("重试成功")]),
        ));

        send_request(&worker, 20);
        send_request(&worker, 21);

        assert_response(&worker, 20, Err(OcrErrorKind::Model));
        assert_response(&worker, 21, Ok("重试成功"));
        assert_eq!(*create_count.lock().unwrap(), 2);
        assert_eq!(*recognize_count.lock().unwrap(), 1);
    }

    #[test]
    fn worker_keeps_loaded_backend_after_recognition_failure() {
        let create_count = Arc::new(Mutex::new(0));
        let recognize_count = Arc::new(Mutex::new(0));
        let worker = spawn_ocr_worker(FakeFactory::new(
            Arc::clone(&create_count),
            Arc::clone(&recognize_count),
            VecDeque::from([Ok(())]),
            VecDeque::from([Err(OcrErrorKind::Recognition), Ok("恢复成功")]),
        ));

        send_request(&worker, 30);
        send_request(&worker, 31);

        assert_response(&worker, 30, Err(OcrErrorKind::Recognition));
        assert_response(&worker, 31, Ok("恢复成功"));
        assert_eq!(*create_count.lock().unwrap(), 1);
        assert_eq!(*recognize_count.lock().unwrap(), 2);
    }

    struct FakeFactory {
        create_count: Arc<Mutex<usize>>,
        recognize_count: Arc<Mutex<usize>>,
        create_results: VecDeque<Result<(), OcrErrorKind>>,
        recognize_results: Arc<Mutex<VecDeque<Result<&'static str, OcrErrorKind>>>>,
    }

    impl FakeFactory {
        fn new(
            create_count: Arc<Mutex<usize>>,
            recognize_count: Arc<Mutex<usize>>,
            create_results: VecDeque<Result<(), OcrErrorKind>>,
            recognize_results: VecDeque<Result<&'static str, OcrErrorKind>>,
        ) -> Self {
            Self {
                create_count,
                recognize_count,
                create_results,
                recognize_results: Arc::new(Mutex::new(recognize_results)),
            }
        }
    }

    impl OcrBackendFactory for FakeFactory {
        fn create(&mut self) -> Result<Box<dyn OcrBackend>, OcrErrorKind> {
            *self.create_count.lock().unwrap() += 1;
            self.create_results.pop_front().unwrap().map(|()| {
                Box::new(FakeBackend {
                    recognize_count: Arc::clone(&self.recognize_count),
                    recognize_results: Arc::clone(&self.recognize_results),
                }) as Box<dyn OcrBackend>
            })
        }
    }

    struct FakeBackend {
        recognize_count: Arc<Mutex<usize>>,
        recognize_results: Arc<Mutex<VecDeque<Result<&'static str, OcrErrorKind>>>>,
    }

    impl OcrBackend for FakeBackend {
        fn recognize(&mut self, _image: RgbaImage) -> Result<String, OcrErrorKind> {
            *self.recognize_count.lock().unwrap() += 1;
            self.recognize_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap()
                .map(str::to_string)
        }
    }

    fn send_request(worker: &super::OcrWorker, request_id: super::OcrRequestId) {
        worker
            .request_tx
            .send(OcrRequest {
                request_id,
                image: RgbaImage::new(1, 1),
            })
            .unwrap();
    }

    fn assert_response(
        worker: &super::OcrWorker,
        request_id: super::OcrRequestId,
        expected: Result<&str, OcrErrorKind>,
    ) {
        let response = worker
            .response_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert_eq!(response.request_id, request_id);
        match (response.result, expected) {
            (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
            (Err(actual), Err(expected)) => {
                assert!(std::mem::discriminant(&actual) == std::mem::discriminant(&expected));
            }
            _ => panic!("response result did not match expectation"),
        }
    }

    fn session_with_old_text() -> OcrSession {
        let mut session = OcrSession::new();
        session.text = "旧文本".to_string();
        session.state = OcrViewState::Result;
        session
    }
}
