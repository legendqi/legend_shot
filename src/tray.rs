#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayCommand {
    Capture,
    Exit,
}

const CAPTURE_MENU_ID: &str = "legend-shot.capture";
const EXIT_MENU_ID: &str = "legend-shot.exit";
const TRAY_ICON_PNG: &[u8] = include_bytes!("icon/tray-focus-32.png");

fn command_for_menu_id(id: &str) -> Option<TrayCommand> {
    match id {
        CAPTURE_MENU_ID => Some(TrayCommand::Capture),
        EXIT_MENU_ID => Some(TrayCommand::Exit),
        _ => None,
    }
}

fn load_icon(bytes: &[u8]) -> Result<tray_icon::Icon, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("托盘图标解码失败: {error}"))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|error| format!("托盘图标创建失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_known_menu_ids() {
        assert_eq!(
            command_for_menu_id(CAPTURE_MENU_ID),
            Some(TrayCommand::Capture)
        );
        assert_eq!(command_for_menu_id(EXIT_MENU_ID), Some(TrayCommand::Exit));
        assert_eq!(command_for_menu_id("legend-shot.unknown"), None);
    }

    #[test]
    fn embedded_tray_icon_is_valid_rgba() {
        assert!(load_icon(TRAY_ICON_PNG).is_ok());
        assert!(load_icon(b"not a png").is_err());
    }
}
