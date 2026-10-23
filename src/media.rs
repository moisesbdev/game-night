//! Box art, screenshots and preview videos. The page asks by catalog title and kind; it never supplies a path.
//!
//! Looked up in order: what a scraper saved (`<data>/media/<slug>/`), the media folders set in Settings (slug or ES-DE layout),
//! then the art Steam already cached for a Steam game. A file only counts when it really is inside the folder it was found in.

use crate::config::{MediaDir, Settings};
use crate::util::{Json, s, slug};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const IMAGE_EXT: [&str; 4] = ["jpg", "png", "jpeg", "webp"];
pub const VIDEO_EXT: [&str; 2] = ["mp4", "webm"];
pub const KINDS: [&str; 4] = ["cover", "shot", "title", "video"];

pub fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        _ => "application/octet-stream",
    }
}

fn folders(kind: &str) -> &'static [&'static str] {
    match kind {
        "cover" => &["covers", "miximages", "boxart"],
        "shot" => &["screenshots", "titlescreens", "fanart"],
        "title" => &["titlescreens"],
        _ => &["videos"],
    }
}

fn exts(kind: &str) -> &'static [&'static str] {
    if kind == "video" { &VIDEO_EXT } else { &IMAGE_EXT }
}

/// `dir/stem.ext` for the first extension that exists, only if it resolves inside `root`.
fn inside(root: &Path, dir: &Path, stem: &str, kind: &str) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    exts(kind).iter().map(|e| dir.join(format!("{stem}.{e}"))).find(|p| p.is_file() && p.canonicalize().is_ok_and(|c| c.starts_with(&root)))
}

fn in_media_dir(m: &MediaDir, game: &Json, kind: &str) -> Option<PathBuf> {
    let root = PathBuf::from(&m.path);
    if m.layout == "esde" {
        let me = game.get("media")?;
        let (sys, file) = (s(me, "system"), s(me, "file"));
        if sys.is_empty() || file.is_empty() {
            return None;
        }
        return folders(kind).iter().find_map(|f| inside(&root, &root.join(sys).join(f), file, kind));
    }
    let sl = slug(s(game, "t"));
    folders(kind).iter().find_map(|f| inside(&root, &root.join(f), &sl, kind)).or_else(|| if kind == "cover" { inside(&root, &root, &sl, kind) } else { None })
}

type Cache = HashMap<(String, String), (Instant, Option<PathBuf>)>;

pub struct Media {
    cache: Mutex<Cache>,
}

impl Default for Media {
    fn default() -> Self {
        Media { cache: Mutex::new(HashMap::new()) }
    }
}

impl Media {
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    /// The file for this game and kind, or None. Remembered for 20 seconds (listing folders for every poster is the slow part).
    pub fn find(&self, game: &Json, kind: &str, settings: &Settings, data_media: &Path, steam_root: Option<&Path>) -> Option<PathBuf> {
        if !KINDS.contains(&kind) {
            return None;
        }
        let key = (s(game, "t").to_string(), kind.to_string());
        if let Some((at, hit)) = self.cache.lock().unwrap().get(&key)
            && at.elapsed() < Duration::from_secs(20)
        {
            return hit.clone();
        }
        let saved = data_media.join(slug(s(game, "t")));
        let found = (if data_media.is_dir() { inside(data_media, &saved, kind, kind) } else { None })
            .or_else(|| settings.media_dirs.iter().find_map(|m| in_media_dir(m, game, kind)))
            .or_else(|| match (kind, steam_root, crate::launch::steam_id(game)) {
                ("cover" | "shot", Some(root), Some(id)) => crate::steam::cached_art(root, id, kind),
                _ => None,
            });
        let mut c = self.cache.lock().unwrap();
        if c.len() > 5000 {
            c.clear();
        }
        c.insert(key, (Instant::now(), found.clone()));
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn layouts_order_and_escapes() {
        let d = std::env::temp_dir().join(format!("gn-media-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (saved, slugdir, esde) = (d.join("data/media"), d.join("art"), d.join("esde"));
        std::fs::create_dir_all(saved.join("it-takes-two")).unwrap();
        std::fs::create_dir_all(slugdir.join("covers")).unwrap();
        std::fs::create_dir_all(esde.join("switch/videos")).unwrap();
        std::fs::write(slugdir.join("covers/it-takes-two.png"), b"x").unwrap();
        std::fs::write(esde.join("switch/videos/Mario Kart [0100].mp4"), b"x").unwrap();
        let settings = Settings {
            media_dirs: vec![
                MediaDir { path: slugdir.to_string_lossy().into(), layout: "slug".into() },
                MediaDir { path: esde.to_string_lossy().into(), layout: "esde".into() },
            ],
            ..Settings::default()
        };
        let m = Media::default();
        let itt = json!({"t": "It Takes Two"});
        assert_eq!(m.find(&itt, "cover", &settings, &saved, None).unwrap().file_name().unwrap(), "it-takes-two.png");
        std::fs::write(saved.join("it-takes-two/cover.jpg"), b"x").unwrap();
        m.clear();
        assert_eq!(m.find(&itt, "cover", &settings, &saved, None).unwrap().file_name().unwrap(), "cover.jpg", "scraped art comes first");
        let mk = json!({"t": "Mario Kart", "media": {"system": "switch", "file": "Mario Kart [0100]"}});
        assert!(m.find(&mk, "video", &settings, &saved, None).is_some());
        assert!(m.find(&mk, "../../etc", &settings, &saved, None).is_none());
        assert!(m.find(&json!({"t": "Nothing"}), "cover", &settings, &saved, None).is_none());
        let _ = std::fs::remove_dir_all(d);
    }
}
