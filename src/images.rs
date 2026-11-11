use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const MAX_ACTIVE: usize = 8;
const MAX_TEXTURES: usize = 300;
const KEEP_FOR: Duration = Duration::from_secs(120);

struct Entry {
    handle: TextureHandle,
    last_used: Instant,
}

/// Downloads images off the ui thread and keeps them around as textures.
pub struct ImageCache {
    textures: HashMap<String, Entry>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<(String, ColorImage)>,
    rx: Receiver<(String, ColorImage)>,
    active: usize,
}

impl Default for ImageCache {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            textures: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
            active: 0,
        }
    }
}

impl ImageCache {
    /// Returns the texture if we already have it, kicks off a download
    /// otherwise. Call it every frame for anything visible.
    pub fn get_or_request(
        &mut self,
        ctx: &Context,
        key: &str,
        url: &str,
        max_dim: u32,
    ) -> Option<TextureHandle> {
        if let Some(entry) = self.textures.get_mut(key) {
            entry.last_used = Instant::now();
            return Some(entry.handle.clone());
        }
        if self.failed.contains(key) || self.in_flight.contains(key) {
            return None;
        }
        if self.active >= MAX_ACTIVE {
            return None;
        }

        self.evict_old();
        self.in_flight.insert(key.to_string());
        self.active += 1;

        let tx = self.tx.clone();
        let key = key.to_string();
        let url = url.to_string();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            if let Some(image) = download(&url, max_dim) {
                let _ = tx.send((key, image));
            } else {
                // Send an empty image as a failure marker
                let _ = tx.send((
                    key,
                    ColorImage::filled([1, 1], eframe::egui::Color32::TRANSPARENT),
                ));
            }
            ctx.request_repaint();
        });

        None
    }

    /// Pulls finished downloads in, call once per frame.
    pub fn poll(&mut self, ctx: &Context) {
        while let Ok((key, image)) = self.rx.try_recv() {
            self.active = self.active.saturating_sub(1);
            self.in_flight.remove(&key);
            if image.size == [1, 1] {
                self.failed.insert(key);
                continue;
            }
            let handle = ctx.load_texture(key.clone(), image, TextureOptions::LINEAR);
            self.textures.insert(
                key,
                Entry {
                    handle,
                    last_used: Instant::now(),
                },
            );
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&TextureHandle> {
        let entry = self.textures.get_mut(key)?;
        entry.last_used = Instant::now();
        Some(&entry.handle)
    }

    /// Forget everything, used when a new package or search comes in.
    pub fn clear(&mut self) {
        self.textures.clear();
        self.failed.clear();
    }

    fn evict_old(&mut self) {
        if self.textures.len() < MAX_TEXTURES {
            return;
        }
        let now = Instant::now();
        self.textures
            .retain(|_, e| now.duration_since(e.last_used) < KEEP_FOR);
        // Still too big? Drop the oldest half
        if self.textures.len() >= MAX_TEXTURES {
            let mut entries: Vec<(Instant, String)> = self
                .textures
                .keys()
                .map(|k| {
                    let t = self.textures.get(k).map(|e| e.last_used).unwrap_or(now);
                    (t, k.clone())
                })
                .collect();
            entries.sort();
            let drop_count = entries.len() / 2;
            for (_, key) in entries.into_iter().take(drop_count) {
                self.textures.remove(&key);
            }
        }
    }
}

fn download(url: &str, max_dim: u32) -> Option<ColorImage> {
    let bytes = reqwest::blocking::get(url).ok()?.bytes().ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    // Keeps the aspect ratio, both sides end up within max_dim
    let img = if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}
