//! Game folders: any folders you choose in Settings, scanned for things that start a game. Nothing is assumed about where
//! games live. Found: shortcuts (`.lnk`, `.url`) and programs (`.exe`) on Windows; `.desktop` files, AppImages and executable
//! files on Linux and macOS. Each becomes an import suggestion; nothing is added until you pick it.

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_FOUND: usize = 2000;
const SKIP_WORDS: [&str; 12] =
    ["unins", "setup", "install", "redist", "vcredist", "directx", "dotnet", "crashreport", "crashhandler", "launcherhelper", "update", "uninstall"];

/// The extensions a game folder may hold that Game Night will open with the system's own handler (the `open` launch kind).
pub const OPEN_EXT: [&str; 3] = ["lnk", "url", "desktop"];

#[derive(Debug, Clone, PartialEq)]
pub struct FoundGame {
    pub title: String,
    pub path: PathBuf,
    pub launch: Value,
}

fn title_from(stem: &str) -> String {
    let t = stem.replace(['_', '.'], " ");
    let t = t.trim_end_matches(|c: char| c.is_whitespace() || c == '-').trim();
    t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect()
}

fn skip(name: &str) -> bool {
    let low = name.to_lowercase();
    SKIP_WORDS.iter().any(|w| low.contains(w))
}

#[cfg(unix)]
fn executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
#[cfg(not(unix))]
fn executable(_: &Path) -> bool {
    false
}

/// A `.desktop` file's Name= (or the file name), or None when it is hidden or not an application.
fn desktop_name(p: &Path) -> Option<String> {
    let t = fs::read_to_string(p).ok()?;
    if t.lines().any(|l| l.trim() == "NoDisplay=true" || l.trim() == "Hidden=true")
        || t.lines().any(|l| l.starts_with("Type=") && l.trim() != "Type=Application")
    {
        return None;
    }
    t.lines().find_map(|l| l.strip_prefix("Name=")).map(|n| n.trim().to_string()).filter(|n| !n.is_empty())
}

fn consider(p: &Path, out: &mut Vec<FoundGame>) {
    let (Some(stem), Some(ext)) = (p.file_stem().and_then(|s| s.to_str()), p.extension().and_then(|e| e.to_str()).map(str::to_lowercase)) else {
        if !cfg!(windows)
            && executable(p)
            && let Some(n) = p.file_name().and_then(|s| s.to_str())
            && !skip(n)
        {
            out.push(FoundGame { title: title_from(n), path: p.into(), launch: json!({"command": [p.to_string_lossy()]}) });
        }
        return;
    };
    if skip(stem) {
        return;
    }
    let path = p.to_string_lossy().to_string();
    match ext.as_str() {
        "lnk" | "url" if cfg!(windows) || ext == "url" => out.push(FoundGame { title: title_from(stem), path: p.into(), launch: json!({"open": path}) }),
        "exe" if cfg!(windows) => out.push(FoundGame { title: title_from(stem), path: p.into(), launch: json!({"command": [path]}) }),
        "desktop" => {
            if let Some(n) = desktop_name(p) {
                out.push(FoundGame { title: title_from(&n), path: p.into(), launch: json!({"open": path}) });
            }
        }
        "appimage" => out.push(FoundGame { title: title_from(stem.split('-').next().unwrap_or(stem)), path: p.into(), launch: json!({"command": [path]}) }),
        _ => {}
    }
}

/// Scan each folder: the files directly in it, and one level of sub-folders (a game's own folder with its program inside).
pub fn scan(dirs: &[String]) -> Vec<FoundGame> {
    let mut out = vec![];
    for d in dirs {
        let root = PathBuf::from(d.trim());
        let Ok(rd) = fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                // one program per game folder: the one named most like the folder
                let Ok(inner) = fs::read_dir(&p) else { continue };
                let mut found = vec![];
                for f in inner.flatten().map(|x| x.path()).filter(|x| x.is_file()) {
                    consider(&f, &mut found);
                }
                let folder = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase().replace([' ', '-', '_'], "");
                found.sort_by_key(|g| {
                    let n = g.title.to_lowercase().replace(' ', "");
                    (!(folder.contains(&n) || n.contains(&folder)), n.len())
                });
                if let Some(mut g) = found.into_iter().next() {
                    g.title = title_from(p.file_name().and_then(|n| n.to_str()).unwrap_or(&g.title));
                    out.push(g);
                }
            } else {
                consider(&p, &mut out);
            }
            if out.len() >= MAX_FOUND {
                return out;
            }
        }
    }
    out.sort_by_key(|g| g.title.to_lowercase());
    out.dedup_by(|a, b| a.title.eq_ignore_ascii_case(&b.title));
    out
}

/// Every connected drive or mount to look at: drive letters C: to Z: on Windows; the home folder and anything under /mnt, /media,
/// /run/media on Linux; /Volumes on macOS. Only existence is checked, nothing is read.
pub fn drives() -> Vec<PathBuf> {
    let mut out = vec![];
    if cfg!(windows) {
        for l in b'C'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", l as char));
            if p.is_dir() {
                out.push(p);
            }
        }
        return out;
    }
    let kids = |p: &Path| -> Vec<PathBuf> { fs::read_dir(p).map(|rd| rd.flatten().map(|e| e.path()).filter(|x| x.is_dir()).collect()).unwrap_or_default() };
    if let Some(h) = std::env::var_os("HOME") {
        out.push(PathBuf::from(h));
    }
    if cfg!(target_os = "macos") {
        out.extend(kids(Path::new("/Volumes")));
    } else {
        out.extend(kids(Path::new("/mnt")));
        for base in ["/media", "/run/media"] {
            for user_or_disk in kids(Path::new(base)) {
                out.push(user_or_disk.clone());
                out.extend(kids(&user_or_disk));
            }
        }
    }
    out.dedup();
    out
}

/// `base` + "a/b/c" with the system's own separators (so a Windows path never shows as `C:\Games/Steam`).
pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(base.to_path_buf(), |p, part| p.join(part))
}

/// Folders where launchers and people usually keep games, relative to a drive (Windows) or the home folder (Linux, macOS).
pub const LIKELY: [&str; 14] = [
    "Games",
    "Epic Games",
    "Program Files/Epic Games",
    "GOG Games",
    "Program Files (x86)/GOG Galaxy/Games",
    "Program Files/GOG Galaxy/Games",
    "XboxGames",
    "Program Files/EA Games",
    "EA Games",
    "Program Files (x86)/Ubisoft/Ubisoft Game Launcher/games",
    "Riot Games",
    "Games/Heroic",
    "Applications",
    "itch",
];

/// Folders on this computer that look like they hold games, each with how many it found, for one-tap adding in Settings.
/// Skips folders already chosen and Steam's own folders (the Steam import covers those).
pub fn suggest(already: &[String]) -> Vec<(PathBuf, usize)> {
    let mut places: Vec<PathBuf> = drives().iter().flat_map(|d| LIKELY.iter().map(move |l| join_rel(d, l))).collect();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from) {
        for d in ["Desktop", "Desktop/Games", "OneDrive/Desktop", "OneDrive/Desktop/Games", "Games", "Applications"] {
            places.push(join_rel(&home, d));
        }
    }
    let chosen: Vec<PathBuf> = already.iter().map(|a| PathBuf::from(a.trim())).collect();
    let same = |a: &Path, b: &Path| a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(x, y)| x == y);
    let mut out: Vec<(PathBuf, usize)> = vec![];
    for p in places {
        if !p.is_dir() || chosen.iter().any(|c| same(c, &p)) || out.iter().any(|(o, _)| same(o, &p)) || p.join("steamapps").exists() {
            continue;
        }
        let n = scan(&[p.to_string_lossy().into()]).len();
        if n > 0 {
            out.push((p, n));
        }
    }
    out
}

/// Ask the person to choose a folder with the system's own picker. None when cancelled or when this system has no picker
/// (Linux without zenity or kdialog, or a build without a window).
pub fn pick_folder(title: &str, start: Option<&str>) -> Option<String> {
    #[cfg(all(feature = "window", any(windows, target_os = "macos")))]
    {
        let mut d = rfd::FileDialog::new().set_title(title);
        if let Some(s) = start.filter(|s| Path::new(s).is_dir()) {
            d = d.set_directory(s);
        }
        d.pick_folder().map(|p| p.to_string_lossy().into_owned())
    }
    #[cfg(not(all(feature = "window", any(windows, target_os = "macos"))))]
    {
        if cfg!(windows) || cfg!(target_os = "macos") {
            return None;
        }
        let start = start.filter(|s| Path::new(s).is_dir()).map(|s| format!("{}/", s.trim_end_matches('/')));
        let tries: [(&str, Vec<String>); 2] = [
            (
                "zenity",
                [vec!["--file-selection".into(), "--directory".into(), format!("--title={title}")], start.iter().map(|s| format!("--filename={s}")).collect()]
                    .concat(),
            ),
            ("kdialog", [vec!["--getexistingdirectory".into(), start.clone().unwrap_or_else(|| ".".into())], vec!["--title".into(), title.into()]].concat()),
        ];
        for (prog, args) in tries {
            if let Ok(o) = std::process::Command::new(prog).args(&args).output() {
                let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                return (o.status.success() && !p.is_empty()).then_some(p);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_are_real_folders() {
        let d = drives();
        assert!(!d.is_empty());
        assert!(d.iter().all(|p| p.is_dir()));
    }

    #[test]
    fn finds_shortcuts_and_game_folders_but_not_installers() {
        let d = std::env::temp_dir().join(format!("gn-folders-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("Hollow Knight")).unwrap();
        fs::write(d.join("Hollow Knight/hollow_knight.exe"), b"x").unwrap();
        fs::write(d.join("Hollow Knight/UnityCrashHandler64.exe"), b"x").unwrap();
        fs::write(d.join("Celeste.url"), "[InternetShortcut]\nURL=steam://rungameid/504230\n").unwrap();
        fs::write(d.join("unins000.exe"), b"x").unwrap();
        fs::write(d.join("Stardew.desktop"), "[Desktop Entry]\nType=Application\nName=Stardew Valley\nExec=stardew\n").unwrap();
        fs::write(d.join("Hidden.desktop"), "[Desktop Entry]\nType=Application\nName=Tool\nNoDisplay=true\n").unwrap();
        let found = scan(&[d.to_string_lossy().into(), "/no/such/folder".into()]);
        let names: Vec<&str> = found.iter().map(|g| g.title.as_str()).collect();
        assert!(names.contains(&"Celeste"));
        assert!(names.contains(&"Stardew Valley"));
        assert!(!names.iter().any(|n| n.to_lowercase().contains("unins") || *n == "Tool"));
        if cfg!(windows) {
            let hk = found.iter().find(|g| g.title == "Hollow Knight").expect("the game folder");
            assert!(hk.launch["command"][0].as_str().unwrap().ends_with("hollow_knight.exe"));
        }
        for g in &found {
            let mut game = crate::catalog::tests::game(&g.title);
            game["launch"] = g.launch.clone();
            assert!(crate::catalog::validate_game(&game).is_empty(), "{:?}", crate::catalog::validate_game(&game));
        }
        let _ = fs::remove_dir_all(d);
    }
}
