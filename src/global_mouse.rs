use crate::focus::{self, FocusedProcess};
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
    if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err() {
        return Err(
            "Wayland session detected: global mouse grab is not available without X11/XWayland"
                .into(),
        );
    }

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
}
