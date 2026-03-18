use std::collections::BTreeSet;
use std::path::Path;

pub const BROWSER_PROCESS_PATTERNS: &[&str] = &[
    "chrome",
    "chromium",
    "firefox",
    "msedge",
    "edge",
    "brave-browser",
    "opera",
    "vivaldi",
    "zen",
    "safari",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedProcess {
    pub app_name: String,
    pub process_id: u32,
    pub process_path: String,
}

impl FocusedProcess {
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        let Some(pattern) = normalize_process_name(pattern) else {
            return false;
        };

        self.aliases().iter().any(|alias| alias == &pattern)
    }

    pub fn primary_alias(&self) -> String {
        self.aliases()
            .into_iter()
            .next()
            .unwrap_or_else(|| "unknown".into())
    }

    pub fn display_name(&self) -> String {
        if !self.app_name.trim().is_empty() {
            return self.app_name.trim().to_string();
        }

        Path::new(&self.process_path)
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".into())
    }

    fn aliases(&self) -> Vec<String> {
        let mut aliases = BTreeSet::new();

        push_alias(&mut aliases, &self.app_name);

        let process_path = Path::new(&self.process_path);
        if let Some(name) = process_path.file_name().and_then(|name| name.to_str()) {
            push_alias(&mut aliases, name);
        }
        if let Some(name) = process_path.file_stem().and_then(|name| name.to_str()) {
            push_alias(&mut aliases, name);
        }

        aliases.into_iter().collect()
    }
}

#[derive(Debug, Clone)]
pub struct FocusTracker {
    self_process_id: u32,
    last_external_process: Option<FocusedProcess>,
}

impl FocusTracker {
    pub fn new(self_process_id: u32) -> Self {
        Self {
            self_process_id,
            last_external_process: None,
        }
    }

    pub fn observe(&mut self, process: Option<FocusedProcess>) -> bool {
        let Some(process) = process else {
            return false;
        };

        if process.process_id == self.self_process_id {
            return false;
        }

        let changed = self.last_external_process.as_ref() != Some(&process);
        if changed {
            self.last_external_process = Some(process);
        }

        changed
    }

    pub fn current_external(&self) -> Option<&FocusedProcess> {
        self.last_external_process.as_ref()
    }
}

pub fn normalize_process_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    Some(
        normalized
            .strip_suffix(".exe")
            .unwrap_or(&normalized)
            .to_string(),
    )
}

fn push_alias(aliases: &mut BTreeSet<String>, value: &str) {
    let Some(alias) = normalize_process_name(value) else {
        return;
    };
    aliases.insert(alias);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn detect_focused_process() -> Option<FocusedProcess> {
    let active_window = active_win_pos_rs::get_active_window().ok()?;

    Some(FocusedProcess {
        app_name: active_window.app_name,
        process_id: u32::try_from(active_window.process_id).ok()?,
        process_path: active_window.process_path.to_string_lossy().to_string(),
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn detect_focused_process() -> Option<FocusedProcess> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(app_name: &str, process_id: u32, process_path: &str) -> FocusedProcess {
        FocusedProcess {
            app_name: app_name.into(),
            process_id,
            process_path: process_path.into(),
        }
    }

    #[test]
    fn matches_pattern_is_case_insensitive_and_ignores_exe_suffix() {
        let process = process("Code", 42, "/usr/share/code/code");

        assert!(process.matches_pattern("code"));
        assert!(process.matches_pattern("CODE"));
        assert!(process.matches_pattern("code.exe"));
    }

    #[test]
    fn primary_alias_prefers_normalized_app_name() {
        let process = process("Firefox", 42, "/usr/bin/firefox");

        assert_eq!(process.primary_alias(), "firefox");
    }

    #[test]
    fn focus_tracker_ignores_current_process() {
        let mut tracker = FocusTracker::new(7);

        assert!(!tracker.observe(Some(process("quicker-rs", 7, "/tmp/quicker-rs"))));
        assert!(tracker.current_external().is_none());
    }

    #[test]
    fn focus_tracker_remembers_last_external_process() {
        let mut tracker = FocusTracker::new(7);
        let code = process("Code", 99, "/usr/bin/code");

        assert!(tracker.observe(Some(code.clone())));
        assert_eq!(tracker.current_external(), Some(&code));
        assert!(!tracker.observe(Some(code)));
    }
}
