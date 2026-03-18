use crate::focus::{self, FocusedProcess};
#[cfg(target_os = "linux")]
use dbus::blocking::Connection;
#[cfg(target_os = "linux")]
use dbus::message::MatchRule;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use xcb::{x, Xid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerButton {
    Right,
    Middle,
}

#[derive(Debug, Clone)]
pub enum GlobalMouseEvent {
    GestureStart {
        screen_pos: (f32, f32),
        button: TriggerButton,
        process: Option<FocusedProcess>,
    },
    GestureMove {
        screen_pos: (f32, f32),
    },
    GestureEnd {
        screen_pos: (f32, f32),
    },
    Unsupported {
        reason: String,
    },
}

pub struct GlobalMouseHook {
    receiver: Receiver<GlobalMouseEvent>,
}

#[cfg(target_os = "linux")]
const KDE_BRIDGE_BUS_NAME: &str = "net.quicker_rs.KWinBridge";
#[cfg(target_os = "linux")]
const KDE_BRIDGE_INTERFACE: &str = "net.quicker_rs.KWinBridge";

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxSessionKind {
    X11,
    Wayland,
    Unknown,
}

impl GlobalMouseHook {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        #[cfg(target_os = "linux")]
        {
            thread::spawn(move || {
                if let Err(err) = run_linux_hook(tx.clone()) {
                    let _ = tx.send(GlobalMouseEvent::Unsupported { reason: err });
                }
            });
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = tx.send(GlobalMouseEvent::Unsupported {
                reason: "Global mouse gestures are currently implemented for Linux/X11 only".into(),
            });
        }

        Self { receiver: rx }
    }

    pub fn try_recv(&self) -> Result<GlobalMouseEvent, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
}

pub fn trigger_button_for_process(process: Option<&FocusedProcess>) -> TriggerButton {
    let Some(process) = process else {
        return TriggerButton::Middle;
    };

    let browser_patterns = [
        "chrome", "chromium", "firefox", "msedge", "edge", "brave", "opera", "vivaldi", "zen",
        "safari",
    ];

    if browser_patterns
        .iter()
        .any(|pattern| process.matches_pattern(pattern))
    {
        TriggerButton::Right
    } else {
        TriggerButton::Middle
    }
}

#[cfg(target_os = "linux")]
fn run_linux_hook(tx: mpsc::Sender<GlobalMouseEvent>) -> Result<(), String> {
    match detect_linux_session_kind() {
        LinuxSessionKind::X11 => return run_linux_x11_hook(tx),
        LinuxSessionKind::Wayland => {
            if is_kde_desktop() {
                return run_kde_wayland_hook(tx);
            }

            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
            return Err(format!(
                "Wayland session detected ({desktop}). Generic global right/middle-button capture is not supported on Wayland; this feature currently only has a KDE/KWin-specific backend"
            ));
        }
        LinuxSessionKind::Unknown => {
            return Err("Unable to determine Linux desktop session type".into());
        }
    }
}

#[cfg(target_os = "linux")]
fn run_linux_x11_hook(tx: mpsc::Sender<GlobalMouseEvent>) -> Result<(), String> {
    let (conn, _) = xcb::Connection::connect(None).map_err(|err| err.to_string())?;
    let setup = conn.get_setup();
    let root = setup
        .roots()
        .next()
        .ok_or_else(|| "No X11 root window found".to_string())?
        .root();

    let mut grabbed_button = trigger_button_for_process(focus::detect_focused_process().as_ref());
    grab_button(&conn, root, grabbed_button)?;
    conn.flush().map_err(|err| err.to_string())?;

    let mut gesture_active = false;
    let mut last_focus_poll = Instant::now();

    loop {
        if !gesture_active && last_focus_poll.elapsed() >= Duration::from_millis(250) {
            last_focus_poll = Instant::now();
            let next_process = focus::detect_focused_process();
            let next_button = trigger_button_for_process(next_process.as_ref());
            if next_button != grabbed_button {
                ungrab_button(&conn, root, grabbed_button)?;
                grab_button(&conn, root, next_button)?;
                conn.flush().map_err(|err| err.to_string())?;
                grabbed_button = next_button;
            }
        }

        match conn.poll_for_event().map_err(|err| err.to_string())? {
            Some(xcb::Event::X(x::Event::ButtonPress(event))) => {
                if matches_button(event.detail(), grabbed_button) {
                    gesture_active = true;
                    let gesture_process = focus::detect_focused_process();
                    let button = trigger_button_for_process(gesture_process.as_ref());
                    let _ = tx.send(GlobalMouseEvent::GestureStart {
                        screen_pos: (event.root_x() as f32, event.root_y() as f32),
                        button,
                        process: gesture_process.clone(),
                    });
                }
            }
            Some(xcb::Event::X(x::Event::MotionNotify(event))) => {
                if gesture_active {
                    let _ = tx.send(GlobalMouseEvent::GestureMove {
                        screen_pos: (event.root_x() as f32, event.root_y() as f32),
                    });
                }
            }
            Some(xcb::Event::X(x::Event::ButtonRelease(event))) => {
                if gesture_active && matches_button(event.detail(), grabbed_button) {
                    gesture_active = false;
                    let _ = tx.send(GlobalMouseEvent::GestureEnd {
                        screen_pos: (event.root_x() as f32, event.root_y() as f32),
                    });
                }
            }
            Some(_) => {}
            None => thread::sleep(Duration::from_millis(8)),
        }
    }
}

#[cfg(target_os = "linux")]
fn run_kde_wayland_hook(tx: mpsc::Sender<GlobalMouseEvent>) -> Result<(), String> {
    let conn = Connection::new_session().map_err(|err| err.to_string())?;
    conn.request_name(KDE_BRIDGE_BUS_NAME, false, true, false)
        .map_err(|err| err.to_string())?;

    let tx_start = tx.clone();
    conn.add_match(
        MatchRule::new_signal(KDE_BRIDGE_INTERFACE, "GestureStart"),
        move |(x, y, button, window_class): (f64, f64, String, String), _, _| {
            let button = match button.as_str() {
                "right" => TriggerButton::Right,
                _ => TriggerButton::Middle,
            };
            let process = if window_class.trim().is_empty() {
                None
            } else {
                Some(FocusedProcess {
                    app_name: window_class.clone(),
                    process_id: 0,
                    process_path: window_class,
                })
            };
            let _ = tx_start.send(GlobalMouseEvent::GestureStart {
                screen_pos: (x as f32, y as f32),
                button,
                process,
            });
            true
        },
    )
    .map_err(|err| err.to_string())?;

    let tx_move = tx.clone();
    conn.add_match(
        MatchRule::new_signal(KDE_BRIDGE_INTERFACE, "GestureMove"),
        move |(x, y): (f64, f64), _, _| {
            let _ = tx_move.send(GlobalMouseEvent::GestureMove {
                screen_pos: (x as f32, y as f32),
            });
            true
        },
    )
    .map_err(|err| err.to_string())?;

    conn.add_match(
        MatchRule::new_signal(KDE_BRIDGE_INTERFACE, "GestureEnd"),
        move |(x, y): (f64, f64), _, _| {
            let _ = tx.send(GlobalMouseEvent::GestureEnd {
                screen_pos: (x as f32, y as f32),
            });
            true
        },
    )
    .map_err(|err| err.to_string())?;

    loop {
        conn.process(Duration::from_millis(250))
            .map_err(|err| err.to_string())?;
    }
}

#[cfg(target_os = "linux")]
fn detect_linux_session_kind() -> LinuxSessionKind {
    match std::env::var("XDG_SESSION_TYPE") {
        Ok(value) if value.eq_ignore_ascii_case("x11") => LinuxSessionKind::X11,
        Ok(value) if value.eq_ignore_ascii_case("wayland") => LinuxSessionKind::Wayland,
        _ => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                LinuxSessionKind::Wayland
            } else if std::env::var("DISPLAY").is_ok() {
                LinuxSessionKind::X11
            } else {
                LinuxSessionKind::Unknown
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn is_kde_desktop() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|value| value.to_ascii_lowercase().contains("kde"))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn grab_button(
    conn: &xcb::Connection,
    root: xcb::x::Window,
    button: TriggerButton,
) -> Result<(), String> {
    use xcb::x;

    let cookie = conn.send_request_checked(&x::GrabButton {
        owner_events: false,
        grab_window: root,
        event_mask: x::EventMask::BUTTON_PRESS
            | x::EventMask::BUTTON_RELEASE
            | x::EventMask::POINTER_MOTION
            | x::EventMask::BUTTON_MOTION,
        pointer_mode: x::GrabMode::Async,
        keyboard_mode: x::GrabMode::Async,
        confine_to: x::Window::none(),
        cursor: x::Cursor::none(),
        button: trigger_button_code(button),
        modifiers: x::ModMask::ANY,
    });
    conn.check_request(cookie).map_err(|err| err.to_string())
}

#[cfg(target_os = "linux")]
fn ungrab_button(
    conn: &xcb::Connection,
    root: xcb::x::Window,
    button: TriggerButton,
) -> Result<(), String> {
    use xcb::x;

    let cookie = conn.send_request_checked(&x::UngrabButton {
        button: trigger_button_code(button),
        grab_window: root,
        modifiers: x::ModMask::ANY,
    });
    conn.check_request(cookie).map_err(|err| err.to_string())
}

#[cfg(target_os = "linux")]
fn matches_button(detail: u8, button: TriggerButton) -> bool {
    detail == trigger_button_code(button) as u8
}

#[cfg(target_os = "linux")]
fn trigger_button_code(button: TriggerButton) -> x::ButtonIndex {
    match button {
        TriggerButton::Middle => x::ButtonIndex::N3,
        TriggerButton::Right => x::ButtonIndex::N2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use std::ffi::OsString;
    #[cfg(target_os = "linux")]
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn process(name: &str, path: &str) -> FocusedProcess {
        FocusedProcess {
            app_name: name.into(),
            process_id: 42,
            process_path: path.into(),
        }
    }

    #[test]
    fn browser_processes_use_right_button() {
        assert_eq!(
            trigger_button_for_process(Some(&process("Firefox", "/usr/bin/firefox"))),
            TriggerButton::Right
        );
        assert_eq!(
            trigger_button_for_process(Some(&process("Chrome", "/opt/google/chrome/chrome"))),
            TriggerButton::Right
        );
    }

    #[test]
    fn non_browser_processes_use_middle_button() {
        assert_eq!(
            trigger_button_for_process(Some(&process(
                "WPS",
                "/opt/kingsoft/wps-office/office6/wps"
            ))),
            TriggerButton::Middle
        );
        assert_eq!(trigger_button_for_process(None), TriggerButton::Middle);
    }

    #[cfg(target_os = "linux")]
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[cfg(target_os = "linux")]
    struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    #[cfg(target_os = "linux")]
    impl EnvGuard {
        fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let lock = env_lock().lock().expect("env test mutex poisoned");
            let mut saved = Vec::new();
            for (key, value) in pairs {
                saved.push((*key, std::env::var_os(key)));
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            Self { _lock: lock, saved }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_linux_session_kind_prefers_declared_session_type() {
        let _guard = EnvGuard::set(&[
            ("XDG_SESSION_TYPE", Some("wayland")),
            ("WAYLAND_DISPLAY", None),
            ("DISPLAY", Some(":1")),
        ]);

        assert_eq!(detect_linux_session_kind(), LinuxSessionKind::Wayland);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_linux_session_kind_falls_back_to_display_variables() {
        let _guard = EnvGuard::set(&[
            ("XDG_SESSION_TYPE", None),
            ("WAYLAND_DISPLAY", None),
            ("DISPLAY", Some(":1")),
        ]);

        assert_eq!(detect_linux_session_kind(), LinuxSessionKind::X11);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn is_kde_desktop_detects_kde_current_desktop() {
        let _guard = EnvGuard::set(&[("XDG_CURRENT_DESKTOP", Some("KDE"))]);
        assert!(is_kde_desktop());
    }
}
