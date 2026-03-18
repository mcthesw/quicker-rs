use serde::{Deserialize, Serialize};
use std::process::Command;

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

            ActionKind::RunShell { script, shell } => {
                let (sh, flag) = if cfg!(target_os = "windows") {
                    match shell.as_str() {
                        "cmd" => ("cmd", "/C"),
                        _ => ("powershell", "-Command"),
                    }
                } else {
                    (shell.as_str(), "-c")
                };

                match Command::new(sh).arg(flag).arg(script).output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if output.status.success() {
                            if stdout.trim().is_empty() {
                                ExecResult::Ok
                            } else {
                                ExecResult::OkWithMessage(stdout)
                            }
                        } else {
                            ExecResult::Err(format!(
                                "Script exited with {}\n{}{}",
                                output.status, stdout, stderr
                            ))
                        }
                    }
                    Err(e) => ExecResult::Err(format!("Failed to run shell: {}", e)),
                }
            }

            ActionKind::CopyText { text } => match arboard::Clipboard::new() {
                Ok(mut cb) => match cb.set_text(text) {
                    Ok(_) => ExecResult::OkWithMessage("Copied to clipboard".into()),
                    Err(e) => ExecResult::Err(format!("Clipboard error: {}", e)),
                },
                Err(e) => ExecResult::Err(format!("Clipboard error: {}", e)),
            },

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
        parts.join(" ")
    }
}
