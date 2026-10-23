//! What the group has said and done: marks per player, confirmed picks (history), whose pick it is, today's vetoes, and progress notes.
//! One JSON file, written atomically, with a daily rolling backup. Every change is made on a copy and only becomes live once written.
//! If the file exists but can't be READ, the store refuses to save rather than overwrite it.

use crate::util::{Json, daily_backup, now_iso, read_json, s, today, write_json};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// loved = "loved it", curious = "looks fun", pass = "not really", meh = "not for us", played = "played it".
pub const MARKS: [&str; 5] = ["loved", "curious", "pass", "meh", "played"];
const KEEP_HISTORY: usize = 2000;

fn valid_pid(p: &str) -> bool {
    !p.is_empty() && p.len() <= 12 && p.chars().all(|c| c.is_ascii_alphanumeric())
}

fn clean(raw: &Value) -> Value {
    let mut marks = Map::new();
    for (t, v) in raw.get("marks").and_then(Value::as_object).into_iter().flatten() {
        let m: Map<String, Value> = v
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(p, m)| valid_pid(p) && MARKS.contains(&m.as_str().unwrap_or("")))
            .map(|(p, m)| (p.clone(), m.clone()))
            .collect();
        if !m.is_empty() {
            marks.insert(t.clone(), Value::Object(m));
        }
    }
    let hist: Vec<Value> = raw
        .get("history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|h| h.get("title").is_some_and(Value::is_string) && h.get("at").is_some_and(Value::is_string))
        .map(|h| json!({"title": s(h, "title"), "at": s(h, "at"), "chose": if valid_pid(s(h, "chose")) { s(h, "chose") } else { "" }, "mode": if s(h, "mode") == "online" { "online" } else { "couch" }}))
        .collect();
    let hist = hist[hist.len().saturating_sub(KEEP_HISTORY)..].to_vec();
    let mut progress = Map::new();
    for (t, v) in raw.get("progress").and_then(Value::as_object).into_iter().flatten() {
        let note: String = s(v, "note").trim().chars().take(280).collect();
        let ongoing = v.get("ongoing").and_then(Value::as_bool).unwrap_or(false);
        if !note.is_empty() || ongoing {
            progress.insert(t.clone(), json!({"note": note, "ongoing": ongoing}));
        }
    }
    let vet = raw.get("vetoes").cloned().unwrap_or(json!({}));
    json!({
        "v": 3, "marks": marks, "history": hist, "turn": if valid_pid(s(raw, "turn")) { s(raw, "turn") } else { "" },
        "turnOnline": if valid_pid(s(raw, "turnOnline")) { s(raw, "turnOnline") } else { "" },
        "vetoes": {"day": s(&vet, "day"), "titles": vet.get("titles").and_then(Value::as_array).map(|a| a.iter().filter(|x| x.is_string()).cloned().collect::<Vec<_>>()).unwrap_or_default()},
        "progress": progress,
    })
}

pub struct Store {
    path: PathBuf,
    state: RwLock<Json>,
    pub load_error: String,
}

impl Store {
    pub fn open(path: &Path) -> Self {
        let mut load_error = String::new();
        let state = match read_json(path) {
            Ok(Some(v)) => clean(&v),
            Ok(None) => clean(&json!({})),
            Err(e) if path.exists() && std::fs::read(path).is_ok() => {
                let _ = std::fs::copy(path, path.with_extension(format!("damaged-{}", now_iso().replace(':', ""))));
                let _ = e;
                clean(&json!({}))
            }
            Err(e) => {
                load_error = format!("The saved state couldn't be read ({e}). Game Night won't overwrite it: close anything that has it open and restart.");
                clean(&json!({}))
            }
        };
        Store { path: path.into(), state: RwLock::new(state), load_error }
    }

    fn commit<R>(&self, f: impl FnOnce(&mut Json) -> Result<R, String>) -> Result<R, String> {
        if !self.load_error.is_empty() {
            return Err(self.load_error.clone());
        }
        let mut w = self.state.write().unwrap();
        let mut new = w.clone();
        let out = f(&mut new)?;
        daily_backup(&self.path, "state", 10);
        write_json(&self.path, &new).map_err(|e| format!("couldn't save: {e}"))?;
        *w = new;
        Ok(out)
    }

    pub fn read<R>(&self, f: impl FnOnce(&Json) -> R) -> R {
        f(&self.state.read().unwrap())
    }

    pub fn marks(&self) -> Json {
        self.read(|st| st["marks"].clone())
    }

    #[cfg(test)]
    pub fn mark(&self, title: &str, pid: &str) -> Option<String> {
        self.read(|st| st["marks"].get(title).and_then(|m| m.get(pid)).and_then(Value::as_str).map(String::from))
    }

    pub fn set_mark(&self, title: &str, pid: &str, mark: Option<&str>) -> Result<(), String> {
        if !valid_pid(pid) {
            return Err("unknown player".into());
        }
        if let Some(m) = mark
            && !MARKS.contains(&m)
        {
            return Err("unknown mark".into());
        }
        self.commit(|st| {
            let marks = st["marks"].as_object_mut().unwrap();
            let e = marks.entry(title.to_string()).or_insert_with(|| json!({}));
            let o = e.as_object_mut().unwrap();
            match mark {
                Some(m) => {
                    o.insert(pid.into(), json!(m));
                }
                None => {
                    o.remove(pid);
                }
            }
            if o.is_empty() {
                marks.remove(title);
            }
            Ok(())
        })
    }

    pub fn progress(&self) -> Json {
        self.read(|st| st["progress"].clone())
    }

    pub fn set_progress(&self, title: &str, note: &str, ongoing: bool) -> Result<Json, String> {
        let note = note.trim();
        if note.chars().count() > 280 {
            return Err("notes are limited to 280 characters".into());
        }
        self.commit(|st| {
            let p = st["progress"].as_object_mut().unwrap();
            if note.is_empty() && !ongoing {
                p.remove(title);
            } else {
                p.insert(title.into(), json!({"note": note, "ongoing": ongoing}));
            }
            Ok(p.get(title).cloned().unwrap_or(json!({"note": "", "ongoing": false})))
        })
    }

    pub fn history(&self) -> Vec<Json> {
        self.read(|st| st["history"].as_array().cloned().unwrap_or_default())
    }

    /// Each way of playing keeps its own turn: "couch" (the original field) and "online".
    fn turn_key(mode: &str) -> &'static str {
        if mode == "online" { "turnOnline" } else { "turn" }
    }

    /// Whose pick it is in this mode: the stored player if they are still in the group, else the group's first player.
    pub fn turn(&self, players: &[String], mode: &str) -> String {
        let t = self.read(|st| s(st, Self::turn_key(mode)).to_string());
        if players.contains(&t) { t } else { players.first().cloned().unwrap_or_default() }
    }

    pub fn set_turn(&self, pid: &str, mode: &str) -> Result<(), String> {
        self.commit(|st| {
            st[Self::turn_key(mode)] = json!(pid);
            Ok(())
        })
    }

    fn next_of(players: &[String], cur: &str) -> String {
        let i = players.iter().position(|p| p == cur).map(|i| (i + 1) % players.len()).unwrap_or(0);
        players.get(i).cloned().unwrap_or_default()
    }

    /// "We'll play this": record who chose it (and how you played), and hand the pick to the next player of this group.
    pub fn confirm(&self, title: &str, players: &[String], mode: &str) -> Result<Json, String> {
        let cur = self.turn(players, mode);
        self.commit(|st| {
            let e = json!({"title": title, "at": now_iso(), "chose": cur, "mode": if mode == "online" { "online" } else { "couch" }});
            let h = st["history"].as_array_mut().unwrap();
            h.push(e.clone());
            if h.len() > KEEP_HISTORY {
                h.remove(0);
            }
            st[Self::turn_key(mode)] = json!(Self::next_of(players, &cur));
            Ok(e)
        })
    }

    /// Take back the most recent pick of this game within 12 hours (it never really started), and give the turn back.
    pub fn undo_confirm(&self, title: &str, mode: &str) -> Result<bool, String> {
        let cutoff = (chrono::Local::now() - chrono::Duration::hours(12)).format("%Y-%m-%dT%H:%M:%S").to_string();
        self.commit(|st| {
            let h = st["history"].as_array_mut().unwrap();
            if let Some(i) = h.iter().rposition(|x| s(x, "title") == title && s(x, "at") >= cutoff.as_str()) {
                let chose = s(&h[i], "chose").to_string();
                h.remove(i);
                if !chose.is_empty() {
                    st[Self::turn_key(mode)] = json!(chose);
                }
                return Ok(true);
            }
            Ok(false)
        })
    }

    pub fn vetoes(&self) -> Vec<String> {
        let day = today();
        self.read(|st| {
            if s(&st["vetoes"], "day") == day {
                st["vetoes"]["titles"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect()
            } else {
                vec![]
            }
        })
    }

    pub fn veto(&self, title: &str) -> Result<(), String> {
        let day = today();
        self.commit(|st| {
            if s(&st["vetoes"], "day") != day {
                st["vetoes"] = json!({"day": day, "titles": []});
            }
            let t = st["vetoes"]["titles"].as_array_mut().unwrap();
            if !t.iter().any(|x| x == title) {
                t.push(json!(title));
            }
            Ok(())
        })
    }

    pub fn clear_vetoes(&self) -> Result<(), String> {
        self.commit(|st| {
            st["vetoes"] = json!({"day": "", "titles": []});
            Ok(())
        })
    }

    /// A rename in the catalog carries marks, notes and history along.
    pub fn rename(&self, old: &str, new: &str) -> Result<(), String> {
        if old == new {
            return Ok(());
        }
        self.commit(|st| {
            for k in ["marks", "progress"] {
                if let Some(v) = st[k].as_object_mut().and_then(|o| o.remove(old)) {
                    st[k][new] = v;
                }
            }
            for h in st["history"].as_array_mut().unwrap() {
                if s(h, "title") == old {
                    h["title"] = json!(new);
                }
            }
            Ok(())
        })
    }

    pub fn export(&self) -> Json {
        self.read(|st| {
            let mut e = st.clone();
            e["exported"] = json!(now_iso());
            e
        })
    }

    /// Bring in an export: marks and notes for games in this catalog, history merged without duplicates. Returns how many marks came in.
    pub fn import(&self, data: &Value, known: &dyn Fn(&str) -> bool) -> Result<usize, String> {
        if !data.is_object() || data.get("marks").is_none() && data.get("history").is_none() {
            return Err("not an export file".into());
        }
        let inc = clean(data);
        self.commit(|st| {
            let mut n = 0;
            for (t, m) in inc["marks"].as_object().unwrap() {
                if known(t) {
                    st["marks"][t] = m.clone();
                    n += 1;
                }
            }
            for (t, p) in inc["progress"].as_object().unwrap() {
                if known(t) {
                    st["progress"][t] = p.clone();
                }
            }
            let h = st["history"].as_array_mut().unwrap();
            for x in inc["history"].as_array().unwrap() {
                if !h.iter().any(|y| s(y, "title") == s(x, "title") && s(y, "at") == s(x, "at")) {
                    h.push(x.clone());
                }
            }
            h.sort_by(|a, b| s(a, "at").cmp(s(b, "at")));
            let len = h.len();
            h.drain(..len.saturating_sub(KEEP_HISTORY));
            Ok(n)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gn-store-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("state.json")
    }

    #[test]
    fn marks_turns_and_vetoes() {
        let p = temp("basic");
        let st = Store::open(&p);
        let players = vec!["p1".to_string(), "p2".into(), "p3".into()];
        st.set_mark("A", "p2", Some("loved")).unwrap();
        assert!(st.set_mark("A", "p2", Some("adored")).is_err());
        assert_eq!(st.mark("A", "p2").as_deref(), Some("loved"));
        assert_eq!(st.turn(&players, "couch"), "p1");
        st.confirm("A", &players, "couch").unwrap();
        st.confirm("B", &players, "couch").unwrap();
        assert_eq!(st.turn(&players, "couch"), "p3");
        st.confirm("C", &players, "couch").unwrap();
        assert_eq!(st.turn(&players, "couch"), "p1", "the pick goes round the whole group");
        assert!(st.undo_confirm("C", "couch").unwrap());
        assert_eq!(st.turn(&players, "couch"), "p3");
        st.veto("A").unwrap();
        assert_eq!(st.vetoes(), vec!["A"]);
        let again = Store::open(&p);
        assert_eq!(again.history().len(), 2);
        assert_eq!(again.vetoes(), vec!["A"]);
        assert_eq!(again.turn(&["p1".into(), "p2".into()], "couch"), "p1", "a player who left the group hands the pick to the first player");
    }

    #[test]
    fn unreadable_state_is_never_overwritten() {
        let p = temp("locked");
        std::fs::create_dir_all(&p).unwrap(); // a folder where the file should be: it exists but can't be read as a file
        let st = Store::open(&p);
        assert!(!st.load_error.is_empty());
        assert!(st.set_mark("A", "p1", Some("loved")).is_err());
    }

    #[test]
    fn import_merges_and_rename_carries_everything() {
        let p = temp("import");
        let st = Store::open(&p);
        st.set_progress("A", "chapter 3", true).unwrap();
        st.confirm("A", &["p1".into()], "couch").unwrap();
        let n = st
            .import(
                &json!({"marks": {"A": {"p1": "loved"}, "Gone": {"p1": "loved"}}, "history": [{"title": "A", "at": "2026-01-01T20:00:00", "chose": "p1"}]}),
                &|t| t == "A",
            )
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(st.history().len(), 2);
        st.rename("A", "A2").unwrap();
        assert_eq!(st.mark("A2", "p1").as_deref(), Some("loved"));
        assert_eq!(s(&st.progress()["A2"], "note"), "chapter 3");
        assert!(st.history().iter().all(|h| s(h, "title") == "A2"));
    }
}
