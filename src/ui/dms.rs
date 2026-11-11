// Direct messages tab: one row per dm channel, sortable by any column.

use eframe::egui::{self, Context, RichText, Ui};

use crate::app::{Act, App};

use super::{avatar, cell};

impl App {
    pub fn ui_dms(&mut self, ui: &mut Ui, ctx: &Context) {
        if self.dm_rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No direct messages found in this package.");
            });
            return;
        }

        let mut acts = Vec::new();
        let mut sort_click: Option<usize> = None;

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                header_cell(
                    ui,
                    250.0,
                    "Name",
                    self.dm_sort_column == Some(0),
                    &mut sort_click,
                    0,
                );
                header_cell(
                    ui,
                    96.0,
                    "First DM Date",
                    self.dm_sort_column == Some(1),
                    &mut sort_click,
                    1,
                );
                header_cell(
                    ui,
                    104.0,
                    "Your Messages",
                    self.dm_sort_column == Some(2),
                    &mut sort_click,
                    2,
                );
                cell(ui, 0.0, |ui| {
                    if ui
                        .selectable_label(self.dm_sort_column == Some(3), "Note")
                        .clicked()
                    {
                        sort_click = Some(3);
                    }
                });
                cell(ui, 160.0, |ui| {
                    ui.label(RichText::new("Actions").strong());
                });
            });

            let height = (ui.available_height() - 30.0).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("dms")
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, |ui| {
                    for row in &self.dm_rows {
                        let response = egui::Frame::new()
                            .inner_margin(egui::Margin::same(4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    cell(ui, 250.0, |ui| {
                                        ui.horizontal(|ui| {
                                            avatar(
                                                ui,
                                                &mut self.image_cache,
                                                ctx,
                                                &row.avatar_url,
                                                row.default_avatar,
                                                36.0,
                                            );
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(&row.username).strong());
                                                ui.label(
                                                    RichText::new(&row.user_id).weak().small(),
                                                );
                                            });
                                        });
                                    });
                                    cell(ui, 96.0, |ui| {
                                        ui.label(&row.date);
                                    });
                                    cell(ui, 104.0, |ui| {
                                        ui.label(row.messages_count.to_string());
                                    });
                                    cell(ui, 0.0, |ui| {
                                        ui.label(&row.note);
                                    });
                                    cell(ui, 160.0, |ui| {
                                        ui.horizontal(|ui| {
                                            if row.can_fetch && ui.button("Fetch info").clicked() {
                                                acts.push(Act::FetchDmUser {
                                                    channel_id: row.channel_id.clone(),
                                                });
                                            }
                                            if ui.button("Open DM").clicked() {
                                                acts.push(Act::OpenDm {
                                                    user_id: row.user_id.clone(),
                                                    channel_id: row.channel_id.clone(),
                                                });
                                            }
                                        });
                                    });
                                });
                            })
                            .response;

                        response.context_menu(|ui| {
                            if ui.button("Copy user id").clicked() {
                                acts.push(Act::Copy(row.user_id.clone()));
                                ui.close();
                            }
                            if ui.button("Copy channel id").clicked() {
                                acts.push(Act::Copy(row.channel_id.clone()));
                                ui.close();
                            }
                            if !row.note.is_empty() && ui.button("Copy note").clicked() {
                                acts.push(Act::Copy(row.note.clone()));
                                ui.close();
                            }
                            if !row.unknown_id {
                                ui.separator();
                                if ui.button("View user").clicked() {
                                    acts.push(Act::ViewUser(row.user_id.clone()));
                                    ui.close();
                                }
                            }
                        });
                    }
                });
        });

        if let Some(column) = sort_click {
            self.sort_dm_rows(column);
        }
        self.handle_acts(acts);
    }

    /// Clicking a header sorts by it, clicking it again flips the order.
    fn sort_dm_rows(&mut self, column: usize) {
        if self.dm_sort_column == Some(column) {
            self.dm_rows.reverse();
            return;
        }
        match column {
            0 => self.dm_rows.sort_by_key(|a| a.username.to_lowercase()),
            1 => self.dm_rows.sort_by(|a, b| a.date.cmp(&b.date)),
            2 => self.dm_rows.sort_by_key(|a| a.messages_count),
            _ => self.dm_rows.sort_by_key(|a| a.note.to_lowercase()),
        }
        self.dm_sort_column = Some(column);
    }
}

fn header_cell(
    ui: &mut Ui,
    width: f32,
    text: &str,
    active: bool,
    sort_click: &mut Option<usize>,
    column: usize,
) {
    cell(ui, width, |ui| {
        if ui.selectable_label(active, text).clicked() {
            *sort_click = Some(column);
        }
    });
}
