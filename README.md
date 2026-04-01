# Legend Shot

Legend Shot 是一个轻量级截图工具，专为 Linux 桌面环境设计。支持全屏截图、区域选择、标注编辑，并可保存到文件或复制到剪贴板。

## 功能特性

- **全屏截图**：自动捕获所有显示器
- **区域选择**：鼠标框选任意区域
- **标注工具**：画笔、矩形、箭头、文字、马赛克、序号标注
- **快捷操作**：双击复制到剪贴板，工具栏保存/复制
- **中文支持**：内置中文字体，界面完全中文化
- **配置持久化**：记住上次保存目录

## 安装

### Ubuntu/Debian (推荐)

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

## 配置快捷键

### GNOME (Ubuntu 默认)

1. 打开 **设置** → **键盘** → **自定义快捷键**
2. 点击 **添加快捷键**
3. 填写：
   - **名称**：Legend Shot
   - **命令**：`legend_shot`
   - **快捷键**：`Ctrl+Alt+A` (或自定义)

### KDE Plasma

1. 打开 **系统设置** → **快捷键**
2. 点击 **编辑** → **新建** → **全局快捷键** → **命令/URL**
3. 设置触发器和命令

## 使用方法

### 基本操作

| 操作 | 说明 |
|------|------|
| 鼠标拖拽 | 框选截图区域 |
| 双击选区 | 复制到剪贴板并关闭 |
| ESC | 取消并退出 |

### 工具栏

框选后，工具栏出现在选区边缘：

| 图标 | 功能 |
|------|------|
| 选择 | 移动/调整选区 |
| 画笔 | 自由绘制 |
| 矩形 | 绘制矩形框 |
| 箭头 | 绘制箭头 |
| 文字 | 添加文字标注 |
| 马赛克 | 模糊敏感区域 |
| 序号 | 添加编号标注 |
| 保存 | 保存到文件 |
| 复制 | 复制到剪贴板 |
| 退出 | 取消截图 |

### 命令行参数

```bash
legend_shot --help

# 测试模式 (自动化脚本)
legend_shot --test "100,100,400,300" --action copy
legend_shot --test "100,100,400,300" --action save --output screenshot.png
```

## 系统要求

- **操作系统**：Ubuntu 18.04+ / Debian 10+ / 其他 Linux 发行版
- **架构**：x86_64
- **依赖**：
  - `fonts-noto-cjk` (中文字体)
  - `xclip` (剪贴板支持)
  - X11 图形环境

## 配置文件

配置保存在 `~/.config/legend_shot/config.json`：

```json
{
  "last_save_dir": "/home/user/Pictures"
}
```

## 更新日志

### v0.1.0 (2026-04-01)

- 首个正式发布版本
- 支持 Ubuntu/Debian 平台
- 集成 egui-file-dialog 替代 GTK 对话框
- 添加中文支持 (NotoSansCJK 字体)
- 配置持久化 (保存目录记忆)
- 内置安装脚本

## 构建

跨平台构建脚本：

```bash
./build.sh setup          # 安装编译目标
./build.sh linux-amd64    # Linux x86_64
./build.sh linux-arm64    # Linux ARM64
./build.sh windows        # Windows
./build.sh macos-arm64    # macOS ARM
./build.sh macos-intel    # macOS Intel
```

## 许可证

MIT License