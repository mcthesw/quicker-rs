use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
))]
use arboard::{GetExtLinux, LinuxClipboardKind};

/// Represents what happens when an action is triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionKind {
    /// Launch a program (with optional arguments)
    RunProgram {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        working_dir: Option<String>,
    },
    /// Open a file with the system default handler
    OpenFile { path: String },
    /// Open a URL in the default browser
    OpenUrl { url: String },
    /// Run a shell command / script
    RunShell {
        script: String,
        /// "bash", "sh", "powershell", "cmd", etc.
        #[serde(default = "default_shell")]
        shell: String,
    },
    /// Copy text to clipboard
    CopyText { text: String },
    /// Open a directory in the file manager
    OpenFolder { path: String },
    /// Search using text currently in the clipboard
    SearchClipboardText {
        #[serde(default = "default_search_url")]
        url_template: String,
    },
    /// Open a URL or file path from clipboard text, with optional fallback search
    OpenClipboardText {
        #[serde(default)]
        fallback_search_url: Option<String>,
    },
    /// Run the clipboard text as a shell command
    RunClipboardText {
        #[serde(default = "default_shell")]
        shell: String,
    },
    /// A group/folder that contains sub-actions
    Group { actions: Vec<Action> },
}

fn default_shell() -> String {
    if cfg!(target_os = "windows") {
        "powershell".into()
    } else {
        "sh".into()
    }
}

fn default_search_url() -> String {
    "https://www.google.com/search?q={query}".into()
}

/// A single action in the launcher panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>, // emoji or icon name
    #[serde(default)]
    pub tags: Vec<String>, // for search
    #[serde(default)]
    pub hotkey: Option<String>, // e.g. "Ctrl+Shift+T"
    pub kind: ActionKind,
}

/// Result of executing an action
pub enum ExecResult {
    Ok,
    OkWithMessage(String),
    Err(String),
}

impl Action {
    /// Execute this action.
    pub fn execute(&self) -> ExecResult {
        match &self.kind {
            ActionKind::RunProgram {
                command,
                args,
                working_dir,
            } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                if let Some(dir) = working_dir {
                    cmd.current_dir(dir);
                }
                // Detach: don't wait for child
                match cmd.spawn() {
                    Ok(_) => ExecResult::Ok,
                    Err(e) => ExecResult::Err(format!("Failed to run '{}': {}", command, e)),
                }
            }

            ActionKind::OpenFile { path } | ActionKind::OpenFolder { path } => {
                match open::that(path) {
                    Ok(_) => ExecResult::Ok,
                    Err(e) => ExecResult::Err(format!("Failed to open '{}': {}", path, e)),
                }
            }

            ActionKind::OpenUrl { url } => match open::that(url) {
                Ok(_) => ExecResult::Ok,
                Err(e) => ExecResult::Err(format!("Failed to open URL '{}': {}", url, e)),
            },

            ActionKind::RunShell { script, shell } => run_shell_command(script, shell),

            ActionKind::CopyText { text } => match arboard::Clipboard::new() {
                Ok(mut cb) => match cb.set_text(text) {
                    Ok(_) => ExecResult::OkWithMessage("Copied to clipboard".into()),
                    Err(e) => ExecResult::Err(format!("Clipboard error: {}", e)),
                },
                Err(e) => ExecResult::Err(format!("Clipboard error: {}", e)),
            },

            ActionKind::SearchClipboardText { url_template } => {
                let clipboard_text = match read_clipboard_text() {
                    Ok(text) => text,
                    Err(err) => return ExecResult::Err(err),
                };
                let encoded = urlencoding::encode(&clipboard_text);
                let url = if url_template.contains("{query}") {
                    url_template.replace("{query}", encoded.as_ref())
                } else {
                    format!("{url_template}{encoded}")
                };
                match open::that(&url) {
                    Ok(_) => ExecResult::OkWithMessage(format!("Searched for: {}", clipboard_text)),
                    Err(e) => {
                        ExecResult::Err(format!("Failed to open search URL '{}': {}", url, e))
                    }
                }
            }

            ActionKind::OpenClipboardText {
                fallback_search_url,
            } => {
                let clipboard_text = match read_clipboard_text() {
                    Ok(text) => text,
                    Err(err) => return ExecResult::Err(err),
                };

                if let Some(target) = clipboard_target(&clipboard_text) {
                    match open::that(&target) {
                        Ok(_) => ExecResult::OkWithMessage(format!("Opened: {}", clipboard_text)),
                        Err(e) => ExecResult::Err(format!("Failed to open '{}': {}", target, e)),
                    }
                } else if let Some(url_template) = fallback_search_url {
                    let encoded = urlencoding::encode(&clipboard_text);
                    let url = if url_template.contains("{query}") {
                        url_template.replace("{query}", encoded.as_ref())
                    } else {
                        format!("{url_template}{encoded}")
                    };
                    match open::that(&url) {
                        Ok(_) => {
                            ExecResult::OkWithMessage(format!("Searched for: {}", clipboard_text))
                        }
                        Err(e) => ExecResult::Err(format!(
                            "Failed to open fallback search '{}': {}",
                            url, e
                        )),
                    }
                } else {
                    ExecResult::Err(
                        "Clipboard does not contain a URL or existing path, and no fallback search URL is configured"
                            .into(),
                    )
                }
            }

            ActionKind::RunClipboardText { shell } => {
                let clipboard_text = match read_clipboard_text() {
                    Ok(text) => text,
                    Err(err) => return ExecResult::Err(err),
                };
                run_shell_command(&clipboard_text, shell)
            }

            ActionKind::Group { .. } => {
                // Groups are navigated in the UI, not "executed"
                ExecResult::Ok
            }
        }
    }

    /// Return searchable text for fuzzy matching.
    pub fn search_text(&self) -> String {
        let mut parts = vec![self.name.clone(), self.description.clone()];
        parts.extend(self.tags.clone());
        if let ActionKind::Group { actions } = &self.kind {
            parts.extend(actions.iter().map(Action::search_text));
        }
        parts.join(" ")
    }
}

fn read_clipboard_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("Clipboard error: {}", e))?;

    if let Some(text) = normalize_clipboard_text(clipboard.get_text().ok()) {
        return Ok(text);
    }

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
    ))]
    {
        if let Some(text) = normalize_clipboard_text(
            clipboard
                .get()
                .clipboard(LinuxClipboardKind::Primary)
                .text()
                .ok(),
        ) {
            return Ok(text);
        }
    }

    Err(
        "No usable text was found in the clipboard. On Linux, select text first or copy it explicitly."
            .into(),
    )
}

fn normalize_clipboard_text(text: Option<String>) -> Option<String> {
    let trimmed = text?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn clipboard_target(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.into());
    }
    if Path::new(trimmed).exists() {
        return Some(trimmed.into());
    }
    if !trimmed.contains(char::is_whitespace) && trimmed.contains('.') {
        return Some(format!("https://{}", trimmed));
    }
    None
}

fn run_shell_command(script: &str, shell: &str) -> ExecResult {
    let (sh, flag) = if cfg!(target_os = "windows") {
        match shell {
            "cmd" => ("cmd", "/C"),
            _ => ("powershell", "-Command"),
        }
    } else {
        (shell, "-c")
    };

    match Command::new(sh).arg(flag).arg(script).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                let message = match (stdout.trim(), stderr.trim()) {
                    ("", "") => None,
                    ("", stderr) => Some(stderr.to_string()),
                    (stdout, "") => Some(stdout.to_string()),
                    (stdout, stderr) => Some(format!("{}\n{}", stdout, stderr)),
                };
                match message {
                    Some(message) => ExecResult::OkWithMessage(message),
                    None => ExecResult::Ok,
                }
            } else {
                ExecResult::Err(format!(
                    "Script exited with {}\n{}{}",
                    output.status, stdout, stderr
                ))
            }
        }
        Err(e) => ExecResult::Err(format!("Failed to run shell '{}': {}", shell, e)),
    }
}
