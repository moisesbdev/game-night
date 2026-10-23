//! The game catalog (`catalog.json`): loading, validation (including a tone check), safe edits with backups, and the wishlist.
//!
//! A game is a JSON object. Required: `t` (title), `sys` (platform label), `where` (couch|online), `p` (players, "2" or "1-4"),
//! `mode`, `gap` (1-5: how little skill difference matters), `vibe` (moods), `len` (quick|evening|long), `why` (one line about it).
//! Optional: `role`, `watch`, `time`, `trailer`, `coop`, `coopKind`, `coopBy`, `one`, `info`, `launch`, `media`, `process`.

use crate::util::{Json, read_json, s, write_json};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub const WHERE: [&str; 2] = ["couch", "online"];
pub const LENGTHS: [&str; 3] = ["quick", "evening", "long"];
pub const VIBES: [&str; 7] = ["cozy", "silly", "action", "brain", "story", "spooky", "compete"];
pub const MAX_GAMES: usize = 1000;
const MAX_TITLE: usize = 80;
const MAX_TEXT: usize = 400;
const KEEP_BACKUPS: usize = 20;
const URL_SCHEMES: [&str; 7] = ["steam://", "com.epicgames.launcher://", "heroic://", "lutris:", "itch://", "goggalaxy://", "uplay://"];
const INFO_KEYS: [&str; 15] = [
    "year",
    "developer",
    "publisher",
    "genres",
    "tags",
    "rating",
    "descriptors",
    "hltb",
    "scores",
    "controller",
    "localPlay",
    "steamAppId",
    "asOf",
    "sources",
    "summary",
];
const PRICE_FIELDS: [(&str, usize); 7] = [("low", 16), ("where", 60), ("regular", 16), ("recentLow", 16), ("note", 120), ("source", 300), ("asOf", 10)];

/// Copy describes the game; it never coaches one player about another. These phrases are refused.
const TONE: [&str; 11] =
    ["newer player", "beginner", "hold you back", "let them", "don't rescue", "go easy", "patroni", "your job", "the weaker", "worse player", "can't hold"];

pub fn tone_issues(text: &str) -> Vec<String> {
    let low = text.to_lowercase().replace('\u{2019}', "'");
    let mut out: Vec<String> = TONE.iter().filter(|p| low.contains(*p) || low.contains(&p.replace('\'', ""))).map(|p| p.to_string()).collect();
    out.sort();
    out.dedup();
    out
}

/// "1-4" -> (1, 4); anything unreadable -> (1, 1).
pub fn prange(p: &str) -> (u32, u32) {
    let mut it = p.splitn(2, '-');
    let lo = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(1);
    let hi = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(lo);
    (lo, hi)
}

fn players_ok(p: &str) -> bool {
    let parts: Vec<&str> = p.split('-').collect();
    !p.is_empty() && parts.len() <= 2 && parts.iter().all(|x| !x.is_empty() && x.len() <= 2 && x.chars().all(|c| c.is_ascii_digit()))
}

pub fn is_video_id(v: &str) -> bool {
    v.len() == 11 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A YouTube video id from a bare id or a watch / youtu.be / embed / shorts / live link; None for anything else.
pub fn parse_video_id(text: &str) -> Option<String> {
    let t = text.trim();
    if is_video_id(t) {
        return Some(t.into());
    }
    let rest = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_lowercase();
    let cand = if host == "youtu.be" {
        path.split(['?', '/']).next().map(String::from)
    } else if ["youtube.com", "www.youtube.com", "m.youtube.com", "youtube-nocookie.com", "www.youtube-nocookie.com"].contains(&host.as_str()) {
        if let Some(q) = path.strip_prefix("watch?") {
            q.split('&').find_map(|kv| kv.strip_prefix("v=")).map(String::from)
        } else {
            let mut seg = path.split('/');
            match seg.next() {
                Some("embed" | "shorts" | "live" | "v") => seg.next().map(|x| x.split('?').next().unwrap_or("").to_string()),
                _ => None,
            }
        }
    } else {
        None
    };
    cand.filter(|c| is_video_id(c))
}

fn short_text(v: &Value, max: usize) -> bool {
    v.as_str().is_some_and(|x| !x.trim().is_empty() && x.chars().count() <= max)
}

fn short_list(v: &Value, limit: usize, each: usize) -> bool {
    v.as_array().is_some_and(|a| a.len() <= limit && a.iter().all(|x| short_text(x, each)))
}

pub fn info_issues(info: &Value) -> Vec<String> {
    let Some(m) = info.as_object() else { return vec!["info must be an object".into()] };
    let mut e = vec![];
    for k in m.keys() {
        if !INFO_KEYS.contains(&k.as_str()) {
            e.push(format!("info has an unknown field '{k}'"));
        }
    }
    if let Some(y) = m.get("year")
        && !y.as_i64().is_some_and(|y| (1970..=2100).contains(&y))
    {
        e.push("info.year must be a four-digit year".into());
    }
    for (k, n) in [("developer", 80), ("publisher", 80), ("rating", 60), ("localPlay", 160), ("summary", 400)] {
        if let Some(v) = m.get(k)
            && !short_text(v, n)
        {
            e.push(format!("info.{k} must be short text (up to {n} characters)"));
        }
    }
    for (k, limit, each) in [("genres", 8, 40), ("tags", 14, 40), ("descriptors", 8, 60)] {
        if let Some(v) = m.get(k)
            && !short_list(v, limit, each)
        {
            e.push(format!("info.{k} must be a short list of short words"));
        }
    }
    if let Some(h) = m.get("hltb")
        && !h.as_object().is_some_and(|o| {
            o.iter().all(|(k, v)| ["main", "extras", "full"].contains(&k.as_str()) && (v.is_null() || v.as_f64().is_some_and(|x| (0.0..=2000.0).contains(&x))))
        })
    {
        e.push("info.hltb is hours: main, extras, full".into());
    }
    if let Some(sc) = m.get("scores")
        && !sc.as_object().is_some_and(|o| {
            o.iter().all(|(k, v)| ["metacritic", "steamPct"].contains(&k.as_str()) && (v.is_null() || v.as_i64().is_some_and(|x| (0..=100).contains(&x))))
        })
    {
        e.push("info.scores is metacritic and steamPct, whole numbers from 0 to 100".into());
    }
    if let Some(c) = m.get("controller")
        && !["full", "partial", "none"].contains(&c.as_str().unwrap_or(""))
    {
        e.push("info.controller must be full, partial or none".into());
    }
    if let Some(a) = m.get("steamAppId")
        && !a.as_u64().is_some_and(|a| a > 0)
    {
        e.push("info.steamAppId must be a number".into());
    }
    if let Some(d) = m.get("asOf")
        && !d.as_str().is_some_and(|d| crate::util::parse_date(d).is_some() && d.len() == 10)
    {
        e.push("info.asOf must be a date like 2026-10-22".into());
    }
    if let Some(src) = m.get("sources")
        && !src.as_array().is_some_and(|a| a.len() <= 8 && a.iter().all(|u| u.as_str().is_some_and(|u| u.starts_with("https://") && u.len() <= 220)))
    {
        e.push("info.sources must be a short list of https links".into());
    }
    e
}

/// How a game starts: `{"steam": 620}`, `{"url": "heroic://launch/..."}` (known launcher links only), `{"open": "C:/Games/Game.lnk"}`
/// (a shortcut file opened by the system: .lnk, .url, .desktop) or `{"command": ["path/to/game", "--arg"], "cwd": "..."}`.
pub fn launch_issues(l: &Value) -> Vec<String> {
    let Some(m) = l.as_object() else { return vec!["launch must be an object".into()] };
    let kinds = ["steam", "url", "command", "open"].iter().filter(|k| m.contains_key(**k)).count();
    if kinds != 1 {
        return vec!["launch needs exactly one of steam, url, open or command".into()];
    }
    if let Some(o) = m.get("open") {
        let ok = o.as_str().is_some_and(|p| p.len() <= 400 && crate::folders::OPEN_EXT.iter().any(|e| p.to_lowercase().ends_with(&format!(".{e}"))));
        if !ok {
            return vec![format!("launch.open must be a shortcut file ({})", crate::folders::OPEN_EXT.join(", "))];
        }
    }
    if let Some(a) = m.get("steam")
        && !a.as_u64().is_some_and(|a| a > 0)
    {
        return vec!["launch.steam must be a Steam app id (a number)".into()];
    }
    if let Some(u) = m.get("url") {
        let u = u.as_str().unwrap_or("");
        if u.len() > 300 || !URL_SCHEMES.iter().any(|p| u.to_lowercase().starts_with(p)) {
            return vec![format!("launch.url must be a game launcher link ({})", URL_SCHEMES.join(", "))];
        }
    }
    if let Some(c) = m.get("command")
        && !c.as_array().is_some_and(|a| !a.is_empty() && a.len() <= 20 && a.iter().all(|x| x.as_str().is_some_and(|x| !x.is_empty() && x.len() <= 400)))
    {
        return vec!["launch.command is a list: the program, then its arguments".into()];
    }
    if let Some(c) = m.get("cwd")
        && !short_text(c, 400)
    {
        return vec!["launch.cwd must be a folder path".into()];
    }
    for k in m.keys() {
        if !["steam", "url", "command", "open", "cwd"].contains(&k.as_str()) {
            return vec![format!("launch has an unknown field '{k}'")];
        }
    }
    vec![]
}

pub fn validate_game(g: &Value) -> Vec<String> {
    let t = s(g, "t");
    if t.trim().is_empty() {
        return vec!["title is missing".into()];
    }
    if t.chars().count() > MAX_TITLE {
        return vec![format!("title is longer than {MAX_TITLE} characters")];
    }
    let pre = format!("{t}: ");
    let mut e = vec![];
    let mut add = |m: String| e.push(format!("{pre}{m}"));
    if !g.get("sys").is_some_and(|v| short_text(v, 24)) {
        add("sys is the platform, short text like PC or Switch".into());
    }
    if !WHERE.contains(&s(g, "where")) {
        add("where must be couch or online".into());
    }
    if !LENGTHS.contains(&s(g, "len")) {
        add("len must be quick, evening or long".into());
    }
    if !g.get("gap").and_then(Value::as_i64).is_some_and(|x| (1..=5).contains(&x)) {
        add("gap must be a whole number from 1 to 5".into());
    }
    let p = s(g, "p");
    if !players_ok(p) {
        add("p (players) looks like '2' or '1-4'".into());
    } else if prange(p).0 > prange(p).1 || prange(p).0 == 0 {
        add("players range is backwards or starts at zero".into());
    }
    if !g.get("mode").is_some_and(|v| short_text(v, 30)) {
        add("mode is short text, like Co-op or Versus".into());
    }
    let vibes = g.get("vibe").and_then(Value::as_array);
    if !vibes.is_some_and(|a| !a.is_empty() && a.iter().all(|v| VIBES.contains(&v.as_str().unwrap_or("")))) {
        add(format!("vibe must be a non-empty list from: {}", VIBES.join(", ")));
    }
    for (f, required) in [("why", true), ("role", false), ("watch", false)] {
        match g.get(f) {
            None | Some(Value::Null) if !required => {}
            Some(v) if short_text(v, MAX_TEXT) => {
                let bad = tone_issues(v.as_str().unwrap_or(""));
                if !bad.is_empty() {
                    add(format!("{f} reads like coaching ({}): describe the game instead", bad.join(", ")));
                }
            }
            _ => add(format!("{f} must be text of up to {MAX_TEXT} characters")),
        }
    }
    if let Some(v) = g.get("time")
        && !short_text(v, 60)
    {
        add("time is a short note like 'about 8 hours to finish'".into());
    }
    for f in ["trailer", "coop"] {
        if let Some(v) = g.get(f)
            && !v.as_str().is_some_and(is_video_id)
        {
            add(format!("{f} must be a YouTube video id (11 characters)"));
        }
    }
    if let Some(v) = g.get("coopBy")
        && (!short_text(v, 60) || g.get("coop").is_none())
    {
        add("coopBy is a short channel name, and needs a co-op clip".into());
    }
    if let Some(v) = g.get("coopKind")
        && (!["local", "online"].contains(&v.as_str().unwrap_or("")) || g.get("coop").is_none())
    {
        add("coopKind must be local or online, and needs a co-op clip".into());
    }
    if g.get("one").is_some_and(|v| !v.is_boolean()) {
        add("one must be true or false".into());
    }
    if let Some(i) = g.get("info") {
        for m in info_issues(i) {
            add(m);
        }
    }
    if let Some(l) = g.get("launch") {
        for m in launch_issues(l) {
            add(m);
        }
    }
    if let Some(m) = g.get("media")
        && !m.as_object().is_some_and(|o| {
            o.iter().all(|(k, v)| {
                ["system", "file"].contains(&k.as_str())
                    && short_text(v, 160)
                    && !v.as_str().unwrap_or("").contains(['/', '\\', '\0'])
                    && v.as_str() != Some("..")
            })
        })
    {
        add("media is {system, file}: names only, no folders".into());
    }
    if let Some(p) = g.get("process")
        && !p.as_str().is_some_and(|p| (1..=60).contains(&p.len()) && !p.contains(['/', '\\']))
    {
        add("process is the game's program name, like Game.exe".into());
    }
    e
}

pub fn wish_issues(w: &Value) -> Vec<String> {
    if !w.get("t").is_some_and(|v| short_text(v, MAX_TITLE)) || !w.get("why").is_some_and(|v| short_text(v, 240)) {
        return vec!["a wishlist entry needs a title and a reason".into()];
    }
    let t = s(w, "t");
    let mut e: Vec<String> = tone_issues(s(w, "why")).into_iter().map(|b| format!("{t}: wishlist text reads like coaching ({b})")).collect();
    if let Some(p) = w.get("price") {
        e.extend(price_issues(p).into_iter().map(|m| format!("{t}: {m}")));
    }
    e
}

pub fn price_issues(p: &Value) -> Vec<String> {
    let Some(m) = p.as_object() else { return vec!["price must be an object".into()] };
    let mut e: Vec<String> = m.keys().filter(|k| !PRICE_FIELDS.iter().any(|(f, _)| f == k)).map(|k| format!("price has an unknown field '{k}'")).collect();
    for (k, n) in PRICE_FIELDS {
        if let Some(v) = m.get(k)
            && !v.as_str().is_some_and(|x| x.chars().count() <= n)
        {
            e.push(format!("price.{k} must be text of up to {n} characters"));
        }
    }
    if !["low", "where", "asOf"].iter().all(|k| m.get(*k).and_then(Value::as_str).is_some_and(|x| !x.is_empty())) {
        e.push("price needs low, where and asOf".into());
    }
    if m.get("source").and_then(Value::as_str).is_some_and(|x| !x.is_empty() && !x.starts_with("https://")) {
        e.push("price.source must be an https link".into());
    }
    e
}

pub fn validate_catalog(data: &Value) -> Vec<String> {
    let Some(games) = data.get("games").and_then(Value::as_array) else { return vec!["games must be a list".into()] };
    if games.len() > MAX_GAMES {
        return vec![format!("more than {MAX_GAMES} games")];
    }
    let mut e = vec![];
    let mut seen = std::collections::HashSet::new();
    for g in games {
        e.extend(validate_game(g));
        if !seen.insert(s(g, "t").to_lowercase()) {
            e.push(format!("{}: duplicate title", s(g, "t")));
        }
    }
    for w in data.get("wishlist").and_then(Value::as_array).into_iter().flatten() {
        e.extend(wish_issues(w));
    }
    e
}

/// The fields a saved game may carry; anything else is dropped on save.
const KEEP: [&str; 24] = [
    "draft", "t", "sys", "where", "p", "mode", "gap", "vibe", "len", "why", "role", "watch", "time", "trailer", "coop", "coopKind", "coopBy", "one", "info",
    "launch", "media", "process", "added", "source",
];

/// Tidy a game coming from the page: links become video ids, blanks are removed, unknown fields dropped. Errors say what to fix.
pub fn tidy_game(raw: &Value) -> Result<Value, String> {
    let mut g: Map<String, Value> = raw.as_object().cloned().ok_or("a game must be an object")?;
    g.retain(|k, v| KEEP.contains(&k.as_str()) && !v.is_null() && v.as_str().is_none_or(|x| !x.trim().is_empty()));
    for f in ["trailer", "coop"] {
        if let Some(v) = g.get(f).and_then(Value::as_str).map(String::from) {
            let id = parse_video_id(&v).ok_or(format!("{f} must be a YouTube link or an 11-character video id"))?;
            g.insert(f.into(), json!(id));
        }
    }
    if !g.contains_key("coop") {
        g.remove("coopBy");
        g.remove("coopKind");
    } else if !g.contains_key("coopKind") {
        g.insert("coopKind".into(), json!("local"));
    }
    for k in ["t", "why", "role", "watch", "time", "sys", "mode", "coopBy"] {
        if let Some(v) = g.get(k).and_then(Value::as_str).map(|x| x.trim().to_string()) {
            g.insert(k.into(), json!(v));
        }
    }
    if let Some(n) = g.get("gap").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|x| x.parse().ok()))) {
        g.insert("gap".into(), json!(n as i64));
    }
    if g.get("one") == Some(&json!(false)) {
        g.remove("one");
    }
    if g.get("info").and_then(Value::as_object).is_some_and(Map::is_empty) {
        g.remove("info");
    }
    let g = Value::Object(g);
    let errs = validate_game(&g);
    if errs.is_empty() { Ok(g) } else { Err(errs.join("; ")) }
}

pub struct Catalog {
    path: PathBuf,
    data: RwLock<Json>,
    pub recovered_from: RwLock<String>,
}

impl Catalog {
    /// Load the catalog; a damaged file is kept aside and the newest good backup is used instead (the page says which).
    pub fn open(path: &Path) -> Self {
        let c = Catalog { path: path.into(), data: RwLock::new(json!({"games": [], "wishlist": []})), recovered_from: RwLock::new(String::new()) };
        c.reload();
        c
    }

    pub fn reload(&self) {
        let loaded = match read_json(&self.path) {
            Ok(Some(v)) if v.get("games").is_some_and(Value::is_array) => Some(v),
            Ok(None) => None,
            _ => {
                let _ = fs::copy(&self.path, self.path.with_extension(format!("damaged-{}", crate::util::now_iso().replace(':', ""))));
                self.newest_backup()
            }
        };
        if let Some(mut v) = loaded {
            if !v.get("wishlist").is_some_and(Value::is_array) {
                v["wishlist"] = json!([]);
            }
            *self.data.write().unwrap() = v;
        }
    }

    fn backups(&self) -> Vec<PathBuf> {
        let dir = self.path.parent().unwrap_or(Path::new(".")).join("backups");
        let mut v: Vec<PathBuf> = fs::read_dir(dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("catalog-"))).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    fn newest_backup(&self) -> Option<Json> {
        for b in self.backups().iter().rev() {
            if let Ok(Some(v)) = read_json(b)
                && v.get("games").is_some_and(Value::is_array)
            {
                *self.recovered_from.write().unwrap() = b.file_name().unwrap_or_default().to_string_lossy().into();
                return Some(v);
            }
        }
        None
    }

    pub fn snapshot(&self) -> Json {
        self.data.read().unwrap().clone()
    }

    pub fn games(&self) -> Vec<Json> {
        self.data.read().unwrap()["games"].as_array().cloned().unwrap_or_default()
    }

    pub fn wishlist(&self) -> Vec<Json> {
        self.data.read().unwrap()["wishlist"].as_array().cloned().unwrap_or_default()
    }

    pub fn get(&self, title: &str) -> Option<Json> {
        self.data.read().unwrap()["games"].as_array()?.iter().find(|g| s(g, "t") == title).cloned()
    }

    pub fn has(&self, title: &str) -> bool {
        self.get(title).is_some()
    }

    pub fn problems(&self) -> Vec<String> {
        validate_catalog(&self.data.read().unwrap())
    }

    /// Apply `change` to a copy, validate the result (all of it, or only the touched wishlist entries), back up, write, then swap it in.
    fn edit<R>(&self, wish_only: bool, change: impl FnOnce(&mut Json) -> Result<R, String>) -> Result<R, String> {
        let mut w = self.data.write().unwrap();
        let mut new = w.clone();
        let out = change(&mut new)?;
        if new == *w {
            return Ok(out);
        }
        let errs = if wish_only {
            let old = w["wishlist"].as_array().cloned().unwrap_or_default();
            new["wishlist"].as_array().into_iter().flatten().filter(|x| !old.contains(x)).flat_map(wish_issues).collect()
        } else {
            validate_catalog(&new)
        };
        if !errs.is_empty() {
            return Err(errs.into_iter().take(3).collect::<Vec<_>>().join("; "));
        }
        if self.path.exists() {
            let dir = self.path.parent().unwrap_or(Path::new(".")).join("backups");
            let _ = fs::create_dir_all(&dir);
            let _ = fs::copy(&self.path, dir.join(format!("catalog-{}.json", crate::util::now_iso().replace(':', "-"))));
            let all = self.backups();
            if all.len() > KEEP_BACKUPS {
                for p in &all[..all.len() - KEEP_BACKUPS] {
                    let _ = fs::remove_file(p);
                }
            }
        }
        write_json(&self.path, &new).map_err(|e| format!("couldn't save the catalog: {e}"))?;
        *w = new;
        Ok(out)
    }

    /// Add or replace a game. `old_title` renames. Returns true when it replaced one.
    pub fn upsert(&self, raw: &Value, create_only: bool, old_title: Option<&str>) -> Result<bool, String> {
        let g = tidy_game(raw)?;
        let t = s(&g, "t").to_string();
        self.edit(false, |d| {
            let games = d["games"].as_array_mut().ok_or("games must be a list")?;
            let key = old_title.unwrap_or(&t).to_string();
            if let Some(i) = games.iter().position(|x| s(x, "t") == key) {
                if create_only {
                    return Err(format!("{t} is already in the catalog"));
                }
                if key != t && games.iter().any(|x| s(x, "t").eq_ignore_ascii_case(&t)) {
                    return Err(format!("{t} is already in the catalog"));
                }
                let mut merged = g.clone();
                if let (Some(old_added), Some(m)) = (games[i].get("added").cloned(), merged.as_object_mut()) {
                    m.entry("added").or_insert(old_added);
                }
                games[i] = merged;
                Ok(true)
            } else {
                if games.iter().any(|x| s(x, "t").eq_ignore_ascii_case(&t)) {
                    return Err(format!("{t} is already in the catalog"));
                }
                let mut g = g.clone();
                g["added"] = json!(crate::util::today());
                games.push(g);
                Ok(false)
            }
        })
    }

    /// Add many at once (an import). Skips titles already present; returns how many were added and what was refused.
    pub fn add_many(&self, raws: &[Value]) -> Result<(usize, Vec<String>), String> {
        let mut refused = vec![];
        let tidy: Vec<Value> = raws.iter().filter_map(|r| tidy_game(r).map_err(|e| refused.push(e)).ok()).collect();
        self.edit(false, |d| {
            let games = d["games"].as_array_mut().ok_or("games must be a list")?;
            let mut n = 0;
            for mut g in tidy {
                if games.len() >= MAX_GAMES {
                    refused.push(format!("{}: the catalog is full", s(&g, "t")));
                    continue;
                }
                if games.iter().any(|x| s(x, "t").eq_ignore_ascii_case(s(&g, "t"))) {
                    continue;
                }
                g["added"] = json!(crate::util::today());
                games.push(g);
                n += 1;
            }
            Ok((n, refused))
        })
    }

    pub fn remove(&self, title: &str) -> Result<bool, String> {
        self.edit(false, |d| {
            let games = d["games"].as_array_mut().ok_or("games must be a list")?;
            let before = games.len();
            games.retain(|x| s(x, "t") != title);
            Ok(games.len() != before)
        })
    }

    /// Fill in fields a scraper found, without overwriting anything already there (except `info`, which is merged key by key).
    pub fn enrich(&self, title: &str, patch: &Map<String, Value>) -> Result<bool, String> {
        self.edit(false, |d| {
            let games = d["games"].as_array_mut().ok_or("games must be a list")?;
            let g = games.iter_mut().find(|x| s(x, "t") == title).ok_or(format!("{title} isn't in the catalog"))?;
            let before = g.clone();
            for (k, v) in patch {
                if k == "info" {
                    let info = g.as_object_mut().unwrap().entry("info").or_insert_with(|| json!({}));
                    if let (Some(dst), Some(src)) = (info.as_object_mut(), v.as_object()) {
                        for (ik, iv) in src {
                            dst.entry(ik.clone()).or_insert_with(|| iv.clone());
                        }
                    }
                } else if g.get(k).is_none() {
                    g[k] = v.clone();
                }
            }
            Ok(*g != before)
        })
    }

    pub fn add_wish(&self, entry: &Value) -> Result<(), String> {
        let t = s(entry, "t").trim().to_string();
        let why = s(entry, "why").trim().to_string();
        let sys = s(entry, "sys").trim().to_string();
        if t.is_empty() || t.chars().count() > MAX_TITLE {
            return Err("A title is needed (80 characters at most)".into());
        }
        if why.is_empty() || why.chars().count() > 240 {
            return Err("A short reason is needed (240 characters at most)".into());
        }
        if sys.is_empty() || sys.chars().count() > 24 {
            return Err("The platform is needed (24 characters at most)".into());
        }
        self.edit(true, |d| {
            if d["games"].as_array().into_iter().flatten().any(|g| s(g, "t").eq_ignore_ascii_case(&t)) {
                return Err(format!("{t} is already in the catalog"));
            }
            let w = d["wishlist"].as_array_mut().ok_or("the wishlist isn't a list")?;
            if w.iter().any(|x| s(x, "t").eq_ignore_ascii_case(&t)) {
                return Err(format!("{t} is already on the wishlist"));
            }
            w.push(json!({"t": t, "why": why, "sys": sys}));
            Ok(())
        })
    }

    pub fn remove_wish(&self, title: &str) -> Result<bool, String> {
        self.edit(true, |d| {
            let w = d["wishlist"].as_array_mut().ok_or("the wishlist isn't a list")?;
            let before = w.len();
            w.retain(|x| s(x, "t") != title);
            Ok(w.len() != before)
        })
    }

    pub fn set_wish_price(&self, title: &str, price: Option<&Value>) -> Result<(), String> {
        if let Some(p) = price {
            let e = price_issues(p);
            if !e.is_empty() {
                return Err(e.join("; "));
            }
        }
        self.edit(true, |d| {
            let w = d["wishlist"].as_array_mut().ok_or("the wishlist isn't a list")?;
            let x = w.iter_mut().find(|x| s(x, "t") == title).ok_or(format!("{title} isn't on the wishlist"))?;
            match price {
                Some(p) => x["price"] = p.clone(),
                None => {
                    x.as_object_mut().map(|o| o.remove("price"));
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn game(t: &str) -> Value {
        json!({"t": t, "sys": "PC", "where": "couch", "p": "1-4", "mode": "Party", "gap": 4, "vibe": ["silly"], "len": "quick", "why": "x. y"})
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gn-cat-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d.join("catalog.json")
    }

    #[test]
    fn a_good_game_passes_and_each_rule_bites() {
        assert!(validate_game(&game("A")).is_empty());
        for (k, v) in [
            ("where", json!("sofa")),
            ("len", json!("forever")),
            ("gap", json!(9)),
            ("p", json!("4-1")),
            ("p", json!("x")),
            ("vibe", json!([])),
            ("vibe", json!(["scary"])),
            ("why", json!("")),
            ("trailer", json!("nope")),
            ("one", json!("yes")),
            ("launch", json!({"url": "https://evil.example"})),
            ("launch", json!({"command": "rm -rf"})),
            ("launch", json!({"steam": 1, "url": "steam://x"})),
            ("media", json!({"file": "../x"})),
            ("info", json!({"year": 1})),
            ("info", json!({"oops": 1})),
        ] {
            let mut g = game("A");
            g[k] = v.clone();
            assert!(!validate_game(&g).is_empty(), "{k} = {v} should be refused");
        }
        let mut g = game("A");
        g["launch"] = json!({"steam": 620});
        g["media"] = json!({"system": "switch", "file": "Some Game [0100]"});
        g["info"] = json!({"year": 2020, "genres": ["party"], "steamAppId": 620, "scores": {"metacritic": 90}});
        assert!(validate_game(&g).is_empty(), "{:?}", validate_game(&g));
    }

    #[test]
    fn the_bundled_sample_is_valid() {
        let v: Value = serde_json::from_str(crate::app::SAMPLE).unwrap();
        assert_eq!(validate_catalog(&v), Vec::<String>::new());
        assert!(v["games"].as_array().unwrap().len() >= 20);
        for g in v["games"].as_array().unwrap() {
            for k in ["ids", "pc", "steam", "launch", "media", "process"] {
                assert!(g.get(k).is_none(), "{}: the sample carries no paths or ids ({k})", s(g, "t"));
            }
        }
    }

    #[test]
    fn tone_check_refuses_coaching() {
        let mut g = game("A");
        g["why"] = json!("Great because you can let them win.");
        assert!(validate_game(&g)[0].contains("coaching"));
        assert_eq!(tone_issues("Don’t rescue them"), vec!["don't rescue"]);
    }

    #[test]
    fn video_links_become_ids() {
        for l in [
            "dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=3",
            "https://youtu.be/dQw4w9WgXcQ?si=x",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
        ] {
            assert_eq!(parse_video_id(l).as_deref(), Some("dQw4w9WgXcQ"), "{l}");
        }
        for l in ["https://vimeo.com/123", "https://www.youtube.com/playlist?list=x", "dQw4w9WgXc\n"] {
            assert_eq!(parse_video_id(l), None, "{l}");
        }
    }

    #[test]
    fn edits_are_validated_backed_up_and_survive_a_reload() {
        let p = temp("edit");
        let c = Catalog::open(&p);
        assert!(!c.upsert(&game("A"), true, None).unwrap());
        assert!(c.upsert(&game("A"), true, None).is_err());
        let mut g = game("A");
        g["trailer"] = json!("https://youtu.be/dQw4w9WgXcQ");
        assert!(c.upsert(&g, false, None).unwrap());
        assert_eq!(s(&c.get("A").unwrap(), "trailer"), "dQw4w9WgXcQ");
        assert!(c.get("A").unwrap().get("added").is_some(), "the date it was added survives an edit");
        let mut bad = game("B");
        bad["gap"] = json!(0);
        assert!(c.upsert(&bad, false, None).is_err());
        assert_eq!(Catalog::open(&p).games().len(), 1);
        assert!(!c.backups().is_empty());
        let mut renamed = game("A2");
        renamed["why"] = json!("renamed");
        c.upsert(&renamed, false, Some("A")).unwrap();
        assert!(c.get("A").is_none() && c.get("A2").is_some());
    }

    #[test]
    fn a_damaged_file_falls_back_to_the_newest_backup() {
        let p = temp("damaged");
        let c = Catalog::open(&p);
        c.upsert(&game("A"), true, None).unwrap();
        c.upsert(&game("B"), true, None).unwrap();
        fs::write(&p, "{ broken").unwrap();
        let again = Catalog::open(&p);
        assert_eq!(again.games().len(), 1, "the backup taken before B was added");
        assert!(!again.recovered_from.read().unwrap().is_empty());
    }

    #[test]
    fn wishlist_and_prices() {
        let p = temp("wish");
        let c = Catalog::open(&p);
        c.add_wish(&json!({"t": "W", "why": "fun for four", "sys": "Switch"})).unwrap();
        assert!(c.add_wish(&json!({"t": "w", "why": "again", "sys": "PC"})).is_err());
        c.set_wish_price("W", Some(&json!({"low": "$9.99", "where": "eShop", "asOf": "2026-10-22", "source": "https://x.example"}))).unwrap();
        assert!(c.set_wish_price("W", Some(&json!({"low": "$1"}))).is_err());
        assert!(c.set_wish_price("W", Some(&json!({"low": "$1", "where": "x", "asOf": "2026-10-22", "source": "http://x"}))).is_err());
        assert_eq!(s(&c.wishlist()[0]["price"], "low"), "$9.99");
        c.set_wish_price("W", None).unwrap();
        assert!(c.wishlist()[0].get("price").is_none());
        assert!(c.remove_wish("W").unwrap());
    }

    #[test]
    fn enrich_never_overwrites() {
        let p = temp("enrich");
        let c = Catalog::open(&p);
        let mut g = game("A");
        g["info"] = json!({"year": 2001});
        c.upsert(&g, true, None).unwrap();
        let patch: Map<String, Value> =
            serde_json::from_value(json!({"info": {"year": 1999, "developer": "D"}, "why": "other", "trailer": "dQw4w9WgXcQ"})).unwrap();
        assert!(c.enrich("A", &patch).unwrap());
        let a = c.get("A").unwrap();
        assert_eq!((a["info"]["year"].as_i64(), s(&a["info"], "developer"), s(&a, "why"), s(&a, "trailer")), (Some(2001), "D", "x. y", "dQw4w9WgXcQ"));
    }
}
