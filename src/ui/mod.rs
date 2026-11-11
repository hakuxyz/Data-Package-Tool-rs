// The UI side of the app: tabs, dialogs and the widgets they share.

mod dms;
mod images;
mod messages;
mod servers;
mod settings_tab;

use eframe::egui::{self, Context, RichText, TextureHandle, Ui};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use crate::app::{Act, App, Dialog, DialogKind, DialogResult, DialogTag};
use crate::images::ImageCache;

/// One of the pngs we ship, decoded once and kept as a texture.
fn bundled(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    thread_local! {
        static CACHE: RefCell<HashMap<String, TextureHandle>> = RefCell::new(HashMap::new());
    }
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(handle) = cache.get(name) {
            return handle.clone();
        }
        let image = image::load_from_memory(bytes).expect("bundled png failed to decode");
        let rgba = image.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        );
        let handle = ctx.load_texture(name, color, egui::TextureOptions::LINEAR);
        cache.insert(name.to_string(), handle.clone());
        handle
    })
}

fn default_avatar(ctx: &Context, id: usize) -> TextureHandle {
    let bytes: &[u8] = match id {
        1 => include_bytes!("../../assets/1.png"),
        2 => include_bytes!("../../assets/2.png"),
        3 => include_bytes!("../../assets/3.png"),
        4 => include_bytes!("../../assets/4.png"),
        5 => include_bytes!("../../assets/5.png"),
        _ => include_bytes!("../../assets/0.png"),
    };
    bundled(ctx, &format!("default-avatar-{id}"), bytes)
}

/// Round profile picture, falls back to the bundled defaults.
fn avatar(
    ui: &mut Ui,
    cache: &mut ImageCache,
    ctx: &Context,
    url: &str,
    default_id: usize,
    size: f32,
) {
    let radius = (size / 2.0) as u8;
    if !url.is_empty() {
        let key = format!("avatar:{url}");
        if let Some(handle) = cache.get_or_request(ctx, &key, url, 128) {
            let id = handle.id();
            ui.add(egui::Image::new((id, egui::vec2(size, size))).corner_radius(radius));
            return;
        }
    }
    let handle = default_avatar(ctx, default_id);
    let id = handle.id();
    ui.add(egui::Image::new((id, egui::vec2(size, size))).corner_radius(radius));
}

/// Small file icon used next to attachments.
fn file_icon(ui: &mut Ui, ctx: &Context, size: f32) {
    let handle = bundled(
        ctx,
        "file-icon",
        include_bytes!("../../assets/FileIcon.png"),
    );
    let id = handle.id();
    ui.add(egui::Image::new((id, egui::vec2(size, size))));
}

/// One fixed width column of a table row.
fn cell(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    if width <= 0.0 {
        add(ui);
        return;
    }
    ui.allocate_ui_with_layout(
        egui::vec2(width, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        add,
    );
}

/// Search box plus the buttons that act on the results. Shared by the
/// messages and images tabs.
fn search_bar(app: &mut App, ui: &mut Ui, ctx: &Context) {
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut app.query)
                .hint_text("Search messages...")
                .desired_width(340.0),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Search").clicked() || enter {
            app.start_search(ctx);
        }
        if ui.button("Search Options...").clicked() {
            app.open_search_options();
        }

        if app.mass_delete.is_some() {
            if ui.button("Stop Deleting").clicked() {
                app.stop_mass_delete();
            }
        } else if ui.button("Delete All Results").clicked() {
            app.mass_delete_prompt = true;
        }
    });

    if app.searching {
        let done = app.search_progress.load(Ordering::Relaxed);
        let total = app.package.as_ref().map(|p| p.channels.len()).unwrap_or(0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!("Searching... {}/{} channels", done, total));
        });
    }
}

/// The opening tab: pick a package and show what came out of it.
pub fn load_tab(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();

    // Dropping a zip onto the window works as well as the file picker
    let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .collect()
    });
    for path in dropped {
        let is_zip = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("zip"))
            .unwrap_or(false);
        if is_zip {
            app.load_path(&ctx, path.to_string_lossy().to_string());
        }
    }

    if app.loading {
        let status = app.load_status.lock().unwrap();
        let fraction = if status.max > 0 {
            status.progress as f32 / status.max as f32
        } else {
            0.0
        };
        ui.vertical_centered(|ui| {
            ui.add_space(70.0);
            ui.add(
                egui::ProgressBar::new(fraction)
                    .text(status.status.as_str())
                    .desired_width(400.0),
            );
            ui.add_space(6.0);
            ui.label(RichText::new("Reading your package...").weak());
        });
        return;
    }

    ui.vertical_centered(|ui| {
        ui.add_space(50.0);
        ui.label(RichText::new("Data Package Tool").heading().strong());
        ui.label("Browse the contents of a Discord data package.");
        ui.add_space(22.0);
        if ui
            .add_sized([260.0, 46.0], egui::Button::new("Load Data Package"))
            .clicked()
        {
            app.pick_and_load(&ctx);
        }
        ui.add_space(8.0);
        ui.label(RichText::new("You can also drop a .zip file on this window.").weak());
    });

    let Some(pkg) = app.package.clone() else {
        return;
    };

    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let url = pkg.user.avatar_url();
            avatar(
                ui,
                &mut app.image_cache,
                &ctx,
                &url,
                pkg.user.default_avatar_id(),
                44.0,
            );
            ui.vertical(|ui| {
                ui.label(RichText::new(pkg.user.display_tag()).strong());
                ui.label(RichText::new(format!("user id {}", pkg.user.id)).weak());
            });
        });

        ui.add_space(8.0);
        ui.label(format!(
            "Package created {}",
            pkg.creation_time.format("%Y-%m-%d %H:%M")
        ));

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(format!("{} messages", pkg.total_messages));
            ui.separator();
            ui.label(format!("{} channels", pkg.channels.len()));
            ui.separator();
            ui.label(format!("{} servers", app.guild_rows.len()));
            ui.separator();
            ui.label(format!("{} dms", app.dm_rows.len()));
        });

        ui.add_space(12.0);
        if ui.button("Load another package").clicked() {
            app.pick_and_load(&ctx);
        }
    });
}

/// The search options dialog, edits a copy of the settings until saved.
pub fn search_options_window(app: &mut App, ctx: &Context) {
    let Some(mut draft) = app.options_draft.take() else {
        return;
    };

    let mut open = true;
    let mut save = false;
    let mut cancel = false;

    egui::Window::new("Search Options")
        .id(egui::Id::unique("dpt_search_options"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(380.0);

            ui.horizontal(|ui| {
                ui.label("Mode");
                let mode = match draft.search_mode.as_str() {
                    "words" => "Words",
                    "regex" => "Regex",
                    _ => "Exact",
                };
                egui::ComboBox::from_id_salt("search_mode")
                    .selected_text(mode)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.search_mode, "exact".into(), "Exact");
                        ui.selectable_value(&mut draft.search_mode, "words".into(), "Words");
                        ui.selectable_value(&mut draft.search_mode, "regex".into(), "Regex");
                    });

                ui.label("Sort");
                let sort = if draft.sort_mode == "asc" {
                    "Oldest first"
                } else {
                    "Newest first"
                };
                egui::ComboBox::from_id_salt("sort_mode")
                    .selected_text(sort)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.sort_mode, "desc".into(), "Newest first");
                        ui.selectable_value(&mut draft.sort_mode, "asc".into(), "Oldest first");
                    });
            });

            ui.separator();
            ui.label(RichText::new("What to include").strong());
            ui.checkbox(&mut draft.exclude_guilds, "Search servers");
            ui.checkbox(&mut draft.exclude_dms, "Search direct messages");
            ui.checkbox(&mut draft.exclude_gdms, "Search group dms");
            ui.checkbox(&mut draft.exclude_deleted, "Include messages you deleted");
            ui.checkbox(&mut draft.case_sensitive, "Case sensitive");

            ui.separator();
            ui.label(RichText::new("Attachments").strong());
            ui.horizontal(|ui| {
                ui.checkbox(&mut draft.has_image, "Has image");
                ui.checkbox(&mut draft.has_video, "Has video");
                ui.checkbox(&mut draft.has_file, "Has file");
            });

            ui.separator();
            ui.label(RichText::new("Date range").strong());
            ui.horizontal(|ui| {
                ui.checkbox(&mut draft.before_enabled, "Before");
                ui.add_enabled(
                    draft.before_enabled,
                    egui::TextEdit::singleline(&mut draft.before_date)
                        .hint_text("yyyy-MM-dd")
                        .desired_width(110.0),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut draft.after_enabled, "After");
                ui.add_enabled(
                    draft.after_enabled,
                    egui::TextEdit::singleline(&mut draft.after_date)
                        .hint_text("yyyy-MM-dd")
                        .desired_width(110.0),
                );
            });

            ui.separator();
            ui.label(RichText::new("Excluded channels and servers").strong());
            remove_list(ui, &mut draft.exclude_ids);

            ui.label(RichText::new("Only these channels and servers").strong());
            remove_list(ui, &mut draft.whitelist_ids);

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                ui.label(RichText::new("Ids can be added in the Settings tab.").weak());
            });
        });

    if save {
        app.options_draft = Some(draft);
        app.save_search_options();
    } else if cancel || !open {
        app.discard_search_options();
    } else {
        app.options_draft = Some(draft);
    }
}

/// Read only list of ids with a remove button on each line.
fn remove_list(ui: &mut Ui, ids: &mut Vec<String>) {
    if ids.is_empty() {
        ui.label(RichText::new("(empty)").weak());
        return;
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
    if let Some(i) = remove {
        ids.remove(i);
    }
}

/// Asks how long to wait between deletes before starting the run.
pub fn mass_delete_window(app: &mut App, ctx: &Context) {
    if !app.mass_delete_prompt || app.mass_delete.is_some() {
        return;
    }

    let count = app.results.len();
    let mut open = true;
    let mut start = false;

    egui::Window::new("Delete Messages")
        .id(egui::Id::unique("dpt_mass_delete"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.label(format!(
                "This will delete {} messages from your account. There is no undo.",
                count
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Delay between deletes");
                ui.add(egui::Slider::new(&mut app.mass_delete_delay, 0..=100));
            });
            ui.horizontal(|ui| {
                ui.label(format!("{} ms", app.mass_delete_delay * 100));
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    start = true;
                }
                if ui.button("Cancel").clicked() {
                    ui.close();
                }
            });
        });

    if start {
        app.mass_delete_prompt = false;
        app.start_mass_delete();
    } else if !open {
        app.mass_delete_prompt = false;
    }
}

/// Draws the dialog on top of everything and does what the answer says.
pub fn dialogs(app: &mut App, ctx: &Context) {
    let Some(mut dlg) = app.dialogs.pop() else {
        return;
    };

    let mut answer: Option<DialogResult> = None;
    let mut open = true;
    let title = dlg.title.clone();

    egui::Window::new(title)
        .id(egui::Id::unique("dpt_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.label(&dlg.text);
            ui.add_space(10.0);

            match dlg.kind {
                DialogKind::Input => {
                    ui.add(
                        egui::TextEdit::singleline(&mut dlg.input)
                            .hint_text("Enter a value")
                            .desired_width(300.0),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            answer = Some(DialogResult::Ok);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(DialogResult::No);
                        }
                    });
                }
                DialogKind::YesNo => {
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            answer = Some(DialogResult::Yes);
                        }
                        if ui.button("No").clicked() {
                            answer = Some(DialogResult::No);
                        }
                    });
                }
                _ => {
                    if ui.button("OK").clicked() {
                        answer = Some(DialogResult::Ok);
                    }
                }
            }
        });

    // Closing with the x counts as "no"
    let answer = match answer {
        Some(answer) => answer,
        None if open => {
            app.dialogs.push(dlg);
            return;
        }
        None => DialogResult::No,
    };

    let Dialog { tag, input, .. } = dlg;

    match (tag, answer) {
        (DialogTag::UpdateAvailable, DialogResult::Yes) => {
            if let Some(url) = app.update_url.take() {
                app.handle_act(Act::OpenUrl(url));
            }
        }
        (DialogTag::JumpImagesOffset, DialogResult::Ok) => {
            if let Ok(index) = input.trim().parse::<usize>() {
                app.images_offset = index.saturating_sub(1);
            }
        }
        (DialogTag::AddExcludeId, DialogResult::Ok) => {
            add_id(app, input, false);
        }
        (DialogTag::AddWhitelistId, DialogResult::Ok) => {
            add_id(app, input, true);
        }
        // Yes means we go on and ask what to type
        (tag @ DialogTag::OpenDmMismatch { .. }, DialogResult::Yes) => {
            app.push_dialog(Dialog {
                title: "Send Message".into(),
                text: "Type the message to send in your selected dm. This makes \
                       Discord bring the dm back."
                    .into(),
                kind: DialogKind::Input,
                tag,
                input: String::new(),
            });
        }
        (tag @ DialogTag::OpenDmMismatch { .. }, DialogResult::Ok) => {
            app.send_mismatch_message(tag, input);
        }
        _ => {}
    }
}

/// Adds an id typed into a dialog to one of the two id lists.
fn add_id(app: &mut App, input: String, whitelist: bool) {
    let id = input.trim();
    if id.is_empty() {
        return;
    }
    let list = if whitelist {
        &mut app.settings.whitelist_ids
    } else {
        &mut app.settings.exclude_ids
    };
    if !list.iter().any(|x| x == id) {
        list.push(id.to_string());
    }
    app.settings.save();
}
