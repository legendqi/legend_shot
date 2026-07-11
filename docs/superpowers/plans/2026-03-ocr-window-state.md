# OCR 结果窗口状态实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** OCR 结果窗口首次以 500×500 显示，支持系统拖动和缩放，并在再次打开时恢复上次的位置与大小。

**架构：** 在 `AppConfig` 中保存 OCR 窗口外部位置和内容区大小；应用切换到 OCR 结果视图时发送根 viewport 的窗口化、装饰、尺寸和位置命令。OCR 结果显示期间从 `ViewportInfo` 采集最新窗口状态并在变化时写入配置；无有效历史位置时居中显示。

**技术栈：** Rust 2024、eframe/egui 0.33、serde_json

---

### 任务 1：窗口状态模型与测试

**文件：**
- 修改：`src/app_default.rs`

- [ ] 添加 `OcrWindowState` 序列化模型，默认内容尺寸为 500×500，位置可选。
- [ ] 添加测试验证旧配置 JSON 可兼容反序列化，以及默认尺寸为 500×500。
- [ ] 运行单项测试并确认新增测试在实现前失败。
- [ ] 实现最少模型代码并确认单项测试通过。

### 任务 2：窗口模式切换和状态持久化

**文件：**
- 修改：`src/app.rs`
- 修改：`src/app_default.rs`

- [ ] 添加测试验证 OCR 视图使用可缩放、带装饰的 500×500 窗口参数，并验证状态更新去重。
- [ ] 运行单项测试确认失败。
- [ ] 在首次进入 OCR 结果视图时退出全屏、开启装饰和缩放，恢复保存尺寸与位置；无位置时居中。
- [ ] OCR 结果显示期间读取 viewport 的 `outer_rect` 和 `inner_rect`，变化后保存配置。
- [ ] 返回重新截图时恢复无装饰全屏截图窗口。
- [ ] 运行单项及全量测试。

### 任务 3：最终验证与提交

**文件：**
- 修改：`src/app.rs`
- 修改：`src/app_default.rs`
- 创建：`docs/superpowers/plans/2026-03-ocr-window-state.md`

- [ ] 运行 `cargo fmt --check`。
- [ ] 运行 `cargo test`。
- [ ] 运行 `cargo build`。
- [ ] 运行 `git diff --check` 并审阅差异。
- [ ] 仅提交本任务相关文件。
