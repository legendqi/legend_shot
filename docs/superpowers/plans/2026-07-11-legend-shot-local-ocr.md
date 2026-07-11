# Legend Shot 本地 OCR 与结果视图实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法跟踪进度。

**目标：** 为 Legend Shot 从零增加本地 OCR：工具栏启动识别，后台复用 OCR 引擎，15 秒 UI 超时，以请求序号屏蔽迟到响应，并提供结果、复制文本、重新截图及失败恢复界面。

**架构：** `src/ocr.rs` 承载可测试的状态机、消息、后端 trait 和单 worker；`src/ocr_oar.rs` 隔离 `oar-ocr 0.7.0` 的真实 API；`ScreenshotApp` 只负责截图/结果视图切换、裁剪、轮询和 UI 事件。引擎在 worker 内惰性初始化一次并复用，UI 超时不强杀推理线程。

**技术栈：** Rust 2024、egui/eframe 0.33.2、`std::sync::mpsc`、`std::thread`、`std::time::Instant`、image 0.25、`oar-ocr = 0.7.0` + `auto-download`、PP-OCRv5 mobile。

---

## 文件结构

**创建：**
- `src/ocr.rs`：状态机、错误分类、请求/响应、worker、后端 trait 及单元测试。
- `src/ocr_oar.rs`：PP-OCRv5 模型常量、内存图像转换、真实后端及文本归一化。
- `src/app_ocr.rs`：应用 OCR 生命周期、重新截图、轮询、复制文本。
- `src/app_ocr_view.rs`：结果/加载/失败界面与纯展示模型测试。

**修改：**
- `Cargo.toml`、`Cargo.lock`：固定 OCR 依赖。
- `src/main.rs`：模块声明、正常 GUI 注入真实 worker。
- `src/app_default.rs`：应用视图、OCR session/worker、截图快照、`Tool::Ocr`。
- `src/app.rs`：顶层视图分支、响应和超时轮询、定时重绘。
- `src/app_handle.rs`：提取可测试的原始像素裁剪，OCR 不合成标注。
- `src/app_draw.rs`：重新截图确认/取消以及输入隔离。
- `src/app_toolbar.rs`：增加 OCR 操作按钮。
- `src/ui.rs`：OCR 文字按钮和结果视图辅助样式。

## 固定产品规则

```rust
pub enum OcrErrorKind {
    Model,
    Timeout,
    Recognition,
}
```

文案：
- `Model`：`OCR 模型下载或加载失败，请检查网络后重试`
- `Timeout`：`识别超时，请重新截图后重试`
- `Recognition`：`识别失败，请稍后重试`

核心不变量：
1. 超时固定 15 秒，从请求提交时计算。
2. 只有当前 `request_id` 的响应可更新状态。
3. 超时、关闭或新请求会使旧请求失效。
4. Recognizing 和 Failed 均不清空旧文本。
5. Recognizing 时复制、重新截图、关闭全部禁用。
6. 模型初始化失败后，下一请求允许重新初始化；初始化成功后永久复用当前 worker 内的引擎。
7. OCR 使用原始截图像素，不识别用户标注。

---

### 任务 1：锁定依赖并完成 oar-ocr API 编译探针

**文件：**
- 修改：`Cargo.toml`
- 修改：`Cargo.lock`
- 临时创建后删除：`src/bin/ocr_api_probe.rs`

- [ ] 运行基线：`cargo test && cargo build`；预期均成功。
- [ ] 在 `[dependencies]` 增加：

```toml
oar-ocr = { version = "=0.7.0", features = ["auto-download"] }
```

- [ ] 创建最小探针，验证 builder，不运行模型下载：

```rust
use oar_ocr::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ocr = OAROCRBuilder::new(
        "pp-ocrv5_mobile_det.onnx",
        "pp-ocrv5_mobile_rec.onnx",
        "ppocrv5_dict.txt",
    ).build()?;
    let _ = ocr;
    Ok(())
}
```

- [ ] 运行 `cargo check --bin ocr_api_probe`；预期 PASS。
- [ ] 扩展探针，依据 **0.7.0 rustdoc 和编译器反馈** 确认：内存图像输入类型、`predict` receiver、结果字段、`text_with_confidence()` 返回类型及引擎 `Send`。不得假造 API，不得升级到 latest 绕过差异。
- [ ] 删除探针，运行 `cargo check`；预期 PASS。
- [ ] 提交：`git commit -m "build: 引入 oar-ocr 0.7.0 自动下载支持"`。

### 任务 2：TDD 实现 OCR 状态机

**文件：**
- 创建：`src/ocr.rs`
- 修改：`src/main.rs`

- [ ] 先写错误文案测试并运行，预期因类型缺失 FAIL。
- [ ] 实现 `OcrErrorKind` 与 `message()`，测试 PASS。
- [ ] 先写以下状态测试：提交进入 Recognizing 并设置 `now + 15s`；当前成功替换文本；当前失败保留旧文本；15 秒超时；迟到响应丢弃；只有最新请求有效；重新截图取消保留旧文本。
- [ ] 实现最小接口：

```rust
pub type OcrRequestId = u64;
pub const OCR_TIMEOUT: Duration = Duration::from_secs(15);

pub enum OcrViewState { Capturing, Result, Recognizing, Failed(OcrErrorKind), Cancelled }

pub struct OcrSession {
    pub state: OcrViewState,
    pub text: String,
    pub active_request_id: Option<OcrRequestId>,
    pub deadline: Option<Instant>,
    pub next_request_id: OcrRequestId,
}

impl OcrSession {
    pub fn new() -> Self;
    pub fn begin_capture(&mut self);
    pub fn cancel_capture(&mut self);
    pub fn submit(&mut self, now: Instant) -> OcrRequestId;
    pub fn apply_response(&mut self, response: OcrResponse) -> bool;
    pub fn expire_if_needed(&mut self, now: Instant) -> bool;
    pub fn cancel(&mut self);
}
```

- [ ] 运行 `cargo test ocr::tests`；预期全部 PASS。
- [ ] 提交：`git commit -m "feat: 实现 OCR 请求状态机与超时规则"`。

### 任务 3：TDD 实现单 worker 与引擎复用

**文件：**
- 修改：`src/ocr.rs`

- [ ] 使用计数 fake factory/backend 写失败测试：两个请求只初始化一次、识别两次。
- [ ] 实现：

```rust
pub struct OcrRequest { pub request_id: OcrRequestId, pub image: image::RgbaImage }
pub struct OcrResponse { pub request_id: OcrRequestId, pub result: Result<String, OcrErrorKind> }

pub trait OcrBackend: Send + 'static {
    fn recognize(&mut self, image: image::RgbaImage) -> Result<String, OcrErrorKind>;
}

pub trait OcrBackendFactory: Send + 'static {
    fn create(&mut self) -> Result<Box<dyn OcrBackend>, OcrErrorKind>;
}

pub struct OcrWorker {
    pub request_tx: mpsc::Sender<OcrRequest>,
    pub response_rx: mpsc::Receiver<OcrResponse>,
}
```

- [ ] worker 在线程内持有 `Option<Box<dyn OcrBackend>>`；成功创建后复用。
- [ ] 写测试：首次 Model 初始化失败，下一请求重新初始化成功。
- [ ] 写测试：已加载 backend 返回 Recognition 时不重建。
- [ ] 运行 `cargo test ocr::tests`；预期全部 PASS。
- [ ] 提交：`git commit -m "feat: 添加可复用 OCR 后台 worker"`。

### 任务 4：实现真实 oar-ocr 适配器

**文件：**
- 创建：`src/ocr_oar.rs`
- 修改：`src/main.rs`

- [ ] 先写纯函数测试：过滤空白区域并按检测顺序以换行拼接；全空输出映射 Recognition。
- [ ] 实现 `join_detected_text` / `normalize_detected_text`，测试 PASS。
- [ ] 实现 `OarOcrFactory` 和 `OarOcrBackend`，模型固定：

```text
pp-ocrv5_mobile_det.onnx
pp-ocrv5_mobile_rec.onnx
ppocrv5_dict.txt
```

- [ ] builder/download/load 错误映射 Model；predict、结果缺失和空文本映射 Recognition；底层错误只写日志。
- [ ] 图像转换与结果读取严格采用任务 1 编译探针确认的 0.7.0 API。
- [ ] 运行 `cargo check`、`cargo test ocr_oar::tests`、`cargo test ocr::tests`；预期 PASS 且测试不下载模型。
- [ ] 提交：`git commit -m "feat: 接入 PP-OCRv5 本地识别适配器"`。

### 任务 5：TDD 集成应用 OCR 生命周期和原始裁剪

**文件：**
- 创建：`src/app_ocr.rs`
- 修改：`src/main.rs`
- 修改：`src/app_default.rs`
- 修改：`src/app_handle.rs`

- [ ] 为纯 `crop_rgba_region` 写测试：正确裁剪像素、拒绝空区域。
- [ ] 实现纯裁剪函数及 `crop_selection_for_ocr()`；OCR 从 `original_screenshots` 取原始选区，不调用 `add_annotations_to_image`。
- [ ] 增加：

```rust
pub enum AppView { Capture, OcrResult }
pub struct CaptureSnapshot {
    pub selection_rect: Option<Rect>,
    pub mouse_selection_rect: Option<MouseSelectionRect>,
    pub annotations: Vec<Annotation>,
}
```

- [ ] `ScreenshotApp` 增加 `app_view`、`ocr_session`、`ocr_worker`、`ocr_capture_snapshot`；Default 不启动真实 worker。
- [ ] 实现 `submit_ocr_for_current_selection`、`poll_ocr`、`begin_ocr_recapture`、`cancel_ocr_recapture`、`finish_ocr_recapture`、`copy_ocr_text`、`close_ocr_result`。
- [ ] 正常 GUI 使用 `spawn_ocr_worker(OarOcrFactory)`；`--test` 模式不启动 OCR，避免下载模型。
- [ ] 运行 `cargo test && cargo check`；再运行现有截图测试模式，确认无 OCR 下载。
- [ ] 提交：`git commit -m "feat: 集成 OCR 请求与重新截图生命周期"`。

### 任务 6：增加工具栏 OCR 首次入口

**文件：**
- 修改：`src/app_default.rs`
- 修改：`src/app_draw.rs`
- 修改：`src/app_toolbar.rs`
- 修改：`src/ui.rs`

- [ ] 先写测试证明 `Tool::Ocr` 是 action 而非 annotation tool。
- [ ] 增加 `Tool::Ocr` 和 `Tool::is_annotation_tool()`；将宽泛工具判断改为该方法。
- [ ] 在 `ui.rs` 增加 42×30 的文字按钮 `OCR`，hover 文案“识别选区文字”，不新增来源不明图标。
- [ ] 工具栏点击 OCR 后立即裁剪并提交，成功切换结果视图；失败不关闭窗口。
- [ ] 调整工具栏宽度，确保 OCR/复制/保存/退出均可见。
- [ ] 运行 `cargo test && cargo check`；预期 PASS。
- [ ] 提交：`git commit -m "feat: 在截图工具栏添加 OCR 操作"`。

### 任务 7：实现结果视图、加载禁用和复制文本

**文件：**
- 创建：`src/app_ocr_view.rs`
- 修改：`src/main.rs`
- 修改：`src/app.rs`
- 修改：`src/app_ocr.rs`

- [ ] 先写纯 `OcrViewModel` 测试：Recognizing 保留文本但禁用操作；Failed 显示旧文本和分类文案。
- [ ] 实现只读、可滚动多行结果视图：标题、spinner、错误提示、复制文本、重新截图、关闭。
- [ ] Recognizing 时三个按钮全部通过 `ui.add_enabled(false, ...)` 禁用；空文本时复制禁用。
- [ ] 使用 egui 0.33.2 经 `cargo check` 验证的复制文本 API。
- [ ] `app.rs` 每帧先 `poll_ocr(Instant::now())`；Recognizing 时每 100ms `request_repaint_after`；按 `AppView` 隔离截图视图和结果视图输入。
- [ ] 运行 `cargo test && cargo check`；预期 PASS。
- [ ] 提交：`git commit -m "feat: 添加 OCR 结果视图与文本复制"`。

### 任务 8：完成重新截图、取消和迟到响应保护

**文件：**
- 修改：`src/app_draw.rs`
- 修改：`src/app_ocr.rs`
- 修改：`src/ocr.rs`

- [ ] 框选释放时，若处于 Capturing，则直接提交新 OCR，不显示标注工具栏。
- [ ] Capturing 下 Escape 优先调用 `cancel_ocr_recapture()`，恢复快照与旧结果，不提交请求、不关闭应用。
- [ ] `begin_ocr_recapture`、复制、关闭均防御性拒绝 Recognizing。
- [ ] 新请求生成新 request_id；旧请求响应无法改变结果。
- [ ] 运行 `cargo test && cargo check`；预期 PASS。
- [ ] 提交：`git commit -m "feat: 完成 OCR 重新截图与迟到响应保护"`。

### 任务 9：真实 smoke test 与最终验证

**文件：**
- 可能修改：`src/ocr_oar.rs`、`src/app_ocr_view.rs`、`Cargo.lock`

- [ ] 用隔离缓存运行：`OAR_HOME=/tmp/legend-shot-oar-home cargo run`。
- [ ] 首次下载/初始化期间 UI 仍能重绘；若超过 15 秒，显示 Timeout，迟到响应不覆盖；下一请求可复用缓存/已初始化引擎。
- [ ] 验证 Model、Timeout、Recognition 三种文案和旧结果保留。
- [ ] 验证重新截图取消、重新截图成功、同进程连续识别复用模型。
- [ ] 运行：

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
cargo build --release
```

- [ ] 所有验证有实际输出证据后，提交：`git commit -m "feat: 完成本地 OCR 结果与重试流程"`。

---

## 验收标准

- 工具栏存在 OCR 入口；首次识别进入结果视图。
- OCR 全程不阻塞 egui UI 线程。
- 15 秒超时后恢复操作，迟到成功/失败均不改变界面。
- 失败和加载均保留旧文本；失败后可重新截图。
- 重新截图取消不提交请求并恢复旧结果。
- 复制的是识别文本，不是截图图片。
- 单 worker 内引擎成功初始化一次并复用。
- 单元测试不下载模型。
- `cargo test`、`cargo build` 通过；最终完成声明必须附验证证据。
