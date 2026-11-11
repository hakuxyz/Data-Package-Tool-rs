// Servers tab: the joined servers list from the activity folder.

use eframe::egui::{self, Color32, Context, RichText, Ui};

use crate::app::{Act, App};

use super::cell;

impl App {
    pub fn ui_servers(&mut self, ui: &mut Ui, _ctx: &Context) {
        if self.guild_rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "No servers found. This list comes from the activity folder of the package.",
                );
            });
            return;
        }

        let missing = self
            .guild_data
            .as_ref()
            .and_then(|g| g.activity_status.missing_data)
            .unwrap_or(false);
        let count = self.guild_rows.len();
        let mut selected = self.selected_guild;
        let mut acts = Vec::new();

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} joined servers", count)).strong());
                if missing {
                    ui.label(
                        RichText::new("activity data is incomplete, older joins may be missing")
                            .weak()
                            .color(Color32::from_rgb(240, 180, 90)),
                    );
                }
            });

            // Column headers
            ui.horizontal(|ui| {
                cell(ui, 96.0, |ui| {
                    ui.label(RichText::new("Date").strong());
                });
                cell(ui, 190.0, |ui| {
                    ui.label(RichText::new("Id").strong());
                });
                cell(ui, 230.0, |ui| {
                    ui.label(RichText::new("Name").strong());
                });
                cell(ui, 130.0, |ui| {
                    ui.label(RichText::new("Join Type").strong());
                });
                cell(ui, 170.0, |ui| {
                    ui.label(RichText::new("Location").strong());
                });
                ui.label(RichText::new("Invites").strong());
            });

            let height = (ui.available_height() - 64.0).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("servers")
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, |ui| {
                    for (i, row) in self.guild_rows.iter().enumerate() {
                        let fill = if selected == Some(i) {
                            Color32::from_rgb(58, 63, 76)
                        } else {
                            Color32::TRANSPARENT
                        };

                        let response = egui::Frame::new()
                            .fill(fill)
                            .inner_margin(egui::Margin::same(3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    cell(ui, 96.0, |ui| {
                                        ui.label(&row.date);
                                    });
                                    cell(ui, 190.0, |ui| {
                                        ui.label(&row.id);
                                    });
                                    cell(ui, 230.0, |ui| {
                                        ui.label(RichText::new(&row.name).strong());
                                    });
                                    cell(ui, 130.0, |ui| {
                                        ui.label(&row.join_type);
                                    });
                                    cell(ui, 170.0, |ui| {
                                        ui.label(&row.location);
                                    });
                                    ui.label(&row.invites);
                                });
                            })
                            .response
                            .interact(egui::Sense::click());

                        if response.clicked() {
                            selected = Some(i);
                        }
                        response.context_menu(|ui| {
                            if ui.button("Copy id").clicked() {
                                acts.push(Act::Copy(row.id.clone()));
                                ui.close();
                            }
                            if ui.button("Copy invites").clicked() {
                                acts.push(Act::Copy(row.invites.clone()));
                                ui.close();
                            }
                        });
                    }
                });
        });

        self.selected_guild = selected;
        self.handle_acts(acts);
    }
}
