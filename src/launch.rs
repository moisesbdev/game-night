//! Starting a game. Only catalog games, only the way their own entry says (`launch`), never a path from the browser,
//! never through a shell. Settings can switch launching off; then Play only reports what it would have done.

use crate::util::Json;
use serde_json::Value;
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
}

fn find_program(p: &str) -> Option<PathBuf> {
    let path = PathBuf::from(p);
    if path.is_absolute() || p.contains(['/', '\\']) {
        return path.is_file().then_some(path);
    }
    let exts: Vec<String> = if cfg!(windows) { vec!["".into(), ".exe".into(), ".bat".into(), ".cmd".into()] } else { vec!["".into()] };
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).find_map(|d| exts.iter().map(|e| d.join(format!("{p}{e}"))).find(|c| c.is_file())))
}

/// What Play would do for this game, and whether it can right now (the program exists, the folder exists).
pub fn plan(game: &Json) -> Plan {
    let no = |reason: &str| Plan { ok: false, reason: reason.into(), preview: String::new(), target: None };
    let Some(l) = game.get("launch").filter(|l| l.is_object()) else { return no("no way to start it is set (Edit the game to add one)") };
    if let Some(id) = l.get("steam").and_then(Value::as_u64) {
        let u = format!("steam://rungameid/{id}");
        return Plan { ok: true, reason: String::new(), preview: u.clone(), target: Some(Target::Url(u)) };
    }
    if let Some(u) = l.get("url").and_then(Value::as_str) {
        if !crate::catalog::launch_issues(l).is_empty() {
            return no("its launcher link isn't one Game Night will open");
        }
        return Plan { ok: true, reason: String::new(), preview: u.into(), target: Some(Target::Url(u.into())) };
    }
    if let Some(o) = l.get("open").and_then(Value::as_str) {
        if !crate::catalog::launch_issues(l).is_empty() {
            return no("its shortcut isn't a kind Game Night will open");
        }
        if !Path::new(o).is_file() {
            return no(&format!("the shortcut {o} isn't there (moved, or its drive isn't connected)"));
        }
        return Plan { ok: true, reason: String::new(), preview: o.into(), target: Some(Target::Url(o.into())) };
    }
    if let Some(argv) = l.get("command").and_then(Value::as_array) {
        let argv: Vec<String> = argv.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        let Some(first) = argv.first() else { return no("its command is empty") };
        let Some(program) = find_program(first) else { return no(&format!("the program {first} isn't there (moved, or its drive isn't connected)")) };
        let cwd = l.get("cwd").and_then(Value::as_str).map(PathBuf::from);
        if let Some(c) = &cwd
            && !c.is_dir()
        {
            return no(&format!("the folder {} isn't there", c.display()));
        }
        return Plan { ok: true, reason: String::new(), preview: argv.join(" "), target: Some(Target::Command { program, args: argv[1..].to_vec(), cwd }) };
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
        assert!(!plan(&json!({"launch": {"command": [exe.to_string_lossy()], "cwd": "/no/such/folder"}})).ok);
        assert_eq!(steam_id(&json!({"info": {"steamAppId": 7}})), Some(7));
    }
}
