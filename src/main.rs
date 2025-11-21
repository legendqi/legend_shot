use crate::app_default::ScreenshotApp;

mod screenshot;
mod ui;
mod app_default;
mod app_draw;
mod app_toolbar;
mod app_handle;
mod app;


fn main() -> eframe::Result<()> {
    
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


