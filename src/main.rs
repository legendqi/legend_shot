mod screenshot;

use crate::screenshot::ScreenshotApp;

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
    
    // let mut initial_pixels = Some(rgba);
    
    eframe::run_native(
        "",
        options,
        Box::new(|_cc| Ok(Box::new(ScreenshotApp::default()))),
    )?;

    Ok(())
}


