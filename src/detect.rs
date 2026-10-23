//! Library detectors: each one knows where one launcher or frontend keeps its own records, finds them by itself, and reads them.
//! Nothing is written, and nothing is assumed about one household: every location is discovered (home folders, the registry,
//! every connected drive). Each detector has a pure part that reads given files (tested) and a thin part that finds them.
//!
//! - ES-DE: `gamelists/<system>/gamelist.xml` (names, descriptions, genres, players, dates) and its downloaded art.
//! - Epic Games: the launcher's install manifests (`*.item`).
//! - GOG Galaxy: the GOG games registered in the Windows registry.
//! - RetroArch: its playlists (`playlists/*.lpl`); games start with their core.
//! - Heroic (Epic and GOG on Linux and Windows) and Lutris (Linux).

use crate::util::{Json, s};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// A game a detector found. `extra` holds catalog fields it could tell (players, a description, moods, `info`).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub source: &'static str,
    pub key: String,
    pub title: String,
    pub sys: String,
    pub launch: Option<Value>,
    pub media: Option<Value>,
    pub extra: Map<String, Value>,
}

/// One library source and what it found. `art` is a media folder the source brings (ES-DE's downloaded_media).
#[derive(Debug, Clone, Default)]
pub struct Source {
    pub id: &'static str,
    pub label: &'static str,
    pub found_at: Vec<PathBuf>,
    pub games: Vec<Candidate>,
    pub art: Option<PathBuf>,
    pub note: String,
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

fn env_dir(k: &str) -> Option<PathBuf> {
    std::env::var_os(k).map(PathBuf::from).filter(|p| p.is_dir())
}

fn unescape(t: &str) -> String {
    t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#39;", "'").replace("&amp;", "&")
}

/// `<tag>value</tag>` inside one XML element (ES-DE writes plain, one-level game entries).
fn xml_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let i = block.find(&open)? + open.len();
    let j = block[i..].find(&format!("</{tag}>"))? + i;
    Some(unescape(block[i..j].trim()))
}

/// Moods from genre words any source uses ("Platform", "Racing, Driving", "Survival horror").
pub fn vibes_from_genres(genres: &str) -> Vec<&'static str> {
    let g = genres.to_lowercase();
    let mut v: Vec<&'static str> = vec![];
    for (words, vibe) in [
        (&["horror", "survival"][..], "spooky"),
        (&["puzzle", "strategy", "tactic"][..], "brain"),
        (&["racing", "driving", "sport", "fighting", "versus"][..], "compete"),
        (&["party", "music", "rhythm", "comedy"][..], "silly"),
        (&["rpg", "role", "adventure", "story", "visual novel"][..], "story"),
        (&["simulation", "life", "casual", "farming"][..], "cozy"),
        (&["platform", "action", "shooter", "beat", "shmup", "arcade"][..], "action"),
    ] {
        if words.iter().any(|w| g.contains(w)) && !v.contains(&vibe) {
            v.push(vibe);
        }
    }
    v.truncate(3);
    v
}

// ---------------------------------------------------------------- ES-DE

/// ES-DE system folder names as platform labels; anything else keeps its folder name.
pub fn esde_platform(system: &str) -> String {
    let m: &[(&str, &str)] = &[
        ("switch", "Switch"),
        ("wiiu", "Wii U"),
        ("wii", "Wii"),
        ("gc", "GameCube"),
        ("n64", "Nintendo 64"),
        ("snes", "SNES"),
        ("nes", "NES"),
        ("famicom", "NES"),
        ("gb", "Game Boy"),
        ("gbc", "Game Boy Color"),
        ("gba", "Game Boy Advance"),
        ("nds", "Nintendo DS"),
        ("n3ds", "Nintendo 3DS"),
        ("psx", "PlayStation"),
        ("ps2", "PlayStation 2"),
        ("ps3", "PlayStation 3"),
        ("ps4", "PlayStation 4"),
        ("psp", "PSP"),
        ("psvita", "PS Vita"),
        ("xbox", "Xbox"),
        ("xbox360", "Xbox 360"),
        ("genesis", "Genesis"),
        ("megadrive", "Genesis"),
        ("mastersystem", "Master System"),
        ("saturn", "Saturn"),
        ("dreamcast", "Dreamcast"),
        ("arcade", "Arcade"),
        ("mame", "Arcade"),
        ("fbneo", "Arcade"),
        ("neogeo", "Neo Geo"),
        ("pcengine", "PC Engine"),
        ("ports", "PC"),
        ("dos", "DOS"),
        ("scummvm", "ScummVM"),
    ];
    m.iter().find(|(k, _)| *k == system).map(|(_, v)| v.to_string()).unwrap_or_else(|| system.to_string())
}

/// ES-DE systems that only point at games another detector already finds (Steam, Epic, GOG, Windows shortcuts) or aren't games.
const ESDE_SKIP: [&str; 9] = ["steam", "windows", "epic", "gog", "desktop", "kodi", "android", "lutris", "heroic"];

/// Games from an ES-DE home folder's gamelists. Hidden entries and folders are left out.
pub fn esde_from(esde_home: &Path) -> Vec<Candidate> {
    let mut out = vec![];
    let Ok(systems) = fs::read_dir(esde_home.join("gamelists")) else { return out };
    for sys in systems.flatten() {
        let system = sys.file_name().to_string_lossy().to_string();
        if ESDE_SKIP.contains(&system.as_str()) {
            continue;
        }
        let Ok(xml) = fs::read_to_string(sys.path().join("gamelist.xml")) else { continue };
        for block in xml.split("<game>").skip(1).filter_map(|b| b.split("</game>").next()) {
            let (Some(path), Some(name)) = (xml_tag(block, "path"), xml_tag(block, "name")) else { continue };
            if xml_tag(block, "hidden").as_deref() == Some("true") || name.trim().is_empty() {
                continue;
            }
            let file = Path::new(&path).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            let mut extra = Map::new();
            let mut info = Map::new();
            if let Some(p) = xml_tag(block, "players").filter(|p| crate::catalog::prange(p).1 >= 1 && p.chars().all(|c| c.is_ascii_digit() || c == '-')) {
                extra.insert("p".into(), json!(if p.contains('-') || p == "1" { p.clone() } else { format!("1-{p}") }));
                if p == "1" {
                    extra.insert("one".into(), json!(true));
                }
            }
            if let Some(d) = xml_tag(block, "desc") {
                let line: String = d.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect();
                if !line.is_empty() && crate::catalog::tone_issues(&line).is_empty() {
                    extra.insert("why".into(), json!(line));
                }
            }
            if let Some(g) = xml_tag(block, "genre") {
                let v = vibes_from_genres(&g);
                if !v.is_empty() {
                    extra.insert("vibe".into(), json!(v));
                }
                info.insert("genres".into(), json!(g.split([',', '/']).map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).take(8).collect::<Vec<_>>()));
            }
            if let Some(y) = xml_tag(block, "releasedate").and_then(|d| d.get(..4).and_then(|y| y.parse::<i64>().ok())).filter(|y| (1970..=2100).contains(y)) {
                info.insert("year".into(), json!(y));
            }
            for (k, tag) in [("developer", "developer"), ("publisher", "publisher")] {
                if let Some(v) = xml_tag(block, tag).filter(|v| !v.is_empty()) {
                    info.insert(k.into(), json!(v.chars().take(80).collect::<String>()));
                }
            }
            if !info.is_empty() {
                extra.insert("info".into(), Value::Object(info));
            }
            out.push(Candidate {
                source: "esde",
                key: format!("{system}/{path}"),
                title: name.chars().take(80).collect(),
                sys: esde_platform(&system),
                launch: None,
                media: Some(json!({"system": system, "file": file})),
                extra,
            });
        }
    }
    out.sort_by_key(|c| (c.sys.clone(), c.title.to_lowercase()));
    out
}

/// Where ES-DE keeps its downloaded art: `MediaDirectory` in es_settings.xml, else `<home>/downloaded_media`.
pub fn esde_media(esde_home: &Path) -> Option<PathBuf> {
    let settings = fs::read_to_string(esde_home.join("settings").join("es_settings.xml")).unwrap_or_default();
    let set = settings
        .lines()
        .find(|l| l.contains("name=\"MediaDirectory\""))
        .and_then(|l| l.split("value=\"").nth(1))
        .and_then(|v| v.split('"').next())
        .map(unescape)
        .filter(|v| !v.is_empty());
    set.map(PathBuf::from).or_else(|| Some(esde_home.join("downloaded_media"))).filter(|p| p.is_dir())
}

/// ES-DE homes on this computer: `~/ES-DE`, the old `~/.emulationstation`, Flatpak's, and portable installs on any drive.
pub fn esde_homes() -> Vec<PathBuf> {
    let mut c: Vec<PathBuf> = vec![];
    if let Some(h) = home() {
        c.push(h.join("ES-DE"));
        c.push(h.join(".emulationstation"));
        c.push(h.join(".var/app/org.es_de.frontend/ES-DE"));
    }
    for d in crate::folders::drives() {
        for rel in ["ES-DE/ES-DE", "ES-DE", "Emulation/ES-DE/ES-DE", "Emulation/ES-DE", "RetroBat/emulationstation/.emulationstation"] {
            c.push(crate::folders::join_rel(&d, rel));
        }
    }
    let mut out: Vec<PathBuf> = vec![];
    for p in c {
        if p.join("gamelists").is_dir() && !out.iter().any(|o| same(o, &p)) {
            out.push(p);
        }
    }
    out
}

// ---------------------------------------------------------------- Epic

/// Installed Epic games from the launcher's manifests (DLC, add-ons and unfinished installs left out).
pub fn epic_from(manifests: &Path) -> Vec<Candidate> {
    let mut out = vec![];
    for e in fs::read_dir(manifests).into_iter().flatten().flatten() {
        if e.path().extension().and_then(|x| x.to_str()) != Some("item") {
            continue;
        }
        let Ok(Some(m)) = crate::util::read_json(&e.path()) else { continue };
        let cats: Vec<String> = crate::util::strs(&m, "AppCategories");
        let main = s(&m, "MainGameAppName");
        let is_addon = !main.is_empty() && main != s(&m, "AppName"); // DLC and add-ons name the game they belong to
        let is_game = cats.iter().any(|c| c == "games") && m["bIsApplication"] != json!(false) && !is_addon;
        if !is_game || m["bIsIncompleteInstall"] == json!(true) || s(&m, "DisplayName").is_empty() {
            continue;
        }
        let url = format!(
            "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
            s(&m, "CatalogNamespace"),
            s(&m, "CatalogItemId"),
            s(&m, "AppName")
        );
        out.push(Candidate {
            source: "epic",
            key: s(&m, "AppName").into(),
            title: crate::steam::clean_name(s(&m, "DisplayName")),
            sys: "PC".into(),
            launch: Some(json!({"url": url})),
            media: None,
            extra: Map::new(),
        });
    }
    out.sort_by_key(|c| c.title.to_lowercase());
    out
}

pub fn epic_manifests() -> Option<PathBuf> {
    let base = env_dir("ProgramData").or_else(|| Some(PathBuf::from(r"C:\ProgramData")))?;
    Some(crate::folders::join_rel(&base, "Epic/EpicGamesLauncher/Data/Manifests")).filter(|p| p.is_dir())
}

// ---------------------------------------------------------------- GOG Galaxy

/// GOG games from the text of `reg query HKLM\SOFTWARE\WOW6432Node\GOG.com\Games /s` (DLC, which depends on a game, left out).
pub fn gog_from_reg(text: &str) -> Vec<Candidate> {
    let mut out = vec![];
    for section in text.split("HKEY_LOCAL_MACHINE").skip(1) {
        let val = |name: &str| -> String {
            section
                .lines()
                .find_map(|l| {
                    let t = l.trim_start();
                    t.strip_prefix(name).filter(|r| r.starts_with(' ')).and_then(|r| r.split_once("REG_SZ")).map(|(_, v)| v.trim().to_string())
                })
                .unwrap_or_default()
        };
        let (id, name, exe, dir) = (val("gameID"), val("gameName"), val("exe"), val("workingDir"));
        if id.is_empty() || name.is_empty() || exe.is_empty() || !val("dependsOn").is_empty() {
            continue;
        }
        let mut launch = json!({"command": [exe]});
        if !dir.is_empty() {
            launch["cwd"] = json!(dir);
        }
        out.push(Candidate {
            source: "gog",
            key: id,
            title: crate::steam::clean_name(&name),
            sys: "PC".into(),
            launch: Some(launch),
            media: None,
            extra: Map::new(),
        });
    }
    out.sort_by_key(|c| c.title.to_lowercase());
    out
}

fn gog_registry() -> String {
    if !cfg!(windows) {
        return String::new();
    }
    let mut c = std::process::Command::new("reg");
    c.args(["query", r"HKLM\SOFTWARE\WOW6432Node\GOG.com\Games", "/s"]);
    crate::sessions::no_window(&mut c);
    c.output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

// ---------------------------------------------------------------- RetroArch

/// "Nintendo - Super Nintendo Entertainment System" -> "SNES"-style short names for the common playlists; others as they are.
fn retroarch_platform(playlist: &str) -> String {
    let p = playlist.to_lowercase();
    let m: &[(&str, &str)] = &[
        ("super nintendo", "SNES"),
        ("nintendo entertainment system", "NES"),
        ("nintendo 64", "Nintendo 64"),
        ("game boy advance", "Game Boy Advance"),
        ("game boy color", "Game Boy Color"),
        ("game boy", "Game Boy"),
        ("nintendo ds", "Nintendo DS"),
        ("gamecube", "GameCube"),
        ("wii", "Wii"),
        ("playstation portable", "PSP"),
        ("playstation 2", "PlayStation 2"),
        ("playstation", "PlayStation"),
        ("mega drive", "Genesis"),
        ("genesis", "Genesis"),
        ("saturn", "Saturn"),
        ("dreamcast", "Dreamcast"),
        ("arcade", "Arcade"),
        ("mame", "Arcade"),
        ("fbneo", "Arcade"),
    ];
    m.iter().find(|(k, _)| p.contains(k)).map(|(_, v)| v.to_string()).unwrap_or_else(|| playlist.split(" - ").last().unwrap_or(playlist).to_string())
}

/// Games from RetroArch playlists. A game whose playlist names a core starts with `retroarch -L <core> <game>`.
pub fn retroarch_from(dir: &Path, exe: Option<&Path>) -> Vec<Candidate> {
    let mut out = vec![];
    for e in fs::read_dir(dir.join("playlists")).into_iter().flatten().flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("lpl") {
            continue;
        }
        let playlist = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let Ok(Some(lpl)) = crate::util::read_json(&path) else { continue };
        for it in lpl["items"].as_array().into_iter().flatten() {
            let rom = s(it, "path");
            let label = s(it, "label");
            if rom.is_empty() {
                continue;
            }
            let title = if label.is_empty() {
                Path::new(rom.split('#').next().unwrap_or(rom)).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()
            } else {
                label.to_string()
            };
            let core = s(it, "core_path");
            let core = if core.is_empty() || core == "DETECT" { s(&lpl, "default_core_path") } else { core };
            let launch = match (exe, core.is_empty() || core == "DETECT") {
                (Some(x), false) => Some(json!({"command": [x.to_string_lossy(), "-L", core, rom]})),
                _ => None,
            };
            let clean: String = title.split(" (").next().unwrap_or(&title).trim().chars().take(80).collect();
            out.push(Candidate {
                source: "retroarch",
                key: format!("{playlist}|{rom}"),
                title: clean,
                sys: retroarch_platform(&playlist),
                launch,
                media: None,
                extra: Map::new(),
            });
        }
    }
    out.sort_by_key(|c| (c.sys.clone(), c.title.to_lowercase()));
    out
}

/// RetroArch's folder (with its playlists) and program: the usual install and config places, and any drive's RetroArch folder.
pub fn retroarch_dirs() -> Vec<(PathBuf, Option<PathBuf>)> {
    let mut c: Vec<PathBuf> = vec![];
    if let Some(a) = env_dir("APPDATA") {
        c.push(a.join("RetroArch"));
    }
    if let Some(h) = home() {
        c.push(h.join(".config/retroarch"));
        c.push(h.join(".var/app/org.libretro.RetroArch/config/retroarch"));
        c.push(h.join("snap/retroarch/current/.config/retroarch"));
        c.push(h.join("Library/Application Support/RetroArch"));
    }
    for d in crate::folders::drives() {
        for rel in ["RetroArch", "RetroArch-Win64", "Emulation/RetroArch", "Program Files/RetroArch", "Emulators/RetroArch"] {
            c.push(crate::folders::join_rel(&d, rel));
        }
    }
    let path_exe = || -> Option<PathBuf> {
        std::env::var_os("PATH")
            .and_then(|ps| std::env::split_paths(&ps).map(|d| d.join(if cfg!(windows) { "retroarch.exe" } else { "retroarch" })).find(|p| p.is_file()))
    };
    let mut out: Vec<(PathBuf, Option<PathBuf>)> = vec![];
    for p in c {
        if p.join("playlists").is_dir() && !out.iter().any(|(o, _)| same(o, &p)) {
            let exe = Some(p.join(if cfg!(windows) { "retroarch.exe" } else { "retroarch" })).filter(|x| x.is_file()).or_else(path_exe);
            out.push((p, exe));
        }
    }
    out
}

// ---------------------------------------------------------------- Heroic and Lutris

/// Heroic's installed Epic (legendary) and GOG games. Both start through Heroic (`heroic://launch/...`).
pub fn heroic_from(config: &Path) -> Vec<Candidate> {
    let mut out = vec![];
    if let Ok(Some(v)) = crate::util::read_json(&config.join("legendaryConfig/legendary/installed.json")) {
        for (app, g) in v.as_object().into_iter().flatten() {
            if g["is_dlc"] == json!(true) || s(g, "title").is_empty() {
                continue;
            }
            out.push(Candidate {
                source: "heroic",
                key: format!("legendary/{app}"),
                title: crate::steam::clean_name(s(g, "title")),
                sys: "PC".into(),
                launch: Some(json!({"url": format!("heroic://launch/legendary/{app}")})),
                media: None,
                extra: Map::new(),
            });
        }
    }
    let titles: Vec<(String, String)> = crate::util::read_json(&config.join("gog_store/library.json"))
        .ok()
        .flatten()
        .map(|v| v["games"].as_array().into_iter().flatten().map(|g| (s(g, "app_name").to_string(), s(g, "title").to_string())).collect())
        .unwrap_or_default();
    if let Ok(Some(v)) = crate::util::read_json(&config.join("gog_store/installed.json")) {
        for g in v["installed"].as_array().into_iter().flatten() {
            let app = s(g, "appName");
            let title = titles.iter().find(|(a, _)| a == app).map(|(_, t)| t.clone()).unwrap_or_default();
            if app.is_empty() || title.is_empty() || g["is_dlc"] == json!(true) {
                continue;
            }
            out.push(Candidate {
                source: "heroic",
                key: format!("gog/{app}"),
                title: crate::steam::clean_name(&title),
                sys: "PC".into(),
                launch: Some(json!({"url": format!("heroic://launch/gog/{app}")})),
                media: None,
                extra: Map::new(),
            });
        }
    }
    out.sort_by_key(|c| c.title.to_lowercase());
    out
}

pub fn heroic_configs() -> Vec<PathBuf> {
    let mut c = vec![];
    if let Some(a) = env_dir("APPDATA") {
        c.push(a.join("heroic"));
    }
    if let Some(h) = home() {
        c.push(h.join(".config/heroic"));
        c.push(h.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"));
        c.push(h.join("Library/Application Support/heroic"));
    }
    c.into_iter().filter(|p| p.is_dir()).collect()
}

/// Lutris's installed games from `lutris --list-games --installed --json`; they start with `lutris:rungame/<slug>`.
pub fn lutris_from_json(text: &str) -> Vec<Candidate> {
    let v: Value = serde_json::from_str(text).unwrap_or(json!([]));
    let mut out: Vec<Candidate> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| !s(g, "slug").is_empty() && !s(g, "name").is_empty())
        .map(|g| Candidate {
            source: "lutris",
            key: s(g, "slug").into(),
            title: s(g, "name").chars().take(80).collect(),
            sys: if s(g, "platform").is_empty() { "PC".into() } else { s(g, "platform").chars().take(24).collect() },
            launch: Some(json!({"url": format!("lutris:rungame/{}", s(g, "slug"))})),
            media: None,
            extra: Map::new(),
        })
        .collect();
    out.sort_by_key(|c| c.title.to_lowercase());
    out
}

fn lutris_list() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let o = std::process::Command::new("lutris").args(["--list-games", "--installed", "--json"]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

// ---------------------------------------------------------------- all together

fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Point each candidate that starts a program at where that program is now (launchers' records can be out of date after a drive
/// letter changes), and leave out the ones whose program isn't anywhere: (still installed, names of the ones that aren't).
pub fn settle(found: Vec<Candidate>, bases: &[PathBuf]) -> (Vec<Candidate>, Vec<String>) {
    let mut keep = vec![];
    let mut gone = vec![];
    for mut c in found {
        let Some(l) = c.launch.clone().filter(|l| l.get("command").is_some()) else {
            keep.push(c);
            continue;
        };
        let p = crate::launch::plan_with(&json!({"launch": l}), bases);
        if !p.ok {
            gone.push(c.title.clone());
            continue;
        }
        if let Some(m) = p.moved {
            c.launch = Some(m);
        }
        keep.push(c);
    }
    (keep, gone)
}

/// Every library on this computer, each with what it found. Steam and game folders are read by their own modules and added by the app.
pub fn all(game_dirs: &[String]) -> Vec<Source> {
    let mut out = vec![];
    let homes = esde_homes();
    let mut esde = Source { id: "esde", label: "ES-DE", ..Default::default() };
    for h in &homes {
        esde.games.extend(esde_from(h));
        esde.art = esde.art.or_else(|| esde_media(h));
    }
    esde.found_at = homes;
    if !esde.found_at.is_empty() {
        esde.note = "Start these in ES-DE; its downloaded box art comes along.".into();
    }
    out.push(esde);
    let mut epic = Source { id: "epic", label: "Epic Games", ..Default::default() };
    if let Some(m) = epic_manifests() {
        epic.games = epic_from(&m);
        epic.found_at = vec![m];
    }
    out.push(epic);
    let reg = gog_registry();
    let (gog, gone) = settle(gog_from_reg(&reg), &crate::launch::search_bases(game_dirs));
    out.push(Source {
        id: "gog",
        label: "GOG Galaxy",
        found_at: if reg.trim().is_empty() { vec![] } else { vec![PathBuf::from(r"HKLM\SOFTWARE\WOW6432Node\GOG.com\Games")] },
        games: gog,
        note: if gone.is_empty() {
            String::new()
        } else {
            format!("GOG Galaxy still lists {} that aren't installed anywhere: {}.", gone.len(), gone.join(", "))
        },
        ..Default::default()
    });
    let mut ra = Source { id: "retroarch", label: "RetroArch", ..Default::default() };
    for (dir, exe) in retroarch_dirs() {
        ra.games.extend(retroarch_from(&dir, exe.as_deref()));
        if exe.is_none() {
            ra.note = "RetroArch's program wasn't found, so these can be drawn but not started from here.".into();
        }
        ra.found_at.push(dir);
    }
    out.push(ra);
    let mut heroic = Source { id: "heroic", label: "Heroic", ..Default::default() };
    for c in heroic_configs() {
        heroic.games.extend(heroic_from(&c));
        heroic.found_at.push(c);
    }
    out.push(heroic);
    let mut lutris = Source { id: "lutris", label: "Lutris", ..Default::default() };
    if let Some(t) = lutris_list() {
        lutris.games = lutris_from_json(&t);
        lutris.found_at = vec![PathBuf::from("lutris")];
    }
    out.push(lutris);
    out
}

/// What a candidate becomes on the shelf: a draft with everything the source could tell.
pub fn draft(c: &Candidate, placeholder_why: &str) -> Json {
    let mut g = json!({"t": c.title, "sys": c.sys, "where": "couch", "p": "1-4", "mode": "Co-op", "gap": 3, "vibe": ["action"], "len": "evening", "why": placeholder_why, "draft": true, "source": c.source});
    if let Some(l) = &c.launch {
        g["launch"] = l.clone();
    }
    if let Some(m) = &c.media {
        g["media"] = m.clone();
    }
    for (k, v) in c.extra.iter().filter(|(k, _)| !k.starts_with('_')) {
        g[k] = v.clone();
    }
    if g["one"] == json!(true) {
        g["mode"] = json!("Take turns");
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gn-detect-{n}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn esde_gamelists_become_drafts_with_details() {
        let h = temp("esde");
        fs::create_dir_all(h.join("gamelists/switch")).unwrap();
        fs::create_dir_all(h.join("gamelists/steam")).unwrap();
        fs::create_dir_all(h.join("downloaded_media/switch/covers")).unwrap();
        fs::write(
            h.join("gamelists/switch/gamelist.xml"),
            r#"<?xml version="1.0"?>
<gameList>
	<game>
		<path>./A Hat in Time [010056E00853A000][v0].nsp</path>
		<name>A Hat in Time</name>
		<desc>A tiny space-travelling girl &amp; her big hat.</desc>
		<releasedate>20191108T000000</releasedate>
		<developer>Gears for Breakfast</developer>
		<genre>Platform</genre>
		<players>1-2</players>
	</game>
	<game><path>./Hidden.nsp</path><name>Hidden</name><hidden>true</hidden></game>
	<game><path>./Solo.xci</path><name>Solo Quest</name><players>1</players><genre>Survival horror</genre></game>
</gameList>"#,
        )
        .unwrap();
        fs::write(h.join("gamelists/steam/gamelist.xml"), "<gameList><game><path>./X.url</path><name>X</name></game></gameList>").unwrap();
        let c = esde_from(&h);
        assert_eq!(
            c.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(),
            vec!["A Hat in Time", "Solo Quest"],
            "hidden entries and the Steam system are left out"
        );
        let hat = &c[0];
        assert_eq!((hat.sys.as_str(), hat.media.clone().unwrap()["file"].as_str()), ("Switch", Some("A Hat in Time [010056E00853A000][v0]")));
        assert_eq!(hat.extra["why"], "A tiny space-travelling girl & her big hat.");
        assert_eq!(hat.extra["info"]["year"], 2019);
        assert_eq!(c[1].extra["p"], "1");
        assert_eq!(c[1].extra["vibe"], json!(["spooky"]));
        let d = draft(&c[1], "placeholder");
        assert_eq!((d["mode"].as_str(), d["one"].as_bool()), (Some("Take turns"), Some(true)));
        for x in &c {
            assert!(crate::catalog::validate_game(&draft(x, "placeholder")).is_empty(), "{:?}", crate::catalog::validate_game(&draft(x, "placeholder")));
        }
        assert_eq!(esde_media(&h), Some(h.join("downloaded_media")));
        let _ = fs::remove_dir_all(h);
    }

    #[test]
    fn epic_manifests_skip_addons_and_unfinished_installs() {
        let m = temp("epic");
        let item = |name: &str, v: Value| fs::write(m.join(format!("{name}.item")), v.to_string()).unwrap();
        item(
            "a",
            json!({"DisplayName": "Fortnite", "AppName": "Fortnite", "CatalogNamespace": "fn", "CatalogItemId": "4fe7", "AppCategories": ["games", "applications"], "bIsApplication": true, "MainGameAppName": ""}),
        );
        item("b", json!({"DisplayName": "Some DLC", "AppName": "dlc1", "AppCategories": ["games"], "MainGameAppName": "Fortnite"}));
        item("c", json!({"DisplayName": "Half Done", "AppName": "hd", "AppCategories": ["games"], "bIsIncompleteInstall": true}));
        item("d", json!({"DisplayName": "Unreal Engine", "AppName": "UE", "AppCategories": ["engines"]}));
        let c = epic_from(&m);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].launch.clone().unwrap()["url"], "com.epicgames.launcher://apps/fn%3A4fe7%3AFortnite?action=launch&silent=true");
        let mut g = crate::catalog::tests::game("Fortnite");
        g["launch"] = c[0].launch.clone().unwrap();
        assert!(crate::catalog::validate_game(&g).is_empty(), "the Epic link is an allowed launcher link");
        let _ = fs::remove_dir_all(m);
    }

    #[test]
    fn gog_registry_text_skips_dlc() {
        let text = "
HKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1207660064
    gameName    REG_SZ    Outlast
    gameID    REG_SZ    1207660064
    exe    REG_SZ    E:\\GAMES\\Outlast\\OutlastLauncher.exe
    exeFile    REG_SZ    OutlastLauncher.exe
    workingDir    REG_SZ    E:\\GAMES\\Outlast
    dependsOn    REG_SZ

HKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1207664463
    gameName    REG_SZ    Outlast: Whistleblower
    gameID    REG_SZ    1207664463
    exe    REG_SZ    E:\\GAMES\\Outlast\\OutlastLauncher.exe
    dependsOn    REG_SZ    1207660064
";
        let c = gog_from_reg(text);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].title, "Outlast");
        assert_eq!(c[0].launch.clone().unwrap(), json!({"command": ["E:\\GAMES\\Outlast\\OutlastLauncher.exe"], "cwd": "E:\\GAMES\\Outlast"}));
        assert!(gog_from_reg("").is_empty());
    }

    #[test]
    fn retroarch_playlists_start_with_their_core() {
        let d = temp("ra");
        fs::create_dir_all(d.join("playlists")).unwrap();
        fs::write(
            d.join("playlists/Nintendo - Super Nintendo Entertainment System.lpl"),
            json!({"version": "1.5", "default_core_path": "", "items": [
                {"path": "C:/roms/snes/Super Mario World (USA).sfc", "label": "Super Mario World (USA)", "core_path": "C:/RA/cores/snes9x_libretro.dll"},
                {"path": "C:/roms/snes/Unknown.sfc", "label": "", "core_path": "DETECT"}]})
            .to_string(),
        )
        .unwrap();
        let exe = d.join("retroarch.exe");
        let c = retroarch_from(&d, Some(&exe));
        assert_eq!(c.len(), 2);
        let smw = c.iter().find(|x| x.title == "Super Mario World").unwrap();
        assert_eq!(smw.sys, "SNES");
        assert_eq!(
            smw.launch.clone().unwrap()["command"],
            json!([exe.to_string_lossy(), "-L", "C:/RA/cores/snes9x_libretro.dll", "C:/roms/snes/Super Mario World (USA).sfc"])
        );
        assert!(c.iter().find(|x| x.title == "Unknown").unwrap().launch.is_none(), "no core named: drawn, not started from here");
        assert!(retroarch_from(&d, None).iter().all(|x| x.launch.is_none()));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn heroic_and_lutris() {
        let h = temp("heroic");
        fs::create_dir_all(h.join("legendaryConfig/legendary")).unwrap();
        fs::create_dir_all(h.join("gog_store")).unwrap();
        fs::write(
            h.join("legendaryConfig/legendary/installed.json"),
            json!({"Sugar": {"title": "Celeste", "is_dlc": false}, "DLC1": {"title": "Extra", "is_dlc": true}}).to_string(),
        )
        .unwrap();
        fs::write(h.join("gog_store/installed.json"), json!({"installed": [{"appName": "1207", "platform": "linux"}]}).to_string()).unwrap();
        fs::write(h.join("gog_store/library.json"), json!({"games": [{"app_name": "1207", "title": "Stardew Valley"}]}).to_string()).unwrap();
        let c = heroic_from(&h);
        assert_eq!(c.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), vec!["Celeste", "Stardew Valley"]);
        assert_eq!(c[1].launch.clone().unwrap()["url"], "heroic://launch/gog/1207");
        let l = lutris_from_json(r#"[{"id": 3, "slug": "hades", "name": "Hades", "runner": "wine", "platform": "Windows"}, {"slug": "", "name": "x"}]"#);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].launch.clone().unwrap()["url"], "lutris:rungame/hades");
        assert!(lutris_from_json("not json").is_empty());
        for x in c.iter().chain(&l) {
            assert!(crate::catalog::validate_game(&draft(x, "p")).is_empty());
        }
        let _ = fs::remove_dir_all(h);
    }

    #[test]
    fn genre_words_become_moods() {
        assert_eq!(vibes_from_genres("Racing, Driving"), vec!["compete"]);
        assert_eq!(vibes_from_genres("Survival horror"), vec!["spooky"]);
        assert_eq!(vibes_from_genres("Puzzle-Platform"), vec!["brain", "action"]);
        assert!(vibes_from_genres("Compilation").is_empty());
    }
}
