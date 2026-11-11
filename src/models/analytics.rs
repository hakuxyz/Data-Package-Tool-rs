use chrono::{DateTime, Local, NaiveDate, TimeZone};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalyticsStatus {
    #[default]
    Unknown,
    NeverDisabled,
    DisabledBefore,
    PartiallyDisabledNow,
    DisabledNow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityFile {
    Analytics,
    Modeling,
    Reporting,
}

#[derive(Debug, Clone, Default)]
pub struct ActivityDataStatus {
    pub status: AnalyticsStatus,
    pub cutoff_date: Option<DateTime<Local>>,
    pub missing_data: Option<bool>,
}

/// One line of activity/analytics (or reporting/modeling).
#[derive(Deserialize, Debug)]
pub struct DAnalyticsEvent {
    #[serde(rename = "event_type")]
    pub event_type: String,
    // invite events use "guild", join events use "guild_id"
    #[serde(rename = "guild", alias = "guild_id")]
    pub guild_id: Option<String>,
    #[serde(rename = "invite", alias = "invite_code")]
    pub invite_code: Option<String>,
    #[serde(rename = "join_type")]
    pub join_type: Option<String>,
    #[serde(rename = "join_method")]
    pub join_method: Option<String>,
    #[serde(rename = "application_id")]
    pub application_id: Option<String>,
    #[serde(rename = "location")]
    pub location: Option<String>,
    #[serde(rename = "control_type")]
    pub control_type: Option<String>,
    #[serde(rename = "timestamp")]
    timestamp_raw: String,
    #[serde(skip)]
    pub timestamp: DateTime<Local>,
}

impl DAnalyticsEvent {
    /// Timestamps arrive as a quoted string, sometimes with extra quotes.
    pub fn parse_timestamp(&mut self) {
        let cleaned = self.timestamp_raw.trim_matches('"');
        self.timestamp = parse_flexible(cleaned).unwrap_or_else(Local::now);
    }
}

#[derive(Debug, Clone)]
pub struct DAnalyticsGuild {
    pub id: String,
    pub join_type: String,
    pub join_method: Option<String>,
    pub application_id: Option<String>,
    pub location: Option<String>,
    pub invites: Vec<String>,
    pub timestamp: DateTime<Local>,
}

/// Accepts the few timestamp shapes Discord uses across package files.
pub fn parse_flexible(s: &str) -> Option<DateTime<Local>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }
    let naive_formats = [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d",
    ];
    for fmt in naive_formats {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            // Unspecified times in the package are local time
            let local = ndt.and_local_timezone(Local).earliest();
            return Some(local.unwrap_or_else(|| Local.from_utc_datetime(&ndt)));
        }
    }
    // Last chance: a bare date
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = date.and_hms_opt(0, 0, 0)?;
        return Some(Local.from_utc_datetime(&ndt));
    }
    None
}
