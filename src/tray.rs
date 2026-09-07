#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayCommand {
    Capture,
    Exit,
}

const CAPTURE_MENU_ID: &str = "legend-shot.capture";
const EXIT_MENU_ID: &str = "legend-shot.exit";
const MACOS_TRAY_ICON_PNG: &[u8] = include_bytes!("icon/tray-focus-32.png");
const LINUX_TRAY_ICON_PNG: &[u8] = include_bytes!("icon/tray-focus-linux-32.png");

fn platform_icon_bytes(is_macos: bool) -> &'static [u8] {
    if is_macos {
        MACOS_TRAY_ICON_PNG
    } else {
        LINUX_TRAY_ICON_PNG
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayMenuSpec {
    id: &'static str,
    label: &'static str,
    command: TrayCommand,
}

fn menu_specs() -> [TrayMenuSpec; 2] {
    [
        TrayMenuSpec {
            id: CAPTURE_MENU_ID,
            label: "截图",
            command: TrayCommand::Capture,
        },
        TrayMenuSpec {
            id: EXIT_MENU_ID,
            label: "退出",
            command: TrayCommand::Exit,
        },
    ]
}

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

fn build_tray(
    command_sender: std::sync::mpsc::Sender<TrayCommand>,
    ctx: egui::Context,
) -> Result<tray_icon::TrayIcon, String> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};

    let specs = menu_specs();
    debug_assert!(
        specs
            .iter()
            .all(|spec| command_for_menu_id(spec.id) == Some(spec.command))
    );
    let capture = MenuItem::with_id(specs[0].id, specs[0].label, true, None);
    let exit = MenuItem::with_id(specs[1].id, specs[1].label, true, None);
    let menu = Menu::with_items(&[&capture, &exit])
        .map_err(|error| format!("托盘菜单创建失败: {error}"))?;

    MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
        if let Some(command) = command_for_menu_id(event.id().as_ref()) {
            let _ = command_sender.send(command);
            ctx.request_repaint();
        }
    }));

    tray_icon::TrayIconBuilder::new()
        .with_tooltip("Legend Shot")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_icon(load_icon(platform_icon_bytes(cfg!(target_os = "macos")))?)
        .with_icon_as_template(cfg!(target_os = "macos"))
        .build()
        .map_err(|error| format!("托盘创建失败: {error}"))
}

pub(crate) struct TrayRuntime {
    command_receiver: std::sync::mpsc::Receiver<TrayCommand>,
    #[cfg(target_os = "macos")]
    _icon: tray_icon::TrayIcon,
    #[cfg(target_os = "linux")]
    shutdown_sender: std::sync::mpsc::Sender<()>,
}

impl TrayRuntime {
    pub(crate) fn start(ctx: egui::Context) -> Result<Self, String> {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();

        #[cfg(target_os = "macos")]
        {
            let icon = build_tray(command_sender, ctx)?;
            Ok(Self {
                command_receiver,
                _icon: icon,
            })
        }

        #[cfg(target_os = "linux")]
        {
            use std::time::Duration;

            let (shutdown_sender, shutdown_receiver) = std::sync::mpsc::channel();
            let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);

            std::thread::Builder::new()
                .name("legend-shot-tray".to_string())
                .spawn(move || {
                    let result = gtk::init()
                        .map_err(|error| format!("GTK 初始化失败: {error}"))
                        .and_then(|_| build_tray(command_sender, ctx));
                    let icon = match result {
                        Ok(icon) => {
                            let _ = ready_sender.send(Ok(()));
                            icon
                        }
                        Err(error) => {
                            let _ = ready_sender.send(Err(error));
                            return;
                        }
                    };

                    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                        if shutdown_receiver.try_recv().is_ok() {
                            gtk::main_quit();
                            gtk::glib::ControlFlow::Break
                        } else {
                            gtk::glib::ControlFlow::Continue
                        }
                    });
                    gtk::main();
                    drop(icon);
                })
                .map_err(|error| format!("托盘线程启动失败: {error}"))?;

            ready_receiver
                .recv_timeout(Duration::from_secs(3))
                .map_err(|error| format!("等待托盘启动失败: {error}"))??;
            Ok(Self {
                command_receiver,
                shutdown_sender,
            })
        }
    }

    pub(crate) fn try_recv(&self) -> Option<TrayCommand> {
        self.command_receiver.try_recv().ok()
    }

    pub(crate) fn shutdown(&mut self) {
        // The menu handler is process-global and lives until the imminent process exit.
        #[cfg(target_os = "linux")]
        let _ = self.shutdown_sender.send(());
    }
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
        assert!(load_icon(platform_icon_bytes(true)).is_ok());
        assert!(load_icon(platform_icon_bytes(false)).is_ok());
        assert!(load_icon(b"not a png").is_err());
    }

    #[test]
    fn linux_tray_icon_is_white_monochrome_on_transparency() {
        let icon = image::load_from_memory(LINUX_TRAY_ICON_PNG)
            .unwrap()
            .into_rgba8();
        let pixels = icon.pixels().collect::<Vec<_>>();

        assert!(pixels.iter().any(|pixel| pixel[3] == 0));
        assert!(pixels.iter().any(|pixel| pixel[3] > 0));
        assert!(
            pixels
                .iter()
                .filter(|pixel| pixel[3] > 0)
                .all(|pixel| pixel[0] == 255 && pixel[1] == 255 && pixel[2] == 255)
        );
    }

    #[test]
    fn menu_contains_only_capture_then_exit() {
        let specs = menu_specs();
        assert_eq!(specs.len(), 2);
        assert_eq!(
            (specs[0].id, specs[0].label, specs[0].command),
            (CAPTURE_MENU_ID, "截图", TrayCommand::Capture)
        );
        assert_eq!(
            (specs[1].id, specs[1].label, specs[1].command),
            (EXIT_MENU_ID, "退出", TrayCommand::Exit)
        );
    }
}
