use chrono::{DateTime, Local};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use super::channel::DChannel;

const IMAGE_EXTENSIONS: [&str; 7] = ["png", "gif", "jpg", "jpeg", "apng", "jfif", "webp"];
const VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "webm", "avi", "mov"];

// Attachment urls look like .../attachments/<channel>/<id>/<name>.<ext>
fn attachment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"attachments/\d+/(\d+)/([\w.-]+\.(\w+))").unwrap())
}

#[derive(Default)]
struct AttachmentMeta {
    id: String,
    file_name: String,
    extension: String,
}

/// One file attached to a message. Url and the fields parsed from it can be
/// rewritten later by "refresh urls", hence the locks.
pub struct DAttachment {
    pub message_id: String,
    meta: Mutex<AttachmentMeta>,
    url: RwLock<String>,
}

impl DAttachment {
    pub fn new(url: &str, message_id: &str) -> Self {
        let att = Self {
            message_id: message_id.to_string(),
            meta: Mutex::new(AttachmentMeta::default()),
            url: RwLock::new(String::new()),
        };
        att.set_url(url.to_string());
        att
    }

    pub fn url(&self) -> String {
        self.url.read().unwrap().clone()
    }

    pub fn id(&self) -> String {
        self.meta.lock().unwrap().id.clone()
    }

    pub fn file_name(&self) -> String {
        self.meta.lock().unwrap().file_name.clone()
    }

    pub fn extension(&self) -> String {
        self.meta.lock().unwrap().extension.clone()
    }

    pub fn set_url(&self, url: String) {
        if let Some(caps) = attachment_re().captures(&url) {
            let mut meta = self.meta.lock().unwrap();
            meta.id = caps[1].to_string();
            meta.file_name = caps[2].to_string();
            meta.extension = caps[3].to_string();
        }
        *self.url.write().unwrap() = url;
    }

    pub fn is_image(&self) -> bool {
        IMAGE_EXTENSIONS.contains(&self.extension().as_str())
    }

    pub fn is_video(&self) -> bool {
        VIDEO_EXTENSIONS.contains(&self.extension().as_str())
    }

    /// Old packages have attachment urls without a signature, those 404 now.
    pub fn is_signed(&self) -> bool {
        self.url().contains("?ex=")
    }
}

pub struct DMessage {
    pub id: String,
    pub timestamp: DateTime<Local>,
    pub content: String,
    pub attachments: Vec<Arc<DAttachment>>,
    pub channel_id: String,
    is_deleted: AtomicBool,
}

impl DMessage {
    pub fn new(
        id: String,
        timestamp: DateTime<Local>,
        content: String,
        channel_id: String,
    ) -> Self {
        Self {
            id,
            timestamp,
            content,
            attachments: Vec::new(),
            channel_id,
            is_deleted: AtomicBool::new(false),
        }
    }

    pub fn is_deleted(&self) -> bool {
        self.is_deleted.load(Ordering::Relaxed)
    }

    pub fn mark_deleted(&self) {
        self.is_deleted.store(true, Ordering::Relaxed);
    }

    /// Path used by the discord:// protocol, e.g. "<guild>/<channel>/<msg>".
    pub fn message_link(&self, channel: &DChannel) -> Result<String, String> {
        let guild = if let Some(g) = &channel.guild {
            g.id.clone()
        } else if channel.is_dm() || channel.is_group_dm() {
            "@me".to_string()
        } else {
            return Err("Unable to find the server this message was sent in. \
                 This usually happens if you've left the server."
                .to_string());
        };
        Ok(format!("{}/{}/{}", guild, self.channel_id, self.id))
    }
}
