//! Play sessions: when a game started and ended, how long it ran, and how it went.
//!
//! A session starts when Game Night launches a game. It ends by itself when the launched program exits (or, for launcher links,
//! when the game's named process disappears), or when someone presses "We're done". Under two minutes counts as a launch that
//! probably failed, and the page asks about that instead of asking for ratings.

use crate::util::{Json, minutes_between, now_iso, read_json, s, write_json};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const MAX_SESSIONS: usize = 2000;
pub const FAILED_UNDER_MINUTES: i64 = 2;
pub const RATINGS: [&str; 3] = ["up", "down", "skip"];
pub const OUTCOMES: [&str; 3] = ["again", "done", "finished"];
const ASK_UNTIL_SKIPS: usize = 3;

pub struct SessionLog {
    path: PathBuf,
    data: RwLock<Json>,
    pub load_error: String,
}

fn new_id() -> String {
    let mut b = [0u8; 6];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl SessionLog {
    pub fn open(path: &Path) -> Self {
        let (data, load_error) = match read_json(path) {
            Ok(Some(v)) => {
                let items: Vec<Value> = v
                    .get("sessions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|x| {
                        x.get("id").is_some_and(Value::is_string)
                            && x.get("title").is_some_and(Value::is_string)
                            && x.get("start").is_some_and(Value::is_string)
                    })
                    .cloned()
                    .collect();
                let n = items.len();
                (json!({"sessions": items[n.saturating_sub(MAX_SESSIONS)..], "resetAt": v.get("resetAt").cloned().unwrap_or(json!({}))}), String::new())
            }
            Ok(None) => (json!({"sessions": [], "resetAt": {}}), String::new()),
            Err(e) => (json!({"sessions": [], "resetAt": {}}), format!("The sessions file couldn't be read ({e}); it won't be overwritten.")),
        };
        SessionLog { path: path.into(), data: RwLock::new(data), load_error }
    }

    fn commit<R>(&self, f: impl FnOnce(&mut Json) -> Option<R>) -> Result<Option<R>, String> {
        if !self.load_error.is_empty() {
            return Err(self.load_error.clone());
        }
        let mut w = self.data.write().unwrap();
        let mut new = w.clone();
        let out = f(&mut new);
        let items = new["sessions"].as_array_mut().unwrap();
        let n = items.len();
        items.drain(..n.saturating_sub(MAX_SESSIONS));
        write_json(&self.path, &json!({"v": 1, "sessions": new["sessions"], "resetAt": new["resetAt"]})).map_err(|e| e.to_string())?;
        *w = new;
        Ok(out)
    }

    fn with<R>(&self, sid: &str, f: impl FnOnce(&mut Json) -> R) -> Result<Option<R>, String> {
        self.commit(|d| d["sessions"].as_array_mut().unwrap().iter_mut().find(|x| s(x, "id") == sid).map(f))
    }

    pub fn items(&self) -> Vec<Json> {
        self.data.read().unwrap()["sessions"].as_array().cloned().unwrap_or_default()
    }

    pub fn start(&self, title: &str) -> Result<Json, String> {
        let sess = json!({"id": new_id(), "title": title, "start": now_iso(), "end": null, "minutes": null, "detected": null, "failed": false, "check": false, "prompted": false, "ratings": {}, "outcome": null});
        let c = sess.clone();
        self.commit(move |d| {
            d["sessions"].as_array_mut().unwrap().push(c);
            Some(())
        })?;
        Ok(sess)
    }

    pub fn get(&self, sid: &str) -> Option<Json> {
        self.items().into_iter().find(|x| s(x, "id") == sid)
    }

    pub fn end(&self, sid: &str, detected: &str) -> Result<Option<Json>, String> {
        self.with(sid, |x| {
            if x["end"].is_null() {
                let end = now_iso();
                let m = minutes_between(s(x, "start"), &end);
                x["end"] = json!(end);
                x["minutes"] = json!(m);
                x["detected"] = json!(if detected == "auto" { "auto" } else { "manual" });
                x["check"] = json!(m < FAILED_UNDER_MINUTES);
            }
            x.clone()
        })
    }

    pub fn mark_failed(&self, sid: &str) -> Result<Option<Json>, String> {
        self.with(sid, |x| {
            x["failed"] = json!(true);
            x["check"] = json!(false);
            x["prompted"] = json!(true);
            x.clone()
        })
    }

    pub fn confirm_started(&self, sid: &str) -> Result<Option<Json>, String> {
        self.with(sid, |x| {
            x["check"] = json!(false);
            x.clone()
        })
    }

    pub fn rate(&self, sid: &str, pid: &str, rating: &str) -> Result<Option<Json>, String> {
        if !RATINGS.contains(&rating) {
            return Err("rating must be up, down or skip".into());
        }
        self.with(sid, |x| {
            x["ratings"][pid] = json!(rating);
            x.clone()
        })
    }

    pub fn set_outcome(&self, sid: &str, outcome: &str) -> Result<Option<Json>, String> {
        if !OUTCOMES.contains(&outcome) {
            return Err("outcome must be again, done or finished".into());
        }
        self.with(sid, |x| {
            x["outcome"] = json!(outcome);
            x["prompted"] = json!(true);
            x.clone()
        })
    }

    pub fn dismiss(&self, sid: &str) -> Result<(), String> {
        self.with(sid, |x| x["prompted"] = json!(true)).map(|_| ())
    }

    /// Someone rated a game by hand: asking them after games starts again.
    pub fn rated_by_hand(&self, pid: &str) {
        let _ = self.commit(|d| {
            d["resetAt"][pid] = json!(now_iso());
            Some(())
        });
    }

    pub fn active(&self) -> Option<Json> {
        self.items().into_iter().rev().find(|x| x["end"].is_null())
    }

    pub fn pending(&self) -> Vec<Json> {
        self.items()
            .into_iter()
            .filter(|x| !x["end"].is_null() && !x["prompted"].as_bool().unwrap_or(false) && !x["failed"].as_bool().unwrap_or(false))
            .collect()
    }

    /// Players to ask how a game went (not those who skipped the last few prompts in a row).
    pub fn ask_whom(&self, players: &[String]) -> Vec<String> {
        let d = self.data.read().unwrap();
        let items = d["sessions"].as_array().unwrap();
        players
            .iter()
            .filter(|p| {
                let reset = s(&d["resetAt"], p);
                let streak = items
                    .iter()
                    .rev()
                    .filter(|x| !x["end"].is_null() && s(x, "start") > reset && x["ratings"].get(p.as_str()).is_some())
                    .take_while(|x| s(&x["ratings"], p) == "skip")
                    .count();
                streak < ASK_UNTIL_SKIPS
            })
            .cloned()
            .collect()
    }

    /// Sessions that count: ended, not a failed launch, at least two minutes.
    pub fn good(&self) -> Vec<Json> {
        self.items()
            .into_iter()
            .filter(|x| !x["end"].is_null() && !x["failed"].as_bool().unwrap_or(false) && x["minutes"].as_i64().unwrap_or(0) >= FAILED_UNDER_MINUTES)
            .collect()
    }

    /// How long this game usually runs for the group, from at least three sessions: {n, low, high} minutes.
    pub fn summary(&self, title: &str) -> Option<Json> {
        let mut m: Vec<i64> = self.good().iter().filter(|x| s(x, "title") == title).filter_map(|x| x["minutes"].as_i64()).collect();
        m.sort();
        let n = m.len();
        if n < 3 {
            return None;
        }
        let (lo, hi) = if n >= 4 { (m[((n - 1) as f64 * 0.25).round() as usize], m[((n - 1) as f64 * 0.75).round() as usize]) } else { (m[0], m[n - 1]) };
        Some(json!({"n": n, "low": lo, "high": hi}))
    }

    pub fn last_dates(&self) -> std::collections::HashMap<String, (i64, String)> {
        let mut out = std::collections::HashMap::new();
        for x in self.good() {
            let e = out.entry(s(&x, "title").to_string()).or_insert((0, String::new()));
            e.0 += 1;
            let d = s(&x, "start").get(..10).unwrap_or("").to_string();
            if d > e.1 {
                e.1 = d;
            }
        }
        out
    }

    pub fn minutes_since(&self, date: &str) -> i64 {
        self.good().iter().filter(|x| s(x, "start") >= date).filter_map(|x| x["minutes"].as_i64()).sum()
    }
}

/// Is a program with this name running? (`tasklist` on Windows, `pgrep -x` elsewhere; no extra packages.)
pub fn process_alive(name: &str) -> bool {
    let out = if cfg!(windows) {
        let mut c = std::process::Command::new("tasklist");
        c.args(["/FI", &format!("IMAGENAME eq {name}"), "/FO", "CSV", "/NH"]);
        no_window(&mut c);
        c.output()
    } else {
        std::process::Command::new("pgrep").args(["-x", name]).output()
    };
    match out {
        Ok(o) if cfg!(windows) => String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&name.to_lowercase()),
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

#[cfg(windows)]
pub fn no_window(c: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashes up for a helper
}
#[cfg(not(windows))]
pub fn no_window(_: &mut std::process::Command) {}

/// End a session when the launched program exits.
pub fn watch_child(log: Arc<SessionLog>, sid: String, mut child: std::process::Child) {
    std::thread::spawn(move || {
        if child.wait().is_ok() {
            let _ = log.end(&sid, "auto");
        }
    });
}

/// End a session when a named process has appeared and then gone (for games started through a launcher link).
pub fn watch_name(log: Arc<SessionLog>, sid: String, name: String) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut seen = false;
        while Instant::now() < deadline && !seen {
            seen = process_alive(&name);
            if !seen {
                std::thread::sleep(Duration::from_secs(10));
            }
        }
        while seen && process_alive(&name) {
            if log.get(&sid).is_none_or(|x| !x["end"].is_null()) {
                return; // ended by hand meanwhile
            }
            std::thread::sleep(Duration::from_secs(10));
        }
        if seen {
            let _ = log.end(&sid, "auto");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gn-sess-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("sessions.json")
    }

    #[test]
    fn lifecycle() {
        let log = SessionLog::open(&temp("life"));
        let sid = s(&log.start("A").unwrap(), "id").to_string();
        assert_eq!(s(&log.active().unwrap(), "id"), sid);
        let e = log.end(&sid, "manual").unwrap().unwrap();
        assert!(e["check"].as_bool().unwrap(), "under two minutes asks whether it started");
        log.confirm_started(&sid).unwrap();
        log.rate(&sid, "p2", "up").unwrap();
        assert!(log.rate(&sid, "p2", "meh").is_err());
        assert_eq!(log.pending().len(), 1);
        log.set_outcome(&sid, "again").unwrap();
        assert!(log.pending().is_empty());
        assert!(log.active().is_none());
        assert_eq!(SessionLog::open(&log.path).items().len(), 1);
    }

    #[test]
    fn stop_asking_after_three_skips_in_a_row() {
        let log = SessionLog::open(&temp("skips"));
        let players = vec!["p1".to_string(), "p2".into()];
        for _ in 0..3 {
            let sid = s(&log.start("A").unwrap(), "id").to_string();
            log.end(&sid, "manual").unwrap();
            log.rate(&sid, "p1", "skip").unwrap();
            log.rate(&sid, "p2", "up").unwrap();
        }
        assert_eq!(log.ask_whom(&players), vec!["p2"]);
        std::thread::sleep(Duration::from_millis(1100));
        log.rated_by_hand("p1");
        assert_eq!(log.ask_whom(&players), players);
    }
}
