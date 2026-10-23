//! Everything the page can ask for, behind the HTTP layer (so it is testable without sockets).

use crate::catalog::Catalog;
use crate::config::{self, Paths, Settings, VERSION};
use crate::journal;
use crate::launch;
use crate::media::Media;
use crate::picker::{self, Context, Play};
use crate::scrape;
use crate::sessions::{self, SessionLog};
use crate::store::Store;
use crate::util::{Json, OsRng, Rng, now_iso, s, today};
use chrono::Datelike;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

pub type R = Result<Json, String>;
/// Starts a game for real; tests swap in one that starts nothing.
pub type Runner = Box<dyn Fn(&launch::Plan) -> Result<Option<std::process::Child>, String> + Send>;
type Availability = (Option<Instant>, HashMap<String, launch::Plan>);
type SteamCache = Option<(Instant, String, crate::steam::Discovery)>;
type SourcesCache = Option<(Instant, Vec<crate::detect::Source>)>;

/// Settings holding secrets: the page only ever sees "set", and sending "set" back keeps the stored value.
const SECRETS: [&str; 4] = ["steamgriddbKey", "thegamesdbKey", "screenscraperDevPassword", "screenscraperPassword"];

/// Twenty well-known games to try the picker with before bringing in your own (no launch set, no paths).
pub const SAMPLE: &str = include_str!("../examples/sample-catalog.json");

pub const DRAFT_WHY: &str = "Added from your library; the details haven't been checked yet.";

pub struct App {
    pub paths: Paths,
    pub settings: RwLock<Settings>,
    pub settings_warning: String,
    pub catalog: Catalog,
    pub store: Store,
    pub sessions: Arc<SessionLog>,
    pub media: Media,
    avail: Mutex<Availability>,
    steam: Mutex<SteamCache>,
    sources: Mutex<SourcesCache>,
    /// Overridden by tests so nothing is ever started for real.
    pub runner: Mutex<Option<Runner>>,
    pub rng: Mutex<Option<Box<dyn Rng + Send>>>,
}

fn bad(m: impl Into<String>) -> String {
    m.into()
}

impl App {
    pub fn new(data: PathBuf) -> Self {
        let paths = Paths::new(data);
        let _ = std::fs::create_dir_all(&paths.data);
        let (settings, settings_warning) = config::load_settings(&paths.settings());
        App {
            catalog: Catalog::open(&paths.catalog()),
            store: Store::open(&paths.state()),
            sessions: Arc::new(SessionLog::open(&paths.sessions())),
            settings: RwLock::new(settings),
            settings_warning,
            media: Media::default(),
            avail: Mutex::new((None, HashMap::new())),
            steam: Mutex::new(None),
            sources: Mutex::new(None),
            runner: Mutex::new(None),
            rng: Mutex::new(None),
            paths,
        }
    }

    fn with_rng<T>(&self, f: impl FnOnce(&mut dyn Rng) -> T) -> T {
        let mut g = self.rng.lock().unwrap();
        match g.as_mut() {
            Some(r) => f(r.as_mut()),
            None => f(&mut OsRng),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    fn players(&self) -> Vec<String> {
        self.settings().player_ids()
    }

    fn need_game(&self, t: &str) -> Result<Json, String> {
        self.catalog.get(t).ok_or_else(|| bad("unknown game"))
    }

    fn need_player(&self, p: &str) -> Result<(), String> {
        if self.settings().is_player(p) { Ok(()) } else { Err(bad("unknown player")) }
    }

    /// Every Steam install and library on this computer (registry, usual places, every drive), looked up at most once a minute.
    pub fn steam(&self) -> crate::steam::Discovery {
        let custom = self.settings().steam_dir;
        let mut c = self.steam.lock().unwrap();
        if let Some((at, key, d)) = c.as_ref()
            && at.elapsed() < Duration::from_secs(60)
            && *key == custom
        {
            return d.clone();
        }
        let d = crate::steam::discover(&custom);
        *c = Some((Instant::now(), custom, d.clone()));
        d
    }

    /// The main Steam install (for Steam's cached art).
    pub fn steam_root(&self) -> Option<PathBuf> {
        self.steam().roots.into_iter().next()
    }

    /// {title: plan}, cached for a few seconds.
    pub fn availability(&self) -> HashMap<String, launch::Plan> {
        let mut a = self.avail.lock().unwrap();
        if a.0.is_none_or(|t| t.elapsed() > Duration::from_secs(8)) {
            let bases = launch::search_bases(&self.settings().game_dirs);
            a.1 = self.catalog.games().iter().map(|g| (s(g, "t").to_string(), launch::plan_with(g, &bases))).collect();
            a.0 = Some(Instant::now());
        }
        a.1.clone()
    }

    pub fn refresh(&self) {
        *self.avail.lock().unwrap() = (None, HashMap::new());
        *self.sources.lock().unwrap() = None;
        self.media.clear();
        self.catalog.reload();
    }

    fn plays(&self) -> HashMap<String, Play> {
        journal::plays_from(&self.sessions.last_dates())
    }

    pub fn context(&self) -> Context {
        let st = self.settings();
        let players = st.player_ids();
        let cutoff = (chrono::Local::now() - chrono::Duration::days(21)).format("%Y-%m-%dT%H:%M:%S").to_string();
        let mut c = Context::new(players.clone(), crate::util::today_date());
        c.marks = self.store.marks();
        c.turn = self.store.turn(&players);
        c.plays = self.plays();
        c.history = self.store.history().iter().filter(|h| s(h, "at") >= cutoff.as_str()).map(|h| s(h, "title").to_string()).collect();
        c.vetoes = self.store.vetoes().into_iter().collect();
        c.ongoing = self.store.progress().as_object().into_iter().flatten().filter(|(_, v)| v["ongoing"] == json!(true)).map(|(t, _)| t.clone()).collect();
        // a game with no launch set (a console game, say) is still drawn; one whose launch is set but broken is not, and says why
        let games = self.catalog.games();
        let has_launch: HashSet<&str> = games.iter().filter(|g| g.get("launch").is_some()).map(|g| s(g, "t")).collect();
        c.unavailable = self.availability().into_iter().filter(|(t, p)| !p.ok && has_launch.contains(t.as_str())).map(|(t, p)| (t, p.reason)).collect();
        c.meh_soft = st.not_for_us == "rarer";
        c
    }

    #[allow(clippy::too_many_arguments)] // the shared lookups are made once per bootstrap and handed to each game
    fn game_view(
        &self,
        g: &Json,
        plays: &HashMap<String, Play>,
        marks: &Json,
        progress: &Json,
        avail: &HashMap<String, launch::Plan>,
        st: &Settings,
        steam: Option<&std::path::Path>,
    ) -> Json {
        let t = s(g, "t");
        let mut v = g.clone();
        let has = |k: &str| self.media.find(g, k, st, &self.paths.media(), steam).is_some();
        let p = avail.get(t);
        v["marks"] = marks.get(t).cloned().unwrap_or(json!({}));
        v["progress"] = progress.get(t).cloned().unwrap_or(json!({"note": "", "ongoing": false}));
        v["plays"] = plays.get(t).map(|p| json!({"count": p.count, "last": p.last})).unwrap_or(Value::Null);
        v["mySessions"] = self.sessions.summary(t).unwrap_or(Value::Null);
        v["cover"] = json!(has("cover"));
        v["video"] = json!(has("video"));
        v["shot"] = json!(has("shot"));
        v["titleShot"] =
            json!(has("title") && self.media.find(g, "title", st, &self.paths.media(), steam) != self.media.find(g, "shot", st, &self.paths.media(), steam));
        v["hasLaunch"] = json!(g.get("launch").is_some());
        v["launchable"] = json!(p.is_some_and(|p| p.ok));
        v["launchWhy"] = json!(p.filter(|p| !p.ok).map(|p| p.reason.clone()).unwrap_or_default());
        v
    }

    pub fn bootstrap(&self) -> R {
        let st = self.settings();
        let players = st.player_ids();
        let (plays, marks, progress, avail, steam) = (self.plays(), self.store.marks(), self.store.progress(), self.availability(), self.steam_root());
        let games: Vec<Json> = self.catalog.games().iter().map(|g| self.game_view(g, &plays, &marks, &progress, &avail, &st, steam.as_deref())).collect();
        let mut platforms: Vec<String> = games.iter().map(|g| s(g, "sys").to_string()).collect();
        platforms.sort();
        platforms.dedup();
        let hist = self.store.history();
        let ctx = self.context();
        let wheel = self.with_rng(|r| picker::preview_wheel(&self.catalog.games(), &json!({}), &ctx, r));
        let warnings: Vec<String> = [
            self.settings_warning.clone(),
            self.store.load_error.clone(),
            self.sessions.load_error.clone(),
            self.catalog.recovered_from.read().unwrap().clone().chars().take(0).collect::<String>(),
        ]
        .into_iter()
        .chain(
            Some(self.catalog.recovered_from.read().unwrap().clone())
                .filter(|r| !r.is_empty())
                .map(|r| format!("The catalog was damaged; using the backup {r}.")),
        )
        .filter(|w| !w.is_empty())
        .collect();
        let notices: Vec<String> = {
            let bad: Vec<&String> = ctx.unavailable.keys().collect();
            if bad.is_empty() {
                vec![]
            } else {
                vec![format!(
                    "{} game{} can't be started right now and won't be drawn: {}.",
                    bad.len(),
                    if bad.len() == 1 { "" } else { "s" },
                    bad.iter().take(4).map(|x| x.as_str()).collect::<Vec<_>>().join(", ")
                )]
            }
        };
        Ok(json!({
            "version": VERSION, "players": st.players, "turn": self.store.turn(&players), "games": games, "wishlist": self.catalog.wishlist(),
            "history": hist[hist.len().saturating_sub(30)..], "vetoes": self.store.vetoes(), "today": today(), "platforms": platforms,
            "settings": self.settings_view(&st), "notices": notices, "warnings": warnings, "problems": self.catalog.problems(), "wheel": wheel,
            "scrape": cfg!(feature = "scrape"),
        }))
    }

    fn settings_view(&self, st: &Settings) -> Json {
        let mut v = serde_json::to_value(st).unwrap_or(json!({}));
        for k in SECRETS {
            if v[k].as_str().is_some_and(|x| !x.is_empty()) {
                v[k] = json!("set"); // never send a secret back to the page
            }
        }
        v["dataDir"] = json!(self.paths.data.to_string_lossy());
        v["steamFound"] = json!(self.steam_root().map(|p| p.to_string_lossy().to_string()));
        v
    }

    pub fn set_mark(&self, t: &str, pid: &str, mark: Option<&str>) -> R {
        self.need_game(t)?;
        self.need_player(pid)?;
        self.store.set_mark(t, pid, mark)?;
        if mark.is_some() {
            self.sessions.rated_by_hand(pid);
        }
        Ok(json!({"marks": self.store.marks().get(t).cloned().unwrap_or(json!({}))}))
    }

    pub fn pick(&self, filters: &Value, seen: &[String]) -> R {
        let ctx = self.context();
        Ok(self.with_rng(|r| picker::pick(&self.catalog.games(), filters, &ctx, r, seen)))
    }

    pub fn preview(&self, filters: &Value) -> R {
        let ctx = self.context();
        let games = self.catalog.games();
        let f = picker::Filters::from_json(filters);
        let c2 = picker::for_filters(&ctx, &f);
        let (m, note) = picker::matching(&games, &f, &c2);
        let pool = picker::pool(&games, &f, &c2).0.len();
        let wheel = self.with_rng(|r| picker::preview_wheel(&games, filters, &ctx, r));
        Ok(json!({"wheel": wheel, "matching": m.iter().map(|g| s(g, "t")).collect::<Vec<_>>(), "pool": pool, "note": note}))
    }

    pub fn finalists(&self, filters: &Value) -> R {
        let ctx = self.context();
        Ok(self.with_rng(|r| picker::finalists(&self.catalog.games(), filters, &ctx, 3, r)))
    }

    pub fn plan(&self, filters: &Value) -> R {
        let ctx = self.context();
        Ok(self.with_rng(|r| picker::plan_evening(&self.catalog.games(), filters, &ctx, r)))
    }

    pub fn taste_set(&self, pid: &str, n: usize) -> R {
        self.need_player(pid)?;
        let marks = self.store.marks();
        let titles = self.with_rng(|r| picker::taste_set(&self.catalog.games(), &marks, pid, n.clamp(1, 20), r));
        let (st, steam) = (self.settings(), self.steam_root());
        Ok(json!({"games": titles.iter().filter_map(|t| self.catalog.get(t)).map(|g| {
            let line = s(&g, "why").split(". ").next().unwrap_or("").trim_end_matches('.').to_string() + ".";
            json!({"title": s(&g, "t"), "line": line, "cover": self.media.find(&g, "cover", &st, &self.paths.media(), steam.as_deref()).is_some()})
        }).collect::<Vec<_>>()}))
    }

    pub fn similar(&self, t: &str) -> R {
        self.need_game(t)?;
        Ok(json!({"titles": picker::similar(&self.catalog.games(), t, 4)}))
    }

    pub fn confirm(&self, t: &str) -> R {
        self.need_game(t)?;
        let players = self.players();
        let e = self.store.confirm(t, &players)?;
        Ok(json!({"entry": e, "turn": self.store.turn(&players)}))
    }

    pub fn veto(&self, t: &str) -> R {
        self.need_game(t)?;
        self.store.veto(t)?;
        Ok(json!({"vetoes": self.store.vetoes()}))
    }

    pub fn clear_vetoes(&self) -> R {
        self.store.clear_vetoes()?;
        Ok(json!({"vetoes": []}))
    }

    pub fn set_turn(&self, pid: &str) -> R {
        self.need_player(pid)?;
        self.store.set_turn(pid)?;
        Ok(json!({"turn": pid}))
    }

    pub fn set_progress(&self, t: &str, note: &str, ongoing: bool) -> R {
        self.need_game(t)?;
        Ok(json!({"progress": self.store.set_progress(t, note, ongoing)?}))
    }

    pub fn launch(&self, t: &str, dry: bool) -> R {
        let g = self.need_game(t)?;
        let p = launch::plan_with(&g, &launch::search_bases(&self.settings().game_dirs));
        if dry || !self.settings().launch_enabled {
            let msg = if !p.ok {
                p.reason.clone()
            } else if dry {
                format!("Would start: {}", p.preview)
            } else {
                format!("Starting games is switched off in Settings. It would start: {}", p.preview)
            };
            return Ok(json!({"started": false, "ok": p.ok, "message": msg, "preview": p.preview}));
        }
        if !p.ok {
            return Ok(json!({"started": false, "ok": false, "message": p.reason, "preview": p.preview}));
        }
        let res = match self.runner.lock().unwrap().as_ref() {
            Some(r) => r(&p),
            None => launch::execute(&p),
        };
        match res {
            Err(e) => Ok(json!({"started": false, "ok": true, "message": e, "preview": p.preview})),
            Ok(child) => {
                let sess = self.sessions.start(t)?;
                let sid = s(&sess, "id").to_string();
                if let Some(c) = child {
                    sessions::watch_child(self.sessions.clone(), sid.clone(), c);
                } else if let Some(name) = g.get("process").and_then(Value::as_str) {
                    sessions::watch_name(self.sessions.clone(), sid.clone(), name.to_string());
                }
                Ok(json!({"started": true, "ok": true, "message": format!("Starting {t}."), "preview": p.preview, "session": sid}))
            }
        }
    }

    pub fn now(&self) -> R {
        let st = self.settings();
        let mut act = self.sessions.active();
        if let Some(a) = act.as_mut() {
            a["minutesSoFar"] = json!(crate::util::minutes_between(s(a, "start"), &now_iso()));
        }
        Ok(
            json!({"active": act, "pending": if st.ask_after_game { self.sessions.pending() } else { vec![] }, "ask": self.sessions.ask_whom(&st.player_ids()), "askEnabled": st.ask_after_game}),
        )
    }

    pub fn end_session(&self, sid: &str) -> R {
        Ok(json!({"session": self.sessions.end(sid, "manual")?.ok_or("unknown session")?}))
    }

    pub fn session_started(&self, sid: &str, ok: bool) -> R {
        let x = self.sessions.get(sid).ok_or("unknown session")?;
        if ok {
            return Ok(json!({"session": self.sessions.confirm_started(sid)?}));
        }
        self.store.undo_confirm(s(&x, "title"))?;
        Ok(json!({"session": self.sessions.mark_failed(sid)?, "turn": self.store.turn(&self.players())}))
    }

    pub fn rate_session(&self, sid: &str, pid: &str, rating: &str) -> R {
        self.need_player(pid)?;
        let x = self.sessions.rate(sid, pid, rating)?.ok_or("unknown session")?;
        let t = s(&x, "title");
        if self.catalog.has(t) {
            match rating {
                "up" => self.store.set_mark(t, pid, Some("loved"))?,
                "down" => self.store.set_mark(t, pid, Some("pass"))?, // "not really"; "not for us" stays a deliberate act
                _ => {}
            }
        }
        Ok(json!({"session": x, "marks": self.store.marks().get(t).cloned().unwrap_or(json!({}))}))
    }

    pub fn session_outcome(&self, sid: &str, outcome: &str) -> R {
        let x = self.sessions.set_outcome(sid, outcome)?.ok_or("unknown session")?;
        let t = s(&x, "title");
        let note = s(&self.store.progress()[t], "note").to_string();
        let p = if self.catalog.has(t) { self.store.set_progress(t, &note, outcome == "again")? } else { json!(null) };
        Ok(json!({"session": x, "progress": p}))
    }

    pub fn dismiss_session(&self, sid: &str) -> R {
        self.sessions.dismiss(sid)?;
        Ok(json!({"ok": true}))
    }

    fn all_nights(&self) -> Vec<Json> {
        journal::nights(&self.sessions.items(), &self.store.history()).0
    }

    pub fn nights(&self) -> R {
        let (mut ns, stats) = journal::nights(&self.sessions.items(), &self.store.history());
        let (st, steam) = (self.settings(), self.steam_root());
        for n in ns.iter_mut() {
            for g in n["games"].as_array_mut().unwrap() {
                let cat = self.catalog.get(s(g, "title"));
                g["inCatalog"] = json!(cat.is_some());
                g["cover"] = json!(cat.is_some_and(|c| self.media.find(&c, "cover", &st, &self.paths.media(), steam.as_deref()).is_some()));
            }
        }
        ns.truncate(365);
        Ok(json!({"nights": ns, "stats": stats}))
    }

    pub fn memories(&self) -> R {
        Ok(json!({"memories": journal::memories(&self.all_nights(), &|t| self.catalog.has(t), &self.players(), 40)}))
    }

    pub fn year(&self, y: Option<i32>) -> R {
        let y = y.filter(|y| (2000..=2100).contains(y)).unwrap_or_else(|| crate::util::today_date().year());
        let games = self.catalog.games();
        let vibes = |t: &str| -> Vec<String> { games.iter().find(|g| s(g, "t") == t).map(|g| crate::util::strs(g, "vibe")).unwrap_or_default() };
        Ok(journal::year(&self.all_nights(), &vibes, &self.players(), y))
    }

    pub fn taste(&self) -> R {
        Ok(journal::taste(&self.catalog.games(), &self.store.marks(), &self.players(), None))
    }

    pub fn history(&self) -> R {
        let hist = self.store.history();
        let mut counts: HashMap<&str, i64> = HashMap::new();
        let mut months: std::collections::BTreeMap<&str, i64> = Default::default();
        let mut chose: HashMap<&str, i64> = HashMap::new();
        for h in &hist {
            *counts.entry(s(h, "title")).or_default() += 1;
            *months.entry(s(h, "at").get(..7).unwrap_or("")).or_default() += 1;
            *chose.entry(s(h, "chose")).or_default() += 1;
        }
        let mut top: Vec<(&str, i64)> = counts.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        Ok(
            json!({"entries": hist.iter().rev().take(200).collect::<Vec<_>>(), "top": top.iter().take(8).map(|(t, n)| json!({"title": t, "count": n})).collect::<Vec<_>>(), "byMonth": months, "chose": chose, "total": hist.len()}),
        )
    }

    pub fn recap(&self, days: i64) -> R {
        let cutoff = (crate::util::today_date() - chrono::Duration::days(days.clamp(1, 90))).format("%Y-%m-%d").to_string();
        let hist: Vec<Json> = self.store.history().into_iter().filter(|h| s(h, "at").get(..10).unwrap_or("") >= cutoff.as_str()).collect();
        let mut counts: Vec<(String, i64)> = vec![];
        for h in &hist {
            match counts.iter_mut().find(|c| c.0 == s(h, "title")) {
                Some(c) => c.1 += 1,
                None => counts.push((s(h, "title").into(), 1)),
            }
        }
        Ok(
            json!({"since": cutoff, "days": days, "confirmed": counts.iter().map(|(t, n)| json!({"title": t, "times": n})).collect::<Vec<_>>(), "total": hist.len(), "minutes": self.sessions.minutes_since(&cutoff)}),
        )
    }

    pub fn wishlist_edit(&self, d: &Value) -> R {
        match s(d, "action") {
            "add" => self.catalog.add_wish(d.get("entry").unwrap_or(&json!({})))?,
            "remove" => {
                self.catalog.remove_wish(s(d, "title"))?;
            }
            "price" => self.catalog.set_wish_price(s(d, "title"), d.get("price").filter(|p| !p.is_null()))?,
            _ => return Err(bad("action must be add, remove or price")),
        }
        Ok(json!({"wishlist": self.catalog.wishlist()}))
    }

    pub fn save_game(&self, game: &Value, create: bool, old_title: Option<&str>) -> R {
        let mut g = game.clone();
        if let Some(o) = g.as_object_mut() {
            o.remove("draft"); // saving from the editor means someone checked it
        }
        let replaced = self.catalog.upsert(&g, create, old_title)?;
        let t = s(&g, "t").trim().to_string();
        if let Some(o) = old_title.filter(|o| *o != t) {
            self.store.rename(o, &t)?;
        }
        self.refresh();
        Ok(json!({"updated": replaced, "title": t}))
    }

    pub fn delete_game(&self, t: &str) -> R {
        self.need_game(t)?;
        self.catalog.remove(t)?;
        self.refresh();
        Ok(json!({"removed": t}))
    }

    pub fn save_settings(&self, changes: &Value) -> R {
        let mut cur = serde_json::to_value(self.settings()).unwrap();
        let obj = changes.as_object().ok_or("settings must be an object")?;
        for (k, v) in obj {
            if SECRETS.contains(&k.as_str()) && v == "set" {
                continue; // the page shows "set" instead of the key; sending it back changes nothing
            }
            if cur.get(k).is_none() {
                return Err(format!("unknown setting {k}"));
            }
            cur[k] = v.clone();
        }
        let new: Settings = serde_json::from_value(cur).map_err(|e| format!("settings: {e}"))?;
        let new = new.clean();
        config::save_settings(&self.paths.settings(), &new).map_err(|e| e.to_string())?;
        *self.settings.write().unwrap() = new.clone();
        self.refresh();
        Ok(json!({"settings": self.settings_view(&new), "players": new.players}))
    }

    pub fn export(&self) -> R {
        Ok(json!({"state": self.store.export(), "catalog": self.catalog.snapshot(), "settings": {"players": self.settings().players}}))
    }

    pub fn import(&self, d: &Value) -> R {
        let state = d.get("state").unwrap_or(d);
        let mut added = 0;
        if let Some(games) = d.get("catalog").and_then(|c| c.get("games")).and_then(Value::as_array) {
            added = self.catalog.add_many(games)?.0;
        }
        let n = self.store.import(state, &|t| self.catalog.has(t))?;
        self.refresh();
        Ok(json!({"imported": n, "gamesAdded": added}))
    }

    // ---- library: Steam and game folders

    /// Every library source on this computer, each with what it found: Steam, ES-DE, Epic, GOG, RetroArch, Heroic, Lutris and your
    /// game folders. Looked up at most once a minute (Refresh looks again).
    pub fn sources(&self) -> Vec<crate::detect::Source> {
        let mut c = self.sources.lock().unwrap();
        if let Some((at, v)) = c.as_ref()
            && at.elapsed() < Duration::from_secs(60)
        {
            return v.clone();
        }
        let st = self.settings();
        let disc = self.steam();
        let steam = crate::detect::Source {
            id: "steam",
            label: "Steam",
            found_at: disc.libraries.iter().map(|(p, _)| p.clone()).collect(),
            games: crate::steam::installed(&disc)
                .into_iter()
                .map(|a| crate::detect::Candidate {
                    source: "steam",
                    key: a.appid.to_string(),
                    title: a.name,
                    sys: "PC".into(),
                    launch: Some(json!({"steam": a.appid})),
                    media: None,
                    extra: Map::new(),
                })
                .collect(),
            ..Default::default()
        };
        let folders = crate::detect::Source {
            id: "folder",
            label: "Game folders",
            found_at: st.game_dirs.iter().map(PathBuf::from).filter(|p| p.is_dir()).collect(),
            games: crate::folders::scan(&st.game_dirs)
                .into_iter()
                .map(|f| crate::detect::Candidate {
                    source: "folder",
                    key: f.path.to_string_lossy().into(),
                    title: f.title,
                    sys: "PC".into(),
                    launch: Some(f.launch),
                    media: None,
                    extra: Map::new(),
                })
                .collect(),
            ..Default::default()
        };
        let mut all = vec![steam];
        all.extend(crate::detect::all(&st.game_dirs));
        all.push(folders);
        *c = Some((Instant::now(), all.clone()));
        all
    }

    pub fn library(&self) -> R {
        let st = self.settings();
        let games = self.catalog.games();
        let titles: HashSet<String> = games.iter().map(|g| s(g, "t").to_lowercase()).collect();
        let ids: HashSet<u64> = games.iter().filter_map(launch::steam_id).collect();
        let media: HashSet<(String, String)> =
            games.iter().filter_map(|g| g.get("media")).map(|m| (s(m, "system").to_string(), s(m, "file").to_string())).collect();
        let on_shelf = |c: &crate::detect::Candidate| {
            titles.contains(&c.title.to_lowercase())
                || (c.source == "steam" && c.key.parse::<u64>().is_ok_and(|id| ids.contains(&id)))
                || c.media.as_ref().is_some_and(|m| media.contains(&(s(m, "system").to_string(), s(m, "file").to_string())))
        };
        let sources: Vec<Json> = self
            .sources()
            .iter()
            .map(|src| {
                json!({"id": src.id, "label": src.label, "foundAt": src.found_at.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(), "note": src.note,
                       "games": src.games.iter().map(|c| json!({"source": c.source, "key": c.key, "title": c.title, "sys": c.sys, "inCatalog": on_shelf(c), "canStart": c.launch.is_some()})).collect::<Vec<_>>()})
            })
            .collect();
        let disc = self.steam();
        Ok(
            json!({"sources": sources, "gameDirs": st.game_dirs, "steamLibraries": disc.libraries.iter().map(|(p, how)| json!({"path": p.to_string_lossy(), "how": how})).collect::<Vec<_>>()}),
        )
    }

    /// Add picked library games as drafts with everything their source could tell (players, a description, moods, art, how to start
    /// them). They are drawn straight away and marked to be checked. Picking ES-DE games also adds ES-DE's art folder.
    pub fn import_library(&self, picks: &Value) -> R {
        let picks = picks.as_array().ok_or("picks must be a list")?;
        let sources = self.sources();
        let mut drafts = vec![];
        let mut art_dirs: Vec<PathBuf> = vec![];
        for p in picks.iter().take(2000) {
            let (src, key) = (s(p, "source"), s(p, "key"));
            let Some(source) = sources.iter().find(|x| x.id == src) else { continue };
            let Some(c) = source.games.iter().find(|c| c.key == key) else { continue };
            drafts.push(crate::detect::draft(c, DRAFT_WHY));
            if let Some(a) = &source.art
                && !art_dirs.contains(a)
            {
                art_dirs.push(a.clone());
            }
        }
        let (n, refused) = self.catalog.add_many(&drafts)?;
        let mut st = self.settings();
        let before = st.media_dirs.len();
        for a in art_dirs {
            let path = a.to_string_lossy().to_string();
            if !st.media_dirs.iter().any(|m| m.path.eq_ignore_ascii_case(&path)) {
                st.media_dirs.push(config::MediaDir { path, layout: "esde".into() });
            }
        }
        if st.media_dirs.len() != before {
            config::save_settings(&self.paths.settings(), &st).map_err(|e| e.to_string())?;
            *self.settings.write().unwrap() = st;
        }
        self.refresh();
        Ok(json!({"added": n, "refused": refused}))
    }

    /// Games whose files were found somewhere new (a drive letter changed, a library moved): [(title, new launch)].
    pub fn moved(&self) -> Vec<(String, Value)> {
        let mut v: Vec<(String, Value)> = self.availability().into_iter().filter_map(|(t, p)| p.moved.map(|m| (t, m))).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Save the new places of moved games in their entries, so they no longer need finding.
    pub fn repair_moved(&self) -> R {
        let moved = self.moved();
        let mut fixed = vec![];
        for (t, launch) in &moved {
            if self.catalog.set_launch(t, launch)? {
                fixed.push(t.clone());
            }
        }
        self.refresh();
        Ok(json!({"fixed": fixed}))
    }

    pub fn add_sample(&self) -> R {
        let v: Value = serde_json::from_str(SAMPLE).map_err(|e| e.to_string())?;
        let (n, refused) = self.catalog.add_many(v["games"].as_array().map(Vec::as_slice).unwrap_or(&[]))?;
        self.refresh();
        Ok(json!({"added": n, "refused": refused}))
    }

    /// Folders on this computer that look like they hold games, for one-tap adding.
    pub fn folder_suggestions(&self) -> R {
        let st = self.settings();
        Ok(
            json!({"folders": crate::folders::suggest(&st.game_dirs).into_iter().map(|(p, n)| json!({"path": p.to_string_lossy(), "games": n})).collect::<Vec<_>>(),
                  "steamLibraries": self.steam().libraries.iter().map(|(p, how)| json!({"path": p.to_string_lossy(), "how": how})).collect::<Vec<_>>()}),
        )
    }

    /// The system's folder picker, opened on this computer. Returns the chosen folder, or null when cancelled or unavailable.
    pub fn pick_folder(&self, title: &str, start: &str) -> R {
        let title: String = if title.trim().is_empty() { "Choose a folder".into() } else { title.chars().take(80).collect() };
        Ok(json!({"path": crate::folders::pick_folder(&title, Some(start).filter(|x| !x.is_empty()))}))
    }

    // ---- scrapers

    pub fn providers(&self) -> R {
        let st = self.settings();
        Ok(
            json!({"providers": scrape::providers(&st).iter().map(|p| json!({"id": p.id(), "label": p.label(), "unavailable": p.unavailable()})).collect::<Vec<_>>()}),
        )
    }

    /// Look a game up with one provider, save its art, and fill in what's missing (a draft's placeholders are replaced).
    #[cfg_attr(not(feature = "scrape"), allow(unused_variables))]
    pub fn scrape(&self, t: &str, provider: &str, force_art: bool) -> R {
        let g = self.need_game(t)?;
        let st = self.settings();
        let all = scrape::providers(&st);
        let p = all.iter().find(|p| p.id() == provider).ok_or("unknown provider")?;
        if let Some(why) = p.unavailable() {
            return Err(why);
        }
        let found = p.find(&g)?;
        #[cfg_attr(not(feature = "scrape"), allow(unused_mut))]
        let mut saved: Vec<String> = vec![];
        #[cfg(feature = "scrape")]
        {
            let dir = self.paths.media().join(crate::util::slug(t));
            for (kind, url) in &found.images {
                if saved.contains(kind) {
                    continue;
                }
                let exists =
                    std::fs::read_dir(&dir).ok().is_some_and(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with(&format!("{kind}."))));
                if exists && !force_art {
                    saved.push(kind.clone());
                    continue;
                }
                if let Ok((bytes, ext)) = scrape::get_image(url) {
                    let _ = std::fs::create_dir_all(&dir);
                    if let Ok(rd) = std::fs::read_dir(&dir) {
                        for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{kind}."))) {
                            let _ = std::fs::remove_file(e.path()); // only our own saved art for this game
                        }
                    }
                    if std::fs::write(dir.join(format!("{kind}.{ext}")), bytes).is_ok() {
                        saved.push(kind.clone());
                    }
                }
            }
        }
        let draft = g.get("draft") == Some(&json!(true));
        let mut patch: Map<String, Value> = Map::new();
        for (k, v) in &found.patch {
            match k.strip_prefix("suggest:") {
                Some(field) if draft => {
                    patch.insert(field.into(), v.clone());
                }
                Some(_) => {}
                None => {
                    patch.insert(k.clone(), v.clone());
                }
            }
        }
        let changed = if draft {
            let mut ng = g.clone();
            for (k, v) in &patch {
                if k == "info" {
                    let info = ng.as_object_mut().unwrap().entry("info").or_insert(json!({}));
                    for (ik, iv) in v.as_object().into_iter().flatten() {
                        info.as_object_mut().unwrap().entry(ik.clone()).or_insert(iv.clone());
                    }
                } else if k != "why" || s(&g, "why") == DRAFT_WHY {
                    ng[k] = v.clone();
                }
            }
            if !(ng.get("one") == Some(&json!(true))) {
                ng.as_object_mut().unwrap().remove("one");
            }
            let c = ng != g;
            if c {
                self.catalog.upsert(&ng, false, None)?;
            }
            c
        } else {
            self.catalog.enrich(t, &patch)?
        };
        self.refresh();
        Ok(json!({"source": found.source, "art": saved, "changed": changed}))
    }

    pub fn media_path(&self, t: &str, kind: &str) -> Option<PathBuf> {
        let g = self.catalog.get(t)?;
        self.media.find(&g, kind, &self.settings(), &self.paths.media(), self.steam_root().as_deref())
    }

    pub fn health(&self) -> R {
        Ok(crate::health::run(self))
    }
}
