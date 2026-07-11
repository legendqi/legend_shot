# OCR 结果窗口美化实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在不改变 OCR 业务流程的前提下，将结果窗口改造成自动适配浅色/深色主题的现代三段式工具窗口。

**架构：** 继续由 `app_ocr_view.rs` 独立负责 OCR 结果展示，将状态文案和按钮可用性集中在纯数据 `OcrViewModel` 中。`ScreenshotApp` 仅新增短生命周期复制成功反馈状态；窗口尺寸约束继续由 `app.rs` 统一管理。

**技术栈：** Rust 2024、egui 0.33.2、eframe 0.33.2、Rust 内置测试框架。

---

## 文件结构

- 修改 `src/app_ocr_view.rs`：扩展展示模型，绘制标题区、内容卡片和操作区，并添加展示模型测试。
- 修改 `src/app_default.rs`：保存复制成功反馈截止时间并在默认构造时初始化。
- 修改 `src/app.rs`：定义并使用 OCR 结果窗口最小尺寸，添加尺寸测试。
- 保留 `src/app_ocr.rs`：复制、重新截图、关闭和 Escape 的业务行为不变。

### 任务 1：锁定展示模型行为

**文件：**
- 修改：`src/app_ocr_view.rs:5-31`
- 测试：`src/app_ocr_view.rs:100-140`

- [ ] **步骤 1：为成功、空结果和复制反馈编写失败测试**

在测试模块中增加断言：成功文本的副标题为 `识别完成 · 4 个字符`，空结果副标题为 `未识别到文字`；定义 `copy_button_label(now, copied_until)` 后，截止时间前返回 `已复制`，截止后返回 `复制文本`。

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test app_ocr_view::tests -- --nocapture`

预期：FAIL，原因是 `OcrViewModel` 尚无 `status_text`、`character_count`，且 `copy_button_label` 尚不存在。

- [ ] **步骤 3：实现最少展示模型代码**

为 `OcrViewModel` 增加：

```rust
status_text: String,
character_count: usize,
is_empty: bool,
```

`from_session` 使用 `session.text.chars().count()` 计算字符数，并按识别中、失败、空结果、成功状态生成明确副标题。增加：

```rust
fn copy_button_label(now: Instant, copied_until: Option<Instant>) -> &'static str {
    if copied_until.is_some_and(|deadline| now < deadline) {
        "已复制"
    } else {
        "复制文本"
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test app_ocr_view::tests -- --nocapture`

预期：该模块测试全部 PASS。

### 任务 2：锁定窗口可用尺寸

**文件：**
- 修改：`src/app.rs:8-9, 98-104, tests`

- [ ] **步骤 1：编写失败的最小尺寸测试**

增加常量测试：

```rust
#[test]
fn ocr_window_minimum_size_supports_modern_layout() {
    assert_eq!(OCR_WINDOW_MIN_SIZE, egui::vec2(440.0, 320.0));
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test ocr_window_minimum_size_supports_modern_layout -- --nocapture`

预期：FAIL，原因是 `OCR_WINDOW_MIN_SIZE` 尚不存在。

- [ ] **步骤 3：定义并应用最小尺寸**

在 `app.rs` 定义：

```rust
const OCR_WINDOW_MIN_SIZE: egui::Vec2 = egui::vec2(440.0, 320.0);
```

在 `configure_ocr_window` 中使用该常量约束恢复尺寸并发送 `ViewportCommand::MinInnerSize`；在 `remember_ocr_window` 中用同一常量过滤无效尺寸。

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test ocr_window_minimum_size_supports_modern_layout -- --nocapture`

预期：PASS。

### 任务 3：实现现代化三段式界面

**文件：**
- 修改：`src/app_ocr_view.rs:33-97`
- 修改：`src/app_default.rs:187-194, ScreenshotApp::default`

- [ ] **步骤 1：增加复制反馈状态**

在 `ScreenshotApp` 增加：

```rust
pub ocr_copied_until: Option<std::time::Instant>,
```

默认值为 `None`。点击复制且 `copy_ocr_text` 成功时，将其设为 `Some(Instant::now() + Duration::from_secs(2))`；反馈有效期间调用 `ctx.request_repaint_after`。

- [ ] **步骤 2：绘制标题区**

使用 20 px 内容外边距，显示加粗主标题和由 `OcrViewModel` 提供的状态副标题。识别中在副标题前显示小型 `Spinner`，失败副标题使用 `ui.visuals().error_fg_color`，普通副标题使用 `weak_text_color()`。

- [ ] **步骤 3：绘制自适应文本卡片**

使用 `egui::Frame` 创建 10 px 圆角、主题派生填充和细描边的内容卡片。卡片占据操作栏之外的剩余高度；识别中显示居中加载状态，空结果显示居中提示，有文本时在垂直滚动区中显示只读、可选中的多行 `TextEdit`。失败且有旧文本时，在文本上方显示分类错误信息。

- [ ] **步骤 4：绘制左右分组操作栏**

用左右布局将“重新截图”“关闭”放在左侧，将主按钮“复制文本/已复制”放在右侧。主按钮填充使用 `ui.visuals().selection.bg_fill`，文字使用 `ui.visuals().selection.stroke.color`，按钮圆角为 8 px。所有按钮继续使用模型中的 enabled 状态。

- [ ] **步骤 5：运行视图模型和业务测试**

运行：`cargo test app_ocr_view::tests app_ocr::tests -- --nocapture`

预期：相关测试全部 PASS。

### 任务 4：格式化、回归验证和提交

**文件：**
- 修改：上述所有文件

- [ ] **步骤 1：格式化并检查**

运行：`cargo fmt --all && cargo fmt --all -- --check`

预期：命令退出码为 0。

- [ ] **步骤 2：运行全量测试**

运行：`cargo test`

预期：所有测试 PASS，无失败。

- [ ] **步骤 3：运行构建**

运行：`cargo build`

预期：构建成功；允许存在项目原有的未使用代码或 Cargo 配置警告，不允许新增编译错误。

- [ ] **步骤 4：检查补丁质量**

运行：`git diff --check && git diff --stat && git status --short`

预期：`git diff --check` 无输出；仅目标源码和计划文档有预期变更，已有未跟踪脚本和截图不纳入提交。

- [ ] **步骤 5：提交实现**

```bash
git add src/app.rs src/app_default.rs src/app_ocr_view.rs docs/superpowers/plans/2026-07-11-ocr-result-window-beautification.md
git commit -m "feat: 美化 OCR 结果窗口"
```
