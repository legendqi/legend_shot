<h1 align="center">Legend Shot</h1>

<p align="center">
  <strong>使用 Rust 编写的轻量级跨平台截图与标注工具。</strong><br />
  框选屏幕、添加标注、使用本地 OCR 提取文字，然后复制或保存结果。
</p>

<p align="center">
  <a href="https://gitee.com/legendqi/legend_shot"><img alt="版本" src="https://img.shields.io/badge/version-0.1.0-orange.svg?style=flat-square" /></a>
  <img alt="支持平台" src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-4c8bf5.svg?style=flat-square" />
  <img alt="Rust 版本" src="https://img.shields.io/badge/Rust-edition%202024-dea584.svg?style=flat-square&logo=rust&logoColor=white" />
  <a href="LICENSE"><img alt="许可证" src="https://img.shields.io/badge/license-Mulan%20PSL%20v2-blue.svg?style=flat-square" /></a>
</p>

<p align="center">
  <a href="#快速开始">快速开始</a> ·
  <a href="#功能特性">功能特性</a> ·
  <a href="#使用方法">使用方法</a> ·
  <a href="#构建">构建</a> ·
  <a href="#项目结构">项目结构</a> ·
  <a href="README.md">English</a>
</p>

## Legend Shot 是什么？

Legend Shot 是一款基于 Rust、`egui` 和 `xcap` 开发的桌面截图工具。在 macOS、Windows 和 Linux X11 上，它启动后常驻托盘（macOS 为菜单栏），通过全局快捷键或菜单“截图”打开截图遮罩。遮罩允许用户框选区域，并通过紧凑的工具栏完成标注、OCR、复制和保存。

项目关注快速截图流程、原生分辨率输出、跨平台一致性和本地处理。OCR 所需模型准备完成后，识别过程在本地运行。

## 项目状态

Legend Shot 目前处于持续开发阶段，当前软件包版本为 `0.1.0`，尚未发布预构建安装包。现阶段请从源码构建并用于开发或体验；在首个稳定版本发布前，API、打包方式和平台集成仍可能调整。

<a id="功能特性"></a>

## 功能特性

| 能力 | 说明 |
| --- | --- |
| 区域截图 | 在当前桌面上显示半透明遮罩，自由框选矩形区域。 |
| 原生分辨率输出 | 保留截图源图分辨率，包括 macOS Retina 缩放环境。 |
| 标注工具栏 | 支持移动选区、画笔、矩形、箭头、文字、马赛克和序号标记。 |
| 多语言文字 | 通过操作系统输入法输入文字，并使用可用的系统字体渲染中日韩字符。 |
| 本地 OCR | 通过 `oar-ocr` 使用 PP-OCRv5 移动端检测与识别模型提取文字。 |
| 复制与保存 | 将带标注的图片复制到剪贴板，或通过原生文件对话框保存为 PNG。 |
| 跨平台界面 | 在 macOS、Windows 和基于 X11 的 Linux 桌面上提供一致的截图流程。 |
| 全局快捷键 | 默认 macOS `Command+Shift+A`，Windows/Linux `Ctrl+Shift+A`；托盘“快捷键设置”中可修改，注册冲突会显示错误并保留旧快捷键。 |
| 选区精调 | 八个边角手柄、尺寸提示、原始像素放大镜，支持方向键移动及调整大小。 |
| 撤销与重做 | 标注、选区移动和调整大小均可撤销/重做；最多保留 100 个编辑动作。 |
| 贴图置顶 | 将带标注的选区作为独立窗口置顶，支持多张贴图、拖动、缩放和不透明度调整。 |
| 配置持久化 | 记住全局快捷键、上次保存目录以及 OCR 结果窗口的位置和大小。 |

## 多显示器支持

Legend Shot 的多显示器路径面向 Windows、macOS 和 Linux X11 上的跨屏框选设计，包括负坐标布局和不同缩放倍率的混合 DPI 显示器。Linux Wayland 暂不支持，请使用 X11 会话。跨屏选区中没有显示器覆盖的桌面间隙会导出为透明像素。

在下列原生发布矩阵于三个平台全部完成前，多显示器功能仍标记为实验性。尤其是 Windows 混合 DPI 下的窗口与输入对齐，必须经过 Windows 原生验证后才能视为可发布状态。

发布前冒烟测试矩阵：

- 单屏、左右排列、上下排列以及负坐标显示器布局。
- 混合 DPI、跨桌面间隙框选以及更换主显示器。
- 画笔、矩形、箭头、文字、马赛克和序号跨屏标注。
- 保存、复制、OCR、双击复制和 `Esc` 取消。
- 在两次截图会话之间断开并重新连接外部显示器。

## 界面预览

首个安装包版本发布前将补充不含个人信息的产品截图。

<a id="快速开始"></a>

## 快速开始

### 环境要求

- 当前稳定版 Rust 工具链和 Cargo。
- Git，以及当前操作系统对应的原生 C/C++ 构建工具链。
- 首次使用 OCR 且本地尚无 PP-OCRv5 模型时，需要网络连接下载模型资源。

Linux 还需要 X11 桌面会话、GTK 3、`libxdo` 和 AppIndicator 实现。`xclip` 用于图片剪贴板，并建议安装 Noto CJK 字体：

```bash
sudo apt update
sudo apt install -y build-essential pkg-config xclip fonts-noto-cjk libgtk-3-dev libxdo-dev libayatana-appindicator3-dev xdg-desktop-portal-gtk
```

如果系统没有 `libayatana-appindicator3-dev`，可改用 `libappindicator3-dev`。桌面会话必须提供 AppIndicator/StatusNotifier 支持；GNOME 环境可能需要启用 AppIndicator 扩展。
系统原生保存对话框通过 XDG Desktop Portal 打开。如果桌面环境尚未自带 Portal 后端，请安装 `xdg-desktop-portal-gtk`，也可以使用 GNOME/KDE 提供的对应后端。

### 从源码运行

```bash
git clone https://gitee.com/legendqi/legend_shot.git
cd legend_shot
cargo run
```

在 macOS 上，首次选择“截图”时才会请求屏幕录制权限。请前往 **系统设置 → 隐私与安全性 → 屏幕与系统录音** 为 Legend Shot 授权；通过 `cargo run` 开发运行时，可能需要给终端应用授权。等待授权期间进程仍会驻留，授权后再次选择“截图”即可。

<a id="使用方法"></a>

## 使用方法

1. 启动 Legend Shot，使用全局快捷键，或从托盘菜单选择“截图”。菜单还提供“快捷键设置”和“退出”。
2. 在一块或多块屏幕上拖动鼠标，框选需要截图的区域。
3. 使用工具栏添加标注、执行 OCR、复制或保存截图。
4. 输入文字后，在 Windows/Linux 使用 `Ctrl+Enter` 完成，在 macOS 使用 `Command+Enter` 完成。
5. 三个平台在完成或取消截图后均返回托盘，通过托盘“退出”终止进程。复制或保存失败时保留选区，并在界面显示错误。

| 操作 | 功能 |
| --- | --- |
| 在遮罩上拖动 | 框选截图区域。 |
| 双击选区内部 | 在选择/移动模式下复制当前选区并返回托盘。 |
| `Esc` | 取消当前操作；关闭最外层截图遮罩时返回托盘。 |
| 移动 | 调整选区位置；选择/移动模式下拖动八个手柄调整大小。 |
| 方向键 / `Shift` + 方向键 | 移动选区 1 / 10 个逻辑像素。 |
| `Alt` + 方向键 / `Alt+Shift` + 方向键 | 调整右边或下边 1 / 10 个逻辑像素；macOS 的 `Alt` 即 `Option`。 |
| `Ctrl/Cmd+Z` | 撤销；工具栏也提供撤销按钮。 |
| `Ctrl/Cmd+Shift+Z` | 重做；Windows/Linux 也支持 `Ctrl+Y`。 |
| `Ctrl/Cmd+C` / `Ctrl/Cmd+S` | 复制 / 保存当前选区。 |
| 画笔 / 矩形 / 箭头 | 添加图形标注。 |
| 文字 | 使用系统输入法添加多语言文字。 |
| 马赛克 | 对敏感内容进行像素化处理。 |
| 序号 | 添加连续编号标记。 |
| OCR | 识别选区文字并打开结果窗口。 |
| 贴图 | 将带标注的选区置顶显示，并结束本次截图。 |
| 复制 / 保存 | 导出带标注的选区图片。 |

文字编辑期间，复制和撤销等快捷键由文字编辑器处理；选区方向键不生效。新编辑会清空重做记录。框选/调整时显示逻辑尺寸与实际输出像素尺寸，混合 DPI 屏幕延续现有最高倍率合成规则。

在全局快捷键设置中点击快捷键卡片，再直接按下新的组合键，例如 `Ctrl+Shift+A` 或 `Command+Shift+A`，然后保存。关闭设置窗口后可测试快捷键。截图和原生保存对话框期间不会积压新的截图请求。

贴图支持拖动移动、滚轮缩放、`Ctrl/Cmd` + 滚轮调整不透明度。右键打开独立操作面板，可恢复原始显示大小、复制、保存或关闭当前贴图；贴图获得焦点时也可使用 `Ctrl/Cmd+C`、`Ctrl/Cmd+S` 和 `Esc`。复制和保存始终使用带标注的原始分辨率图片，不受显示缩放和不透明度影响。

再次截图时，已有贴图会临时隐藏，完成、取消或截图失败后恢复。打开贴图保存对话框时也会临时隐藏贴图，避免遮挡对话框。可同时保留多张贴图，关闭其中一张不会影响其他贴图或托盘常驻。贴图仅在本次运行期间保留，退出应用后清除。

## 命令行测试模式

Legend Shot 提供非交互式截图模式，便于开发和冒烟测试：

`--test` 的生命周期保持不变，也不会创建托盘图标。

```bash
# 截取指定区域并复制到剪贴板
cargo run -- --test "100,100,400,300" --action copy

# 截取指定区域并保存到文件
cargo run -- --test "100,100,400,300" --action save --output screenshot.png
```

区域格式为屏幕坐标 `x,y,width,height`，支持的操作为 `copy` 和 `save`。

<a id="构建"></a>

## 构建

### 本机 Release 构建

为了获得最可靠的结果，建议在目标操作系统上执行构建：

```bash
cargo build --release
```

可执行文件位于 `target/release/legend_shot`，Windows 下为 `legend_shot.exe`。

### 构建脚本

`build.sh` 封装了各编译目标的 Cargo 命令：

| 命令 | 目标平台 |
| --- | --- |
| `./build.sh setup` | 安装脚本中配置的 Rust 编译目标。 |
| `./build.sh linux-amd64` | Linux x86_64。 |
| `./build.sh linux-arm64` | Linux ARM64。 |
| `./build.sh windows` | Windows x86_64。 |
| `./build.sh macos-arm64` | macOS Apple Silicon。 |
| `./build.sh macos-intel` | macOS Intel。 |
| `./build.sh all` | 依次执行所有目标构建。 |

仅安装 Rust target 通常不足以完成桌面程序的交叉编译，还需要对应的平台 SDK、原生库、链接器和 OCR 运行时依赖。正式发布时，推荐在各目标操作系统本机或对应 CI Runner 上构建。

交叉构建 `linux-arm64` 时，还需安装 `gcc-aarch64-linux-gnu`，并在 `/usr/lib/aarch64-linux-gnu/pkgconfig` 中提供 ARM64 的 GTK/AppIndicator 开发包。构建预检会检查该目标架构目录，不会把宿主架构库误判为可用。

## 平台说明

| 平台 | 注意事项 |
| --- | --- |
| macOS | 启动后常驻菜单栏；首次执行截图时请求“屏幕与系统录音”权限，从终端启动开发版本时权限身份可能显示为终端。 |
| Windows | 启动后常驻系统托盘，完成截图后返回托盘；通过 `arboard` 使用原生剪贴板，中文渲染会优先使用系统中的微软雅黑或黑体。 |
| Linux | 启动后常驻顶部栏，当前以 X11 为目标；桌面会话需支持 AppIndicator/StatusNotifier，复制 PNG 图片需要 `xclip`。 |

## OCR 与隐私

Legend Shot 通过 `oar-ocr` 使用 PP-OCRv5 移动端检测与识别模型。首次使用时可能自动下载模型文件；模型加载完成后，OCR 推理在本地执行，Legend Shot 不会主动上传截图内容。

## 配置文件

应用使用 Rust `directories` 库解析各平台的标准配置目录，并在其中保存 `config.json`。当前持久化内容包括：

- 全局截图快捷键。
- 上次保存截图时使用的目录。
- OCR 结果窗口的位置和大小。

<a id="项目结构"></a>

## 项目结构

```text
src/
├── main.rs           # CLI 参数、启动流程、字体和平台窗口设置
├── app_default.rs    # 应用状态、工具、标注和配置
├── app.rs            # egui 主更新循环以及视图/窗口切换
├── app_draw.rs       # 屏幕绘制、选区和鼠标/键盘输入
├── app_toolbar.rs    # 标注工具栏和画布文字编辑器
├── app_history.rs   # 编辑历史与选区/标注撤销重做
├── app_settings.rs  # 全局快捷键设置窗口
├── app_pin.rs       # 贴图置顶、显示控制和原图导出
├── hotkey.rs        # 系统热键注册、冲突恢复与事件门控
├── selection.rs     # 选区几何、手柄、尺寸与放大镜
├── app_handle.rs     # 裁剪、标注导出、剪贴板和文件保存
├── app_ocr.rs        # OCR 截图流程和结果状态切换
├── app_ocr_view.rs   # OCR 结果界面
├── ocr.rs            # OCR 会话、工作线程、超时和错误模型
├── ocr_oar.rs        # oar-ocr 后端与 PP-OCRv5 结果整理
├── tray.rs           # 原生菜单创建以及 macOS/Linux 托盘运行时
└── ui.rs             # 图标、纹理、字体和图片绘制工具
```

项目以 `ScreenshotApp` 作为中心状态，通过不同文件中的 `impl` 块拆分职责。屏幕截图和界面绘制使用不同坐标系，并将窗口逻辑坐标映射回截图原生像素。

## 开发与验证

```bash
cargo build
cargo test --all-targets
```

部分测试会创建真实截图应用，因此需要与开发运行相同的屏幕录制或辅助功能权限。

## 路线图

- 发布 Windows、Linux 和 macOS 的签名安装包。
- 补充干净的产品截图和发行说明。
- 在 CI 中扩展多平台测试与自动打包。

## 参与贡献

欢迎通过 [gitee.com/legendqi/legend_shot](https://gitee.com/legendqi/legend_shot) 提交 Issue 或 Pull Request。报告问题时，请附上操作系统、桌面/显示器配置、复现步骤以及相关终端输出。

## 许可证

Legend Shot 使用[木兰宽松许可证第 2 版](LICENSE)。

## 致谢

Legend Shot 基于 [`egui`](https://github.com/emilk/egui)、[`xcap`](https://github.com/nashaofu/xcap)、[`arboard`](https://github.com/1Password/arboard) 和 [`oar-ocr`](https://github.com/GreatV/oar-ocr) 等开源项目构建。
