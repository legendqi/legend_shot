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