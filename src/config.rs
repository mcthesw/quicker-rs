use crate::action::{Action, ActionKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A profile is a named set of actions (like Quicker's "scenes").
/// You can have a default profile and app-specific profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// If set, this profile activates when one of these process names is focused.
    #[serde(default)]
    pub match_processes: Vec<String>,
    pub actions: Vec<Action>,
}

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global hotkey to toggle the panel (e.g. "Super+Space")
    #[serde(default = "default_toggle_hotkey")]
    pub toggle_hotkey: String,

    /// Number of columns in the action grid
    #[serde(default = "default_columns")]
    pub columns: usize,

    /// Panel width
    #[serde(default = "default_width")]
    pub panel_width: f32,

    /// Panel height
    #[serde(default = "default_height")]
    pub panel_height: f32,

    /// All profiles
    pub profiles: Vec<Profile>,
}

fn default_toggle_hotkey() -> String {
    "Alt+Space".into()
}
fn default_columns() -> usize {
    4
}
fn default_width() -> f32 {
    600.0
}
fn default_height() -> f32 {
    500.0
}

impl Config {
    /// Path to the config file.
    pub fn config_path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("quicker-rs");
        std::fs::create_dir_all(&dir).ok();
        dir.join("config.toml")
    }

    /// Load config from disk, or create a default one.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(cfg) => {
                        tracing::info!("Loaded config from {}", path.display());
                        return cfg;
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse config: {e}");
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read config: {e}");
                }
            }
        }
        let cfg = Self::default();
        cfg.save();
        cfg
    }

    /// Save config to disk.
    pub fn save(&self) {
        let path = Self::config_path();
        match toml::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::error!("Failed to write config: {e}");
                } else {
                    tracing::info!("Saved config to {}", path.display());
                }
            }
            Err(e) => tracing::error!("Failed to serialize config: {e}"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            toggle_hotkey: default_toggle_hotkey(),
            columns: default_columns(),
            panel_width: default_width(),
            panel_height: default_height(),
            profiles: vec![Profile {
                name: "Default".into(),
                description: "General-purpose actions".into(),
                match_processes: vec![],
                actions: example_actions(),
            }],
        }
    }
}

/// Starter actions so the panel isn't empty on first launch.
fn example_actions() -> Vec<Action> {
    let mut actions = vec![];

    // Cross-platform examples
    actions.push(Action {
        name: "Terminal".into(),
        description: "Open a terminal emulator".into(),
        icon: Some("🖥".into()),
        tags: vec!["shell".into(), "console".into(), "term".into()],
        hotkey: None,
        kind: ActionKind::RunProgram {
            command: if cfg!(target_os = "windows") {
                "wt".into() // Windows Terminal
            } else if cfg!(target_os = "macos") {
                "/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal".into()
            } else {
                // Try common Linux terminals
                which::which("kitty")
                    .or_else(|_| which::which("alacritty"))
                    .or_else(|_| which::which("gnome-terminal"))
                    .or_else(|_| which::which("konsole"))
                    .or_else(|_| which::which("xterm"))
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "xterm".into())
            },
            args: vec![],
            working_dir: None,
        },
    });

    actions.push(Action {
        name: "File Manager".into(),
        description: "Open home directory".into(),
        icon: Some("📁".into()),
        tags: vec!["files".into(), "explorer".into(), "nautilus".into()],
        hotkey: None,
        kind: ActionKind::OpenFolder {
            path: dirs::home_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        },
    });

    actions.push(Action {
        name: "Web Browser".into(),
        description: "Open default browser".into(),
        icon: Some("🌐".into()),
        tags: vec!["browser".into(), "firefox".into(), "chrome".into()],
        hotkey: None,
        kind: ActionKind::OpenUrl {
            url: "https://google.com".into(),
        },
    });

    actions.push(Action {
        name: "System Info".into(),
        description: "Show basic system information".into(),
        icon: Some("ℹ️".into()),
        tags: vec!["system".into(), "info".into(), "uname".into()],
        hotkey: None,
        kind: ActionKind::RunShell {
            script: if cfg!(target_os = "windows") {
                "systeminfo | Select-Object -First 20".into()
            } else {
                "uname -a && echo '---' && uptime && echo '---' && free -h 2>/dev/null || vm_stat 2>/dev/null".into()
            },
            shell: default_shell(),
        },
    });

    actions.push(Action {
        name: "IP Address".into(),
        description: "Show network IP addresses".into(),
        icon: Some("📡".into()),
        tags: vec!["ip".into(), "network".into(), "address".into()],
        hotkey: None,
        kind: ActionKind::RunShell {
            script: if cfg!(target_os = "windows") {
                "ipconfig | findstr IPv4".into()
            } else {
                "ip -brief addr 2>/dev/null || ifconfig 2>/dev/null | grep inet".into()
            },
            shell: default_shell(),
        },
    });

    actions.push(Action {
        name: "Clipboard History".into(),
        description: "Copy a useful snippet".into(),
        icon: Some("📋".into()),
        tags: vec!["clipboard".into(), "copy".into()],
        hotkey: None,
        kind: ActionKind::CopyText {
            text: "Hello from Quicker-RS!".into(),
        },
    });

    actions
}

fn default_shell() -> String {
    if cfg!(target_os = "windows") {
        "powershell".into()
    } else {
        "sh".into()
    }
}
