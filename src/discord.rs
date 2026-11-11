use base64::Engine;
use reqwest::blocking::Client;
use reqwest::Method;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

use crate::models::{DAttachment, DUser};

const CHROME_VERSION_URL: &str =
    "https://versionhistory.googleapis.com/v1/chrome/platforms/win/channels/stable/versions";
const DISCORD_APP_URL: &str = "https://discord.com/app";

/// Browser impersonation headers, filled in once on first use.
pub struct Headers {
    pub user_agent: String,
    pub browser_version: String,
    pub client_build_number: i64,
    pub super_properties: String,
    pub greased_version: String,
}

static HEADERS: OnceLock<Headers> = OnceLock::new();
static CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build http client")
    })
}

/// Grabs the latest chrome version and client build number. Falls back to
/// fixed values when offline, most things still work without exact headers.
pub fn headers() -> &'static Headers {
    HEADERS.get_or_init(|| {
        let chrome = fetch_chrome_version().unwrap_or_else(|| "131".to_string());
        let build = fetch_build_number().unwrap_or(99999);
        let browser_version_full = format!("{}.0.0.0", chrome);
        let user_agent = format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/{} Safari/537.36",
            browser_version_full
        );
        let super_properties = build_super_properties(&user_agent, &browser_version_full, build);
        Headers {
            user_agent,
            browser_version: chrome.clone(),
            client_build_number: build,
            super_properties,
            greased_version: greased_chrome_version(&chrome),
        }
    })
}

fn fetch_chrome_version() -> Option<String> {
    let res = client().get(CHROME_VERSION_URL).send().ok()?;
    let body = res.text().ok()?;
    let data: Value = serde_json::from_str(&body).ok()?;
    let latest = data
        .get("versions")?
        .as_array()?
        .first()?
        .get("version")?
        .as_str()?;
    Some(latest.split('.').next().unwrap_or("131").to_string())
}

fn fetch_build_number() -> Option<i64> {
    let res = client().get(DISCORD_APP_URL).send().ok()?;
    let body = res.text().ok()?;
    let re = regex::Regex::new(r#""BUILD_NUMBER":"(\d+)""#).ok()?;
    re.captures(&body)?.get(1)?.as_str().parse().ok()
}

fn build_super_properties(user_agent: &str, browser_version: &str, build: i64) -> String {
    let props = json!({
        "os": "Windows",
        "browser": "Chrome",
        "device": "",
        "system_locale": "en-US",
        "has_client_mods": false,
        "browser_user_agent": user_agent,
        "browser_version": browser_version,
        "os_version": "10",
        "referrer": "",
        "referring_domain": "",
        "referrer_current": "",
        "referring_domain_current": "",
        "release_channel": "stable",
        "client_build_number": build,
        "client_event_source": null,
        "client_launch_id": Uuid::new_v4().to_string(),
        "client_heartbeat_session_id": Uuid::new_v4().to_string(),
        "launch_signature": clean_launch_signature(),
        "client_app_state": "focused"
    });
    base64::engine::general_purpose::STANDARD.encode(props.to_string())
}

/// Same trick Discord's client uses: random uuid with some bits cleared.
fn clean_launch_signature() -> String {
    const DETECTION_BITS: u128 = 664_939_786_591_977_264_366_699_312_723_265_536;
    let raw = Uuid::new_v4().as_u128();
    let masked = raw & !DETECTION_BITS;
    Uuid::from_u128(masked).to_string()
}

/// Chrome's client hints header, the version gets shuffled around a bit.
fn greased_chrome_version(version: &str) -> String {
    let seed: usize = version.parse().unwrap_or(131);
    let greasy_chars = [' ', '(', ':', '-', '.', '/', ')', ';', '=', '?', '_'];
    let greased_versions = ["8", "99", "24"];

    let brand = format!(
        "Not{}A{}Brand",
        greasy_chars[seed % greasy_chars.len()],
        greasy_chars[(seed + 1) % greasy_chars.len()]
    );
    let brand_version = greased_versions[seed % greased_versions.len()];

    let mut list: Vec<(&str, &str)> = vec![
        (&brand, brand_version),
        ("Chromium", version),
        ("Google Chrome", version),
    ];
    // Rotate the list based on the seed, same order as the real client
    let orders: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let order = orders[seed % orders.len()];
    let original = list.clone();
    for (i, &idx) in order.iter().enumerate() {
        list[idx] = original[i];
    }

    list.iter()
        .map(|(name, ver)| format!("\"{}\";v=\"{}\"", name, ver))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Headers every request to discord.com sends, unless told otherwise.
pub fn default_browser_headers(extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let h = headers();
    let mut out: Vec<(String, String)> = vec![
        ("Accept".into(), "*/*".into()),
        ("Accept-Language".into(), "en-US,en;q=0.5".into()),
        ("sec-ch-ua".into(), h.greased_version.clone()),
        ("sec-ch-ua-mobile".into(), "?0".into()),
        ("sec-ch-ua-platform".into(), "\"Windows\"".into()),
        ("sec-fetch-dest".into(), "empty".into()),
        ("sec-fetch-mode".into(), "cors".into()),
        ("sec-fetch-site".into(), "same-origin".into()),
        ("User-Agent".into(), h.user_agent.clone()),
        ("x-debug-options".into(), "bugReporterEnabled".into()),
        ("x-discord-locale".into(), "en-US".into()),
        ("x-discord-timezone".into(), "America/New_York".into()),
        ("x-super-properties".into(), h.super_properties.clone()),
    ];
    for (k, v) in extra {
        out.push((k.to_string(), v.to_string()));
    }
    out
}

pub struct Response {
    pub status: u16,
    pub body: String,
}

/// One place where requests go out, mirrors the old DRequest class.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
    include_default_headers: bool,
) -> Result<Response, String> {
    let method = Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut req = client().request(method, url);

    if include_default_headers {
        for (k, v) in default_browser_headers(headers) {
            req = req.header(k, v);
        }
    } else {
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
    }

    if let Some(body) = body {
        req = req.header("Content-Type", "application/json").body(body);
    }

    let res = req.send().map_err(|e| format!("Request error: {}", e))?;
    let status = res.status().as_u16();
    let text = res.text().unwrap_or_default();
    Ok(Response { status, body: text })
}

/// The first part of a token is the owner's id, base64 encoded.
pub fn validate_token(token: &str, user_id: Option<&str>) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let mut id_part = parts[0].to_string();
    while !id_part.len().is_multiple_of(4) {
        id_part.push('=');
    }
    match base64::engine::general_purpose::STANDARD.decode(&id_part) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(id) => match user_id {
                Some(want) => id == want,
                None => true,
            },
            Err(_) => false,
        },
        Err(_) => false,
    }
}

pub fn snowflake_to_timestamp(snowflake: &str) -> Option<i64> {
    let id: i64 = snowflake.parse().ok()?;
    Some((id >> 22) + 1_420_070_400_000)
}

pub fn timestamp_to_snowflake(ms: i64) -> String {
    ((ms - 1_420_070_400_000) << 22).to_string()
}

// --- flows the UI triggers -------------------------------------------------

pub enum DeleteOutcome {
    Deleted,
    /// Discord says the message is already gone
    AlreadyGone,
}

pub fn delete_message(
    channel_id: &str,
    message_id: &str,
    token: &str,
) -> Result<DeleteOutcome, String> {
    let url = format!(
        "https://discord.com/api/v9/channels/{}/messages/{}",
        channel_id, message_id
    );
    let res = request("DELETE", &url, &[("Authorization", token)], None, true)?;

    match res.status {
        404 | 204 => Ok(DeleteOutcome::AlreadyGone),
        403 => Err("You don't have access to this message.".into()),
        429 => Err("Rate limited by Discord, try again later.".into()),
        200..=299 => Ok(DeleteOutcome::Deleted),
        _ => Err(discord_error(&res)),
    }
}

/// Turns a discord error body into something we can show the user.
fn discord_error(res: &Response) -> String {
    if let Ok(data) = serde_json::from_str::<Value>(&res.body) {
        let code = data.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        let message = data
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return match code {
            50001 => "You don't have access to the channel of this message!".to_string(),
            50083 => "This message is in an archived thread!".to_string(),
            _ => format!("Discord error ({}): {}", code, message),
        };
    }
    format!("Request error: {} {}", res.status, res.body)
}

/// Opens (or reopens) a dm and returns the channel id discord picked.
pub fn open_dm(user_id: &str, token: &str) -> Result<String, String> {
    let ctx = base64::engine::general_purpose::STANDARD.encode("{}");
    let body = json!({ "recipients": [user_id] }).to_string();
    let res = request(
        "POST",
        "https://discord.com/api/v9/users/@me/channels",
        &[
            ("Authorization", token),
            ("X-Context-Properties", ctx.as_str()),
        ],
        Some(body),
        true,
    )?;

    if res.status != 200 {
        return Err(format!("Request error: {} {}", res.status, res.body));
    }
    let data: Value = serde_json::from_str(&res.body).map_err(|e| e.to_string())?;
    data.get("id")
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Discord returned no channel id".to_string())
}

pub fn send_message(channel_id: &str, content: &str, token: &str) -> Result<(), String> {
    let body = json!({
        "content": content,
        "flags": 0,
        "nonce": timestamp_to_snowflake(chrono::Local::now().timestamp_millis()),
        "tts": false
    })
    .to_string();
    let url = format!(
        "https://discord.com/api/v9/channels/{}/messages",
        channel_id
    );
    let res = request("POST", &url, &[("Authorization", token)], Some(body), true)?;
    if res.status != 200 {
        return Err(format!("Failed to send dm: {} {}", res.status, res.body));
    }
    Ok(())
}

pub fn fetch_user_from_channel(channel_id: &str, token: &str) -> Result<DUser, String> {
    let url = format!("https://discord.com/api/v9/channels/{}", channel_id);
    let res = request("GET", &url, &[("Authorization", token)], None, true)?;
    if res.status != 200 {
        return Err(format!("Status code {} - {}", res.status, res.body));
    }
    let data: Value = serde_json::from_str(&res.body).map_err(|e| e.to_string())?;
    let recipient = data
        .get("recipients")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(|| "No recipients in response".to_string())?;
    serde_json::from_value(recipient).map_err(|e| e.to_string())
}

pub fn fetch_user_from_lookup(user_id: &str, bot_token: &str) -> Result<DUser, String> {
    let url = format!("https://discord.com/api/v9/users/{}", user_id);
    let res = request(
        "GET",
        &url,
        &[("Authorization", &format!("Bot {}", bot_token))],
        None,
        false,
    )?;
    if res.status != 200 {
        return Err(format!("Status code {} - {}", res.status, res.body));
    }
    serde_json::from_str(&res.body).map_err(|e| e.to_string())
}

/// Old packages have unsigned attachment urls which404 now, a bot token can
/// ask discord for fresh ones. Batches of 50, like the api expects.
pub fn refresh_attachments(
    attachments: &[std::sync::Arc<DAttachment>],
    bot_token: &str,
) -> Result<usize, String> {
    let batch_size = 50;
    let mut refreshed = 0;

    for (i, chunk) in attachments.chunks(batch_size).enumerate() {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
        let owned: Vec<String> = chunk.iter().map(|a| a.url()).collect();
        let urls: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        let body = json!({ "attachment_urls": urls }).to_string();
        let res = request(
            "POST",
            "https://discord.com/api/v9/attachments/refresh-urls",
            &[("Authorization", &format!("Bot {}", bot_token))],
            Some(body),
            false,
        )?;

        if res.status != 200 {
            return Err(format!(
                "Failed to refresh attachments: status code {} - {}",
                res.status, res.body
            ));
        }

        let data: Value = serde_json::from_str(&res.body).map_err(|e| e.to_string())?;
        let list = data
            .get("refreshed_urls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for entry in list {
            let original = entry.get("original").and_then(|o| o.as_str()).unwrap_or("");
            let fresh = entry
                .get("refreshed")
                .and_then(|o| o.as_str())
                .unwrap_or("");
            if original.is_empty() || fresh.is_empty() {
                continue;
            }
            if let Some(att) = attachments.iter().find(|a| a.url() == original) {
                att.set_url(fresh.to_string());
                refreshed += 1;
            }
        }
    }

    Ok(refreshed)
}

/// Hands a url to the OS, discord:// links open the desktop app.
pub fn open_external(url: &str) -> Result<(), String> {
    open::that(url).map_err(|e| format!("Couldn't open {}: {}", url, e))
}

/// Opens a channel/user in the chosen discord instance, or the browser.
pub fn launch_discord(path: &str, instance: &str) -> Result<(), String> {
    if instance == "default" || instance.is_empty() {
        return open_external(&format!("discord://-/{}", path));
    }

    if let Some(host) = instance.strip_prefix("web_") {
        let hostname = match host {
            "stable" => "discord.com",
            "ptb" => "ptb.discord.com",
            "canary" => "canary.discord.com",
            _ => return Err(format!("Invalid settings value: {}", instance)),
        };
        return open_external(&format!("https://{}/{}", hostname, path));
    }

    launch_desktop_discord(path, instance)
}

/// Finds the installed desktop app and hands the url to it.
#[cfg(windows)]
fn launch_desktop_discord(path: &str, instance: &str) -> Result<(), String> {
    let folder_name = match instance {
        "stable" => "Discord",
        "ptb" => "DiscordPTB",
        "canary" => "DiscordCanary",
        _ => return Err(format!("Invalid settings value: {}", instance)),
    };
    let base = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join(folder_name);
    if !base.exists() {
        return Err(format!(
            "Couldn't find Discord folder path for {}",
            instance
        ));
    }
    let entries = std::fs::read_dir(&base).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let exe = entry.path().join(format!("{}.exe", folder_name));
        if name.starts_with("app-") && exe.exists() {
            std::process::Command::new(exe)
                .args(["--url", "--", &format!("discord://-/{}", path)])
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err("Couldn't find the Discord exe file".into())
}

/// No desktop app to talk to outside windows, use the protocol instead.
#[cfg(not(windows))]
fn launch_desktop_discord(path: &str, _instance: &str) -> Result<(), String> {
    open_external(&format!("discord://-/{}", path))
}
