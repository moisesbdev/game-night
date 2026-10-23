//! Your Steam library, read from Steam's own files on this computer (no account, no network): where Steam keeps its libraries,
//! which games are installed, and the box art Steam already cached. Works the same on Windows, Linux (native, Flatpak) and macOS.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A minimal reader for Valve's KeyValues text format (`"key" "value"` and `"key" { ... }`).
#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Map(BTreeMap<String, Vdf>),
}

impl Vdf {
    pub fn get(&self, k: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(m) => m.iter().find(|(key, _)| key.eq_ignore_ascii_case(k)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }
    pub fn str(&self, k: &str) -> Option<&str> {
        match self.get(k)? {
            Vdf::Str(s) => Some(s),
            Vdf::Map(_) => None,
        }
    }
    pub fn entries(&self) -> Vec<(&String, &Vdf)> {
        match self {
            Vdf::Map(m) => m.iter().collect(),
            Vdf::Str(_) => vec![],
        }
    }
}

pub fn parse_vdf(text: &str) -> Vdf {
    let mut tokens = vec![];
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = it.next() {
                    match c {
                        '\\' => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(o) => s.push(o),
                            None => {}
                        },
                        '"' => break,
                        o => s.push(o),
                    }
                }
                tokens.push(Some(s));
            }
            '{' => tokens.push(None),
            '}' => tokens.push(Some("\u{0}}".into())),
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    fn block(t: &[Option<String>], i: &mut usize) -> BTreeMap<String, Vdf> {
        let mut m = BTreeMap::new();
        while *i < t.len() {
            match &t[*i] {
                Some(k) if k == "\u{0}}" => {
                    *i += 1;
                    break;
                }
                Some(k) => {
                    let key = k.clone();
                    *i += 1;
                    match t.get(*i) {
                        Some(None) => {
                            *i += 1;
                            m.insert(key, Vdf::Map(block(t, i)));
                        }
                        Some(Some(v)) if v != "\u{0}}" => {
                            m.insert(key, Vdf::Str(v.clone()));
                            *i += 1;
                        }
                        _ => {}
                    }
                }
                None => *i += 1,
            }
        }
        m
    }
    let mut i = 0;
    Vdf::Map(block(&tokens, &mut i))
}

/// Where Steam usually lives, plus the folder set in Settings first.
pub fn candidates(custom: &str) -> Vec<PathBuf> {
    let mut v = vec![];
    if !custom.trim().is_empty() {
        v.push(PathBuf::from(custom.trim()));
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        v.extend(registry_paths());
        for k in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(p) = env(k) {
                v.push(p.join("Steam"));
            }
        }
        v.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    } else if let Some(h) = env("HOME") {
        if cfg!(target_os = "macos") {
            v.push(h.join("Library/Application Support/Steam"));
        } else {
            v.push(h.join(".local/share/Steam"));
            v.push(h.join(".steam/steam"));
            v.push(h.join(".steam/root"));
            v.push(h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
            v.push(h.join("snap/steam/common/.local/share/Steam"));
        }
    }
    v
}

/// Where Steam says it is installed (Windows keeps it in the registry; read with the built-in `reg` tool, no extra libraries).
fn registry_paths() -> Vec<PathBuf> {
    let mut out = vec![];
    for (key, value) in
        [(r"HKCU\Software\Valve\Steam", "SteamPath"), (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"), (r"HKLM\SOFTWARE\Valve\Steam", "InstallPath")]
    {
        let mut c = std::process::Command::new("reg");
        c.args(["query", key, "/v", value]);
        crate::sessions::no_window(&mut c);
        if let Ok(o) = c.output() {
            let text = String::from_utf8_lossy(&o.stdout);
            if let Some(p) = text.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, p)| p.trim().to_string())).filter(|p| !p.is_empty()) {
                out.push(PathBuf::from(p.replace('/', "\\")));
            }
        }
    }
    out
}

/// Folder names Steam libraries usually get on a drive's top level (and one level down, under Games).
const LIBRARY_NAMES: [&str; 9] = [
    "SteamLibrary",
    "Steam",
    "Steam Library",
    "steam",
    "Program Files (x86)/Steam",
    "Program Files/Steam",
    "Games/Steam",
    "Games/SteamLibrary",
    "Games/Steam Library",
];

/// Every Steam install and library on this computer: the installs Steam knows about (Settings, the registry, the usual places),
/// the libraries their `libraryfolders.vdf` lists, and library folders found on any connected drive that Steam's list doesn't mention
/// (a drive moved from another PC, or a second install). Nothing is written; only folder names are checked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Discovery {
    /// Installs (have `steamapps` and are where Steam itself lives): used for Steam's cached art.
    pub roots: Vec<PathBuf>,
    /// Every library folder (has `steamapps`), with how it was found: "steam" (listed by Steam) or "drive" (found by looking).
    pub libraries: Vec<(PathBuf, &'static str)>,
}

pub fn discover(custom: &str) -> Discovery {
    let mut d = Discovery::default();
    for c in candidates(custom) {
        if c.join("steamapps").is_dir() && !d.roots.iter().any(|r| same(r, &c)) {
            d.roots.push(c);
        }
    }
    for drive in crate::folders::drives() {
        for p in libraries_on(&drive) {
            {
                let is_install = p.join("steam.exe").exists() || p.join("steam.sh").exists();
                if is_install && p.join("steamapps").join("libraryfolders.vdf").is_file() && !d.roots.iter().any(|r| same(r, &p)) {
                    d.roots.push(p.clone()); // a second Steam install on another drive
                }
                if !d.libraries.iter().any(|(l, _)| same(l, &p)) {
                    d.libraries.push((p, "drive"));
                }
            }
        }
    }
    for r in d.roots.clone() {
        for l in libraries(&r) {
            match d.libraries.iter_mut().find(|(x, _)| same(x, &l)) {
                Some(e) => e.1 = "steam",
                None => d.libraries.push((l, "steam")),
            }
        }
    }
    d.libraries.retain(|(l, _)| has_manifests(l));
    d
}

/// Steam library folders at the usual places on one drive (or mount).
pub fn libraries_on(drive: &Path) -> Vec<PathBuf> {
    LIBRARY_NAMES.iter().map(|n| crate::folders::join_rel(drive, n)).filter(|p| p.join("steamapps").is_dir()).collect()
}

fn has_manifests(lib: &Path) -> bool {
    fs::read_dir(lib.join("steamapps")).is_ok_and(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("appmanifest_")))
}

/// Every Steam library folder (from libraryfolders.vdf), the main one first.
pub fn libraries(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.to_path_buf()];
    if let Ok(t) = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
        let v = parse_vdf(&t);
        if let Some(lf) = v.get("libraryfolders") {
            for (_, e) in lf.entries() {
                let p = e.str("path").map(PathBuf::from).or_else(|| match e {
                    Vdf::Str(s) => Some(PathBuf::from(s)),
                    _ => None,
                });
                if let Some(p) = p
                    && p.join("steamapps").is_dir()
                    && !out.iter().any(|o| same(o, &p))
                {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub appid: u64,
    pub name: String,
    pub size_on_disk: u64,
}

/// Steam runtimes, Proton, redistributables and tools: never games.
fn is_tool(appid: u64, name: &str) -> bool {
    // runtimes and redistributables, SteamVR, and apps that aren't games (Wallpaper Engine, Lossless Scaling, Soundpad, Blender)
    const IDS: [u64; 12] = [228980, 1070560, 1391110, 1628350, 1493710, 250820, 1826330, 2180100, 431960, 993090, 629520, 365670];
    const WORDS: [&str; 8] = ["proton", "steam linux runtime", "steamworks common", "steamvr", "redistributable", "dedicated server", "soundtrack", "sdk"];
    let low = name.to_lowercase();
    IDS.contains(&appid) || WORDS.iter().any(|w| low.contains(w))
}

/// "Battlefield(TM) 6" -> "Battlefield 6": trademark signs dropped, spaces tidied.
pub fn clean_name(n: &str) -> String {
    n.replace(['\u{2122}', '\u{00AE}', '\u{00A9}'], "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Installed games across every library found, by name.
pub fn installed(d: &Discovery) -> Vec<App> {
    let mut out: Vec<App> = vec![];
    for (lib, _) in &d.libraries {
        let Ok(rd) = fs::read_dir(lib.join("steamapps")) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
                continue;
            }
            let Ok(t) = fs::read_to_string(e.path()) else { continue };
            let v = parse_vdf(&t);
            let Some(st) = v.get("AppState") else { continue };
            let (Some(id), Some(name)) = (st.str("appid").and_then(|x| x.parse().ok()), st.str("name")) else { continue };
            if is_tool(id, name) || out.iter().any(|a| a.appid == id) {
                continue;
            }
            out.push(App { appid: id, name: clean_name(name), size_on_disk: st.str("SizeOnDisk").and_then(|x| x.parse().ok()).unwrap_or(0) });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Art Steam already downloaded for an app: `cover` (portrait capsule), `shot` (hero image) or `header`. Old and new cache layouts.
pub fn cached_art(root: &Path, appid: u64, kind: &str) -> Option<PathBuf> {
    let names: &[&str] = match kind {
        "cover" => &["library_600x900_2x.jpg", "library_600x900.jpg"],
        "shot" => &["library_hero.jpg", "header.jpg"],
        _ => &["header.jpg"],
    };
    let cache = root.join("appcache").join("librarycache");
    for n in names {
        for p in [cache.join(appid.to_string()).join(n), cache.join(format!("{appid}_{n}"))] {
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // newer clients keep the files in hashed sub-folders of <appid>/
    let dir = cache.join(appid.to_string());
    fs::read_dir(&dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|sub| names.iter().map(|n| sub.path().join(n)).find(|p| p.is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"ROOT"
		"apps" { "620" "123" }
	}
	"1"
	{
		"path"		"SECOND"
	}
}"#;

    #[test]
    fn reads_libraries_manifests_and_art() {
        let d = std::env::temp_dir().join(format!("gn-steam-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let (root, second) = (d.join("Steam"), d.join("Games"));
        for p in [&root, &second] {
            fs::create_dir_all(p.join("steamapps")).unwrap();
        }
        let esc = |p: &Path| p.to_string_lossy().replace('\\', "\\\\");
        fs::write(root.join("steamapps/libraryfolders.vdf"), LIB.replace("ROOT", &esc(&root)).replace("SECOND", &esc(&second))).unwrap();
        fs::write(
            root.join("steamapps/appmanifest_620.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n\t\"SizeOnDisk\"\t\"100\"\n}\n",
        )
        .unwrap();
        fs::write(second.join("steamapps/appmanifest_228980.acf"), "\"AppState\" { \"appid\" \"228980\" \"name\" \"Steamworks Common Redistributables\" }")
            .unwrap();
        fs::write(second.join("steamapps/appmanifest_1426210.acf"), "\"AppState\" { \"appid\" \"1426210\" \"name\" \"It Takes Two\" // comment\n }").unwrap();
        assert_eq!(libraries(&root).len(), 2);
        let disc = Discovery { roots: vec![root.clone()], libraries: libraries(&root).into_iter().map(|l| (l, "steam")).collect() };
        let apps = installed(&disc);
        assert_eq!(apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["It Takes Two", "Portal 2"]);
        fs::create_dir_all(root.join("appcache/librarycache/620")).unwrap();
        fs::write(root.join("appcache/librarycache/620/library_600x900.jpg"), b"x").unwrap();
        fs::write(root.join("appcache/librarycache/1426210_header.jpg"), b"x").unwrap();
        assert!(cached_art(&root, 620, "cover").is_some());
        assert!(cached_art(&root, 1426210, "shot").is_some(), "the old flat layout");
        assert!(cached_art(&root, 999, "cover").is_none());
        assert!(discover(&root.to_string_lossy()).roots.iter().any(|r| same(r, &root)), "the folder set in Settings is found");
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn a_library_on_another_drive_is_found_by_its_usual_name() {
        let drive = std::env::temp_dir().join(format!("gn-drive-{}", std::process::id()));
        let _ = fs::remove_dir_all(&drive);
        fs::create_dir_all(drive.join("SteamLibrary/steamapps")).unwrap();
        fs::create_dir_all(drive.join("Games/Steam Library/steamapps")).unwrap();
        fs::create_dir_all(drive.join("Not Steam/steamapps")).unwrap();
        let found = libraries_on(&drive);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(!has_manifests(&drive.join("SteamLibrary")), "an empty library holds no games");
        fs::write(drive.join("SteamLibrary/steamapps/appmanifest_1.acf"), "x").unwrap();
        assert!(has_manifests(&drive.join("SteamLibrary")));
        let _ = fs::remove_dir_all(drive);
    }

    #[test]
    fn names_lose_trademark_signs() {
        assert_eq!(clean_name("Call of Duty\u{00AE}: Black Ops 6"), "Call of Duty: Black Ops 6");
        assert_eq!(clean_name("Battlefield\u{2122}  6 "), "Battlefield 6");
    }

    #[test]
    fn vdf_escapes_and_nesting() {
        let v = parse_vdf(r#""a" { "b" "x\"y" "c" { "d" "1" } }"#);
        assert_eq!(v.get("a").unwrap().str("b"), Some("x\"y"));
        assert_eq!(v.get("a").unwrap().get("c").unwrap().str("d"), Some("1"));
    }
}
