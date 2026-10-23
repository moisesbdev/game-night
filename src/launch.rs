//! Starting a game. Only catalog games, only the way their own entry says (`launch`), never a path from the browser,
//! never through a shell. Settings can switch launching off; then Play only reports what it would have done.

use crate::util::Json;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Url(String),
    Command { program: PathBuf, args: Vec<String>, cwd: Option<PathBuf> },
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub ok: bool,
    pub reason: String,
    pub preview: String,
    pub target: Option<Target>,
    /// The game's files were found somewhere else (a drive letter changed, a library moved): the entry's new `launch`.
    pub moved: Option<Value>,
}

/// The parts of a path after its drive or root: `E:\GAMES\Outlast\x.exe` -> ["GAMES", "Outlast", "x.exe"].
fn parts(p: &Path) -> Vec<String> {
    p.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(x) => Some(x.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// A file named `name` (any case) inside `dir`, at most a few folders down (a game's program is often in `Binaries/Win64`).
fn find_inside(dir: &Path, name: &str, depth: u32, budget: &mut u32) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut sub = vec![];
    for e in rd.flatten() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let p = e.path();
        if p.is_dir() {
            sub.push(p);
        } else if p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)) {
            return Some(p);
        }
    }
    if depth == 0 {
        return None;
    }
    sub.into_iter().find_map(|d| find_inside(&d, name, depth - 1, budget))
}

/// Where a missing file went when its drive letter changed or its library moved: the same folders under another drive or game folder
/// (`E:\GAMES\Outlast\x.exe` -> `H:\Games\Outlast\x.exe`), else the program found by name inside the same game folder elsewhere.
/// At least a game folder and the file must match, so a common name alone (game.exe at a drive's root) never counts.
pub fn relocate(path: &Path, bases: &[PathBuf], want_dir: bool) -> Option<PathBuf> {
    let ok = |p: &Path| if want_dir { p.is_dir() } else { p.is_file() };
    if ok(path) {
        return Some(path.to_path_buf());
    }
    let comps = parts(path);
    let min = if want_dir { 1 } else { 2 };
    for k in 0..comps.len() {
        let rest = &comps[k..];
        if rest.len() < min {
            break;
        }
        for base in bases {
            let c = rest.iter().fold(base.clone(), |p, x| p.join(x));
            if ok(&c) {
                return Some(c);
            }
        }
    }
    if want_dir {
        return None;
    }
    let name = comps.last()?;
    for k in 0..comps.len().saturating_sub(1) {
        let folders = &comps[k..comps.len() - 1];
        for base in bases {
            let d = folders.iter().fold(base.clone(), |p, x| p.join(x));
            if d.is_dir() && d != *base {
                let mut budget = 20_000;
                if let Some(f) = find_inside(&d, name, 5, &mut budget) {
                    return Some(f);
                }
            }
        }
    }
    None
}

/// Where to look for moved games: the game folders from Settings, the places games usually live on every drive (Games, Epic Games,
/// GOG Games...), whether or not they were added in Settings, and every drive or mount itself.
pub fn search_bases(game_dirs: &[String]) -> Vec<PathBuf> {
    let drives = crate::folders::drives();
    let mut b: Vec<PathBuf> = game_dirs.iter().map(PathBuf::from).filter(|p| p.is_dir()).collect();
    b.extend(drives.iter().flat_map(|d| crate::folders::LIKELY.iter().map(move |l| crate::folders::join_rel(d, l))).filter(|p| p.is_dir()));
    b.extend(drives);
    b
}

fn find_program(p: &str) -> Option<PathBuf> {
    let path = PathBuf::from(p);
    if path.is_absolute() || p.contains(['/', '\\']) {
        return path.is_file().then_some(path);
    }
    let exts: Vec<String> = if cfg!(windows) { vec!["".into(), ".exe".into(), ".bat".into(), ".cmd".into()] } else { vec!["".into()] };
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).find_map(|d| exts.iter().map(|e| d.join(format!("{p}{e}"))).find(|c| c.is_file())))
}

/// What Play would do for this game, and whether it can right now (the program exists, the folder exists). Nothing is searched.
#[cfg(test)]
pub fn plan(game: &Json) -> Plan {
    plan_with(game, &[])
}

/// Like `plan`, and when a program, shortcut or folder is missing, looks for where it went (see `relocate`) under `bases`.
pub fn plan_with(game: &Json, bases: &[PathBuf]) -> Plan {
    let no = |reason: &str| Plan { ok: false, reason: reason.into(), preview: String::new(), target: None, moved: None };
    let Some(l) = game.get("launch").filter(|l| l.is_object()) else { return no("no way to start it is set (Edit the game to add one)") };
    if let Some(id) = l.get("steam").and_then(Value::as_u64) {
        let u = format!("steam://rungameid/{id}");
        return Plan { ok: true, reason: String::new(), preview: u.clone(), target: Some(Target::Url(u)), moved: None };
    }
    if let Some(u) = l.get("url").and_then(Value::as_str) {
        if !crate::catalog::launch_issues(l).is_empty() {
            return no("its launcher link isn't one Game Night will open");
        }
        return Plan { ok: true, reason: String::new(), preview: u.into(), target: Some(Target::Url(u.into())), moved: None };
    }
    if let Some(o) = l.get("open").and_then(Value::as_str) {
        if !crate::catalog::launch_issues(l).is_empty() {
            return no("its shortcut isn't a kind Game Night will open");
        }
        let Some(found) = relocate(Path::new(o), bases, false) else {
            return no(&format!("the shortcut {o} isn't there (moved, or its drive isn't connected)"));
        };
        let f = found.to_string_lossy().to_string();
        let moved = (f != o).then(|| json!({"open": f}));
        return Plan { ok: true, reason: String::new(), preview: f.clone(), target: Some(Target::Url(f)), moved };
    }
    if let Some(argv) = l.get("command").and_then(Value::as_array) {
        let argv: Vec<String> = argv.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        let Some(first) = argv.first() else { return no("its command is empty") };
        let program = match find_program(first) {
            Some(p) => p,
            None if Path::new(first).is_absolute() => match relocate(Path::new(first), bases, false) {
                Some(p) => p,
                None => return no(&format!("the program {first} isn't there (moved, or its drive isn't connected)")),
            },
            None => return no(&format!("the program {first} isn't there (moved, or its drive isn't connected)")),
        };
        let cwd_was = l.get("cwd").and_then(Value::as_str);
        let mut cwd = cwd_was.map(PathBuf::from);
        if let Some(c) = cwd.clone()
            && !c.is_dir()
        {
            // the start-in folder moved with the game: find it the same way, else start in the program's own folder
            match relocate(&c, bases, true).or_else(|| program.parent().map(Path::to_path_buf)) {
                Some(d) => cwd = Some(d),
                None => return no(&format!("the folder {} isn't there", c.display())),
            }
        }
        let first_now = program.to_string_lossy().to_string();
        let cwd_now = cwd.as_ref().map(|c| c.to_string_lossy().to_string());
        let program_moved = Path::new(first).is_absolute() && first_now != *first;
        let cwd_moved = cwd_was.is_some() && cwd_now.as_deref() != cwd_was;
        let moved = (program_moved || cwd_moved).then(|| {
            let mut v = argv.clone();
            v[0] = first_now.clone();
            let mut m = json!({"command": v});
            if let (Some(c), Some(_)) = (&cwd_now, cwd_was) {
                m["cwd"] = json!(c);
            }
            m
        });
        let mut shown = argv.clone();
        shown[0] = first_now;
        return Plan {
            ok: true,
            reason: String::new(),
            preview: shown.join(" "),
            target: Some(Target::Command { program, args: argv[1..].to_vec(), cwd }),
            moved,
        };
    }
    no("its launch entry isn't understood")
}

/// Open a launcher link with the system's own handler (no shell, no string concatenation).
pub fn open_url(u: &str) -> std::io::Result<Child> {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", u]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(u);
        c
    } else if u.to_lowercase().ends_with(".desktop") {
        let mut c = Command::new("gio"); // xdg-open would open a .desktop file in a text editor instead of running it
        c.args(["launch", u]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(u);
        c
    };
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
}

/// Start it. Returns the child when Game Night started the program itself (so the session can end when it exits).
pub fn execute(p: &Plan) -> Result<Option<Child>, String> {
    match &p.target {
        None => Err(p.reason.clone()),
        Some(Target::Url(u)) => open_url(u).map(|_| None).map_err(|e| format!("couldn't open {u}: {e}")),
        Some(Target::Command { program, args, cwd }) => {
            let mut c = Command::new(program);
            c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            if let Some(d) = cwd.as_deref().or(program.parent().filter(|d| !d.as_os_str().is_empty())) {
                c.current_dir(d);
            }
            c.spawn().map(Some).map_err(|e| format!("couldn't start {}: {e}", program.display()))
        }
    }
}

pub fn steam_id(game: &Json) -> Option<u64> {
    game.get("launch")
        .and_then(|l| l.get("steam"))
        .and_then(Value::as_u64)
        .or_else(|| game.get("info").and_then(|i| i.get("steamAppId")).and_then(Value::as_u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_game_on_a_drive_whose_letter_changed_is_found() {
        let root = std::env::temp_dir().join(format!("gn-relocate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (old, new) = (root.join("old-drive"), root.join("new-drive"));
        std::fs::create_dir_all(new.join("Games/Outlast")).unwrap();
        std::fs::create_dir_all(new.join("Games/Little Nightmares/Atlas/Binaries/Win64")).unwrap();
        std::fs::write(new.join("Games/Outlast/OutlastLauncher.exe"), b"x").unwrap();
        std::fs::write(new.join("Games/Little Nightmares/Atlas/Binaries/Win64/LittleNightmares.exe"), b"x").unwrap();
        std::fs::write(new.join("game.exe"), b"x").unwrap();
        let bases = vec![new.clone()];
        // the same folders under another drive
        let outlast = old.join("Games/Outlast/OutlastLauncher.exe");
        assert_eq!(relocate(&outlast, &bases, false), Some(new.join("Games/Outlast/OutlastLauncher.exe")));
        // the program found by name inside the game's folder
        let ln = old.join("Games/Little Nightmares/LittleNightmares.exe");
        let found = relocate(&ln, &bases, false).unwrap();
        assert!(found.ends_with("Win64/LittleNightmares.exe"), "{found:?}");
        // a common name alone at a drive's root is never taken for a game, and a game that isn't anywhere stays missing
        assert_eq!(relocate(&old.join("game.exe"), &bases, false), None);
        assert_eq!(relocate(&old.join("Games/Silent Hill 2/SHProto.exe"), &bases, false), None);
        // the plan uses the new place and reports the entry's new launch
        let g = json!({"launch": {"command": [outlast.to_string_lossy()], "cwd": old.join("Games/Outlast").to_string_lossy()}});
        assert!(!plan(&g).ok, "without searching it can't start");
        let p = plan_with(&g, &bases);
        assert!(p.ok, "{}", p.reason);
        let m = p.moved.unwrap();
        assert_eq!(Path::new(m["command"][0].as_str().unwrap()), new.join("Games/Outlast/OutlastLauncher.exe"));
        assert_eq!(Path::new(m["cwd"].as_str().unwrap()), new.join("Games/Outlast"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plans() {
        assert!(!plan(&json!({"t": "A"})).ok);
        let p = plan(&json!({"launch": {"steam": 620}}));
        assert!(p.ok);
        assert_eq!(p.target, Some(Target::Url("steam://rungameid/620".into())));
        assert!(!plan(&json!({"launch": {"url": "https://evil.example"}})).ok, "only launcher links");
        assert!(plan(&json!({"launch": {"url": "heroic://launch/legendary/abc"}})).ok);
        let missing = plan(&json!({"launch": {"command": ["/definitely/not/here/game"]}}));
        assert!(!missing.ok && missing.reason.contains("isn't there"));
        let exe = std::env::current_exe().unwrap();
        let ok = plan(&json!({"launch": {"command": [exe.to_string_lossy(), "--flag"]}}));
        assert!(ok.ok, "{}", ok.reason);
        let gone = plan(&json!({"launch": {"command": [exe.to_string_lossy()], "cwd": "/no/such/folder"}}));
        assert!(gone.ok, "a start-in folder that's gone falls back to the program's own folder");
        assert_eq!(Path::new(gone.moved.unwrap()["cwd"].as_str().unwrap()), exe.parent().unwrap());
        assert_eq!(steam_id(&json!({"info": {"steamAppId": 7}})), Some(7));
    }
}
