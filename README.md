# Legend_Shot

Legend_Shot 是一个截图工具，它允许用户捕获屏幕、进行注释和编辑截图。该工具具有直观的用户界面，提供多种功能如选择区域、绘制形状、添加文本以及保存或复制截图。

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