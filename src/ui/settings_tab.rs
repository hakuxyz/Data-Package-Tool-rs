// Settings tab: tokens, discord instance, search ids and the credits.

use eframe::egui::{self, Context, RichText, Ui};

use crate::app::{App, Dialog, DialogKind, DialogTag};
use crate::settings::Settings;

const ORIGINAL_REPO: &str = "https://github.com/aamiaa/Data-Package-Tool";

// Values that end up in settings.json, labels are what the user sees.
const INSTANCES: [(&str, &str); 7] = [
    ("default", "Discord (default)"),
    ("stable", "Discord"),
    ("ptb", "Discord PTB"),
    ("canary", "Discord Canary"),
    ("web_stable", "Web (stable)"),
    ("web_ptb", "Web (PTB)"),
    ("web_canary", "Web (Canary)"),
];

impl App {
    pub fn ui_settings(&mut self, ui: &mut Ui, _ctx: &Context) {
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("Tokens").heading());
                ui.label(
                    RichText::new(
                        "Tokens stay in memory for this session only, they are never saved to disk.",
                    )
                    .weak(),
                );

                ui.horizontal(|ui| {
                    ui.label("User token");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.user_token)
                            .password(true)
                            .hint_text("needed to delete messages and reopen dms")
                            .desired_width(460.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Bot token");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.bot_token)
                            .password(true)
                            .hint_text("needed to view attachments from older packages")
                            .desired_width(460.0),
                    );
                });

                ui.separator();
                ui.label(RichText::new("Discord").heading());

                ui.horizontal(|ui| {
                    ui.label("Open messages in");
                    let selected = instance_label(&self.settings.discord_instance);
                    egui::ComboBox::from_id_salt("discord_instance")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for (value, label) in INSTANCES {
                                ui.selectable_value(
                                    &mut self.settings.discord_instance,
                                    value.to_string(),
                                    label,
                                );
                            }
                        });
                });

                if ui
                    .checkbox(
                        &mut self.settings.check_for_updates,
                        "Check for updates when the app starts",
                    )
                    .changed()
                {
                    self.settings.save();
                }
                if ui.button("Check for updates now").clicked() {
                    self.check_for_updates();
                }

                ui.separator();
                ui.label(RichText::new("Search").heading());
                ui.horizontal(|ui| {
                    if ui.button("Search Options...").clicked() {
                        self.open_search_options();
                    }
                    ui.label(search_summary(&self.settings));
                });

                ui.add_space(6.0);
                ui.label(RichText::new("Excluded channels and servers").strong());
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        self.push_dialog(input_dialog(
                            "Add Excluded Id",
                            "Enter a channel or server id.",
                            DialogTag::AddExcludeId,
                        ));
                    }
                });
                if id_list(ui, &mut self.settings.exclude_ids) {
                    self.settings.save();
                }

                ui.add_space(6.0);
                ui.label(RichText::new("Only these channels and servers").strong());
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        self.push_dialog(input_dialog(
                            "Add Whitelisted Id",
                            "Enter a channel or server id.",
                            DialogTag::AddWhitelistId,
                        ));
                    }
                });
                if id_list(ui, &mut self.settings.whitelist_ids) {
                    self.settings.save();
                }

                ui.separator();
                ui.label(RichText::new("About").heading());
                ui.horizontal(|ui| {
                    ui.label("Original tool by");
                    ui.hyperlink_to("aamiaa", ORIGINAL_REPO);
                    ui.label(", this is a Rust rewrite of it.");
                });
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Settings live in {}",
                        crate::settings::settings_path().display()
                    ));
                    if ui.button("Open folder").clicked() {
                        let folder = crate::settings::settings_path()
                            .parent()
                            .map(|p| p.to_path_buf());
                        if let Some(folder) = folder {
                            if let Err(e) = open::that(&folder) {
                                self.push_dialog(Dialog::error(format!(
                                    "Couldn't open the folder: {}",
                                    e
                                )));
                            }
                        }
                    }
                });
            });
    }
}

fn input_dialog(title: &str, text: &str, tag: DialogTag) -> Dialog {
    Dialog {
        title: title.to_string(),
        text: text.to_string(),
        kind: DialogKind::Input,
        tag,
        input: String::new(),
    }
}

/// Shows the id list with a remove button on each line, true if it changed.
fn id_list(ui: &mut Ui, ids: &mut Vec<String>) -> bool {
    if ids.is_empty() {
        ui.label(RichText::new("(empty)").weak());
        return false;
    }

    let mut remove: Option<usize> = None;
    for (i, id) in ids.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(id);
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }

    match remove {
        Some(i) => {
            ids.remove(i);
            true
        }
        None => false,
    }
}

fn instance_label(value: &str) -> String {
    INSTANCES
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| "Discord (default)".to_string())
}

fn search_summary(settings: &Settings) -> String {
    let mode = match settings.search_mode.as_str() {
        "words" => "words",
        "regex" => "regex",
        _ => "exact",
    };
    format!(
        "{} search, {} first, {} excluded, {} whitelisted",
        mode,
        if settings.sort_mode == "asc" {
            "oldest"
        } else {
            "newest"
        },
        settings.exclude_ids.len(),
        settings.whitelist_ids.len()
    )
}
