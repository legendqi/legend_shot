use crate::app_default::{AppLifecycle, ScreenshotApp};
use crate::hotkey::{HotkeyRuntime, default_capture_shortcut, normalize_capture_shortcut};

#[derive(Default)]
pub(crate) struct ShortcutSettings {
    pub open: bool,
    pub draft: String,
    pub error: Option<String>,
    pub saved: bool,
    pub recording: bool,
}

fn settings_viewport_builder() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title("Legend Shot · 快捷键设置")
        .with_inner_size([480.0, 260.0])
        .with_min_inner_size([480.0, 260.0])
        .with_max_inner_size([480.0, 260.0])
        .with_resizable(false)
        .with_decorations(true)
        .with_transparent(false)
        .with_visible(true)
}

fn shortcut_from_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<String> {
    let key = match key {
        egui::Key::Num0 => "0".to_owned(),
        egui::Key::Num1 => "1".to_owned(),
        egui::Key::Num2 => "2".to_owned(),
        egui::Key::Num3 => "3".to_owned(),
        egui::Key::Num4 => "4".to_owned(),
        egui::Key::Num5 => "5".to_owned(),
        egui::Key::Num6 => "6".to_owned(),
        egui::Key::Num7 => "7".to_owned(),
        egui::Key::Num8 => "8".to_owned(),
        egui::Key::Num9 => "9".to_owned(),
        egui::Key::Backtick => "`".to_owned(),
        egui::Key::OpenBracket => "[".to_owned(),
        egui::Key::CloseBracket => "]".to_owned(),
        egui::Key::Equals | egui::Key::Plus => "=".to_owned(),
        egui::Key::Colon | egui::Key::Semicolon => ";".to_owned(),
        egui::Key::Pipe | egui::Key::Backslash => "\\".to_owned(),
        egui::Key::Questionmark | egui::Key::Slash => "/".to_owned(),
        egui::Key::Exclamationmark => "1".to_owned(),
        egui::Key::Escape => return None,
        other => format!("{other:?}"),
    };
    let mut parts = Vec::with_capacity(4);
    if modifiers.mac_cmd {
        parts.push("Command");
    }
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(&key);
    let shortcut = parts.join("+");
    normalize_capture_shortcut(&shortcut)
        .is_ok()
        .then_some(shortcut)
}

fn shortcut_keycaps(value: &str, macos: bool) -> Vec<String> {
    value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "super" | "command" | "cmd" if macos => "⌘".to_owned(),
            "super" | "command" | "cmd" => "Win".to_owned(),
            "control" | "ctrl" if macos => "⌃".to_owned(),
            "control" | "ctrl" => "Ctrl".to_owned(),
            "option" | "alt" if macos => "⌥".to_owned(),
            "option" | "alt" => "Alt".to_owned(),
            "shift" if macos => "⇧".to_owned(),
            "shift" => "Shift".to_owned(),
            _ => part
                .strip_prefix("Key")
                .or_else(|| part.strip_prefix("Digit"))
                .unwrap_or(part)
                .to_owned(),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacKeycapSymbol {
    Command,
    Shift,
    Option,
    Control,
}

impl MacKeycapSymbol {
    fn accessibility_label(self) -> &'static str {
        match self {
            Self::Command => "Command",
            Self::Shift => "Shift",
            Self::Option => "Option",
            Self::Control => "Control",
        }
    }
}

fn mac_keycap_symbol(label: &str) -> Option<MacKeycapSymbol> {
    match label {
        "⌘" => Some(MacKeycapSymbol::Command),
        "⇧" => Some(MacKeycapSymbol::Shift),
        "⌥" => Some(MacKeycapSymbol::Option),
        "⌃" => Some(MacKeycapSymbol::Control),
        _ => None,
    }
}

fn draw_mac_keycap_symbol(ui: &mut egui::Ui, symbol: MacKeycapSymbol) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            ui.is_enabled(),
            symbol.accessibility_label(),
        )
    });
    let center = rect.center();
    let point = |x: f32, y: f32| center + egui::vec2(x, y);
    let stroke = egui::Stroke::new(1.6, egui::Color32::from_rgb(232, 237, 245));

    match symbol {
        MacKeycapSymbol::Command => {
            for offset in [
                egui::vec2(-4.0, -4.0),
                egui::vec2(4.0, -4.0),
                egui::vec2(-4.0, 4.0),
                egui::vec2(4.0, 4.0),
            ] {
                ui.painter().circle_stroke(center + offset, 3.0, stroke);
            }
            ui.painter()
                .line_segment([point(-4.0, -4.0), point(4.0, -4.0)], stroke);
            ui.painter()
                .line_segment([point(-4.0, 4.0), point(4.0, 4.0)], stroke);
            ui.painter()
                .line_segment([point(-4.0, -4.0), point(-4.0, 4.0)], stroke);
            ui.painter()
                .line_segment([point(4.0, -4.0), point(4.0, 4.0)], stroke);
        }
        MacKeycapSymbol::Shift => {
            ui.painter().add(egui::Shape::closed_line(
                vec![
                    point(-3.0, 7.0),
                    point(-3.0, 1.0),
                    point(-7.0, 1.0),
                    point(0.0, -7.0),
                    point(7.0, 1.0),
                    point(3.0, 1.0),
                    point(3.0, 7.0),
                ],
                stroke,
            ));
        }
        MacKeycapSymbol::Option => {
            ui.painter()
                .line_segment([point(-7.0, -5.0), point(-3.0, -5.0)], stroke);
            ui.painter()
                .line_segment([point(-3.0, -5.0), point(4.0, 5.0)], stroke);
            ui.painter()
                .line_segment([point(4.0, 5.0), point(7.0, 5.0)], stroke);
            ui.painter()
                .line_segment([point(2.0, -5.0), point(7.0, -5.0)], stroke);
        }
        MacKeycapSymbol::Control => {
            ui.painter().add(egui::Shape::line(
                vec![point(-6.0, 3.0), point(0.0, -3.0), point(6.0, 3.0)],
                stroke,
            ));
        }
    }
}

fn draw_keycap(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(13, 17, 23))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(63, 72, 86)))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            if let Some(symbol) = mac_keycap_symbol(label) {
                draw_mac_keycap_symbol(ui, symbol);
            } else {
                ui.label(
                    egui::RichText::new(label)
                        .family(egui::FontFamily::Monospace)
                        .size(15.0)
                        .strong()
                        .color(egui::Color32::from_rgb(232, 237, 245)),
                );
            }
        });
}

impl ScreenshotApp {
    pub(crate) fn initialize_hotkeys(&mut self, ctx: &egui::Context) {
        let result = HotkeyRuntime::new(ctx.clone()).and_then(|mut runtime| {
            let result = runtime.set_shortcut(&self.config.capture_shortcut);
            runtime.set_capture_enabled(
                self.lifecycle == AppLifecycle::TrayIdle
                    && !self.shortcut_settings.open
                    && !self.pins.is_saving(),
            );
            self.hotkey_runtime = Some(runtime);
            result
        });
        if let Err(error) = result {
            self.open_shortcut_settings(ctx);
            self.shortcut_settings.error = Some(error);
        }
    }

    pub(crate) fn open_shortcut_settings(&mut self, ctx: &egui::Context) {
        if self.lifecycle != AppLifecycle::TrayIdle || self.pins.is_saving() {
            return;
        }
        self.shortcut_settings.open = true;
        if let Some(runtime) = &self.hotkey_runtime {
            runtime.set_capture_enabled(false);
        }
        self.shortcut_settings.draft = self.config.capture_shortcut.clone();
        self.shortcut_settings.saved = false;
        self.shortcut_settings.recording = false;
        ctx.send_viewport_cmd_to(settings_viewport_id(), egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn apply_shortcut_setting(&mut self, ctx: &egui::Context) -> Result<(), String> {
        let shortcut = normalize_capture_shortcut(&self.shortcut_settings.draft)?;
        if self.hotkey_runtime.is_none() {
            self.hotkey_runtime = Some(HotkeyRuntime::new(ctx.clone())?);
        }
        self.hotkey_runtime
            .as_mut()
            .unwrap()
            .set_shortcut(&shortcut)?;
        self.config.capture_shortcut = shortcut.clone();
        self.shortcut_settings.draft = shortcut;
        self.save_config_result()
            .map_err(|error| format!("快捷键本次已生效，但无法保存设置：{error}"))
    }

    pub(crate) fn draw_shortcut_settings(&mut self, ctx: &egui::Context) {
        if !self.shortcut_settings.open {
            return;
        }
        let mut close = false;
        let mut apply = false;
        ctx.show_viewport_immediate(
            settings_viewport_id(),
            settings_viewport_builder(),
            |settings_ctx, _| {
                let was_recording = self.shortcut_settings.recording;
                if was_recording {
                    let key_event = settings_ctx.input(|input| {
                        input.events.iter().find_map(|event| match event {
                            egui::Event::Key {
                                key,
                                pressed: true,
                                repeat: false,
                                modifiers,
                                ..
                            } => Some((*key, *modifiers)),
                            _ => None,
                        })
                    });
                    if let Some((egui::Key::Escape, _)) = key_event {
                        self.shortcut_settings.recording = false;
                        self.shortcut_settings.error = None;
                    } else if let Some((key, modifiers)) = key_event {
                        if let Some(shortcut) = shortcut_from_key(key, modifiers) {
                            self.shortcut_settings.draft = shortcut;
                            self.shortcut_settings.recording = false;
                            self.shortcut_settings.saved = false;
                            self.shortcut_settings.error = None;
                        } else {
                            self.shortcut_settings.error =
                                Some("请同时按下修饰键和一个普通按键".to_owned());
                        }
                    }
                }
                close = settings_ctx.input(|input| {
                    input.viewport().close_requested()
                        || (!was_recording && input.key_pressed(egui::Key::Escape))
                });

                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(egui::Color32::from_rgb(15, 18, 23)))
                    .show(settings_ctx, |ui| {
                        egui::Frame::NONE.inner_margin(16.0).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (icon_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(36.0, 36.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    icon_rect,
                                    10.0,
                                    egui::Color32::from_rgb(18, 98, 235),
                                );
                                let c = icon_rect.center();
                                let stroke = egui::Stroke::new(1.8, egui::Color32::WHITE);
                                ui.painter().rect_stroke(
                                    egui::Rect::from_center_size(c, egui::vec2(18.0, 12.0)),
                                    3.0,
                                    stroke,
                                    egui::StrokeKind::Inside,
                                );
                                ui.painter().line_segment(
                                    [c + egui::vec2(-5.0, -2.0), c + egui::vec2(-5.0, 2.0)],
                                    stroke,
                                );
                                ui.painter().line_segment(
                                    [c + egui::vec2(0.0, -2.0), c + egui::vec2(0.0, 2.0)],
                                    stroke,
                                );
                                ui.painter().line_segment(
                                    [c + egui::vec2(5.0, -2.0), c + egui::vec2(5.0, 2.0)],
                                    stroke,
                                );
                                ui.add_space(3.0);
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("快捷键设置")
                                            .size(20.0)
                                            .strong()
                                            .color(egui::Color32::from_rgb(242, 245, 250)),
                                    );
                                    ui.label(
                                        egui::RichText::new("设置随时唤起截图的按键组合")
                                            .size(12.5)
                                            .color(egui::Color32::from_rgb(139, 149, 164)),
                                    );
                                });
                            });
                            ui.add_space(12.0);

                            let card = egui::Frame::new()
                                .fill(if self.shortcut_settings.recording {
                                    egui::Color32::from_rgb(19, 39, 70)
                                } else {
                                    egui::Color32::from_rgb(26, 31, 39)
                                })
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    if self.shortcut_settings.recording {
                                        egui::Color32::from_rgb(41, 121, 255)
                                    } else {
                                        egui::Color32::from_rgb(48, 56, 68)
                                    },
                                ))
                                .corner_radius(11.0)
                                .inner_margin(egui::Margin::symmetric(14, 11))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new("全局截图")
                                                    .size(14.0)
                                                    .strong()
                                                    .color(egui::Color32::from_rgb(228, 233, 241)),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    if self.shortcut_settings.recording {
                                                        "请按下新的组合键"
                                                    } else {
                                                        "点击右侧按键即可修改"
                                                    },
                                                )
                                                .size(12.0)
                                                .color(if self.shortcut_settings.recording {
                                                    egui::Color32::from_rgb(100, 164, 255)
                                                } else {
                                                    egui::Color32::from_rgb(125, 136, 151)
                                                }),
                                            );
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let mut keycaps = shortcut_keycaps(
                                                    &self.shortcut_settings.draft,
                                                    cfg!(target_os = "macos"),
                                                );
                                                keycaps.reverse();
                                                for keycap in keycaps {
                                                    draw_keycap(ui, &keycap);
                                                }
                                            },
                                        );
                                    });
                                });
                            let card_response = card.response.interact(egui::Sense::click());
                            if card_response.clicked() {
                                self.shortcut_settings.recording = true;
                                self.shortcut_settings.error = None;
                                self.shortcut_settings.saved = false;
                                settings_ctx.request_repaint();
                            }

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                let save = egui::Button::new(
                                    egui::RichText::new("保存快捷键")
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(18, 105, 240))
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(8.0)
                                .min_size(egui::vec2(116.0, 34.0));
                                apply = ui
                                    .add_enabled(!self.shortcut_settings.recording, save)
                                    .clicked();
                                let restore = egui::Button::new(
                                    egui::RichText::new("恢复默认")
                                        .color(egui::Color32::from_rgb(205, 212, 223)),
                                )
                                .fill(egui::Color32::from_rgb(33, 39, 48))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(53, 62, 75)))
                                .corner_radius(8.0)
                                .min_size(egui::vec2(100.0, 34.0));
                                if ui.add(restore).clicked() {
                                    self.shortcut_settings.draft = default_capture_shortcut();
                                    self.shortcut_settings.recording = false;
                                    self.shortcut_settings.saved = false;
                                    self.shortcut_settings.error = None;
                                }
                            });
                            ui.add_space(8.0);
                            let (status, color) = if let Some(error) = &self.shortcut_settings.error
                            {
                                (error.as_str(), egui::Color32::from_rgb(241, 104, 104))
                            } else if self.shortcut_settings.saved {
                                (
                                    "快捷键已保存，可以关闭窗口后测试",
                                    egui::Color32::from_rgb(76, 201, 140),
                                )
                            } else {
                                (
                                    "快捷键冲突时会保留原设置，也可继续从托盘截图",
                                    egui::Color32::from_rgb(117, 128, 143),
                                )
                            };
                            ui.label(egui::RichText::new(status).size(11.5).color(color));
                        });
                    });
            },
        );
        if apply {
            match self.apply_shortcut_setting(ctx) {
                Ok(()) => {
                    self.shortcut_settings.saved = true;
                    self.shortcut_settings.error = None;
                }
                Err(error) => {
                    self.shortcut_settings.error = Some(error);
                    self.shortcut_settings.saved = false;
                }
            }
            ctx.request_repaint();
        }
        if close {
            self.shortcut_settings.open = false;
            self.shortcut_settings.recording = false;
            if let Some(runtime) = &self.hotkey_runtime {
                runtime.set_capture_enabled(
                    self.lifecycle == AppLifecycle::TrayIdle && !self.pins.is_saving(),
                );
            }
            ctx.send_viewport_cmd_to(settings_viewport_id(), egui::ViewportCommand::Close);
        }
    }
}

fn settings_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("legend-shot-shortcut-settings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_recorder_converts_pressed_key_and_modifiers_to_valid_shortcut() {
        let command_shift = egui::Modifiers {
            shift: true,
            mac_cmd: true,
            command: true,
            ..egui::Modifiers::NONE
        };
        let ctrl_alt = egui::Modifiers {
            ctrl: true,
            alt: true,
            command: true,
            ..egui::Modifiers::NONE
        };

        assert_eq!(
            shortcut_from_key(egui::Key::A, command_shift),
            Some("Command+Shift+A".to_owned())
        );
        assert_eq!(
            shortcut_from_key(egui::Key::Num7, ctrl_alt),
            Some("Ctrl+Alt+7".to_owned())
        );
        assert_eq!(shortcut_from_key(egui::Key::A, egui::Modifiers::NONE), None);
        assert!(normalize_capture_shortcut("Command+Shift+A").is_ok());
        assert!(normalize_capture_shortcut("Ctrl+Alt+7").is_ok());
    }

    #[test]
    fn shortcut_keycaps_use_platform_labels_and_clean_key_names() {
        assert_eq!(
            shortcut_keycaps("super+shift+KeyA", true),
            vec!["⌘", "⇧", "A"]
        );
        assert_eq!(
            shortcut_keycaps("control+alt+Digit7", false),
            vec!["Ctrl", "Alt", "7"]
        );
    }

    #[test]
    fn macos_modifier_keycaps_use_vector_symbols_instead_of_font_glyphs() {
        assert_eq!(mac_keycap_symbol("⌘"), Some(MacKeycapSymbol::Command));
        assert_eq!(mac_keycap_symbol("⇧"), Some(MacKeycapSymbol::Shift));
        assert_eq!(mac_keycap_symbol("⌥"), Some(MacKeycapSymbol::Option));
        assert_eq!(mac_keycap_symbol("⌃"), Some(MacKeycapSymbol::Control));
        assert_eq!(mac_keycap_symbol("A"), None);
        assert_eq!(MacKeycapSymbol::Command.accessibility_label(), "Command");
        assert_eq!(MacKeycapSymbol::Shift.accessibility_label(), "Shift");
    }

    #[test]
    fn shortcut_settings_window_is_compact_and_fixed_size() {
        let builder = settings_viewport_builder();
        assert_eq!(builder.inner_size, Some(egui::vec2(480.0, 260.0)));
        assert_eq!(builder.min_inner_size, Some(egui::vec2(480.0, 260.0)));
        assert_eq!(builder.max_inner_size, Some(egui::vec2(480.0, 260.0)));
        assert_eq!(builder.resizable, Some(false));
    }

    #[test]
    fn invalid_shortcut_does_not_change_configuration_or_create_os_runtime() {
        let mut app = ScreenshotApp::default();
        let old = app.config.capture_shortcut.clone();
        app.shortcut_settings.draft = "invalid+++shortcut".to_string();
        assert!(
            app.apply_shortcut_setting(&egui::Context::default())
                .is_err()
        );
        assert_eq!(app.config.capture_shortcut, old);
        assert!(app.hotkey_runtime.is_none());
    }

    #[test]
    fn old_config_gets_a_valid_shortcut_and_preserves_existing_settings() {
        let config: crate::app_default::AppConfig =
            serde_json::from_str(r#"{"last_save_dir":"/tmp/screenshots"}"#).unwrap();
        assert!(normalize_capture_shortcut(&config.capture_shortcut).is_ok());
        assert_eq!(
            config.last_save_dir.unwrap(),
            std::path::PathBuf::from("/tmp/screenshots")
        );
    }

    #[test]
    fn shortcut_and_other_settings_round_trip_to_disk() {
        let mut app = ScreenshotApp {
            config_path: std::env::temp_dir().join(format!(
                "legend-shot-config-test-{}.json",
                std::process::id()
            )),
            ..Default::default()
        };
        app.config.capture_shortcut = "Ctrl+Alt+X".to_string();
        app.config.last_save_dir = Some(std::path::PathBuf::from("/tmp/screenshots"));
        app.save_config_result().unwrap();
        let json = std::fs::read_to_string(&app.config_path).unwrap();
        std::fs::remove_file(&app.config_path).unwrap();
        let restored: crate::app_default::AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.capture_shortcut, "Ctrl+Alt+X");
        assert_eq!(restored.last_save_dir, app.config.last_save_dir);
    }
}
