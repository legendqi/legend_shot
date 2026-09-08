use std::sync::{Arc, Mutex, Once, Weak, mpsc};

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub(crate) fn default_capture_shortcut() -> String {
    if cfg!(target_os = "macos") {
        "Command+Shift+A".to_string()
    } else {
        "Ctrl+Shift+A".to_string()
    }
}

pub(crate) fn normalize_capture_shortcut(value: &str) -> Result<String, String> {
    parse_capture_shortcut(value).map(|hotkey| hotkey.to_string())
}

#[derive(Default)]
struct EventGate {
    active_id: Option<u32>,
    pressed: bool,
    capture_enabled: bool,
}

impl EventGate {
    fn activate(&mut self, id: u32) {
        self.active_id = Some(id);
        self.pressed = false;
    }

    fn accepts(&mut self, event: GlobalHotKeyEvent) -> bool {
        if self.active_id != Some(event.id) {
            return false;
        }
        match event.state {
            HotKeyState::Pressed => {
                let was_pressed = std::mem::replace(&mut self.pressed, true);
                self.capture_enabled && !was_pressed
            }
            HotKeyState::Released => {
                self.pressed = false;
                false
            }
        }
    }
}

fn parse_capture_shortcut(value: &str) -> Result<HotKey, String> {
    let hotkey = value.trim().parse::<HotKey>().map_err(|_| {
        "快捷键格式无效，请使用修饰键和一个按键，例如 Ctrl+Shift+A 或 Command+Shift+A".to_string()
    })?;
    if hotkey.mods.is_empty() {
        return Err("快捷键必须包含 Ctrl、Command、Alt 或 Shift 修饰键".to_string());
    }
    Ok(hotkey)
}

trait Registrar {
    fn register(&self, hotkey: HotKey) -> Result<(), String>;
    fn unregister(&self, hotkey: HotKey) -> Result<(), String>;
}

impl Registrar for GlobalHotKeyManager {
    fn register(&self, hotkey: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::register(self, hotkey).map_err(|error| error.to_string())
    }

    fn unregister(&self, hotkey: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::unregister(self, hotkey).map_err(|error| error.to_string())
    }
}

#[derive(Default)]
struct Registration {
    active: Option<HotKey>,
    owned: Vec<HotKey>,
}

impl Registration {
    fn set_shortcut(&mut self, value: &str, registrar: &impl Registrar) -> Result<(), String> {
        let next = parse_capture_shortcut(value)?;
        if self.active == Some(next) {
            return Ok(());
        }
        registrar
            .register(next)
            .map_err(|error| format!("快捷键注册失败，可能已被其他应用占用：{error}"))?;
        self.owned.push(next);
        if let Some(previous) = self.active {
            if let Err(error) = registrar.unregister(previous) {
                if registrar.unregister(next).is_ok() {
                    self.owned.retain(|hotkey| *hotkey != next);
                }
                return Err(format!("原快捷键释放失败，未更改设置：{error}"));
            }
            self.owned.retain(|hotkey| *hotkey != previous);
        }
        self.active = Some(next);
        Ok(())
    }
}

struct EventTarget {
    gate: Mutex<EventGate>,
    trigger_sender: mpsc::SyncSender<()>,
    ctx: egui::Context,
}

impl EventTarget {
    fn set_capture_enabled(&self, enabled: bool, trigger_receiver: &mpsc::Receiver<()>) {
        let mut gate = self.gate.lock().unwrap_or_else(|error| error.into_inner());
        if gate.capture_enabled != enabled {
            gate.capture_enabled = enabled;
            // Serialize with dispatch: requests from the previous UI mode
            // must not survive a capture or settings transition. Retain the
            // key state so reenabling while held does not start a capture.
            while trigger_receiver.try_recv().is_ok() {}
        }
    }

    fn dispatch(&self, event: GlobalHotKeyEvent) {
        let gate = self.gate.lock();
        let Ok(mut gate) = gate else {
            return;
        };
        // Keep sending under the gate lock so a shortcut switch can discard
        // all earlier notifications before accepting events for the new key.
        let accepted = gate.accepts(event);
        if accepted {
            let _ = self.trigger_sender.try_send(());
        }
        drop(gate);
        if accepted {
            self.ctx.request_repaint();
        }
    }
}

// global-hotkey 0.8 installs its process-wide handler only once. Forward
// through a weak target so a later runtime can retry after initialization
// errors, and the handler does not keep a dropped UI context alive.
static INSTALL_EVENT_HANDLER: Once = Once::new();
static EVENT_TARGET: Mutex<Weak<EventTarget>> = Mutex::new(Weak::new());

pub(crate) struct HotkeyRuntime {
    manager: GlobalHotKeyManager,
    registration: Registration,
    event_target: Arc<EventTarget>,
    trigger_receiver: mpsc::Receiver<()>,
}

impl HotkeyRuntime {
    /// Create on the eframe UI thread; macOS and Windows require that same
    /// thread to own the native event loop. Register separately so the UI can
    /// remain available after a shortcut conflict. Capture emission stays
    /// disabled until the application explicitly enables its idle mode.
    pub(crate) fn new(ctx: egui::Context) -> Result<Self, String> {
        let manager =
            GlobalHotKeyManager::new().map_err(|error| format!("全局快捷键初始化失败：{error}"))?;
        let (trigger_sender, trigger_receiver) = mpsc::sync_channel(1);
        let event_target = Arc::new(EventTarget {
            gate: Mutex::new(EventGate::default()),
            trigger_sender,
            ctx,
        });
        let mut target = EVENT_TARGET
            .lock()
            .map_err(|_| "全局快捷键事件状态不可用".to_string())?;
        if target.upgrade().is_some() {
            return Err("全局快捷键管理器已经启动".to_string());
        }
        *target = Arc::downgrade(&event_target);
        INSTALL_EVENT_HANDLER.call_once(|| {
            GlobalHotKeyEvent::set_event_handler(Some(|event| {
                let target = EVENT_TARGET.lock().ok().and_then(|target| target.upgrade());
                if let Some(target) = target {
                    target.dispatch(event);
                }
            }));
        });
        drop(target);
        Ok(Self {
            manager,
            registration: Registration::default(),
            event_target,
            trigger_receiver,
        })
    }

    pub(crate) fn set_shortcut(&mut self, value: &str) -> Result<(), String> {
        let previous = self.registration.active;
        self.registration.set_shortcut(value, &self.manager)?;
        if previous != self.registration.active {
            let mut gate = self
                .event_target
                .gate
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            gate.activate(self.registration.active.expect("registered shortcut").id());
            while self.trigger_receiver.try_recv().is_ok() {}
        }
        Ok(())
    }

    pub(crate) fn take_triggered(&self) -> bool {
        self.trigger_receiver.try_recv().is_ok()
    }

    pub(crate) fn set_capture_enabled(&self, enabled: bool) {
        self.event_target
            .set_capture_enabled(enabled, &self.trigger_receiver);
    }
}

impl Drop for HotkeyRuntime {
    fn drop(&mut self) {
        if let Ok(mut gate) = self.event_target.gate.lock() {
            gate.active_id = None;
        }
        if let Ok(mut target) = EVENT_TARGET.lock()
            && target.ptr_eq(&Arc::downgrade(&self.event_target))
        {
            *target = Weak::new();
        }
        for hotkey in self.registration.owned.drain(..) {
            if let Err(error) = self.manager.unregister(hotkey) {
                eprintln!("全局快捷键注销失败：{error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;

    use global_hotkey::hotkey::{Code, Modifiers};

    use super::*;

    #[derive(Default)]
    struct NativeRegistrationStub {
        registered: RefCell<HashSet<HotKey>>,
        conflict: Cell<Option<HotKey>>,
        unregister_failure: Cell<Option<HotKey>>,
        operations: RefCell<Vec<(&'static str, HotKey)>>,
    }

    impl Registrar for NativeRegistrationStub {
        fn register(&self, hotkey: HotKey) -> Result<(), String> {
            self.operations.borrow_mut().push(("register", hotkey));
            if self.conflict.get() == Some(hotkey) || !self.registered.borrow_mut().insert(hotkey) {
                return Err("conflict".into());
            }
            Ok(())
        }

        fn unregister(&self, hotkey: HotKey) -> Result<(), String> {
            self.operations.borrow_mut().push(("unregister", hotkey));
            if self.unregister_failure.get() == Some(hotkey) {
                return Err("unregister failed".into());
            }
            self.registered.borrow_mut().remove(&hotkey);
            Ok(())
        }
    }

    #[test]
    fn parses_modifier_aliases_and_whitespace() {
        let parsed = parse_capture_shortcut(" Ctrl + Shift + a ").unwrap();
        assert_eq!(parsed.key, Code::KeyA);
        assert_eq!(parsed.mods, Modifiers::CONTROL | Modifiers::SHIFT);
        assert_eq!(
            parse_capture_shortcut("Command+Shift+A").unwrap(),
            HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyA)
        );
    }

    #[test]
    fn rejects_bare_keys_empty_input_and_malformed_combinations() {
        for input in [
            "",
            " ",
            "A",
            "F8",
            "Ctrl+",
            "Ctrl+Shift",
            "Ctrl+A+B",
            "Ctrl++A",
            "Ctrl+Unknown",
        ] {
            assert!(parse_capture_shortcut(input).is_err(), "accepted {input:?}");
        }
    }

    #[test]
    fn conflict_keeps_previous_shortcut_registered_and_active() {
        let native = NativeRegistrationStub::default();
        let mut registration = Registration::default();
        let previous = HotKey::new(Some(Modifiers::CONTROL), Code::KeyA);
        let next = HotKey::new(Some(Modifiers::CONTROL), Code::KeyB);
        registration.set_shortcut("Ctrl+A", &native).unwrap();
        native.conflict.set(Some(next));

        assert!(registration.set_shortcut("Ctrl+B", &native).is_err());
        assert_eq!(registration.active, Some(previous));
        assert_eq!(*native.registered.borrow(), HashSet::from([previous]));
    }

    #[test]
    fn invalid_shortcut_does_not_touch_previous_registration() {
        let native = NativeRegistrationStub::default();
        let mut registration = Registration::default();
        registration.set_shortcut("Ctrl+A", &native).unwrap();
        native.operations.borrow_mut().clear();

        assert!(registration.set_shortcut("B", &native).is_err());
        assert_eq!(registration.active.unwrap().key, Code::KeyA);
        assert!(native.operations.borrow().is_empty());
    }

    #[test]
    fn switch_registers_new_before_releasing_previous_shortcut() {
        let native = NativeRegistrationStub::default();
        let mut registration = Registration::default();
        let previous = HotKey::new(Some(Modifiers::CONTROL), Code::KeyA);
        let next = HotKey::new(Some(Modifiers::CONTROL), Code::KeyB);
        registration.set_shortcut("Ctrl+A", &native).unwrap();
        native.operations.borrow_mut().clear();

        registration.set_shortcut("Ctrl+B", &native).unwrap();
        assert_eq!(registration.active, Some(next));
        assert_eq!(*native.registered.borrow(), HashSet::from([next]));
        assert_eq!(
            *native.operations.borrow(),
            vec![("register", next), ("unregister", previous)]
        );
    }

    #[test]
    fn equivalent_shortcut_does_not_register_twice() {
        let native = NativeRegistrationStub::default();
        let mut registration = Registration::default();
        registration.set_shortcut("Ctrl+A", &native).unwrap();
        native.operations.borrow_mut().clear();

        registration.set_shortcut("Control+KeyA", &native).unwrap();
        assert_eq!(registration.active.unwrap().key, Code::KeyA);
        assert!(native.operations.borrow().is_empty());
    }

    #[test]
    fn release_failure_rolls_back_new_registration() {
        let native = NativeRegistrationStub::default();
        let mut registration = Registration::default();
        let previous = HotKey::new(Some(Modifiers::CONTROL), Code::KeyA);
        registration.set_shortcut("Ctrl+A", &native).unwrap();
        native.unregister_failure.set(Some(previous));

        assert!(registration.set_shortcut("Ctrl+B", &native).is_err());
        assert_eq!(registration.active, Some(previous));
        assert_eq!(*native.registered.borrow(), HashSet::from([previous]));
    }

    #[test]
    fn only_active_press_events_trigger_once_until_release() {
        let mut gate = EventGate {
            capture_enabled: true,
            ..EventGate::default()
        };
        let event = |id, state| GlobalHotKeyEvent { id, state };
        assert!(!gate.accepts(event(10, HotKeyState::Pressed)));
        gate.activate(10);
        assert!(!gate.accepts(event(11, HotKeyState::Pressed)));
        assert!(!gate.accepts(event(10, HotKeyState::Released)));
        assert!(gate.accepts(event(10, HotKeyState::Pressed)));
        assert!(!gate.accepts(event(10, HotKeyState::Pressed)));
        assert!(!gate.accepts(event(11, HotKeyState::Released)));
        assert!(!gate.accepts(event(10, HotKeyState::Pressed)));
        assert!(!gate.accepts(event(10, HotKeyState::Released)));
        assert!(gate.accepts(event(10, HotKeyState::Pressed)));
    }

    #[test]
    fn changing_shortcut_discards_previous_pressed_state() {
        let mut gate = EventGate {
            capture_enabled: true,
            ..EventGate::default()
        };
        gate.activate(10);
        assert!(gate.accepts(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed
        }));
        gate.activate(11);
        assert!(!gate.accepts(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed
        }));
        assert!(gate.accepts(GlobalHotKeyEvent {
            id: 11,
            state: HotKeyState::Pressed
        }));
    }

    fn event_target() -> (EventTarget, mpsc::Receiver<()>) {
        let (trigger_sender, trigger_receiver) = mpsc::sync_channel(1);
        let mut gate = EventGate::default();
        gate.activate(10);
        (
            EventTarget {
                gate: Mutex::new(gate),
                trigger_sender,
                ctx: egui::Context::default(),
            },
            trigger_receiver,
        )
    }

    #[test]
    fn disabled_capture_does_not_emit_triggers() {
        let (target, receiver) = event_target();
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed,
        });
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Released,
        });
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn enabling_capture_while_key_is_held_requires_a_new_press() {
        let (target, receiver) = event_target();
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed,
        });
        target.set_capture_enabled(true, &receiver);
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed,
        });
        assert!(receiver.try_recv().is_err());

        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Released,
        });
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed,
        });
        assert!(receiver.try_recv().is_ok());
    }

    #[test]
    fn capture_mode_transitions_discard_waiting_trigger() {
        let (target, receiver) = event_target();
        target.set_capture_enabled(true, &receiver);
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed,
        });
        target.set_capture_enabled(false, &receiver);
        assert!(receiver.try_recv().is_err());
        target.set_capture_enabled(true, &receiver);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn repeated_idle_mode_updates_preserve_waiting_trigger() {
        let (target, receiver) = event_target();
        target.set_capture_enabled(true, &receiver);
        target.dispatch(GlobalHotKeyEvent {
            id: 10,
            state: HotKeyState::Pressed,
        });
        target.set_capture_enabled(true, &receiver);
        assert!(receiver.try_recv().is_ok());
    }
}
