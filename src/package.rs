use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex, OnceLock};
use zip::ZipArchive;

use crate::models::*;

/// Progress info the UI polls while we parse in a background thread.
#[derive(Default, Clone)]
pub struct LoadStatus {
    pub progress: usize,
    pub max: usize,
    pub status: String,
    pub finished: bool,
}

pub type StatusHandle = Arc<Mutex<LoadStatus>>;

pub fn new_status() -> StatusHandle {
    Arc::new(Mutex::new(LoadStatus::default()))
}

pub struct DataPackage {
    pub user: DUser,
    pub avatar: Option<Vec<u8>>,
    pub channels: Vec<Arc<DChannel>>,
    pub channels_map: HashMap<String, Arc<DChannel>>,
    pub messages_map: HashMap<String, Arc<DMessage>>,
    pub users_map: HashMap<String, DUser>,
    pub image_attachments: Vec<Arc<DAttachment>>,
    pub guild_names_map: HashMap<String, String>,
    pub creation_time: DateTime<Local>,
    pub total_messages: usize,
    pub uses_unsigned_cdn_links: bool,
}

impl Default for DataPackage {
    fn default() -> Self {
        Self {
            user: DUser {
                id: String::new(),
                username: None,
                display_name: None,
                discriminator: None,
                avatar_hash: None,
                relationships: Vec::new(),
                notes: HashMap::new(),
            },
            avatar: None,
            channels: Vec::new(),
            channels_map: HashMap::new(),
            messages_map: HashMap::new(),
            users_map: HashMap::new(),
            image_attachments: Vec::new(),
            guild_names_map: HashMap::new(),
            creation_time: Local::now(),
            total_messages: 0,
            uses_unsigned_cdn_links: false,
        }
    }
}

/// Result of the second pass, which reads the activity folder.
#[derive(Default)]
pub struct GuildData {
    pub joined_guilds: Vec<DAnalyticsGuild>,
    pub activity_status: ActivityDataStatus,
}

fn messages_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)messages/(c?(\d+))/messages\.(csv|json)").unwrap())
}

fn channel_json_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)messages/(c?\d+)/channel\.json").unwrap())
}

fn avatar_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)account/avatar\.[a-z]+").unwrap())
}

fn reporting_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)activity/reporting/events.+\.json").unwrap())
}

fn modeling_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)activity/modeling/events.+\.json").unwrap())
}

fn analytics_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)activity/analytics/events.+\.json").unwrap())
}

fn read_entry(zip: &mut ZipArchive<File>, name: &str) -> Result<String, String> {
    let mut entry = zip.by_name(name).map_err(|e| format!("{}: {}", name, e))?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|e| format!("{}: {}", name, e))?;
    Ok(text)
}

/// Entry names vary in casing between package versions, so compare loosely.
fn find_entry(names: &[String], wanted: &str) -> Option<String> {
    names
        .iter()
        .find(|n| n.eq_ignore_ascii_case(wanted))
        .cloned()
}

fn entry_time(zip: &mut ZipArchive<File>, name: &str) -> Option<DateTime<Local>> {
    let entry = zip.by_name(name).ok()?;
    let dt = entry.last_modified()?;
    let naive: NaiveDateTime = TryFrom::try_from(dt).ok()?;
    Some(
        naive
            .and_local_timezone(Local)
            .earliest()
            .unwrap_or_else(|| Local.from_utc_datetime(&naive)),
    )
}

fn set_status(status: &StatusHandle, progress: usize, max: usize, text: String) {
    let mut st = status.lock().unwrap();
    st.progress = progress;
    st.max = max;
    st.status = text;
}

#[derive(Deserialize)]
struct JsonMessage {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Timestamp")]
    timestamp: String,
    #[serde(rename = "Contents")]
    contents: Option<String>,
    #[serde(rename = "Attachments")]
    attachments: Option<String>,
}

/// Parses the whole package. Runs on a worker thread, progress goes into `status`.
pub fn load(path: &str, status: &StatusHandle) -> Result<DataPackage, String> {
    let start = Local::now();
    let file = File::open(path).map_err(|e| format!("Couldn't open file: {}", e))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("Not a valid zip file: {}", e))?;
    let total_entries = zip.len();

    let names: Vec<String> = zip
        .file_names()
        .filter_map(|n| n.ok().map(|n| n.to_string()))
        .collect();

    // The messages folder is what this whole tool is about, check it first.
    let messages_index = find_entry(&names, "messages/index.json")
        .ok_or_else(|| "Invalid data package: missing messages/index.json".to_string())?;
    let creation_time = entry_time(&mut zip, &messages_index).unwrap_or_else(Local::now);

    let mut pkg = DataPackage {
        creation_time,
        ..Default::default()
    };

    // Account folder might be missing from a partial package
    let user_entry = find_entry(&names, "account/user.json");
    if let Some(user_entry) = user_entry {
        let text = read_entry(&mut zip, &user_entry)?;
        pkg.user = serde_json::from_str(&text).map_err(|e| format!("account/user.json: {}", e))?;
    } else {
        pkg.user = DUser {
            username: Some("You".into()),
            display_name: Some("You".into()),
            discriminator: Some("0".into()),
            ..Default::default()
        };
        set_status(status, 0, total_entries, "Determining user id".into());
        pkg.user.id = guess_user_id(&mut zip, &names, status, total_entries)?;
    }

    // Friends and blocked users double as our user lookup table
    for rel in &pkg.user.relationships {
        if !rel.user.is_deleted_user() {
            pkg.users_map.insert(rel.user.id.clone(), rel.user.clone());
        }
    }

    let channel_names_text = read_entry(&mut zip, &messages_index)?;
    let channel_names_map: HashMap<String, String> = serde_json::from_str(&channel_names_text)
        .map_err(|e| format!("messages/index.json: {}", e))?;

    if let Some(guilds_index) = find_entry(&names, "servers/index.json") {
        let text = read_entry(&mut zip, &guilds_index)?;
        pkg.guild_names_map = serde_json::from_str(&text).unwrap_or_default();
    }

    let mut avatar: Option<Vec<u8>> = None;

    for i in 0..total_entries {
        let (name, is_file) = {
            let entry = zip
                .by_index(i)
                .map_err(|e| format!("zip entry {}: {}", i, e))?;
            (
                entry.name().map(|n| n.to_string()).unwrap_or_default(),
                entry.is_file(),
            )
        };
        if !is_file {
            continue;
        }
        set_status(
            status,
            i + 1,
            total_entries,
            format!("Reading {}\n{}/{}", name, i + 1, total_entries),
        );

        if let Some(caps) = messages_re().captures(&name) {
            let channel_folder = caps[1].to_string();
            let channel_id = caps[2].to_string();
            let extension = caps[3].to_string();

            let channel_entry =
                find_entry(&names, &format!("messages/{}/channel.json", channel_folder))
                    .ok_or_else(|| format!("Missing channel.json for channel {}", channel_id))?;
            let channel_text = read_entry(&mut zip, &channel_entry)?;
            let mut channel: DChannel =
                serde_json::from_str(&channel_text).map_err(|e| format!("channel.json: {}", e))?;

            // Old packages literally write "Deleted User" as a recipient id
            if channel.is_dm() {
                for id in &mut channel.recipient_ids {
                    if id.as_str() == "Deleted User" {
                        *id = DELETED_USER_ID.to_string();
                    }
                }
            }

            let map_name = channel_names_map.get(&channel_id).cloned();

            if channel.is_dm() {
                let recipient = channel.other_dm_recipient(&pkg.user.id)?;
                channel.dm_recipient_id = recipient.clone();

                // Recreate users we only know the name of from the channel map
                let known = pkg.users_map.contains_key(&recipient);
                if !known && recipient != DELETED_USER_ID {
                    let from_map = map_name.as_deref().and_then(parse_dm_channel_name);
                    if let Some((username, discrim)) = from_map {
                        pkg.users_map.insert(
                            recipient.clone(),
                            DUser {
                                id: recipient.clone(),
                                username: Some(username),
                                discriminator: Some(discrim),
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            if channel.name.is_none() && map_name.is_some() {
                // e.g. "Unknown channel in <server name>"
                channel.fallback_name = map_name;
            }

            let content = read_entry(&mut zip, &name)?;
            match extension.as_str() {
                "csv" => load_messages_from_csv(&content, &mut channel),
                _ => load_messages_from_json(&content, &mut channel),
            }

            for msg in &channel.messages {
                pkg.messages_map.insert(msg.id.clone(), msg.clone());
                for att in &msg.attachments {
                    if att.is_image() {
                        pkg.image_attachments.push(att.clone());
                    }
                }
            }

            pkg.total_messages += channel.messages.len();
            pkg.channels_map
                .insert(channel.id.clone(), Arc::new(channel));
        } else if avatar_re().is_match(&name) {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| format!("zip entry {}: {}", i, e))?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            avatar = Some(buf);
        }
    }

    // HashMap iteration order is random, so put the channels back in zip order
    let mut ordered: Vec<Arc<DChannel>> = Vec::new();
    for name in &names {
        if let Some(caps) = messages_re().captures(name) {
            if let Some(ch) = pkg.channels_map.get(&caps[2]) {
                ordered.push(ch.clone());
            }
        }
    }
    pkg.channels = ordered;

    pkg.avatar = avatar;
    pkg.image_attachments
        .sort_by_key(|a| std::cmp::Reverse(a.message_id.parse::<u64>().unwrap_or(0)));
    pkg.uses_unsigned_cdn_links = pkg
        .image_attachments
        .first()
        .map(|a| !a.is_signed())
        .unwrap_or(false);

    let elapsed = (Local::now() - start).num_seconds().max(0);
    set_status(
        status,
        total_entries,
        total_entries,
        format!(
            "Finished! Parsed {} messages in {}s\nPackage created at: {}",
            group_digits(pkg.total_messages),
            elapsed,
            pkg.creation_time.format("%d.%m.%Y")
        ),
    );
    status.lock().unwrap().finished = true;
    Ok(pkg)
}

/// Partial packages have no Account folder, so count dm recipients instead.
fn guess_user_id(
    zip: &mut ZipArchive<File>,
    names: &[String],
    status: &StatusHandle,
    total: usize,
) -> Result<String, String> {
    let mut counts: HashMap<String, i32> = HashMap::new();

    for (i, name) in names.iter().enumerate() {
        if i % 50 == 0 {
            set_status(
                status,
                i,
                total,
                format!("Determining user id\n{}/{}", i, total),
            );
        }
        if !channel_json_re().is_match(name) {
            continue;
        }
        let text = read_entry(zip, name)?;
        let channel: DChannel = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !channel.is_dm() {
            continue;
        }
        for id in &channel.recipient_ids {
            if id != DELETED_USER_ID {
                *counts.entry(id.clone()).or_insert(0) += 1;
            }
        }
    }

    most_common_recipient(&counts).ok_or_else(|| {
        "Unable to determine your user id! Please request a new data package and make sure to \
         tick the \"Account\" option."
            .to_string()
    })
}

fn push_message(
    channel: &mut DChannel,
    id: &str,
    timestamp: &str,
    contents: &str,
    attachments: &str,
) {
    let ts = parse_flexible(timestamp).unwrap_or_else(|| Local.timestamp_opt(0, 0).unwrap());
    let mut msg = DMessage::new(id.to_string(), ts, contents.to_string(), channel.id.clone());
    if !attachments.is_empty() {
        for url in attachments.split(' ') {
            if url.is_empty() {
                continue;
            }
            let att = DAttachment::new(url, &msg.id);
            msg.attachments.push(Arc::new(att));
        }
    }
    channel.messages.push(Arc::new(msg));
}

fn load_messages_from_csv(content: &str, channel: &mut DChannel) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(content.as_bytes());
    for result in reader.records() {
        let record = match result {
            Ok(r) => r,
            Err(_) => continue,
        };
        let get = |i: usize| record.get(i).unwrap_or("").to_string();
        if get(0) == "ID" {
            continue; // header row
        }
        if record.len() < 3 {
            continue;
        }
        push_message(channel, &get(0), &get(1), &get(2), &get(3));
    }
}

fn load_messages_from_json(content: &str, channel: &mut DChannel) {
    let messages: Vec<JsonMessage> = match serde_json::from_str(content) {
        Ok(m) => m,
        Err(_) => return,
    };
    for m in messages {
        push_message(
            channel,
            &m.id,
            &m.timestamp,
            m.contents.as_deref().unwrap_or(""),
            m.attachments.as_deref().unwrap_or(""),
        );
    }
}

/// Second pass: figure out which activity file holds the server history and
/// turn it into the joined servers list.
///
/// Which file to use depends on when the package was made, see issue #12:
/// packages before 05.2024 use activity/reporting, after 06.2024
/// activity/analytics, and in-between we peek at the account age.
pub fn load_guilds(path: &str, status: &StatusHandle) -> Result<GuildData, String> {
    let file = File::open(path).map_err(|e| format!("Couldn't open file: {}", e))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("Not a valid zip: {}", e))?;
    let names: Vec<String> = zip
        .file_names()
        .filter_map(|n| n.ok().map(|n| n.to_string()))
        .collect();

    let reporting = names.iter().find(|n| reporting_re().is_match(n)).cloned();
    let modeling = names.iter().find(|n| modeling_re().is_match(n)).cloned();
    let analytics = names.iter().find(|n| analytics_re().is_match(n)).cloned();

    let Some(reporting) = reporting else {
        // Partial package without an activity folder
        status.lock().unwrap().finished = true;
        return Ok(GuildData::default());
    };

    let reporting_time = entry_time(&mut zip, &reporting).unwrap_or_else(Local::now);
    let mut activity_status = ActivityDataStatus::default();

    let start_date = Local.with_ymd_and_hms(2024, 5, 1, 0, 0, 0).unwrap();
    let end_date = Local.with_ymd_and_hms(2024, 6, 30, 23, 59, 59).unwrap();

    let mut file_to_use = if reporting_time < start_date {
        activity_status.missing_data = Some(false);
        ActivityFile::Reporting
    } else if reporting_time > end_date {
        ActivityFile::Analytics
    } else {
        // Fuzzy window: decide by account age and whether old events survived
        let (register_date, has_old_events) = scan_reporting(&mut zip, &reporting, status)?;
        let register_date = register_date.ok_or("Couldn't find the register event")?;
        if register_date.year() == 2024 || has_old_events {
            activity_status.missing_data = Some(false);
            ActivityFile::Reporting
        } else {
            ActivityFile::Analytics
        }
    };

    if file_to_use == ActivityFile::Analytics && analytics.is_none() {
        if modeling.is_none() {
            // Analytics turned off, everything old is gone from the package
            activity_status.status = AnalyticsStatus::DisabledNow;
            activity_status.missing_data = Some(true);
            file_to_use = ActivityFile::Reporting;
        } else {
            activity_status.status = AnalyticsStatus::PartiallyDisabledNow;
            activity_status.missing_data = Some(true);
            file_to_use = ActivityFile::Modeling;
        }
    }

    let entry_to_use = match file_to_use {
        ActivityFile::Analytics => analytics,
        ActivityFile::Modeling => modeling,
        ActivityFile::Reporting => Some(reporting),
    }
    .ok_or("No activity file found")?;

    let entry = zip
        .by_name(&entry_to_use)
        .map_err(|e| format!("{}: {}", entry_to_use, e))?;
    let entry_size = entry.size().max(1);
    let reader = BufReader::new(entry);

    let mut data = GuildData {
        activity_status,
        ..Default::default()
    };
    let mut accepted_invites: Vec<DAnalyticsEvent> = Vec::new();

    let mut line = String::new();
    let mut reader = reader;
    let mut bytes_read: u64 = 0;
    loop {
        line.clear();
        let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes_read += n as u64;
        let pct = ((bytes_read as f64 / entry_size as f64) * 100.0) as usize;
        set_status(status, pct.min(100), 100, String::new());

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        process_analytics_line(trimmed, &mut data, &mut accepted_invites);
    }

    // Merge invite accepts into the guild list, keeping the earliest join date
    accepted_invites.sort_by_key(|e| e.timestamp);
    for event in &accepted_invites {
        let Some(guild_id) = &event.guild_id else {
            continue;
        };
        if let Some(guild) = data.joined_guilds.iter_mut().find(|g| &g.id == guild_id) {
            if let Some(code) = &event.invite_code {
                if !guild.invites.contains(code) {
                    guild.invites.push(code.clone());
                }
            }
            if event.timestamp < guild.timestamp {
                guild.timestamp = event.timestamp;
            }
        } else {
            data.joined_guilds.push(DAnalyticsGuild {
                id: guild_id.clone(),
                join_type: "invite".into(),
                join_method: None,
                application_id: None,
                location: None,
                invites: event.invite_code.clone().into_iter().collect(),
                timestamp: event.timestamp,
            });
        }
    }

    data.joined_guilds
        .sort_by_key(|g| std::cmp::Reverse(g.timestamp));
    status.lock().unwrap().finished = true;
    Ok(data)
}

/// Reads just enough of activity/reporting to find the register event and
/// whether any event from before 2024 survived.
fn scan_reporting(
    zip: &mut ZipArchive<File>,
    reporting: &str,
    status: &StatusHandle,
) -> Result<(Option<DateTime<Local>>, bool), String> {
    let entry = zip
        .by_name(reporting)
        .map_err(|e| format!("{}: {}", reporting, e))?;
    let entry_size = entry.size().max(1);
    let mut reader = BufReader::new(entry);

    let ignore_prefixes = ["payment_", "subscription_"];
    let mut register_date = None;
    let mut has_old_events = false;
    let mut bytes_read: u64 = 0;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes_read += n as u64;
        let pct = ((bytes_read as f64 / entry_size as f64) * 100.0) as usize;
        set_status(status, pct.min(100), 100, String::new());

        let trimmed = line.trim();
        if trimmed.starts_with("{\"event_type\":\"register\"") {
            if let Some(ev) = parse_event(trimmed) {
                register_date = Some(ev.timestamp);
            }
        } else if !has_old_events && !trimmed.is_empty() {
            if let Some(ev) = parse_event(trimmed) {
                if ev.timestamp.year() < 2024
                    && !ignore_prefixes.iter().any(|p| ev.event_type.starts_with(p))
                {
                    has_old_events = true;
                }
            }
        }

        if register_date.is_some() && has_old_events {
            break;
        }
    }

    Ok((register_date, has_old_events))
}

fn parse_event(line: &str) -> Option<DAnalyticsEvent> {
    let mut ev: DAnalyticsEvent = serde_json::from_str(line).ok()?;
    ev.parse_timestamp();
    Some(ev)
}

/// Cheap prefix check first, we only care about four event types.
fn process_analytics_line(
    line: &str,
    data: &mut GuildData,
    accepted_invites: &mut Vec<DAnalyticsEvent>,
) {
    let interesting = line.starts_with("{\"event_type\":\"guild_joined")
        || line.starts_with("{\"event_type\":\"create_guild")
        || line.starts_with("{\"event_type\":\"accepted_instant_invite")
        || line.starts_with("{\"event_type\":\"privacy_control_updated");
    if !interesting {
        return;
    }
    let Some(event) = parse_event(line) else {
        return;
    };

    match event.event_type.as_str() {
        "guild_joined" | "guild_joined_pending" => {
            let Some(guild_id) = &event.guild_id else { return };
            if let Some(guild) = data.joined_guilds.iter_mut().find(|g| &g.id == guild_id) {
                if let Some(code) = &event.invite_code {
                    if !guild.invites.contains(code) {
                        guild.invites.push(code.clone());
                    }
                }
                if event.timestamp < guild.timestamp {
                    guild.timestamp = event.timestamp;
                }
            } else {
                data.joined_guilds.push(DAnalyticsGuild {
                    id: guild_id.clone(),
                    join_type: event.join_type.unwrap_or_default(),
                    join_method: event.join_method,
                    application_id: event.application_id,
                    location: event.location,
                    invites: event.invite_code.into_iter().collect(),
                    timestamp: event.timestamp,
                });
            }
        }
        "create_guild" => {
            let Some(guild_id) = &event.guild_id else { return };
            data.joined_guilds.push(DAnalyticsGuild {
                id: guild_id.clone(),
                join_type: "created by you".into(),
                join_method: None,
                application_id: None,
                location: None,
                invites: Vec::new(),
                timestamp: event.timestamp,
            });
        }
        "accepted_instant_invite" => {
            if event.guild_id.is_some() {
                accepted_invites.push(event);
            }
        }
        "privacy_control_updated"
            // Only present in activity/analytics, which means analytics was
            // toggled at some point in the past
            if event.control_type.as_deref() == Some("usage_statistics") =>
        {
            data.activity_status.missing_data.get_or_insert(true);
            data.activity_status.status = AnalyticsStatus::DisabledBefore;
            let cutoff = data.activity_status.cutoff_date;
            if cutoff.is_none() || Some(event.timestamp) < cutoff {
                data.activity_status.cutoff_date = Some(event.timestamp);
            }
        }
        _ => {}
    }
}

/// "1 234 567" style, matches what the old tool showed.
fn group_digits(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
