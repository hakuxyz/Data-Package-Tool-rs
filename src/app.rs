use chrono::{Local, TimeZone};
use eframe::egui::{self, Context, Ui};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use crate::consts;
use crate::discord;
use crate::images::ImageCache;
use crate::models::*;
use crate::package::{self, DataPackage, GuildData, StatusHandle};
use crate::search::{self, SearchOptions};
use crate::settings::Settings;
use crate::ui;

pub const PAGE_SIZE: usize = 500;
pub const IMAGES_PER_PAGE: usize = 36;
pub const APP_NAME: &str = "Data Package Tool";
const UPDATE_URL: &str =
    "https://raw.githubusercontent.com/hakuxyz/Data-Package-Tool-rs/main/version.txt";
const RELEASES_URL: &str = "https://github.com/hakuxyz/Data-Package-Tool-rs/releases/latest";

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Tab {
    Load,
    Messages,
    Images,
    Servers,
    Dms,
    Settings,
}

#[derive(Clone, PartialEq)]
pub enum DialogKind {
    Info,
    Warn,
    Error,
    YesNo,
    Input,
}

/// What a dialog was opened for, so we know what to do with the answer.
#[derive(Clone, PartialEq)]
pub enum DialogTag {
    None,
    ConfirmMassDelete,
    DiscardSearchOptions,
    JumpImagesOffset,
    AddExcludeId,
    AddWhitelistId,
    UpdateAvailable,
    OpenDmMismatch {
        user_id: String,
        expected: String,
        received: String,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub enum DialogResult {
    Ok,
    Yes,
    No,
}

pub struct Dialog {
    pub title: String,
    pub text: String,
    pub kind: DialogKind,
    pub tag: DialogTag,
    pub input: String,
}

impl Dialog {
    pub fn info(text: impl Into<String>) -> Self {
        Self::with_title("Information", text, DialogKind::Info)
    }
    pub fn warn(text: impl Into<String>) -> Self {
        Self::with_title("Warning", text, DialogKind::Warn)
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self::with_title("Error", text, DialogKind::Error)
    }
    pub fn confirm(title: &str, text: impl Into<String>, tag: DialogTag) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            kind: DialogKind::YesNo,
            tag,
            input: String::new(),
        }
    }
    fn with_title(title: &str, text: impl Into<String>, kind: DialogKind) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            kind,
            tag: DialogTag::None,
            input: String::new(),
        }
    }
}

/// Things a row widget wants the app to do after it finished drawing.
pub enum Act {
    Dialog(Dialog),
    OpenUrl(String),
    LaunchDiscord(String),
    Copy(String),
    DeleteMessage(Arc<DMessage>),
    OpenDm { user_id: String, channel_id: String },
    ViewUser(String),
    FetchDmUser { channel_id: String },
    FetchMessageUser(Arc<DMessage>),
    JumpGuild(String),
    StartMassDelete,
}

/// One line of the dm list, mirrors the old DmsListEntry.
pub struct DmRow {
    pub channel_id: String,
    pub user_id: String,
    pub username: String,
    pub avatar_url: String,
    pub default_avatar: usize,
    pub date: String,
    pub messages_count: usize,
    pub note: String,
    pub can_fetch: bool,
    pub unknown_id: bool,
}

pub struct GuildRow {
    pub date: String,
    pub id: String,
    pub name: String,
    pub join_type: String,
    pub location: String,
    pub invites: String,
}

/// Message content split into plain text and clickable links.
pub enum Seg {
    Text(String),
    Link(String),
}

pub struct RenderedMsg {
    pub msg: Arc<DMessage>,
    pub segs: Vec<Seg>,
}

/// Messages from background threads.
enum Msg {
    Loaded(Result<Arc<DataPackage>, String>),
    GuildsLoaded(Result<GuildData, String>),
    SearchDone(Vec<Arc<DMessage>>),
    FileSize(String, String),
    DmFetched {
        channel_id: String,
        user: DUser,
    },
    MessageResolved {
        channel_id: String,
        user: DUser,
    },
    OpenDmDone {
        user_id: String,
        channel_id: String,
        expected: Option<String>,
    },
    MessageDeleted(String),
    MassDeleteFinished,
    RefreshDone(usize),
    Fail(String),
    UpdateCheck(Option<String>),
}

pub struct MassDelete {
    stop: Arc<AtomicBool>,
}

pub struct App {
    pub settings: Settings,
    pub package: Option<Arc<DataPackage>>,
    pub guild_data: Option<GuildData>,
    pub load_status: StatusHandle,
    pub guilds_status: StatusHandle,
    pub tab: Tab,
    pub loading: bool,
    pub warned_about_links: bool,
    pub analytics_warned: bool,

    // messages tab
    pub query: String,
    pub results: Vec<Arc<DMessage>>,
    pub image_list: Vec<Arc<DAttachment>>,
    pub rendered: Arc<Vec<RenderedMsg>>,
    pub results_offset: usize,
    pub results_label: String,
    pub searching: bool,
    pub search_progress: Arc<AtomicUsize>,

    // images tab
    pub images_offset: usize,
    pub images_label: String,

    // dms tab
    pub dm_rows: Vec<DmRow>,
    pub dm_sort_column: Option<usize>,

    // servers tab
    pub guild_rows: Vec<GuildRow>,
    pub selected_guild: Option<usize>,

    // settings that live in memory only, like the original
    pub user_token: String,
    pub bot_token: String,

    // ui state
    pub dialogs: Vec<Dialog>,
    pub options_draft: Option<Settings>,
    pub mass_delete_prompt: bool,
    pub mass_delete_delay: u32,
    pub mass_delete: Option<MassDelete>,
    pub update_url: Option<String>,

    pub image_cache: ImageCache,
    file_sizes: HashMap<String, String>,
    sizes_in_flight: std::collections::HashSet<String>,

    /// Real egui context, background threads use it to ask for a repaint
    pub ctx: Context,
    update_checked: bool,

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(49, 51, 56);
        visuals.panel_fill = egui::Color32::from_rgb(43, 45, 49);
        cc.egui_ctx.set_visuals(visuals);

        Self {
            settings: Settings::load(),
            package: None,
            guild_data: None,
            load_status: package::new_status(),
            guilds_status: package::new_status(),
            tab: Tab::Load,
            loading: false,
            warned_about_links: false,
            analytics_warned: false,
            query: String::new(),
            results: Vec::new(),
            image_list: Vec::new(),
            rendered: Arc::new(Vec::new()),
            results_offset: 0,
            results_label: String::new(),
            searching: false,
            search_progress: Arc::new(AtomicUsize::new(0)),
            images_offset: 0,
            images_label: String::new(),
            dm_rows: Vec::new(),
            dm_sort_column: None,
            guild_rows: Vec::new(),
            selected_guild: None,
            user_token: String::new(),
            bot_token: String::new(),
            dialogs: Vec::new(),
            options_draft: None,
            mass_delete_prompt: false,
            mass_delete_delay: 10,
            mass_delete: None,
            update_url: None,
            image_cache: ImageCache::default(),
            file_sizes: HashMap::new(),
            sizes_in_flight: std::collections::HashSet::new(),
            ctx: cc.egui_ctx.clone(),
            update_checked: false,
            tx,
            rx,
        }
    }

    // --- background threads ---------------------------------------------

    fn poll_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Loaded(result) => self.on_loaded(result),
                Msg::GuildsLoaded(result) => self.on_guilds_loaded(result),
                Msg::SearchDone(results) => self.on_search_done(results),
                Msg::FileSize(url, size) => {
                    self.file_sizes.insert(url, size);
                }
                Msg::DmFetched { channel_id, user } => self.on_dm_fetched(channel_id, user),
                Msg::MessageResolved { channel_id, user } => {
                    self.on_message_resolved(channel_id, user)
                }
                Msg::OpenDmDone {
                    user_id,
                    channel_id,
                    expected,
                } => self.on_open_dm_done(user_id, channel_id, expected),
                Msg::MessageDeleted(id) => {
                    self.settings.add_deleted_message(&id);
                    self.settings.save();
                }
                Msg::MassDeleteFinished => {
                    self.mass_delete = None;
                    self.push_dialog(Dialog::info("Mass Delete Finished!"));
                }
                Msg::RefreshDone(n) => {
                    if n > 0 {
                        self.push_dialog(Dialog::info(format!(
                            "Refreshed {} attachment links.",
                            n
                        )));
                    }
                    self.build_page();
                }
                Msg::Fail(text) => self.push_dialog(Dialog::error(text)),
                Msg::UpdateCheck(newer) => {
                    if let Some(version) = newer {
                        self.update_url = Some(RELEASES_URL.to_string());
                        self.push_dialog(Dialog {
                            title: "Data Package Tool".into(),
                            text: format!(
                                "A new version is available ({}). Would you like to open the download page now?",
                                version
                            ),
                            kind: DialogKind::YesNo,
                            tag: DialogTag::UpdateAvailable,
                            input: String::new(),
                        });
                    }
                }
            }
        }
    }

    pub fn push_dialog(&mut self, dialog: Dialog) {
        self.dialogs.push(dialog);
    }

    pub fn handle_acts(&mut self, acts: Vec<Act>) {
        for act in acts {
            self.handle_act(act);
        }
    }

    pub fn handle_act(&mut self, act: Act) {
        match act {
            Act::Dialog(d) => self.push_dialog(d),
            Act::OpenUrl(url) => {
                if let Err(e) = discord::open_external(&url) {
                    self.push_dialog(Dialog::error(e));
                }
            }
            Act::LaunchDiscord(path) => {
                let instance = self.settings.discord_instance.clone();
                if let Err(e) = discord::launch_discord(&path, &instance) {
                    self.push_dialog(Dialog::error(e));
                }
            }
            Act::Copy(text) => self.ctx.copy_text(text),
            Act::DeleteMessage(msg) => self.delete_message(msg),
            Act::OpenDm {
                user_id,
                channel_id,
            } => self.open_dm(user_id, channel_id),
            Act::ViewUser(user_id) => {
                self.handle_act(Act::LaunchDiscord(format!("users/{}", user_id)));
            }
            Act::FetchDmUser { channel_id } => self.fetch_dm_user(channel_id),
            Act::FetchMessageUser(msg) => self.fetch_message_user(msg),
            Act::JumpGuild(guild_id) => self.jump_to_guild(guild_id),
            Act::StartMassDelete => self.start_mass_delete(),
        }
    }

    // --- loading --------------------------------------------------------

    pub fn pick_and_load(&mut self, ctx: &Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Discord Package files", &["zip"])
            .pick_file()
        else {
            return;
        };
        self.load_path(ctx, path.to_string_lossy().to_string());
    }

    /// Spawns the two parse threads for a zip, results come back as messages.
    pub fn load_path(&mut self, ctx: &Context, path: String) {
        self.loading = true;
        self.package = None;
        self.guild_data = None;
        self.guild_rows.clear();
        self.dm_rows.clear();
        self.results.clear();
        self.rendered = Arc::new(Vec::new());
        self.results_label.clear();
        self.image_list.clear();
        self.images_offset = 0;
        self.images_label.clear();
        self.load_status = package::new_status();
        self.guilds_status = package::new_status();
        self.image_cache.clear();
        self.tab = Tab::Load;

        let load_status = self.load_status.clone();
        let guilds_status = self.guilds_status.clone();
        let tx1 = self.tx.clone();
        let tx2 = self.tx.clone();
        let ctx1 = ctx.clone();
        let ctx2 = ctx.clone();
        let path_for_guilds = path.clone();

        std::thread::spawn(move || {
            let result = package::load(&path, &load_status).map(Arc::new);
            let _ = tx1.send(Msg::Loaded(result));
            ctx1.request_repaint();
        });
        std::thread::spawn(move || {
            let result = package::load_guilds(&path_for_guilds, &guilds_status);
            let _ = tx2.send(Msg::GuildsLoaded(result));
            ctx2.request_repaint();
        });
    }

    fn on_loaded(&mut self, result: Result<Arc<DataPackage>, String>) {
        self.loading = false;
        match result {
            Ok(pkg) => {
                // Remember which messages we deleted in earlier sessions
                let deleted = self.settings.deleted_ids();
                for id in deleted {
                    if let Some(msg) = pkg.messages_map.get(id) {
                        msg.mark_deleted();
                    }
                }
                self.build_dm_rows(&pkg);
                self.image_list = pkg.image_attachments.clone();
                self.package = Some(pkg);

                let pkg = self.package.clone().unwrap();
                if pkg.uses_unsigned_cdn_links && !self.warned_about_links {
                    self.warned_about_links = true;
                    self.push_dialog(Dialog::warn(consts::UNSIGNED_LINKS_WARNING));
                }
            }
            Err(e) => self.push_dialog(Dialog::error(format!("An error occurred:\n\n{}", e))),
        }
    }

    fn on_guilds_loaded(&mut self, result: Result<GuildData, String>) {
        match result {
            Ok(data) => {
                self.guild_data = Some(data);
                self.build_guild_rows();
                self.maybe_warn_analytics();
            }
            Err(e) => self.push_dialog(Dialog::error(format!("An error occurred:\n\n{}", e))),
        }
    }

    pub fn build_guild_rows(&mut self) {
        let (Some(pkg), Some(guilds)) = (&self.package, &self.guild_data) else {
            return;
        };
        self.guild_rows = guilds
            .joined_guilds
            .iter()
            .map(|g| GuildRow {
                date: g.timestamp.format("%Y-%m-%d").to_string(),
                id: g.id.clone(),
                name: pkg.guild_names_map.get(&g.id).cloned().unwrap_or_default(),
                join_type: g.join_type.clone(),
                location: g.location.clone().unwrap_or_default(),
                invites: g.invites.join(", "),
            })
            .collect();
        self.selected_guild = None;
    }

    pub fn build_dm_rows(&mut self, pkg: &Arc<DataPackage>) {
        let mut channels: Vec<&Arc<DChannel>> = pkg.channels.iter().filter(|c| c.is_dm()).collect();
        channels.sort_by_key(|c| std::cmp::Reverse(c.id.parse::<u64>().unwrap_or(0)));

        let mut rows = Vec::with_capacity(channels.len());
        for channel in channels {
            let recipient_id = channel.dm_recipient_id.clone();
            let recipient = pkg.users_map.get(&recipient_id);

            let avatar_url = recipient
                .filter(|u| u.has_avatar())
                .map(|u| u.avatar_url())
                .unwrap_or_default();
            let default_avatar = recipient.map(|u| u.default_avatar_id()).unwrap_or(0);

            let mut user_id = recipient_id.clone();
            let mut unknown_id = recipient_id == DELETED_USER_ID;
            // A resolved deleted user id takes over
            if unknown_id {
                if let Some(resolved) = self.settings.resolved_user_for(&channel.id) {
                    user_id = resolved.clone();
                    unknown_id = false;
                }
            }

            let username = if unknown_id {
                "(Deleted User)".to_string()
            } else {
                recipient
                    .map(|u| u.tag())
                    .unwrap_or("(Unknown User)".to_string())
            };

            let note = recipient
                .map(|u| u.notes.get(&user_id).cloned().unwrap_or_default())
                .unwrap_or_default();

            let date = discord::snowflake_to_timestamp(&channel.id)
                .and_then(|ms| Local.timestamp_opt(ms, 0).single())
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            rows.push(DmRow {
                channel_id: channel.id.clone(),
                user_id,
                username,
                avatar_url,
                default_avatar,
                date,
                messages_count: channel.messages.len(),
                note,
                can_fetch: unknown_id,
                unknown_id,
            });
        }
        self.dm_rows = rows;
        self.dm_sort_column = None;
    }

    pub fn maybe_warn_analytics(&mut self) {
        let missing = self
            .guild_data
            .as_ref()
            .and_then(|g| g.activity_status.missing_data)
            .unwrap_or(false);
        if !missing || self.analytics_warned || self.tab != Tab::Servers {
            return;
        }
        self.analytics_warned = true;
        let status = self.guild_data.as_ref().map(|g| g.activity_status.status);
        let text = match status {
            Some(AnalyticsStatus::DisabledNow) => "It seems like your Discord account has analytics disabled.\nSince disabling analytics deletes past events in your package, the Servers tab will be limited to around past 6 months!".to_string(),
            Some(AnalyticsStatus::DisabledBefore) => {
                let cutoff = self
                    .guild_data
                    .as_ref()
                    .and_then(|g| g.activity_status.cutoff_date)
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                format!("It seems like you've disabled analytics on your Discord account in the past.\nSince disabling analytics deletes past events in your package, the Servers tab will be limited to the date when you've re-enabled analytics, that is {}.", cutoff)
            }
            Some(AnalyticsStatus::PartiallyDisabledNow) => "It seems like you have some analytics disabled on your Discord account.\nSince disabling analytics deletes past events in your package, the Servers tab might show info from a limited time span!".to_string(),
            _ => return,
        };
        self.push_dialog(Dialog::warn(text));
    }

    pub fn jump_to_guild(&mut self, guild_id: String) {
        if let Some(idx) = self.guild_rows.iter().position(|r| r.id == guild_id) {
            self.tab = Tab::Servers;
            self.selected_guild = Some(idx);
        } else {
            self.push_dialog(Dialog::error(
                "Server could not be found in your joined servers list (bug?)",
            ));
        }
    }

    // --- search ---------------------------------------------------------

    pub fn start_search(&mut self, ctx: &Context) {
        let Some(pkg) = self.package.clone() else {
            self.push_dialog(Dialog::error("Load a data package first!"));
            return;
        };
        let opts = SearchOptions::from_settings(&self.settings);
        if let Err(e) = search::validate_query(&opts, &self.query) {
            self.push_dialog(Dialog::error(e));
            return;
        }

        self.results.clear();
        self.rendered = Arc::new(Vec::new());
        self.results_offset = 0;
        self.results_label.clear();
        self.searching = true;
        self.search_progress = Arc::new(AtomicUsize::new(0));
        self.image_cache.clear();

        let progress = self.search_progress.clone();
        let query = self.query.clone();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let results = search::search(&pkg, &opts, &query, Some(&progress));
            let _ = tx.send(Msg::SearchDone(results));
            ctx.request_repaint();
        });
    }

    fn on_search_done(&mut self, results: Vec<Arc<DMessage>>) {
        self.searching = false;
        self.results = results;
        self.results_offset = 0;
        self.images_offset = 0;

        let mut images: Vec<Arc<DAttachment>> = Vec::new();
        for msg in &self.results {
            images.extend(msg.attachments.iter().filter(|a| a.is_image()).cloned());
        }
        self.image_list = images;

        if self.results.is_empty() {
            self.results_label = "No results".to_string();
            self.rendered = Arc::new(Vec::new());
            return;
        }
        self.build_page();
        self.maybe_refresh_unsigned();
    }

    pub fn build_page(&mut self) {
        if self.results.is_empty() {
            self.results_label = "No results".to_string();
            self.rendered = Arc::new(Vec::new());
            return;
        }
        let start = self.results_offset;
        let end = (start + PAGE_SIZE).min(self.results.len());
        let slice = self.results[start..end].to_vec();
        self.rendered = Arc::new(
            slice
                .iter()
                .map(|msg| RenderedMsg {
                    segs: parse_content(&msg.content),
                    msg: msg.clone(),
                })
                .collect(),
        );
        self.results_label = format!("{}-{} of {}", start + 1, end, self.results.len());
    }

    pub fn page_prev(&mut self) {
        if self.results_offset >= PAGE_SIZE {
            self.results_offset -= PAGE_SIZE;
            self.build_page();
        }
    }

    pub fn page_next(&mut self) {
        if self.results.len() <= PAGE_SIZE {
            return;
        }
        if self.results_offset + PAGE_SIZE >= self.results.len() {
            return;
        }
        self.results_offset += PAGE_SIZE;
        self.build_page();
    }

    /// Old packages have dead attachment urls, a bot token can fix the ones
    /// on the current page.
    fn maybe_refresh_unsigned(&mut self) {
        let Some(pkg) = self.package.clone() else {
            return;
        };
        if !pkg.uses_unsigned_cdn_links || self.bot_token.is_empty() {
            return;
        }
        let mut need: Vec<Arc<DAttachment>> = Vec::new();
        for rm in self.rendered.iter() {
            for att in &rm.msg.attachments {
                if !att.is_signed() {
                    need.push(att.clone());
                }
            }
        }
        if need.is_empty() {
            return;
        }
        let token = self.bot_token.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let msg = match discord::refresh_attachments(&need, &token) {
                Ok(n) => Msg::RefreshDone(n),
                Err(e) => Msg::Fail(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    // --- token guarded actions -----------------------------------------

    fn user_token_ok(&mut self) -> bool {
        if self.user_token.trim().is_empty() {
            self.push_dialog(Dialog::error(consts::MISSING_TOKEN));
            return false;
        }
        let expected = self.package.as_ref().map(|p| p.user.id.clone());
        if !discord::validate_token(self.user_token.trim(), expected.as_deref()) {
            self.push_dialog(Dialog::error(consts::INVALID_TOKEN));
            return false;
        }
        true
    }

    fn bot_token_ok(&mut self) -> bool {
        if self.bot_token.trim().is_empty() {
            self.push_dialog(Dialog::error(consts::MISSING_BOT_TOKEN));
            return false;
        }
        if !discord::validate_token(self.bot_token.trim(), None) {
            self.push_dialog(Dialog::error(consts::INVALID_BOT_TOKEN));
            return false;
        }
        let own_id = self.package.as_ref().map(|p| p.user.id.clone());
        if discord::validate_token(self.bot_token.trim(), own_id.as_deref()) {
            self.push_dialog(Dialog::error(consts::WRONG_TOKEN_TYPE));
            return false;
        }
        true
    }

    fn delete_message(&mut self, msg: Arc<DMessage>) {
        if !self.user_token_ok() {
            return;
        }
        let token = self.user_token.trim().to_string();
        let channel_id = msg.channel_id.clone();
        let message_id = msg.id.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            match discord::delete_message(&channel_id, &message_id, &token) {
                Ok(_) => {
                    msg.mark_deleted();
                    let _ = tx.send(Msg::MessageDeleted(message_id));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Fail(e));
                }
            }
            ctx.request_repaint();
        });
    }

    fn open_dm(&mut self, user_id: String, channel_id: String) {
        if let Some(pkg) = &self.package {
            if let Some(ch) = pkg.channels_map.get(&channel_id) {
                if ch.has_duplicates() {
                    self.push_dialog(Dialog::warn(consts::DUPLICATE_DM_WARNING));
                }
            }
        }
        if !self.user_token_ok() {
            return;
        }
        let token = self.user_token.trim().to_string();
        let expected = channel_id.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let msg = match discord::open_dm(&user_id, &token) {
                Ok(received) => Msg::OpenDmDone {
                    user_id,
                    channel_id: received,
                    expected: Some(expected),
                },
                Err(e) => Msg::Fail(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn on_open_dm_done(&mut self, user_id: String, channel_id: String, expected: Option<String>) {
        let mismatch = expected
            .as_deref()
            .map(|e| e != channel_id)
            .unwrap_or(false);
        if mismatch {
            let expected = expected.unwrap_or_default();
            self.push_dialog(Dialog {
                title: "Warning".into(),
                text: "Discord didn't open your selected dm. You can still attempt to reopen it \
                       by sending a message in it.\n\nWould you like to send a message in your \
                       selected dm?"
                    .into(),
                kind: DialogKind::YesNo,
                tag: DialogTag::OpenDmMismatch {
                    user_id,
                    expected,
                    received: channel_id,
                },
                input: String::new(),
            });
            return;
        }
        self.handle_act(Act::LaunchDiscord(format!("channels/@me/{}", channel_id)));
    }

    /// Second half of the reopen flow: poke the selected dm so discord
    /// brings it back.
    pub fn send_mismatch_message(&mut self, tag: DialogTag, text: String) {
        let DialogTag::OpenDmMismatch {
            expected, received, ..
        } = tag
        else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let channel_id = if !expected.is_empty() {
            expected
        } else {
            received
        };
        let token = self.user_token.trim().to_string();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let msg = match discord::send_message(&channel_id, &text, &token) {
                Ok(()) => Msg::OpenDmDone {
                    user_id: String::new(),
                    channel_id,
                    expected: None,
                },
                Err(e) => Msg::Fail(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn fetch_dm_user(&mut self, channel_id: String) {
        if self.package.is_none() {
            return;
        }
        let row = self.dm_rows.iter().find(|r| r.channel_id == channel_id);
        let Some(row) = row else { return };
        let unknown = row.unknown_id;
        let user_id = row.user_id.clone();

        if unknown {
            if !self.user_token_ok() {
                return;
            }
            let token = self.user_token.trim().to_string();
            let tx = self.tx.clone();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let msg = match discord::fetch_user_from_channel(&channel_id, &token) {
                    Ok(user) => Msg::DmFetched {
                        channel_id: channel_id.clone(),
                        user,
                    },
                    Err(e) => Msg::Fail(e),
                };
                let _ = tx.send(msg);
                ctx.request_repaint();
            });
        } else {
            if !self.bot_token_ok() {
                return;
            }
            let token = self.bot_token.trim().to_string();
            let tx = self.tx.clone();
            let ctx = self.ctx.clone();
            let channel_id2 = channel_id.clone();
            std::thread::spawn(move || {
                let msg = match discord::fetch_user_from_lookup(&user_id, &token) {
                    Ok(user) => Msg::DmFetched {
                        channel_id: channel_id2,
                        user,
                    },
                    Err(e) => Msg::Fail(e),
                };
                let _ = tx.send(msg);
                ctx.request_repaint();
            });
        }
    }

    fn on_dm_fetched(&mut self, channel_id: String, user: DUser) {
        // Update the row we asked about
        if let Some(row) = self.dm_rows.iter_mut().find(|r| r.channel_id == channel_id) {
            row.user_id = user.id.clone();
            row.username = user.tag();
            row.avatar_url = user.avatar_url();
            row.default_avatar = user.default_avatar_id();
            row.can_fetch = false;
            row.unknown_id = false;
            if let Some(pkg) = &self.package {
                row.note = pkg.user.notes.get(&user.id).cloned().unwrap_or_default();
            }
        }

        // Remember the mapping for later runs
        self.settings.resolve_deleted_user(&channel_id, &user.id);
        self.settings.save();

        // Mark channels talking to the same user as duplicates
        if let Some(pkg) = &self.package {
            let mut targets: Vec<String> = Vec::new();
            for ch in &pkg.channels {
                if !ch.is_dm() || ch.id == channel_id {
                    continue;
                }
                if ch.dm_recipient_id == user.id {
                    ch.set_has_duplicates(true);
                    targets.push(ch.id.clone());
                }
            }
            for id in targets {
                if let Some(ch) = pkg.channels_map.get(&id) {
                    ch.set_has_duplicates(true);
                }
            }
            if let Some(ch) = pkg.channels_map.get(&channel_id) {
                ch.set_has_duplicates(true);
            }
        }
    }

    fn fetch_message_user(&mut self, msg: Arc<DMessage>) {
        if !self.user_token_ok() {
            return;
        }
        let token = self.user_token.trim().to_string();
        let channel_id = msg.channel_id.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = discord::fetch_user_from_channel(&channel_id, &token);
            let msg = match result {
                Ok(user) => Msg::MessageResolved {
                    channel_id: channel_id.clone(),
                    user,
                },
                Err(e) => Msg::Fail(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn on_message_resolved(&mut self, channel_id: String, user: DUser) {
        self.settings.resolve_deleted_user(&channel_id, &user.id);
        self.settings.save();
        if let Some(row) = self.dm_rows.iter_mut().find(|r| r.channel_id == channel_id) {
            row.user_id = user.id.clone();
            row.username = user.tag();
            row.avatar_url = user.avatar_url();
            row.default_avatar = user.default_avatar_id();
            row.unknown_id = false;
            row.can_fetch = false;
        }
    }

    // --- mass delete ----------------------------------------------------

    pub fn start_mass_delete(&mut self) {
        if self.mass_delete.is_some() {
            return;
        }
        if self.results.is_empty() {
            self.push_dialog(Dialog::error("You need to search for something first!"));
            return;
        }
        if !self.user_token_ok() {
            return;
        }

        let stop = Arc::new(AtomicBool::new(false));
        self.mass_delete = Some(MassDelete { stop: stop.clone() });

        let results = self.results.clone();
        let token = self.user_token.trim().to_string();
        let delay = self.mass_delete_delay as u64 * 100;
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();

        std::thread::spawn(move || {
            for msg in results {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if msg.is_deleted() {
                    continue;
                }
                let mut attempts = 0;
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    attempts += 1;
                    if attempts > 20 {
                        let _ = tx.send(Msg::Fail("Too many retries, stopping.".into()));
                        break;
                    }
                    match discord::delete_message(&msg.channel_id, &msg.id, &token) {
                        Ok(_) => {
                            msg.mark_deleted();
                            let _ = tx.send(Msg::MessageDeleted(msg.id.clone()));
                            break;
                        }
                        Err(e) if e.contains("Rate limited") => {
                            // Sleep whatever discord asked for, then retry
                            std::thread::sleep(Duration::from_secs(3));
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::Fail(e));
                            return;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(delay));
            }
            let _ = tx.send(Msg::MassDeleteFinished);
            ctx.request_repaint();
        });
    }

    pub fn stop_mass_delete(&mut self) {
        if let Some(md) = self.mass_delete.take() {
            md.stop.store(true, Ordering::Relaxed);
        }
    }

    // --- file sizes -----------------------------------------------------

    pub fn request_file_size(&mut self, url: &str) {
        if self.file_sizes.contains_key(url) || self.sizes_in_flight.contains(url) {
            return;
        }
        self.sizes_in_flight.insert(url.to_string());
        let url_owned = url.to_string();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let label = head_file_size(&url_owned);
            let _ = tx.send(Msg::FileSize(url_owned, label));
            ctx.request_repaint();
        });
    }

    pub fn file_size(&self, url: &str) -> &str {
        self.file_sizes
            .get(url)
            .map(|s| s.as_str())
            .unwrap_or("...")
    }

    // --- search options window -----------------------------------------

    pub fn open_search_options(&mut self) {
        self.options_draft = Some(self.settings.clone());
    }

    pub fn save_search_options(&mut self) {
        if let Some(draft) = self.options_draft.take() {
            self.settings = draft;
            self.settings.save();
        }
    }

    pub fn discard_search_options(&mut self) {
        self.options_draft = None;
    }

    // --- update check ---------------------------------------------------

    pub fn check_for_updates(&self) {
        if !self.settings.check_for_updates {
            return;
        }
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let found = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .ok()
                .and_then(|c| c.get(UPDATE_URL).send().ok()?.text().ok())
                .map(|text| text.trim().to_string())
                .filter(|v| !v.is_empty() && v != env!("CARGO_PKG_VERSION"));
            let _ = tx.send(Msg::UpdateCheck(found));
            ctx.request_repaint();
        });
    }
}

/// Splits message text into text and link segments for rendering.
pub fn parse_content(content: &str) -> Vec<Seg> {
    let re = regex::Regex::new(r#"https?://[^\s<>"']+"#).unwrap();
    let mut segs = Vec::new();
    let mut last = 0;
    for m in re.find_iter(content) {
        let mut url = m.as_str();
        // Trailing punctuation rarely belongs to the link
        let trimmed = url.trim_end_matches(['.', ',', ';', ':', ')', '!', '?']);
        url = trimmed;
        if m.start() > last {
            segs.push(Seg::Text(content[last..m.start()].to_string()));
        }
        if !url.is_empty() {
            segs.push(Seg::Link(url.to_string()));
        }
        last = m.start() + m.as_str().len();
    }
    if last < content.len() {
        segs.push(Seg::Text(content[last..].to_string()));
    }
    segs
}

/// Same idea as the old Util.SizeSuffix.
pub fn size_suffix(value: i64, decimals: usize) -> String {
    const SUFFIXES: [&str; 9] = ["bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    if value == 0 {
        return "0 bytes".to_string();
    }
    let negative = value < 0;
    let value = value.unsigned_abs() as f64;
    let mut mag = (value.log(1024.0) as usize).min(SUFFIXES.len() - 1);
    let mut adjusted = value / 1024f64.powi(mag as i32);
    if (adjusted.round() as i64) >= 1000 {
        mag += 1;
        adjusted /= 1024.0;
    }
    format!(
        "{}{:.dec$} {}",
        if negative { "-" } else { "" },
        adjusted,
        SUFFIXES[mag],
        dec = decimals
    )
}

fn head_file_size(url: &str) -> String {
    match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => match client.head(url).send() {
            Ok(res) if res.status() == 200 => res
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok())
                .map(|len| size_suffix(len, 2))
                .unwrap_or_else(|| "Unknown size".into()),
            Ok(res) if res.status() == 404 => "Attachment deleted".into(),
            _ => "Unknown size".into(),
        },
        Err(_) => "Unknown size".into(),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_messages();
        self.image_cache.poll(&ctx);
        if !self.update_checked {
            self.update_checked = true;
            self.check_for_updates();
        }

        let mut title = format!("{} v{}", APP_NAME, env!("CARGO_PKG_VERSION"));
        if self.update_url.is_some() {
            title.push_str(" (Update Available)");
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        let mut selected = self.tab;
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                self.tab_buttons(ui, &mut selected);
            });
        });
        self.tab = selected;

        egui::CentralPanel::default().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            match self.tab {
                Tab::Load => ui::load_tab(self, ui),
                Tab::Messages => self.ui_messages(ui, &ctx),
                Tab::Images => self.ui_images(ui, &ctx),
                Tab::Servers => self.ui_servers(ui, &ctx),
                Tab::Dms => self.ui_dms(ui, &ctx),
                Tab::Settings => self.ui_settings(ui, &ctx),
            }
        });

        ui::search_options_window(self, &ctx);
        ui::mass_delete_window(self, &ctx);
        ui::dialogs(self, &ctx);

        // Keep animating while downloads are running
        if self.searching || self.loading {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.settings.save();
    }
}

impl App {
    fn tab_buttons(&mut self, ui: &mut Ui, selected: &mut Tab) {
        let dm_count = self.dm_rows.len();
        let guild_count = self.guild_rows.len();
        let missing = self
            .guild_data
            .as_ref()
            .and_then(|g| g.activity_status.missing_data)
            .unwrap_or(false);

        let labels = [
            (Tab::Load, "Load Data Package".to_string()),
            (Tab::Messages, "Messages".to_string()),
            (Tab::Images, "Images".to_string()),
            (
                Tab::Servers,
                format!(
                    "Servers - {}{}",
                    guild_count,
                    if missing { "+" } else { "" }
                ),
            ),
            (Tab::Dms, format!("Direct Messages - {}", dm_count)),
            (Tab::Settings, "Settings".to_string()),
        ];

        for (tab, label) in labels {
            let response = ui.selectable_label(self.tab == tab, label);
            if response.clicked() {
                *selected = tab;
                if tab == Tab::Servers {
                    self.maybe_warn_analytics();
                }
            }
        }
    }
}
