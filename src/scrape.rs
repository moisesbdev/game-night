//! Scrapers: fill in a game's details and art from online sources, only when someone presses a button.
//!
//! Each source is a `Provider`; adding one is one more impl. Built in: the Steam store (no key) and SteamGridDB (art; needs a key
//! from Settings). Results never overwrite what someone wrote, except on a draft (a game imported but not yet checked), whose
//! placeholder fields are replaced. Images are only downloaded from the providers' own image hosts, at most 10 MB each.
#![cfg_attr(not(feature = "scrape"), allow(dead_code))] // without online lookups the helpers are compiled but unused

use crate::util::{Json, s};
use serde_json::{Map, Value, json};

#[derive(Debug, Default)]
pub struct Found {
    /// Fields for the game (`info` merged key by key).
    pub patch: Map<String, Value>,
    /// (kind, url) of images to save: "cover", "shot".
    pub images: Vec<(String, String)>,
    pub source: String,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Why it can't run right now (a missing key), or None.
    fn unavailable(&self) -> Option<String>;
    fn find(&self, game: &Json) -> Result<Found, String>;
}

pub const IMAGE_HOSTS: [&str; 6] = [
    "shared.cloudflare.steamstatic.com",
    "cdn.cloudflare.steamstatic.com",
    "shared.akamai.steamstatic.com",
    "cdn.akamai.steamstatic.com",
    "cdn2.steamgriddb.com",
    "cdn.steamgriddb.com",
];

pub fn host_ok(url: &str) -> bool {
    url.strip_prefix("https://").and_then(|r| r.split('/').next()).is_some_and(|h| IMAGE_HOSTS.contains(&h))
}

/// Plain text from Steam's HTML-ish descriptions.
pub fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in text.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn norm(t: &str) -> String {
    t.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// What Steam's categories and genres say about playing together, as catalog fields (used to fill in drafts).
pub fn suggest_from_steam(d: &Value) -> Map<String, Value> {
    let cats: Vec<i64> = d["categories"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_i64()).collect();
    let has = |ids: &[i64]| ids.iter().any(|i| cats.contains(i));
    let couch = has(&[24, 37, 39, 44]);
    let online = has(&[36, 38, 47, 48, 49, 27]);
    let coop = has(&[9, 38, 39, 48]);
    let versus = has(&[36, 37, 47, 49]);
    let multi = couch || online || has(&[1]);
    let mut m = Map::new();
    m.insert("where".into(), json!(if couch || !online { "couch" } else { "online" }));
    m.insert("p".into(), json!(if multi { "1-4" } else { "1" }));
    m.insert(
        "mode".into(),
        json!(match (coop, versus) {
            (true, true) => "Versus / co-op",
            (true, false) => "Co-op",
            (false, true) => "Versus",
            _ if multi => "Party",
            _ => "Take turns",
        }),
    );
    if !multi {
        m.insert("one".into(), json!(true));
    }
    let genres: Vec<String> = d["genres"].as_array().into_iter().flatten().filter_map(|g| g["description"].as_str().map(str::to_lowercase)).collect();
    let text = format!("{} {}", s(d, "short_description"), s(d, "name")).to_lowercase();
    let mut vibe: Vec<&str> = vec![];
    fn add(v: &mut Vec<&'static str>, x: &'static str) {
        if !v.contains(&x) {
            v.push(x);
        }
    }
    for g in &genres {
        match g.as_str() {
            "action" => add(&mut vibe, "action"),
            "adventure" | "rpg" => add(&mut vibe, "story"),
            "casual" | "simulation" => add(&mut vibe, "cozy"),
            "strategy" => add(&mut vibe, "brain"),
            "racing" | "sports" => add(&mut vibe, "compete"),
            _ => {}
        }
    }
    if ["horror", "terror", "zombie", "haunted"].iter().any(|w| text.contains(w)) {
        add(&mut vibe, "spooky");
    }
    if text.contains("puzzle") {
        add(&mut vibe, "brain");
    }
    if versus {
        add(&mut vibe, "compete");
    }
    if text.contains("party") || text.contains("chaotic") || text.contains("hilarious") {
        add(&mut vibe, "silly");
    }
    if vibe.is_empty() {
        add(&mut vibe, "action");
    }
    vibe.truncate(3);
    m.insert("vibe".into(), json!(vibe));
    m
}

/// The `info` object and art from a Steam appdetails `data` block.
pub fn from_steam_details(id: u64, d: &Value) -> Found {
    let mut info = Map::new();
    info.insert("steamAppId".into(), json!(id));
    let year = s(&d["release_date"], "date")
        .split(|c: char| !c.is_ascii_digit())
        .find(|w| w.len() == 4)
        .and_then(|y| y.parse::<i64>().ok())
        .filter(|y| (1970..=2100).contains(y));
    if let Some(y) = year {
        info.insert("year".into(), json!(y));
    }
    for (k, src) in [("developer", "developers"), ("publisher", "publishers")] {
        if let Some(v) =
            d[src].as_array().and_then(|a| a.first()).and_then(Value::as_str).map(|v| v.chars().take(80).collect::<String>()).filter(|v| !v.trim().is_empty())
        {
            info.insert(k.into(), json!(v));
        }
    }
    let genres: Vec<String> = d["genres"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| g["description"].as_str().map(|x| x.to_lowercase().chars().take(40).collect()))
        .take(8)
        .collect();
    if !genres.is_empty() {
        info.insert("genres".into(), json!(genres));
    }
    if let Some(m) = d["metacritic"]["score"].as_i64().filter(|x| (0..=100).contains(x)) {
        info.insert("scores".into(), json!({"metacritic": m}));
    }
    if let Some(c) = d["controller_support"].as_str().filter(|c| ["full", "partial"].contains(c)) {
        info.insert("controller".into(), json!(c));
    }
    let summary: String = plain(s(d, "short_description")).chars().take(400).collect();
    if !summary.is_empty() {
        info.insert("summary".into(), json!(summary));
    }
    info.insert("asOf".into(), json!(crate::util::today()));
    info.insert("sources".into(), json!([format!("https://store.steampowered.com/app/{id}/")]));
    let mut patch = Map::new();
    patch.insert("info".into(), Value::Object(info));
    if !summary.is_empty() && crate::catalog::tone_issues(&summary).is_empty() {
        let why: String = summary.chars().take(300).collect();
        patch.insert("why".into(), json!(why));
    }
    let mut images = vec![("cover".to_string(), format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{id}/library_600x900_2x.jpg"))];
    if let Some(u) = d["screenshots"].as_array().and_then(|a| a.first()).and_then(|x| x["path_full"].as_str()) {
        images.push(("shot".into(), u.split('?').next().unwrap_or(u).to_string()));
    }
    if let Some(h) = d["header_image"].as_str() {
        images.push(("cover".into(), h.split('?').next().unwrap_or(h).to_string()));
    }
    Found { patch, images, source: "Steam store".into() }
}

#[cfg(feature = "scrape")]
mod net {
    use super::*;
    use std::io::Read;
    use std::time::Duration;

    pub fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).user_agent(concat!("GameNight/", env!("CARGO_PKG_VERSION"))).build().into()
    }

    pub fn get_json(url: &str, bearer: Option<&str>) -> Result<Value, String> {
        let mut req = agent().get(url);
        if let Some(k) = bearer {
            req = req.header("Authorization", &format!("Bearer {k}"));
        }
        let mut r = req.call().map_err(|e| e.to_string())?;
        r.body_mut().with_config().limit(4 << 20).read_json::<Value>().map_err(|e| e.to_string())
    }

    /// An image from an allowed host: (bytes, extension). Refuses anything that isn't an image or is over 10 MB.
    pub fn get_image(url: &str) -> Result<(Vec<u8>, &'static str), String> {
        if !host_ok(url) {
            return Err(format!("not an allowed image host: {url}"));
        }
        let mut r = agent().get(url).call().map_err(|e| e.to_string())?;
        let ext = match r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("") {
            t if t.starts_with("image/jpeg") => "jpg",
            t if t.starts_with("image/png") => "png",
            t if t.starts_with("image/webp") => "webp",
            t => return Err(format!("not an image ({t})")),
        };
        let mut buf = vec![];
        r.body_mut().as_reader().take(10 << 20).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok((buf, ext))
    }

    pub fn enc(t: &str) -> String {
        t.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
    }
}

#[cfg(feature = "scrape")]
pub use net::get_image;

pub struct SteamStore;

impl Provider for SteamStore {
    fn id(&self) -> &'static str {
        "steam"
    }
    fn label(&self) -> &'static str {
        "Steam store"
    }
    fn unavailable(&self) -> Option<String> {
        if cfg!(feature = "scrape") { None } else { Some("this build was made without online lookups".into()) }
    }
    #[cfg(not(feature = "scrape"))]
    fn find(&self, _: &Json) -> Result<Found, String> {
        Err("this build was made without online lookups".into())
    }
    #[cfg(feature = "scrape")]
    fn find(&self, game: &Json) -> Result<Found, String> {
        let id = match crate::launch::steam_id(game) {
            Some(id) => id,
            None => {
                let r = net::get_json(&format!("https://store.steampowered.com/api/storesearch/?term={}&cc=us&l=english", net::enc(s(game, "t"))), None)?;
                let want = norm(s(game, "t"));
                let items = r["items"].as_array().cloned().unwrap_or_default();
                items
                    .iter()
                    .find(|i| norm(s(i, "name")) == want)
                    .or_else(|| {
                        items.iter().find(|i| {
                            let n = norm(s(i, "name"));
                            !want.is_empty() && (n.starts_with(&want) || want.starts_with(&n))
                        })
                    })
                    .and_then(|i| i["id"].as_u64())
                    .ok_or("not found on the Steam store")?
            }
        };
        let r = net::get_json(&format!("https://store.steampowered.com/api/appdetails?appids={id}&l=english"), None)?;
        let d = &r[id.to_string()];
        if d["success"] != json!(true) {
            return Err("the Steam store has no details for it".into());
        }
        let mut f = from_steam_details(id, &d["data"]);
        for (k, v) in suggest_from_steam(&d["data"]) {
            f.patch.insert(format!("suggest:{k}"), v);
        }
        Ok(f)
    }
}

pub struct SteamGridDb {
    pub key: String,
}

impl Provider for SteamGridDb {
    fn id(&self) -> &'static str {
        "steamgriddb"
    }
    fn label(&self) -> &'static str {
        "SteamGridDB"
    }
    fn unavailable(&self) -> Option<String> {
        if !cfg!(feature = "scrape") {
            Some("this build was made without online lookups".into())
        } else if self.key.is_empty() {
            Some("add a SteamGridDB API key in Settings".into())
        } else {
            None
        }
    }
    #[cfg(not(feature = "scrape"))]
    fn find(&self, _: &Json) -> Result<Found, String> {
        Err("this build was made without online lookups".into())
    }
    #[cfg(feature = "scrape")]
    fn find(&self, game: &Json) -> Result<Found, String> {
        if let Some(why) = self.unavailable() {
            return Err(why);
        }
        let base = "https://www.steamgriddb.com/api/v2";
        let key = Some(self.key.as_str());
        let gid = match crate::launch::steam_id(game) {
            Some(app) => net::get_json(&format!("{base}/games/steam/{app}"), key)?["data"]["id"].as_u64(),
            None => net::get_json(&format!("{base}/search/autocomplete/{}", net::enc(s(game, "t"))), key)?["data"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|x| x["id"].as_u64()),
        }
        .ok_or("not found on SteamGridDB")?;
        let mut images = vec![];
        if let Some(u) = net::get_json(&format!("{base}/grids/game/{gid}?dimensions=600x900"), key)?["data"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|x| x["url"].as_str())
        {
            images.push(("cover".to_string(), u.to_string()));
        }
        if let Some(u) = net::get_json(&format!("{base}/heroes/game/{gid}"), key)?["data"].as_array().and_then(|a| a.first()).and_then(|x| x["url"].as_str()) {
            images.push(("shot".to_string(), u.to_string()));
        }
        Ok(Found { patch: Map::new(), images, source: "SteamGridDB".into() })
    }
}

pub fn providers(settings: &crate::config::Settings) -> Vec<Box<dyn Provider>> {
    vec![Box::new(SteamStore), Box::new(SteamGridDb { key: settings.steamgriddb_key.clone() })]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details() -> Value {
        json!({
            "name": "It Takes Two", "short_description": "Embark on the craziest journey &amp; a <b>co-op</b> platform adventure.",
            "developers": ["Hazelight"], "publishers": ["Electronic Arts"], "release_date": {"date": "26 Mar, 2021"},
            "genres": [{"description": "Action"}, {"description": "Adventure"}], "categories": [{"id": 9}, {"id": 39}, {"id": 38}, {"id": 44}],
            "metacritic": {"score": 88}, "controller_support": "full", "screenshots": [{"path_full": "https://shared.akamai.steamstatic.com/x/ss_1.jpg?t=1"}],
        })
    }

    #[test]
    fn steam_details_become_valid_info() {
        let f = from_steam_details(1426210, &details());
        let info = &f.patch["info"];
        assert!(crate::catalog::info_issues(info).is_empty(), "{:?}", crate::catalog::info_issues(info));
        assert_eq!((info["year"].as_i64(), s(info, "developer")), (Some(2021), "Hazelight"));
        assert_eq!(s(&Value::Object(f.patch.clone()), "why"), "Embark on the craziest journey & a co-op platform adventure.");
        assert_eq!(f.images[1].1, "https://shared.akamai.steamstatic.com/x/ss_1.jpg");
        assert!(f.images.iter().all(|(_, u)| host_ok(u)));
    }

    #[test]
    fn categories_suggest_how_it_is_played() {
        let m = suggest_from_steam(&details());
        assert_eq!((s(&m["where"], ""), m["where"].as_str(), m["mode"].as_str(), m["p"].as_str()), ("", Some("couch"), Some("Co-op"), Some("1-4")));
        let solo = suggest_from_steam(
            &json!({"name": "Dread", "short_description": "A survival horror story.", "categories": [{"id": 2}], "genres": [{"description": "Adventure"}]}),
        );
        assert_eq!((solo["p"].as_str(), solo["one"].as_bool(), solo["mode"].as_str()), (Some("1"), Some(true), Some("Take turns")));
        assert!(solo["vibe"].as_array().unwrap().contains(&json!("spooky")));
    }

    #[test]
    fn only_known_image_hosts() {
        assert!(host_ok("https://cdn2.steamgriddb.com/grid/x.png"));
        assert!(!host_ok("http://cdn2.steamgriddb.com/grid/x.png"));
        assert!(!host_ok("https://evil.example/cdn2.steamgriddb.com/x.png"));
    }
}
