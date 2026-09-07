use crate::app_default::{AppConfig, ScreenshotApp};
use crate::ocr::spawn_ocr_worker;
use crate::ocr_oar::OarOcrFactory;
use crate::ui::load_cjk_font;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

mod app;
mod app_default;
mod app_draw;
mod app_handle;
mod app_ocr;
mod app_ocr_view;
mod app_toolbar;
mod ocr;
mod ocr_oar;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod tray;
mod ui;

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

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacosCapturePermissionAction {
    Capture,
    RequestAndExit,
}

#[cfg(target_os = "macos")]
fn macos_capture_permission_action(has_permission: bool) -> MacosCapturePermissionAction {
    if has_permission {
        MacosCapturePermissionAction::Capture
    } else {
        MacosCapturePermissionAction::RequestAndExit
    }
}

#[cfg(target_os = "macos")]
fn ensure_macos_screen_capture_permission() -> eframe::Result<()> {
    use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

    match macos_capture_permission_action(CGPreflightScreenCaptureAccess()) {
        MacosCapturePermissionAction::Capture => Ok(()),
        MacosCapturePermissionAction::RequestAndExit => {
            let _ = CGRequestScreenCaptureAccess();
            Err(eframe::Error::AppCreation(Box::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "没有屏幕录制权限。请在“系统设置 → 隐私与安全性 → 屏幕与系统录音”中允许系统弹窗对应的 legend_shot（开发模式下可能显示为终端），然后重新运行应用。开发版本重新编译后可能需要再次授权。",
            ))))
        }
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();

    #[cfg(target_os = "macos")]
    ensure_macos_screen_capture_permission()?;

    // 自动测试模式
    if let Some(region) = args.test {
        return run_test_mode(&region, &args.action, args.output.as_deref());
    }

    // 加载配置
    let config_path = get_config_path();
    let config = load_config(&config_path);

    // 正常 GUI 模式
    let mut app = ScreenshotApp::with_config(config, config_path);
    app.ocr_worker = Some(spawn_ocr_worker(OarOcrFactory::new()));

    // 所有平台在窗口显示前截图，避免截图包含遮罩层
    #[cfg(target_os = "macos")]
    app.capture_screens().map_err(|error| {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(format!(
            "捕获屏幕失败: {error}"
        ))))
    })?;

    #[cfg(not(target_os = "macos"))]
    let _ = app.capture_screens();

    let capture_style = crate::app::capture_window_style();
    let mut viewport = egui::ViewportBuilder::default()
        .with_fullscreen(capture_style.fullscreen)
        .with_decorations(capture_style.decorations)
        .with_resizable(capture_style.resizable)
        .with_maximize_button(capture_style.maximize_button)
        .with_minimize_button(capture_style.minimize_button)
        .with_close_button(capture_style.close_button)
        .with_visible(false)
        .with_transparent(true);

    #[cfg(target_os = "macos")]
    if let Some(screen) = app
        .screens
        .iter()
        .find(|screen| screen.is_primary().unwrap_or(false))
        .or_else(|| app.screens.first())
    {
        if let (Ok(x), Ok(y), Ok(width), Ok(height)) =
            (screen.x(), screen.y(), screen.width(), screen.height())
        {
            viewport = viewport
                .with_position(egui::pos2(x as f32, y as f32))
                .with_inner_size(egui::vec2(width as f32, height as f32))
                .with_window_level(egui::WindowLevel::AlwaysOnTop);
        }
    }

    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if capture_style.accessory_application {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

            builder
                .with_activation_policy(ActivationPolicy::Accessory)
                .with_default_menu(false);
        }));
    }

    eframe::run_native(
        "",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            if let Some(font_data) = load_cjk_font() {
                fonts.font_data.insert(
                    "cjk_font".to_owned(),
                    Arc::new(egui::FontData::from_owned(font_data.as_ref().clone())),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "cjk_font".to_owned());
            }

            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

fn get_config_path() -> PathBuf {
    let dirs = directories::ProjectDirs::from("com", "legend", "legend_shot");
    dirs.map(|d| d.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from("legend_shot_config.json"))
}

fn load_config(path: &PathBuf) -> AppConfig {
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
    }
    AppConfig::default()
}

fn new_test_mode_app() -> ScreenshotApp {
    ScreenshotApp::default()
}

/// 自动测试模式: 直接截图并保存/复制到剪贴板
fn run_test_mode(region: &str, action: &str, output: Option<&str>) -> eframe::Result<()> {
    // 解析区域参数
    let parts: Vec<i32> = region
        .split(',')
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
    let mut app = new_test_mode_app();
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

#[cfg(test)]
mod tests {
    use super::new_test_mode_app;

    #[cfg(target_os = "macos")]
    use super::{MacosCapturePermissionAction, macos_capture_permission_action};

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_denied_screen_capture_permission_stops_before_capture() {
        assert_eq!(
            macos_capture_permission_action(false),
            MacosCapturePermissionAction::RequestAndExit
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_granted_screen_capture_permission_allows_capture() {
        assert_eq!(
            macos_capture_permission_action(true),
            MacosCapturePermissionAction::Capture
        );
    }

    #[test]
    fn test_mode_constructor_uses_default_without_ocr_worker() {
        let app = new_test_mode_app();

        assert!(app.ocr_worker.is_none());
    }
}
