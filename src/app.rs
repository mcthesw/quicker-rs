use crate::action::{Action, ActionKind, ExecResult};
use crate::config::Config;
use crate::focus::{self, FocusTracker};
use crate::search::SearchEngine;
use eframe::egui;
use std::time::{Duration, Instant};

const FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(800);

/// Notification that disappears after a timeout.
struct Toast {
    message: String,
    is_error: bool,
    expires: std::time::Instant,
}

/// Which screen/view is active.
#[derive(Default, PartialEq)]
enum View {
    #[default]
    Panel,
    Settings,
    ActionEditor,
    ScriptOutput,
}

pub struct QuickerApp {
    config: Config,
    search: SearchEngine,
    query: String,
    active_profile: usize,
    focus_tracker: FocusTracker,
    last_focus_poll: Instant,
    needs_focus_profile_sync: bool,
    group_path: Vec<usize>,
    view: View,
    toast: Option<Toast>,
    script_output: String,

    // Action editor state
    edit_name: String,
    edit_desc: String,
    edit_icon: String,
    edit_tags: String,
    edit_kind_idx: usize,
    edit_field1: String, // command / path / url / script / text / template
    edit_field2: String, // args / shell / fallback url
    edit_field3: String, // working_dir
}

impl QuickerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Style: make it look clean
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.visuals.window_corner_radius = egui::CornerRadius::same(8);
        style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);
        style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
        style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
        style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
        cc.egui_ctx.set_style(style);

        Self {
            config: Config::load(),
            search: SearchEngine::new(),
            query: String::new(),
            active_profile: 0,
            focus_tracker: FocusTracker::new(std::process::id()),
            last_focus_poll: Instant::now() - FOCUS_POLL_INTERVAL,
            needs_focus_profile_sync: true,
            group_path: Vec::new(),
            view: View::Panel,
            toast: None,
            script_output: String::new(),
            edit_name: String::new(),
            edit_desc: String::new(),
            edit_icon: String::new(),
            edit_tags: String::new(),
            edit_kind_idx: 0,
            edit_field1: String::new(),
            edit_field2: String::new(),
            edit_field3: String::new(),
        }
    }

    fn show_toast(&mut self, msg: String, is_error: bool) {
        self.toast = Some(Toast {
            message: msg,
            is_error,
            expires: std::time::Instant::now() + std::time::Duration::from_secs(3),
        });
    }

    fn execute_action(&mut self, action: &Action) {
        match action.execute() {
            ExecResult::Ok => {
                self.show_toast(format!("✓ {}", action.name), false);
            }
            ExecResult::OkWithMessage(msg) => {
                if msg.len() > 100 {
                    self.script_output = msg;
                    self.view = View::ScriptOutput;
                } else {
                    self.show_toast(msg, false);
                }
            }
            ExecResult::Err(e) => {
                self.show_toast(e, true);
            }
        }
    }

    fn set_active_profile(&mut self, profile_idx: usize) {
        if self.active_profile != profile_idx {
            self.active_profile = profile_idx;
            self.group_path.clear();
            self.query.clear();
        }
    }

    fn poll_focused_process(&mut self) {
        if self.last_focus_poll.elapsed() < FOCUS_POLL_INTERVAL {
            return;
        }

        self.last_focus_poll = Instant::now();
        if self.focus_tracker.observe(focus::detect_focused_process()) {
            self.needs_focus_profile_sync = true;
        }
    }

    fn sync_profile_to_focus(&mut self) {
        let Some(process) = self.focus_tracker.current_external().cloned() else {
            return;
        };

        let profile_idx = self.config.matching_profile_index(&process).unwrap_or(0);
        self.set_active_profile(profile_idx);
    }

    fn reset_editor(&mut self) {
        self.edit_name.clear();
        self.edit_desc.clear();
        self.edit_icon.clear();
        self.edit_tags.clear();
        self.edit_kind_idx = 0;
        self.edit_field1.clear();
        self.edit_field2.clear();
        self.edit_field3.clear();
    }

    fn current_profile_actions(&self) -> &[Action] {
        self.config
            .profiles
            .get(self.active_profile)
            .map(|p| p.actions.as_slice())
            .unwrap_or(&[])
    }

    fn actions_at_path<'a>(actions: &'a [Action], path: &[usize]) -> Option<&'a [Action]> {
        let mut current = actions;
        for &idx in path {
            let action = current.get(idx)?;
            match &action.kind {
                ActionKind::Group { actions } => current = actions,
                _ => return None,
            }
        }
        Some(current)
    }

    fn actions_at_path_mut<'a>(
        actions: &'a mut Vec<Action>,
        path: &[usize],
    ) -> Option<&'a mut Vec<Action>> {
        if let Some((&idx, rest)) = path.split_first() {
            let action = actions.get_mut(idx)?;
            match &mut action.kind {
                ActionKind::Group { actions } => Self::actions_at_path_mut(actions, rest),
                _ => None,
            }
        } else {
            Some(actions)
        }
    }

    fn current_actions(&self) -> &[Action] {
        Self::actions_at_path(self.current_profile_actions(), &self.group_path)
            .unwrap_or_else(|| self.current_profile_actions())
    }

    fn current_actions_mut(&mut self) -> Option<&mut Vec<Action>> {
        let path = self.group_path.clone();
        let profile = self.config.profiles.get_mut(self.active_profile)?;
        Self::actions_at_path_mut(&mut profile.actions, &path)
    }

    fn group_titles(&self) -> Vec<String> {
        let mut titles = Vec::new();
        let mut current = self.current_profile_actions();
        for &idx in &self.group_path {
            let Some(action) = current.get(idx) else {
                break;
            };
            titles.push(action.name.clone());
            match &action.kind {
                ActionKind::Group { actions } => current = actions,
                _ => break,
            }
        }
        titles
    }

    fn open_group(&mut self, action_idx: usize) {
        self.group_path.push(action_idx);
        self.query.clear();
    }

    fn leave_group(&mut self) {
        self.group_path.pop();
        self.query.clear();
    }

    fn render_panel(&mut self, ui: &mut egui::Ui) {
        // --- Profile tabs ---
        let mut clicked_profile = None;
        ui.horizontal(|ui| {
            for (i, profile) in self.config.profiles.iter().enumerate() {
                let selected = i == self.active_profile;
                if ui.selectable_label(selected, &profile.name).clicked() {
                    clicked_profile = Some(i);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    self.view = View::Settings;
                }
                if ui.button("＋").on_hover_text("Add action").clicked() {
                    self.reset_editor();
                    self.view = View::ActionEditor;
                }
            });
        });
        if let Some(profile_idx) = clicked_profile {
            self.set_active_profile(profile_idx);
            self.needs_focus_profile_sync = false;
        }

        ui.separator();

        if !self.group_path.is_empty() {
            ui.horizontal_wrapped(|ui| {
                if ui.button("← Back").clicked() {
                    self.leave_group();
                }
                ui.label(egui::RichText::new("Root").weak());
                for title in self.group_titles() {
                    ui.label(egui::RichText::new("›").weak());
                    ui.label(title);
                }
            });
            ui.add_space(4.0);
        }

        // --- Search bar ---
        let search_response = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("🔍 Search actions...")
                .desired_width(f32::INFINITY),
        );
        // Auto-focus on startup
        if ui.memory(|m| m.focused().is_none()) {
            search_response.request_focus();
        }

        ui.add_space(4.0);

        // --- Action grid ---
        let actions = self.current_actions().to_vec();
        let results = self.search.search(&self.query, &actions);
        let cols = self.config.columns;

        if results.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("No actions found").size(16.0).weak());
                ui.add_space(8.0);
                ui.label("Add some actions or adjust your search.");
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut clicked_action: Option<(usize, Action)> = None;
                egui::Grid::new("action_grid")
                    .num_columns(cols)
                    .spacing([8.0, 8.0])
                    .min_col_width(ui.available_width() / cols as f32 - 8.0)
                    .show(ui, |ui| {
                        for (i, (_score, action_idx, action)) in results.iter().enumerate() {
                            if i > 0 && i % cols == 0 {
                                ui.end_row();
                            }
                            let btn_width = ui
                                .available_width()
                                .min((self.config.panel_width - 32.0) / cols as f32 - 8.0);

                            let default_icon = match &action.kind {
                                ActionKind::Group { .. } => "📂",
                                _ => "▶",
                            };
                            let icon = action.icon.as_deref().unwrap_or(default_icon);
                            let label = match &action.kind {
                                ActionKind::Group { .. } => format!("{} {} ›", icon, action.name),
                                _ => format!("{} {}", icon, action.name),
                            };

                            let btn = egui::Button::new(egui::RichText::new(&label).size(14.0))
                                .min_size(egui::vec2(btn_width, 48.0));

                            let response = ui.add(btn);

                            if !action.description.is_empty() {
                                response.clone().on_hover_text(&action.description);
                            }

                            if response.clicked() {
                                clicked_action = Some((*action_idx, (*action).clone()));
                            }
                        }
                        // If only one result and Enter pressed, run it
                        if results.len() == 1 && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            clicked_action = Some((results[0].1, results[0].2.clone()));
                        }
                    });

                // Execute outside the borrow
                if let Some((action_idx, action)) = clicked_action {
                    match action.kind {
                        ActionKind::Group { .. } => self.open_group(action_idx),
                        _ => self.execute_action(&action),
                    }
                }
            });
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                self.view = View::Panel;
                self.needs_focus_profile_sync = true;
            }
            ui.heading("Settings");
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let current_process = self.focus_tracker.current_external().cloned();
            let current_process_alias = current_process.as_ref().map(|process| process.primary_alias());
            let current_process_label = current_process
                .as_ref()
                .map(|process| process.display_name())
                .unwrap_or_else(|| "Unavailable".into());
            let mut profile_rules_changed = false;

            ui.label("Toggle Hotkey:");
            ui.text_edit_singleline(&mut self.config.toggle_hotkey);
            ui.add_space(8.0);

            ui.label("Grid Columns:");
            ui.add(egui::Slider::new(&mut self.config.columns, 2..=8));
            ui.add_space(8.0);

            ui.label("Panel Width:");
            ui.add(egui::Slider::new(
                &mut self.config.panel_width,
                300.0..=1200.0,
            ));

            ui.label("Panel Height:");
            ui.add(egui::Slider::new(
                &mut self.config.panel_height,
                200.0..=900.0,
            ));

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            ui.heading("Profiles");
            ui.label(format!("Current focused process: {}", current_process_label));
            ui.label(
                egui::RichText::new(
                    "Profiles with match rules auto-activate. The first profile is the fallback when nothing matches.",
                )
                .weak()
                .small(),
            );
            ui.add_space(8.0);

            let mut to_delete: Option<usize> = None;
            let can_delete_profiles = self.config.profiles.len() > 1;
            for (i, profile) in self.config.profiles.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut profile.name);
                    ui.label(format!("({} actions)", profile.actions.len()));
                    if let Some(alias) = current_process_alias.as_deref() {
                        if ui.small_button(format!("Use {}", alias)).clicked()
                            && !profile
                                .match_processes
                                .iter()
                                .any(|value| value.eq_ignore_ascii_case(alias))
                        {
                            profile.match_processes.push(alias.into());
                            profile_rules_changed = true;
                        }
                    }
                    if can_delete_profiles {
                        if ui.small_button("🗑").clicked() {
                            to_delete = Some(i);
                        }
                    }
                });
                let mut process_matches = profile.match_processes.join(", ");
                ui.label("Match focused processes (comma-separated):");
                if ui.text_edit_singleline(&mut process_matches).changed() {
                    profile.match_processes = process_matches
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect();
                    profile_rules_changed = true;
                }
                ui.add_space(8.0);
            }
            if let Some(idx) = to_delete {
                self.config.profiles.remove(idx);
                if self.active_profile >= self.config.profiles.len() {
                    self.active_profile = 0;
                }
                self.needs_focus_profile_sync = true;
            }
            if ui.button("Add Profile").clicked() {
                self.config.profiles.push(crate::config::Profile {
                    name: format!("Profile {}", self.config.profiles.len() + 1),
                    description: String::new(),
                    match_processes: vec![],
                    actions: vec![],
                });
                self.needs_focus_profile_sync = true;
            }

            if profile_rules_changed {
                self.needs_focus_profile_sync = true;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("💾 Save Config").clicked() {
                    self.config.save();
                    self.show_toast("Config saved!".into(), false);
                    self.needs_focus_profile_sync = true;
                }
                ui.label(
                    egui::RichText::new(format!("Config: {}", Config::config_path().display()))
                        .weak()
                        .small(),
                );
            });
        });
    }

    fn render_action_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("← Cancel").clicked() {
                self.view = View::Panel;
                self.needs_focus_profile_sync = true;
            }
            ui.heading("Add Action");
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.edit_name);

            ui.label("Description:");
            ui.text_edit_singleline(&mut self.edit_desc);

            ui.label("Icon (emoji):");
            ui.text_edit_singleline(&mut self.edit_icon);

            ui.label("Tags (comma-separated):");
            ui.text_edit_singleline(&mut self.edit_tags);

            ui.add_space(8.0);
            ui.label("Action Type:");

            let kinds = [
                "Run Program",
                "Open File",
                "Open URL",
                "Run Shell Script",
                "Copy Text",
                "Open Folder",
                "Group",
                "Search Clipboard Text",
                "Open Clipboard Text",
                "Run Clipboard Text",
            ];
            egui::ComboBox::from_id_salt("action_kind")
                .selected_text(kinds[self.edit_kind_idx])
                .show_ui(ui, |ui| {
                    for (i, kind) in kinds.iter().enumerate() {
                        ui.selectable_value(&mut self.edit_kind_idx, i, *kind);
                    }
                });

            ui.add_space(8.0);

            match self.edit_kind_idx {
                0 => {
                    ui.label("Command / executable path:");
                    ui.text_edit_singleline(&mut self.edit_field1);
                    ui.label("Arguments (space-separated):");
                    ui.text_edit_singleline(&mut self.edit_field2);
                    ui.label("Working directory (optional):");
                    ui.text_edit_singleline(&mut self.edit_field3);
                }
                1 => {
                    ui.label("File path:");
                    ui.text_edit_singleline(&mut self.edit_field1);
                }
                2 => {
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut self.edit_field1);
                }
                3 => {
                    ui.label("Shell (sh, bash, powershell, cmd):");
                    ui.text_edit_singleline(&mut self.edit_field2);
                    ui.label("Script:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.edit_field1)
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .code_editor(),
                    );
                }
                4 => {
                    ui.label("Text to copy:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.edit_field1)
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                }
                5 => {
                    ui.label("Folder path:");
                    ui.text_edit_singleline(&mut self.edit_field1);
                }
                6 => {
                    ui.label("Creates an empty group. Open it from the panel and add child actions inside it.");
                }
                7 => {
                    ui.label("Search URL template:");
                    ui.text_edit_singleline(&mut self.edit_field1);
                    ui.label("Use {query} where clipboard text should be inserted.");
                }
                8 => {
                    ui.label("Fallback search URL template (optional):");
                    ui.text_edit_singleline(&mut self.edit_field2);
                    ui.label("If clipboard is not a URL/path, use {query} in this template to search it.");
                }
                9 => {
                    ui.label("Shell (sh, bash, powershell, cmd):");
                    ui.text_edit_singleline(&mut self.edit_field2);
                    ui.label("Runs the current clipboard text as a command.");
                }
                _ => {}
            }

            ui.add_space(16.0);

            if ui.button("✓ Save Action").clicked() && !self.edit_name.is_empty() {
                let kind = match self.edit_kind_idx {
                    0 => ActionKind::RunProgram {
                        command: self.edit_field1.clone(),
                        args: self
                            .edit_field2
                            .split_whitespace()
                            .map(String::from)
                            .collect(),
                        working_dir: if self.edit_field3.is_empty() {
                            None
                        } else {
                            Some(self.edit_field3.clone())
                        },
                    },
                    1 => ActionKind::OpenFile {
                        path: self.edit_field1.clone(),
                    },
                    2 => ActionKind::OpenUrl {
                        url: self.edit_field1.clone(),
                    },
                    3 => ActionKind::RunShell {
                        script: self.edit_field1.clone(),
                        shell: if self.edit_field2.is_empty() {
                            "sh".into()
                        } else {
                            self.edit_field2.clone()
                        },
                    },
                    4 => ActionKind::CopyText {
                        text: self.edit_field1.clone(),
                    },
                    5 => ActionKind::OpenFolder {
                        path: self.edit_field1.clone(),
                    },
                    6 => ActionKind::Group { actions: vec![] },
                    7 => ActionKind::SearchClipboardText {
                        url_template: if self.edit_field1.is_empty() {
                            "https://www.google.com/search?q={query}".into()
                        } else {
                            self.edit_field1.clone()
                        },
                    },
                    8 => ActionKind::OpenClipboardText {
                        fallback_search_url: if self.edit_field2.is_empty() {
                            Some("https://www.google.com/search?q={query}".into())
                        } else {
                            Some(self.edit_field2.clone())
                        },
                    },
                    9 => ActionKind::RunClipboardText {
                        shell: if self.edit_field2.is_empty() {
                            "sh".into()
                        } else {
                            self.edit_field2.clone()
                        },
                    },
                    _ => unreachable!(),
                };

                let action = Action {
                    name: self.edit_name.clone(),
                    description: self.edit_desc.clone(),
                    icon: if self.edit_icon.is_empty() {
                        None
                    } else {
                        Some(self.edit_icon.clone())
                    },
                    tags: self
                        .edit_tags
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                    hotkey: None,
                    kind,
                };

                if let Some(actions) = self.current_actions_mut() {
                    actions.push(action);
                }
                self.config.save();
                self.show_toast("Action added!".into(), false);
                self.view = View::Panel;
                self.needs_focus_profile_sync = true;
            }
        });
    }

    fn render_script_output(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                self.view = View::Panel;
                self.needs_focus_profile_sync = true;
            }
            ui.heading("Script Output");
            if ui.button("📋 Copy").clicked() {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&self.script_output);
                    self.show_toast("Copied!".into(), false);
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.script_output.as_str())
                    .desired_width(f32::INFINITY)
                    .code_editor(),
            );
        });
    }

    fn render_toast(&mut self, ctx: &egui::Context) {
        if let Some(toast) = &self.toast {
            if std::time::Instant::now() > toast.expires {
                self.toast = None;
                return;
            }
            egui::Area::new(egui::Id::new("toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -20.0))
                .show(ctx, |ui| {
                    let color = if toast.is_error {
                        egui::Color32::from_rgb(220, 50, 50)
                    } else {
                        egui::Color32::from_rgb(50, 160, 80)
                    };
                    egui::Frame::new()
                        .fill(color)
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(16, 8))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&toast.message)
                                    .color(egui::Color32::WHITE)
                                    .size(14.0),
                            );
                        });
                });
            ctx.request_repaint();
        }
    }
}

impl eframe::App for QuickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.view != View::Panel {
                self.view = View::Panel;
                self.needs_focus_profile_sync = true;
            } else if !self.group_path.is_empty() {
                self.leave_group();
            }
        }

        self.poll_focused_process();
        if self.view == View::Panel && self.needs_focus_profile_sync {
            self.sync_profile_to_focus();
            self.needs_focus_profile_sync = false;
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.view {
            View::Panel => self.render_panel(ui),
            View::Settings => self.render_settings(ui),
            View::ActionEditor => self.render_action_editor(ui),
            View::ScriptOutput => self.render_script_output(ui),
        });

        self.render_toast(ctx);
        ctx.request_repaint_after(FOCUS_POLL_INTERVAL);
    }
}
