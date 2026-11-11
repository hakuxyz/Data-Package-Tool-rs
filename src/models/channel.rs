use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::message::DMessage;

/// Discord channel types we care about. Raw values match Discord's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)] // names mirror discord's own docs
pub enum ChannelType {
    GUILD_TEXT,
    DM,
    GUILD_VOICE,
    GROUP_DM,
    GUILD_CATEGORY,
    GUILD_ANNOUNCEMENT,
    GUILD_STORE,
    ANNOUNCEMENT_THREAD,
    PUBLIC_THREAD,
    PRIVATE_THREAD,
    GUILD_STAGE_VOICE,
    GUILD_DIRECTORY,
    GUILD_FORUM,
    GUILD_MEDIA,
    LOBBY,
    EPHEMERAL_DM,
}

impl ChannelType {
    fn from_raw(v: i64) -> Self {
        match v {
            1 => Self::DM,
            2 => Self::GUILD_VOICE,
            3 => Self::GROUP_DM,
            4 => Self::GUILD_CATEGORY,
            5 => Self::GUILD_ANNOUNCEMENT,
            6 => Self::GUILD_STORE,
            10 => Self::ANNOUNCEMENT_THREAD,
            11 => Self::PUBLIC_THREAD,
            12 => Self::PRIVATE_THREAD,
            13 => Self::GUILD_STAGE_VOICE,
            14 => Self::GUILD_DIRECTORY,
            15 => Self::GUILD_FORUM,
            16 => Self::GUILD_MEDIA,
            17 => Self::LOBBY,
            18 => Self::EPHEMERAL_DM,
            _ => Self::GUILD_TEXT,
        }
    }
}

// channel.json writes the type as a plain number
impl<'de> Deserialize<'de> for ChannelType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = ChannelType;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a channel type number")
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<ChannelType, E> {
                Ok(ChannelType::from_raw(v))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<ChannelType, E> {
                Ok(ChannelType::from_raw(v as i64))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<ChannelType, E> {
                Ok(ChannelType::from_raw(v as i64))
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Deserialize, Clone)]
pub struct DPartialGuild {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct DChannel {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    #[serde(rename = "name")]
    pub name: Option<String>,
    #[serde(rename = "guild")]
    pub guild: Option<DPartialGuild>,
    #[serde(rename = "recipients")]
    pub recipient_ids: Vec<String>,

    // Filled in while loading, before the channel gets shared out.
    #[serde(skip)]
    pub messages: Vec<Arc<DMessage>>,
    #[serde(skip)]
    pub dm_recipient_id: String,
    #[serde(skip)]
    pub fallback_name: Option<String>,
    #[serde(skip)]
    pub has_duplicates: AtomicBool,
}

impl DChannel {
    pub fn is_dm(&self) -> bool {
        self.channel_type == ChannelType::DM
    }

    pub fn is_group_dm(&self) -> bool {
        self.channel_type == ChannelType::GROUP_DM
    }

    pub fn is_voice(&self) -> bool {
        matches!(
            self.channel_type,
            ChannelType::GUILD_VOICE | ChannelType::GUILD_STAGE_VOICE
        )
    }

    pub fn has_duplicates(&self) -> bool {
        self.has_duplicates.load(Ordering::Relaxed)
    }

    pub fn set_has_duplicates(&self, v: bool) {
        self.has_duplicates.store(v, Ordering::Relaxed);
    }

    /// The other person in a 1 on 1 dm.
    pub fn other_dm_recipient(&self, user_id: &str) -> Result<String, String> {
        if !self.is_dm() {
            return Err("other_dm_recipient can only be used on dm channels".into());
        }
        self.recipient_ids
            .iter()
            .find(|id| id.as_str() != user_id)
            .cloned()
            .ok_or_else(|| "no other recipient found".into())
    }

    /// Name shown in the message list, whatever we can scrape up.
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.name {
            if self.is_dm() || self.is_group_dm() || self.is_voice() {
                return name.clone();
            }
            return format!("#{}", name);
        }
        if let Some(fallback) = &self.fallback_name {
            return fallback.clone();
        }
        String::new()
    }
}

/// "Direct Message with someone#1234" from the channel name map.
pub fn parse_dm_channel_name(name: &str) -> Option<(String, String)> {
    let re = regex::Regex::new(r"^Direct Message with (.+)#(\d{1,4})$").unwrap();
    re.captures(name)
        .map(|c| (c[1].to_string(), c[2].to_string()))
}

/// Used when a package has no Account folder.
pub fn most_common_recipient(counts: &HashMap<String, i32>) -> Option<String> {
    counts
        .iter()
        .filter(|(_, n)| **n > 2)
        .max_by_key(|(_, n)| **n)
        .map(|(id, _)| id.clone())
}
