// Images tab: a grid of the pictures found in the current search.

use eframe::egui::{self, Context, RichText, Ui};
use std::sync::Arc;

use crate::app::{Act, App, Dialog, DialogKind, DialogTag, IMAGES_PER_PAGE};
use crate::models::DAttachment;

use super::search_bar;

impl App {
    pub fn ui_images(&mut self, ui: &mut Ui, ctx: &Context) {
        let items = self.image_list.clone();
        search_bar(self, ui, ctx);

        if items.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No images to show. Load a package and search first.");
            });
            return;
        }

        if self.images_offset >= items.len() {
            self.images_offset = 0;
        }
        let start = self.images_offset;
        let page: Vec<Arc<DAttachment>> = items[start..]
            .iter()
            .take(IMAGES_PER_PAGE)
            .cloned()
            .collect();
        self.images_label = format!("{}-{} of {}", start + 1, start + page.len(), items.len());

        let cell = 156.0;
        let mut acts = Vec::new();
        let mut open_url: Option<String> = None;

        ui.horizontal_wrapped(|ui| {
            for att in &page {
                let url = att.url();
                let name = att.file_name();
                let key = format!("image:{}", att.id());

                // One head request per file, the reply shows up as a label
                self.request_file_size(&url);

                ui.allocate_ui_with_layout(
                    egui::vec2(cell, cell),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(cell - 10.0, cell - 48.0),
                            egui::Sense::click(),
                        );

                        if let Some(handle) = self.image_cache.get_or_request(ctx, &key, &url, 480)
                        {
                            let id = handle.id();
                            let tex = handle.size_vec2();
                            let scale = (rect.width() / tex.x).min(rect.height() / tex.y).min(1.0);
                            let size = egui::vec2(tex.x * scale, tex.y * scale);
                            let image_rect = egui::Rect::from_center_size(rect.center(), size);
                            ui.put(image_rect, egui::Image::new((id, size)).corner_radius(4u8));
                        } else {
                            // Still downloading, keep the space
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(37, 39, 43))
                                .corner_radius(4u8)
                                .show(ui, |ui| {
                                    ui.add_sized(rect.size(), egui::Spinner::new());
                                });
                        }

                        ui.add(egui::Label::new(name.clone()).truncate());
                        ui.label(RichText::new(self.file_size(&url)).weak().small());

                        if response.clicked() {
                            open_url = Some(url.clone());
                        }
                        response.context_menu(|ui| {
                            if ui.button("Open in browser").clicked() {
                                open_url = Some(url.clone());
                                ui.close();
                            }
                            if ui.button("Copy url").clicked() {
                                acts.push(Act::Copy(url.clone()));
                                ui.close();
                            }
                        });
                    },
                );
            }
        });

        if let Some(url) = open_url {
            self.handle_act(Act::OpenUrl(url));
        }
        self.handle_acts(acts);

        ui.horizontal(|ui| {
            if ui.button("< Previous").clicked() {
                self.images_offset = self.images_offset.saturating_sub(IMAGES_PER_PAGE);
            }
            if ui.button("Next >").clicked() && self.images_offset + IMAGES_PER_PAGE < items.len() {
                self.images_offset += IMAGES_PER_PAGE;
            }
            ui.label(&self.images_label);
            if ui.button("Jump to...").clicked() {
                self.push_dialog(Dialog {
                    title: "Jump to Image".into(),
                    text: "Enter the number of the image to jump to, counting from 1.".into(),
                    kind: DialogKind::Input,
                    tag: DialogTag::JumpImagesOffset,
                    input: String::new(),
                });
            }
        });
    }
}
