use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Everything the tool remembers between runs. Stored as json.
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    // search
    pub search_mode: String, // exact | words | regex
    pub sort_mode: String,   // asc | desc
    pub exclude_guilds: bool,
    pub exclude_dms: bool,
    pub exclude_gdms: bool,
    pub exclude_deleted: bool,
    pub has_image: bool,
    pub has_video: bool,
    pub has_file: bool,
    pub case_sensitive: bool,
    pub before_enabled: bool,
    pub after_enabled: bool,
    pub before_date: String, // yyyy-MM-dd
    pub after_date: String,
    pub exclude_ids: Vec<String>,
    pub whitelist_ids: Vec<String>,

    // general
    pub discord_instance: String, // default | stable | ptb | canary | web_*
    pub check_for_updates: bool,

    // stuff we accumulate while using the tool
    pub deleted_message_ids: Vec<String>,
    // channel id -> user id, for deleted users we managed to resolve
    pub resolved_deleted_users: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            search_mode: "exact".into(),
            sort_mode: "desc".into(),
            exclude_guilds: false,
            exclude_dms: false,
            exclude_gdms: false,
            exclude_deleted: false,
            has_image: false,
            has_video: false,
            has_file: false,
            case_sensitive: false,
            before_enabled: false,
            after_enabled: false,
            before_date: "2015-01-01".into(),
            after_date: "2035-01-01".into(),
            exclude_ids: Vec::new(),
            whitelist_ids: Vec::new(),
            discord_instance: "default".into(),
            check_for_updates: true,
            deleted_message_ids: Vec::new(),
            resolved_deleted_users: HashMap::new(),
        }
    }
}

pub fn settings_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("data-package-tool").join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        match std::fs::read_to_string(settings_path()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    pub fn deleted_ids(&self) -> HashSet<&str> {
        self.deleted_message_ids
            .iter()
            .map(|s| s.as_str())
            .collect()
    }

    pub fn resolved_user_for(&self, channel_id: &str) -> Option<&String> {
        self.resolved_deleted_users.get(channel_id)
    }

    /// The ids lists are stored as plain vecs, the search wants sets.
    pub fn exclude_id_set(&self) -> HashSet<&str> {
        self.exclude_ids.iter().map(|s| s.as_str()).collect()
    }

    pub fn whitelist_id_set(&self) -> HashSet<&str> {
        self.whitelist_ids.iter().map(|s| s.as_str()).collect()
    }

    pub fn add_deleted_message(&mut self, id: &str) {
        if !self.deleted_message_ids.iter().any(|x| x == id) {
            self.deleted_message_ids.push(id.to_string());
        }
    }

    pub fn resolve_deleted_user(&mut self, channel_id: &str, user_id: &str) {
        self.resolved_deleted_users
            .insert(channel_id.to_string(), user_id.to_string());
    }
}
