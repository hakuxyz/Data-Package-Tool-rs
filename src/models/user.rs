use serde::Deserialize;
use std::collections::HashMap;

pub const DELETED_USER_ID: &str = "456226577798135808";

#[derive(Deserialize, Clone)]
pub struct DRelationship {
    #[serde(rename = "id")]
    #[allow(dead_code)]
    pub id: String,
    // Packages from before 04.2024 use a number here, newer ones a string.
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub relationship_type: serde_json::Value,
    #[serde(rename = "nickname")]
    #[allow(dead_code)]
    pub nickname: Option<String>,
    #[serde(rename = "user")]
    pub user: DUser,
}

#[derive(Deserialize, Clone, Default)]
pub struct DUser {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "username")]
    pub username: Option<String>,
    #[serde(rename = "global_name")]
    pub display_name: Option<String>,
    #[serde(rename = "discriminator")]
    pub discriminator: Option<String>,
    // relationship entries call it "avatar" instead
    #[serde(rename = "avatar_hash", alias = "avatar")]
    pub avatar_hash: Option<String>,
    #[serde(default)]
    pub relationships: Vec<DRelationship>,
    #[serde(default)]
    pub notes: HashMap<String, String>,
}

impl DUser {
    /// Pomelo accounts dropped the #discriminator.
    pub fn is_pomelo(&self) -> bool {
        matches!(
            self.discriminator.as_deref(),
            Some("0") | Some("0000") | None
        )
    }

    pub fn is_deleted_user(&self) -> bool {
        self.id == DELETED_USER_ID
    }

    pub fn tag(&self) -> String {
        match (&self.username, &self.discriminator) {
            (Some(u), Some(d)) if !self.is_pomelo() => format!("{}#{}", u, d),
            (Some(u), _) => u.clone(),
            _ => self.id.clone(),
        }
    }

    /// The name we show in the UI, display name wins if there is one.
    pub fn display_tag(&self) -> String {
        self.display_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.tag())
    }

    pub fn default_avatar_id(&self) -> usize {
        if self.is_pomelo() {
            (self.id.parse::<i64>().unwrap_or(0) >> 22).rem_euclid(6) as usize
        } else {
            self.discriminator
                .as_deref()
                .and_then(|d| d.parse::<i64>().ok())
                .unwrap_or(0)
                .rem_euclid(5) as usize
        }
    }

    pub fn avatar_url(&self) -> String {
        match &self.avatar_hash {
            Some(hash) if !hash.is_empty() => {
                format!(
                    "https://cdn.discordapp.com/avatars/{}/{}.png?size=64",
                    self.id, hash
                )
            }
            _ => format!(
                "https://cdn.discordapp.com/embed/avatars/{}.png?size=64",
                self.default_avatar_id()
            ),
        }
    }

    pub fn has_avatar(&self) -> bool {
        matches!(&self.avatar_hash, Some(h) if !h.is_empty())
    }
}
