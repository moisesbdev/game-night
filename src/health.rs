//! Read-only checks for More > Health: is everything in place, and what to do when it isn't.

use crate::app::App;
use crate::util::{Json, now_iso, s};
use serde_json::json;

fn check(id: &str, title: &str, level: &str, detail: impl Into<String>, advice: impl Into<String>) -> Json {
    json!({"id": id, "title": title, "level": level, "detail": detail.into(), "advice": advice.into()})
}

pub fn worst(checks: &[Json]) -> &'static str {
    let rank = |l: &str| match l {
        "fail" => 3,
        "warn" => 2,
        "info" => 1,
        _ => 0,
    };
    match checks.iter().map(|c| rank(s(c, "level"))).max().unwrap_or(0) {
        3 => "fail",
        2 => "warn",
        1 => "info",
        _ => "ok",
    }
}

pub fn run(app: &App) -> Json {
    let st = app.settings();
    let games = app.catalog.games();
    let mut out = vec![];
    for (id, w) in [("settings", &app.settings_warning), ("state", &app.store.load_error), ("sessions", &app.sessions.load_error)] {
        if !w.is_empty() {
            out.push(check(id, "Saved data", "fail", w.clone(), "Close anything that has the file open, or fix it, then restart Game Night."));
        }
    }
    let probe = app.paths.data.join(".write-test");
    let writable = std::fs::write(&probe, b"ok").is_ok() && std::fs::remove_file(&probe).is_ok();
    out.push(check(
        "data",
        "Data folder",
        if writable { "ok" } else { "fail" },
        app.paths.data.to_string_lossy(),
        if writable { "" } else { "Game Night can't write here. Choose another folder with --data-dir." },
    ));
    let probs = app.catalog.problems();
    out.push(check(
        "catalog",
        "Game catalog",
        if probs.is_empty() { "ok" } else { "fail" },
        if probs.is_empty() { format!("{} games, no problems", games.len()) } else { probs.iter().take(3).cloned().collect::<Vec<_>>().join("; ") },
        if probs.is_empty() { "" } else { "Fix the entries named above (Edit on the Shelf, or catalog.json)." },
    ));
    if games.is_empty() {
        out.push(check(
            "empty",
            "No games yet",
            "warn",
            "The shelf is empty.",
            "Shelf > Library adds your Steam games and game folders; Shelf > Add a game adds one by hand.",
        ));
    }
    let avail = app.availability();
    let broken: Vec<String> = games
        .iter()
        .filter(|g| g.get("launch").is_some())
        .filter_map(|g| avail.get(s(g, "t")).filter(|p| !p.ok).map(|p| format!("{} ({})", s(g, "t"), p.reason)))
        .collect();
    let no_launch = games.iter().filter(|g| g.get("launch").is_none()).count();
    out.push(if broken.is_empty() {
        check(
            "launch",
            "Ready to play",
            "ok",
            format!(
                "every game with a way to start it can start{}",
                if no_launch > 0 { format!("; {no_launch} have none set (fine for console games)") } else { String::new() }
            ),
            "",
        )
    } else {
        check(
            "launch",
            "Ready to play",
            "warn",
            format!(
                "{} can't be started right now and are left out of the draw: {}",
                broken.len(),
                broken.iter().take(4).cloned().collect::<Vec<_>>().join(" | ")
            ),
            "Reconnect the drive, or Edit the game to point at where it is now.",
        )
    });
    if !st.launch_enabled {
        out.push(check("launch-off", "Play buttons", "info", "Starting games is switched off in Settings.", ""));
    }
    let drafts: Vec<&str> = games.iter().filter(|g| g.get("draft") == Some(&json!(true))).map(|g| s(g, "t")).collect();
    if !drafts.is_empty() {
        out.push(check(
            "drafts",
            "Games to check",
            "info",
            format!(
                "{} added from your library still have placeholder details: {}",
                drafts.len(),
                drafts.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            ),
            "Open one on the Shelf and press Look it up, or Edit it.",
        ));
    }
    let no_art: Vec<&str> = games.iter().filter(|g| app.media_path(s(g, "t"), "cover").is_none()).map(|g| s(g, "t")).collect();
    if !no_art.is_empty() {
        out.push(check(
            "art",
            "Box art",
            "info",
            format!("{} game(s) have no box art: {}", no_art.len(), no_art.iter().take(5).cloned().collect::<Vec<_>>().join(", ")),
            "Look them up on the Shelf, or add an art folder in Settings.",
        ));
    }
    match app.steam_root() {
        Some(r) => out.push(check("steam", "Steam", "ok", format!("found at {}", r.display()), "")),
        None => out.push(check("steam", "Steam", "info", "not found on this computer", "If Steam is installed somewhere unusual, set its folder in Settings.")),
    }
    for d in &st.game_dirs {
        if !std::path::Path::new(d).is_dir() {
            out.push(check("folder", "Game folder", "warn", format!("{d} isn't there"), "Reconnect its drive, or remove it in Settings."));
        }
    }
    let backups = std::fs::read_dir(app.paths.data.join("backups"))
        .map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("state-")).count())
        .unwrap_or(0);
    out.push(check(
        "backup",
        "Backups",
        "ok",
        format!("{backups} daily copies of your marks and history are kept in the data folder"),
        "Settings > Export gives you a copy to keep elsewhere.",
    ));
    json!({"checks": out, "worst": worst(&out), "at": now_iso()})
}
