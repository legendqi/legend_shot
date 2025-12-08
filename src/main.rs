use crate::app_default::ScreenshotApp;

mod ui;
mod app_default;
mod app_draw;
mod app_toolbar;
mod app_handle;
mod app;

fn main() -> eframe::Result<()> {
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

