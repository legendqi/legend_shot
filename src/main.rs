use crate::app_default::ScreenshotApp;
use clap::Parser;

mod ui;
mod app_default;
mod app_draw;
mod app_toolbar;
mod app_handle;
mod app;

/// Legend Shot - 截图工具
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 自动测试模式: 指定截图区域 "x,y,width,height"
    /// 例如: --test "100,100,400,300"
    #[arg(short, long)]
    test: Option<String>,

    /// 自动测试模式下执行的操作: copy(复制到剪贴板) 或 save(保存到文件)
    #[arg(short, long, default_value = "copy")]
    action: String,

    /// 输出文件路径 (仅当 action=save 时有效)
    #[arg(short, long)]
    output: Option<String>,
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();

    // 自动测试模式
    if let Some(region) = args.test {
        return run_test_mode(&region, &args.action, args.output.as_deref());
    }

    // 正常 GUI 模式
    let mut app = ScreenshotApp::default();
    let _ = app.capture_screens();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_fullscreen(true)
            // .with_maximized(true) // windows下会导致下方任务栏显示白条
            .with_decorations(false)
            // .with_always_on_top() // windows下会导致文件保存对话框无法弹出
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_close_button(false)
            .with_max_inner_size(egui::vec2(4096., 4096.))
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "",
        options,
        Box::new(|_cc| {
            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

/// 自动测试模式: 直接截图并保存/复制到剪贴板
fn run_test_mode(region: &str, action: &str, output: Option<&str>) -> eframe::Result<()> {
    // 解析区域参数
    let parts: Vec<i32> = region.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if parts.len() != 4 {
        eprintln!("错误: 区域参数格式不正确，应为 'x,y,width,height'");
        eprintln!("示例: --test \"100,100,400,300\"");
        std::process::exit(1);
    }

    let x = parts[0];
    let y = parts[1];
    let width = parts[2];
    let height = parts[3];

    eprintln!("自动测试模式: 区域=({},{},{},{})", x, y, width, height);

    // 初始化应用并捕获屏幕
    let mut app = ScreenshotApp::default();
    if let Err(e) = app.capture_screens() {
        eprintln!("错误: 无法捕获屏幕 - {}", e);
        std::process::exit(1);
    }

    // 设置选择区域 (使用鼠标坐标)
    app.mouse_selection_rect = Some(crate::app_default::MouseSelectionRect {
        start: (x, y),
        end: (x + width, y + height),
    });

    // 设置窗口坐标的选择区域
    use eframe::emath::Rect;
    use egui::Pos2;
    app.selection_rect = Some(Rect::from_min_size(
        Pos2::new(x as f32, y as f32),
        egui::vec2(width as f32, height as f32),
    ));

    eprintln!("调试: mouse_selection_rect = {:?}", app.mouse_selection_rect);
    eprintln!("调试: selection_rect = {:?}", app.selection_rect);

    // 执行操作
    match action {
        "copy" => {
            eprintln!("执行: 复制到剪贴板");
            match app.copy_to_clipboard() {
                Ok(_) => {
                    eprintln!("成功: 图片已复制到剪贴板");
                    // 验证剪贴板
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        match clipboard.get_image() {
                            Ok(img) => {
                                eprintln!("验证: 剪贴板图片尺寸 {}x{}", img.width, img.height);
                                eprintln!("测试通过!");
                            }
                            Err(e) => {
                                eprintln!("验证失败: 无法从剪贴板读取图片 - {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("失败: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "save" => {
            let output_path = output.unwrap_or("screenshot_test.png");
            eprintln!("执行: 保存到 {}", output_path);

            // 获取裁剪的图片
            if let Some(cropped) = app.crop_selection_for_test() {
                if let Err(e) = cropped.save(output_path) {
                    eprintln!("失败: 无法保存文件 - {}", e);
                    std::process::exit(1);
                }
                eprintln!("成功: 图片已保存到 {}", output_path);
            } else {
                eprintln!("失败: 无法裁剪图片");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("错误: 未知操作 '{}', 支持: copy, save", action);
            std::process::exit(1);
        }
    }

    Ok(())
}

