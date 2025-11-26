use crate::app_default::ScreenshotApp;

mod screenshot;
mod ui;
mod app_default;
mod app_draw;
mod app_toolbar;
mod app_handle;
mod app;

fn main() -> eframe::Result<()> {

    // 在 Windows 上禁用 DPI 缩放感知
    #[cfg(target_os = "windows")]
    set_dpi_awareness();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_maximized(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_close_button(false)
            .with_transparent(true),
        ..Default::default()
    };
    
    
    eframe::run_native(
        "",
        options,
        Box::new(|_cc| Ok(Box::new(ScreenshotApp::default()))),
    )?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn set_dpi_awareness() {
    use winapi::um::shellscalingapi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE};
    use winapi::um::winuser::SetProcessDPIAware;

    // 尝试使用较新的 API（Windows 10+）
    let result = unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) };

    // 如果失败，回退到旧版 API
    if result != 0 {
        unsafe { SetProcessDPIAware() };
    }
}


