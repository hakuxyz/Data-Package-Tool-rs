// Messages tab: the search results, one row per message.

use eframe::egui::{self, Color32, Context, RichText, Ui};
use std::sync::Arc;

use crate::app::{Act, App, RenderedMsg, Seg};
use crate::models::DAttachment;
use crate::package::DataPackage;

use super::{avatar, file_icon, search_bar};

impl App {
    pub fn ui_messages(&mut self, ui: &mut Ui, ctx: &Context) {
        let pkg = self.package.clone();
        search_bar(self, ui, ctx);

        let Some(pkg) = pkg else {
            ui.centered_and_justified(|ui| {
                ui.label("Load a data package first.");
            });
            return;
        };

        let rendered = self.rendered.clone();
        let count = self.results.len();
        let label = self.results_label.clone();

        ui.vertical(|ui| {
            let height = (ui.available_height() - 34.0).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("messages")
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, |ui| {
                    for rm in rendered.iter() {
                        self.message_row(ui, ctx, &pkg, rm);
                    }
                });

            ui.horizontal(|ui| {
                if ui.button("< Previous").clicked() {
                    self.page_prev();
                }
                if ui.button("Next >").clicked() {
                    self.page_next();
                }
                if label.is_empty() {
                    ui.label(RichText::new("Type something and press Search.").weak());
                } else {
                    ui.label(&label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{} results", count));
                });
            });
        });
    }

    /// One message: avatar, channel name, text and its attachments.
    fn message_row(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        pkg: &Arc<DataPackage>,
        rm: &RenderedMsg,
    ) {
        let msg = &rm.msg;
        let channel = pkg.channels_map.get(&msg.channel_id);
        let name = channel
            .map(|c| c.display_name())
            .unwrap_or_else(|| "Unknown channel".to_string());
        let is_dm = channel
            .map(|c| c.is_dm() || c.is_group_dm())
            .unwrap_or(false);

        let (avatar_url, default_id) = match channel
            .filter(|_| is_dm)
            .and_then(|c| pkg.users_map.get(&c.dm_recipient_id))
        {
            Some(user) => (user.avatar_url(), user.default_avatar_id()),
            None => (String::new(), 0),
        };

        let deleted = msg.is_deleted();
        let time = msg.timestamp.format("%Y-%m-%d %H:%M").to_string();

        let response = egui::Frame::new()
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if is_dm {
                        avatar(
                            ui,
                            &mut self.image_cache,
                            ctx,
                            &avatar_url,
                            default_id,
                            34.0,
                        );
                    } else {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(58, 60, 66))
                            .corner_radius(17u8)
                            .show(ui, |ui| {
                                ui.add_sized([34.0, 34.0], egui::Label::new("#"));
                            });
                    }

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&name).strong());
                            ui.label(RichText::new(&time).weak().small());
                            if deleted {
                                ui.label(
                                    RichText::new("deleted")
                                        .small()
                                        .color(Color32::from_rgb(255, 120, 120)),
                                );
                            }
                        });

                        if !msg.content.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for seg in &rm.segs {
                                    match seg {
                                        Seg::Text(text) => {
                                            ui.label(text);
                                        }
                                        Seg::Link(url) => {
                                            ui.hyperlink(url);
                                        }
                                    }
                                }
                            });
                        }

                        for att in &msg.attachments {
                            attachment_row(ui, ctx, att);
                        }
                    });
                });
            })
            .response;

        let mut acts = Vec::new();
        let link = channel.and_then(|c| msg.message_link(c).ok());
        let guild_id = channel.and_then(|c| c.guild.as_ref().map(|g| g.id.clone()));

        response.context_menu(|ui| {
            if let Some(path) = &link {
                if ui.button("Go to message").clicked() {
                    acts.push(Act::LaunchDiscord(path.clone()));
                    ui.close();
                }
                if ui.button("Copy message link").clicked() {
                    let web = format!("https://discord.com/channels/{}", path);
                    acts.push(Act::Copy(web));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Copy message").clicked() {
                acts.push(Act::Copy(msg.content.clone()));
                ui.close();
            }
            if ui.button("Copy message id").clicked() {
                acts.push(Act::Copy(msg.id.clone()));
                ui.close();
            }
            if ui.button("Copy channel id").clicked() {
                acts.push(Act::Copy(msg.channel_id.clone()));
                ui.close();
            }
            if let Some(id) = &guild_id {
                if ui.button("Copy guild id").clicked() {
                    acts.push(Act::Copy(id.clone()));
                    ui.close();
                }
                if ui.button("View server in Servers tab").clicked() {
                    acts.push(Act::JumpGuild(id.clone()));
                    ui.close();
                }
            }
            if !deleted {
                ui.separator();
                if ui.button("Delete message").clicked() {
                    acts.push(Act::DeleteMessage(msg.clone()));
                    ui.close();
                }
            }
        });

        self.handle_acts(acts);
    }
}

/// One attachment line, clicking the name opens it in the browser.
fn attachment_row(ui: &mut Ui, ctx: &Context, att: &DAttachment) {
    ui.horizontal(|ui| {
        file_icon(ui, ctx, 16.0);
        let name = att.file_name();
        if ui.link(&name).clicked() {
            let _ = crate::discord::open_external(&att.url());
        }
        ui.label(RichText::new(format!(".{}", att.extension())).weak());
    });
}
