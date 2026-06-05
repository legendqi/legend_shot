# Legend_Shot

Legend_Shot 是一个截图工具，它允许用户捕获屏幕、进行注释和编辑截图。该工具具有直观的用户界面，提供多种功能如选择区域、绘制形状、添加文本以及保存或复制截图。
## 设计和实现说明
该截图应用使用跨平台截图库xcap实现，egui跨平台UI库进行标注，arboard操作系统剪贴板。
<br>
Windows和Linux下，启动时全屏启动，全屏灰色半透明蒙层，鼠标框选，画框选和移出半透明蒙层。
<br>
MacOS下，启动时全屏启动，因显示机制不同，直接覆盖半透明蒙层会全屏黑色，先截一张全屏的图，显示全屏图，再全屏灰色半透明蒙层，鼠标框选，画框选和移出半透明蒙层

<br>
画笔，矩形框选，移动，马赛克，序号，箭头都是利用egui画布功能实现
<br>
复制是复制到系统剪贴板，同时为了适配当前tauri应用无法复制图片到输入框，保存一份到系统临时目录中(Windows，Linux，MacOS各部相同)返回给vue，vue拿到图片发送，然后删除临时文件。后期输入框支持了可删除保存逻辑，复制完成后退出
<br>
保存是调起系统文件管理对话框，默认名字为时间戳screenshot_%Y-%m-%d_%H-%M-%S.png,可选择保存位置。保存完成后退出
<br>
## 功能特性

- **屏幕捕捉**：能够捕捉整个屏幕或选定区域。
- **注释工具**：提供多种注释工具，包括笔刷、矩形、移动、马赛克等。
- **文本输入**：允许在截图上添加自定义文本。
- **保存与复制**：可以将截图保存到本地或复制到剪贴板。

## 构建说明

要构建此项目，请运行以下命令之一：

- `./build.sh setup`：安装编译目标。
- `./build.sh setup_targets`：设置构建目标。
- `./build.sh build_linux_amd64`：为 Linux AMD64 构建。
- `./build.sh build_linux_arm64`：为 Linux ARM64 构建。
- `./build.sh build_windows`：为 Windows 构建。
- `./build.sh build_macos_arm64`：为 macOS ARM64 构建。
- `./build.sh build_macos_intel`：为 macOS Intel 构建。
- `./build.sh build_all`：为所有平台构建。

## 功能说明
- app_draw.rs 是监控鼠标和键盘的输入，同时绘制相关内容
- app_main.rs 是主函数，启动程序
- app_utils.rs 是一些工具函数
- app_handle.rs 是处理截图结果处理，包括复制到系统剪切板和保存本地
- app_toolbar.rs 是标注工具定义，可根据需要注释相关代码
```rust
    self.purple_icon_button(ui, Tool::MoveBox, ctx, MOVE_ICON, "move"); //移动
    self.purple_icon_button(ui, Tool::Pen, ctx, PEN_ICON, "pen"); //画笔
    self.purple_icon_button(ui, Tool::Rectangle, ctx, RECTANGLE_ICON, "rectangle"); //矩形
    self.purple_icon_button(ui, Tool::Arrow, ctx, ARROW_ICON, "arrow"); //箭头
    self.purple_icon_button(ui, Tool::Text, ctx, WORD_ICON, "word"); //文字
    self.purple_icon_button(ui, Tool::Mosaic, ctx, MOSAIC_ICON, "mosaic"); //马赛克
    self.purple_icon_button(ui, Tool::Number, ctx, NUMBER_ICON, "number"); //序号
    self.custom_color_picker(ui, ctx); //颜色选择
```


## 使用方法

启动应用后，您可以使用提供的工具进行截图和注释。使用工具栏中的按钮来选择不同的注释工具，添加文本，或者保存和复制截图。

## 贡献

欢迎贡献！如果您有兴趣改进 Legend_Shot，请查阅源代码并提交 Pull Request。

## 许可证

此项目使用 MIT 许可证。详情请查看仓库中的 LICENSE 文件。

---

## Ubuntu 支持 (v0.1.0)

本次更新主要针对 Ubuntu/Debian 平台进行了适配和优化。

### 安装 (Ubuntu/Debian)

一键安装：
```bash
curl -sL https://gitee.com/andnnl/legend_shot/releases/download/v0.1.0/download-install.sh | sudo bash
```

或手动安装：
```bash
# 下载
wget https://gitee.com/andnnl/legend_shot/releases/download/v0.1.0/legend_shot-0.1.0-x86_64

# 安装
chmod +x legend_shot-0.1.0-x86_64
sudo mv legend_shot-0.1.0-x86_64 /usr/local/bin/legend_shot

# 安装依赖
sudo apt-get install -y fonts-noto-cjk xclip
```

### 从源码构建

```bash
git clone https://gitee.com/andnnl/legend_shot.git
cd legend_shot
cargo build --release
sudo cp target/release/legend_shot /usr/local/bin/
```

### 交叉编译 (glibc)

编译适用于 Ubuntu/Debian/Fedora 等主流发行版的 glibc 版本：

```bash
# 安装编译目标（如果跨架构）
rustup target add x86_64-unknown-linux-gnu

# 编译
cargo build --release

# 输出文件
file target/release/legend_shot
# ELF 64-bit LSB pie executable, x86-64, dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, stripped
```

### 交叉编译 (musl)

使用 [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) 编译 musl 目标（适用于 Alpine Linux 等 musl 系统）：

```bash
# 安装 zig 和 cargo-zigbuild
# 参考: https://ziglang.org/download/ 和 https://github.com/rust-cross/cargo-zigbuild
pip install cargo-zigbuild
rustup target add x86_64-unknown-linux-musl

# 编译
RUSTFLAGS="-C target-feature=-crt-static" \
  PKG_CONFIG_ALLOW_CROSS=1 \
  PKG_CONFIG_SYSROOT_DIR=/ \
  cargo zigbuild --release --target=x86_64-unknown-linux-musl

# 输出文件
ls target/x86_64-unknown-linux-musl/release/legend_shot
```

> **注意**：musl 二进制需要目标机器安装 musl 运行时（`sudo apt install musl` 或 Alpine Linux 自带）。

### 配置快捷键

#### GNOME (Ubuntu 默认)

1. 打开 **设置** → **键盘** → **自定义快捷键**
2. 点击 **添加快捷键**
3. 填写：
   - **名称**：Legend Shot
   - **命令**：`legend_shot`
   - **快捷键**：`Ctrl+Alt+A` (或自定义)

#### KDE Plasma

1. 打开 **系统设置** → **快捷键**
2. 点击 **编辑** → **新建** → **全局快捷键** → **命令/URL**
3. 设置触发器和命令

### 操作说明

| 操作 | 说明 |
|------|------|
| 鼠标拖拽 | 框选截图区域 |
| 双击选区 | 复制到剪贴板并关闭 |
| ESC | 取消并退出 |

框选后工具栏出现在选区边缘，支持：选择、画笔、矩形、箭头、文字、马赛克、序号、保存、复制、退出。

### 命令行参数

```bash
legend_shot --help

# 测试模式
legend_shot --test "100,100,400,300" --action copy
legend_shot --test "100,100,400,300" --action save --output screenshot.png
```

### 系统要求

- Ubuntu 18.04+ / Debian 10+
- x86_64 架构
- 依赖：`fonts-noto-cjk`, `xclip`, X11 图形环境

### 配置文件

配置保存在 `~/.config/legend_shot/config.json`，会记住上次保存目录。

### 更新日志

#### v0.1.0 (2026-04-01)

- 支持 Ubuntu/Debian 平台
- 集成 egui-file-dialog 替代 GTK 对话框
- 添加中文支持 (NotoSansCJK 字体)
- 配置持久化 (保存目录记忆)
- 内置安装脚本