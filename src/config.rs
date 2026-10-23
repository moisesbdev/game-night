//! Where Game Night keeps its files, and the settings people can change from the page.
//!
//! Data lives in one folder: `--data-dir`, else `GAMENIGHT_DATA`, else a `data` folder next to the program when a file named
//! `portable.txt` sits beside it, else the platform's usual place (`%APPDATA%\GameNight`, `$XDG_DATA_HOME/game-night` or
//! `~/.local/share/game-night`, `~/Library/Application Support/GameNight`). Nothing about any one household is built in.

use crate::util::{read_json, write_json};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_PLAYERS: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: String,
    pub name: String,
    /// Their Steam profile (a link, SteamID64 or custom name), to see which games everyone owns. Optional.
    #[serde(default)]
    pub steam: String,
    /// Plays on the couch (the same screen) and/or online. Couch by default.
    #[serde(default = "yes")]
    pub couch: bool,
    #[serde(default)]
    pub online: bool,
}

fn yes() -> bool {
    true
}

/// A folder of box art, screenshots and preview videos. `slug`: `<dir>/covers/<slug>.png` and so on.
/// `esde`: ES-DE's `downloaded_media` layout, `<dir>/<system>/covers/<file>.png`, matched by a game's `media.system` and `media.file`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MediaDir {
    pub path: String,
    #[serde(default = "slug_layout")]
    pub layout: String,
}
fn slug_layout() -> String {
    "slug".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub players: Vec<Player>,
    /// How you're playing tonight: "couch" or "online". Each has its own group of players (and its own turn).
    pub mode: String,
    /// What one person's "not for us" does: "leave-out" (the game isn't drawn) or "rarer" (x0.3).
    pub not_for_us: String,
    pub launch_enabled: bool,
    pub ask_after_game: bool,
    pub trailers_enabled: bool,
    pub port: u16,
    pub media_dirs: Vec<MediaDir>,
    /// Folders to look through for games (shortcuts, programs, .desktop files). None by default.
    pub game_dirs: Vec<String>,
    /// The Steam folder, when it isn't where Steam usually installs.
    pub steam_dir: String,
    /// Optional key for SteamGridDB artwork (https://www.steamgriddb.com/profile/preferences/api).
    pub steamgriddb_key: String,
    /// Steam Web API key (steamcommunity.com/dev/apikey), for reading the players' Steam libraries.
    pub steam_api_key: String,
    /// TheGamesDB API key (requested on forums.thegamesdb.net).
    pub thegamesdb_key: String,
    /// ScreenScraper software credentials (requested on screenscraper.fr's forum) and, optionally, your own account.
    pub screenscraper_dev_id: String,
    pub screenscraper_dev_password: String,
    pub screenscraper_user: String,
    pub screenscraper_password: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            players: vec![
                Player { id: "p1".into(), name: "Player 1".into(), steam: String::new(), couch: true, online: false },
                Player { id: "p2".into(), name: "Player 2".into(), steam: String::new(), couch: true, online: false },
            ],
            mode: "couch".into(),
            not_for_us: "leave-out".into(),
            launch_enabled: true,
            ask_after_game: true,
            trailers_enabled: true,
            port: 8770,
            media_dirs: vec![],
            game_dirs: vec![],
            steam_dir: String::new(),
            steamgriddb_key: String::new(),
            steam_api_key: String::new(),
            thegamesdb_key: String::new(),
            screenscraper_dev_id: String::new(),
            screenscraper_dev_password: String::new(),
            screenscraper_user: String::new(),
            screenscraper_password: String::new(),
        }
    }
}

impl Settings {
    pub fn player_ids(&self) -> Vec<String> {
        self.players.iter().map(|p| p.id.clone()).collect()
    }

    /// The two ways of playing are in use once someone plays online and someone plays on the couch.
    pub fn has_modes(&self) -> bool {
        self.players.iter().any(|p| p.online) && self.players.iter().any(|p| p.couch)
    }

    /// The mode in effect: "online" or "couch" when both groups exist, else "" (everyone plays every way; nothing is filtered).
    pub fn active_mode(&self) -> &str {
        if !self.has_modes() {
            ""
        } else if self.mode == "online" {
            "online"
        } else {
            "couch"
        }
    }

    /// Who's playing in the mode in effect (everyone when the modes aren't in use).
    pub fn group(&self) -> Vec<String> {
        match self.active_mode() {
            "online" => self.players.iter().filter(|p| p.online).map(|p| p.id.clone()).collect(),
            "couch" => self.players.iter().filter(|p| p.couch).map(|p| p.id.clone()).collect(),
            _ => self.player_ids(),
        }
    }

    /// The online group (for "what you all own on Steam"), or everyone when nobody is marked online.
    pub fn online_group(&self) -> Vec<String> {
        let g: Vec<String> = self.players.iter().filter(|p| p.online).map(|p| p.id.clone()).collect();
        if g.is_empty() { self.player_ids() } else { g }
    }

    pub fn is_player(&self, id: &str) -> bool {
        self.players.iter().any(|p| p.id == id)
    }

    /// Fix anything a hand edit could break: names trimmed and present, unique ids, 1 to 8 players, known option values.
    pub fn clean(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.players.retain(|p| !p.id.is_empty() && p.id.len() <= 12 && p.id.chars().all(|c| c.is_ascii_alphanumeric()) && seen.insert(p.id.clone()));
        self.players.truncate(MAX_PLAYERS);
        for (n, p) in self.players.iter_mut().enumerate() {
            p.name = p.name.trim().chars().take(24).collect();
            p.steam = p.steam.trim().chars().take(200).collect();
            if p.name.is_empty() {
                p.name = format!("Player {}", n + 1);
            }
        }
        if self.players.is_empty() {
            self.players = Settings::default().players;
        }
        if !matches!(self.mode.as_str(), "couch" | "online") {
            self.mode = "couch".into();
        }
        if !self.players.iter().any(|p| p.couch) {
            self.players.iter_mut().for_each(|p| p.couch = true); // someone always plays on the couch
        }
        if !matches!(self.not_for_us.as_str(), "leave-out" | "rarer") {
            self.not_for_us = "leave-out".into();
        }
        if self.port < 1024 {
            self.port = 8770;
        }
        self.game_dirs = self.game_dirs.iter().map(|d| d.trim().to_string()).filter(|d| !d.is_empty() && d.len() <= 400).take(20).collect();
        self.media_dirs.retain(|m| !m.path.trim().is_empty() && matches!(m.layout.as_str(), "slug" | "esde"));
        self.steamgriddb_key = self.steamgriddb_key.trim().chars().filter(|c| c.is_ascii_alphanumeric()).take(64).collect();
        self.steam_api_key = self.steam_api_key.trim().chars().filter(|c| c.is_ascii_alphanumeric()).take(64).collect();
        self.thegamesdb_key = self.thegamesdb_key.trim().chars().filter(|c| c.is_ascii_alphanumeric()).take(128).collect();
        for v in [&mut self.screenscraper_dev_id, &mut self.screenscraper_dev_password, &mut self.screenscraper_user, &mut self.screenscraper_password] {
            *v = v.trim().chars().filter(|c| !c.is_control()).take(100).collect();
        }
        self
    }
}

#[derive(Clone, Debug)]
pub struct Paths {
    pub data: PathBuf,
}

impl Paths {
    pub fn new(data: PathBuf) -> Self {
        Paths { data }
    }
    pub fn catalog(&self) -> PathBuf {
        self.data.join("catalog.json")
    }
    pub fn state(&self) -> PathBuf {
        self.data.join("state.json")
    }
    pub fn sessions(&self) -> PathBuf {
        self.data.join("sessions.json")
    }
    pub fn settings(&self) -> PathBuf {
        self.data.join("settings.json")
    }
    /// Art the scrapers saved: `media/<slug>/cover.jpg`, `shot.jpg`.
    pub fn media(&self) -> PathBuf {
        self.data.join("media")
    }
}

/// The data folder, by the rules at the top of this file.
pub fn data_dir(flag: Option<&str>) -> PathBuf {
    if let Some(f) = flag.filter(|f| !f.is_empty()) {
        return PathBuf::from(f);
    }
    if let Ok(e) = std::env::var("GAMENIGHT_DATA")
        && !e.is_empty()
    {
        return PathBuf::from(e);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("portable.txt").exists()
    {
        return dir.join("data");
    }
    platform_dir()
}

fn platform_dir() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        return env("APPDATA").unwrap_or_else(|| PathBuf::from(".")).join("GameNight");
    }
    let home = env("HOME").unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/GameNight");
    }
    env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")).join("game-night")
}

/// Settings from disk. A file that exists but can't be read turns launching off and says why (fail closed).
pub fn load_settings(path: &Path) -> (Settings, String) {
    match read_json(path) {
        Ok(None) => (Settings::default(), String::new()),
        Ok(Some(v)) => match serde_json::from_value::<Settings>(v) {
            Ok(s) => (s.clean(), String::new()),
            Err(e) => (
                Settings { launch_enabled: false, ..Settings::default() },
                format!("The settings file couldn't be read ({e}), so Play buttons are off until it is fixed or saved again."),
            ),
        },
        Err(e) => (
            Settings { launch_enabled: false, ..Settings::default() },
            format!("The settings file couldn't be read ({e}), so Play buttons are off until it is fixed or saved again."),
        ),
    }
}

pub fn save_settings(path: &Path, s: &Settings) -> std::io::Result<()> {
    write_json(path, &serde_json::to_value(s).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_repairs_hand_edits() {
        let s = Settings {
            players: vec![
                Player { id: "p1".into(), name: "  ".into(), steam: String::new(), couch: true, online: false },
                Player { id: "p1".into(), name: "dup".into(), steam: String::new(), couch: true, online: false },
                Player { id: "bad id!".into(), name: "x".into(), steam: String::new(), couch: true, online: false },
                Player { id: "p2".into(), name: "A very long name that goes on and on".into(), steam: String::new(), couch: true, online: false },
            ],
            not_for_us: "nonsense".into(),
            port: 80,
            ..Settings::default()
        }
        .clean();
        assert_eq!(s.player_ids(), vec!["p1", "p2"]);
        assert_eq!(s.players[0].name, "Player 1");
        assert_eq!(s.players[1].name.chars().count(), 24);
        assert_eq!(s.not_for_us, "leave-out");
        assert_eq!(s.port, 8770);
    }

    #[test]
    fn unreadable_settings_fail_closed() {
        let d = std::env::temp_dir().join(format!("gn-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("settings.json");
        std::fs::write(&p, "{ not json").unwrap();
        let (s, warn) = load_settings(&p);
        assert!(!s.launch_enabled);
        assert!(!warn.is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
