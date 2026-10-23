//! Choosing a game. The server decides (one implementation, covered by statistics tests); the page only animates the result.
//!
//! The winner is drawn from the WHOLE pool by cumulative weight with the operating system's secure random source. The names that
//! flick past and the wheel are decoration. Every multiplier lives in `WEIGHT_TABLE`, and the explanation under a result is built
//! from the same factors that made the weight, so the two can never drift apart.

use crate::catalog::prange;
use crate::util::{Json, Rng, b, i, s, shuffle, strs};
use chrono::NaiveDate;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub const WHEEL_SIZE: usize = 12;
pub const REEL_SIZE: usize = 24;
const RUNNERS: usize = 2;

pub const W_BASE: f64 = 1.5;
pub const W_PLAYED_MIN: f64 = 0.6;
pub const W_PLAYED_SPAN: f64 = 0.9;
pub const W_PLAYED_DAYS: f64 = 60.0;
pub const W_PLAYED_UNKNOWN: f64 = 0.8;
pub const W_LOVED: f64 = 1.6;
pub const W_TURN_LOVED: f64 = 1.5;
pub const W_CURIOUS: f64 = 1.3;
pub const W_PASS: f64 = 0.6;
pub const W_MEH: f64 = 0.3;
pub const W_ONGOING: f64 = 1.8;
pub const W_REST: f64 = 0.15;
pub const W_SOLO: f64 = 0.3;

pub fn weight_table() -> Json {
    json!([
        {"key": "base", "label": "Starting share", "value": format!("{W_BASE}"), "text": "Every game starts with the same share."},
        {"key": "played", "label": "Played recently", "value": format!("{W_PLAYED_MIN} just after, back to normal after about {W_PLAYED_DAYS} days"), "text": "A game played lately comes up a little less."},
        {"key": "loved", "label": "Loved it", "value": format!("x{W_LOVED} for each player who loved it"), "text": "Someone in the group loved it."},
        {"key": "turn", "label": "Favourite of whoever's pick it is", "value": format!("x{W_TURN_LOVED}"), "text": "It's a favourite of the person choosing tonight."},
        {"key": "curious", "label": "Looks fun", "value": format!("x{W_CURIOUS} for each player"), "text": "Someone said it looks fun."},
        {"key": "pass", "label": "Not really", "value": format!("x{W_PASS} for each player"), "text": "Someone said it isn't really their thing."},
        {"key": "meh", "label": "Not for us", "value": format!("left out, or x{W_MEH} when Settings says rarer"), "text": "Someone marked it not for us."},
        {"key": "ongoing", "label": "In progress", "value": format!("x{W_ONGOING}"), "text": "It's marked as in progress (a save you're partway through)."},
        {"key": "solo", "label": "Strictly one player", "value": format!("x{W_SOLO}, full share under One controller or spooky"), "text": "It's a one-player game, so it comes up less often unless you asked for One controller or something spooky."},
        {"key": "rest", "label": "One of the last three picks", "value": format!("x{W_REST}"), "text": "It was one of the last three picks (within three weeks), so it rests."},
    ])
}

fn table_text(key: &str) -> String {
    weight_table().as_array().unwrap().iter().find(|w| s(w, "key") == key).map(|w| s(w, "text").to_string()).unwrap_or_default()
}

#[derive(Clone, Debug, Default)]
pub struct Play {
    pub count: i64,
    pub last: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Context {
    pub marks: Json,
    pub players: Vec<String>,
    pub turn: String,
    pub plays: HashMap<String, Play>,
    /// Titles picked in the last three weeks, oldest first.
    pub history: Vec<String>,
    pub vetoes: HashSet<String>,
    pub ongoing: HashSet<String>,
    pub unavailable: HashMap<String, String>,
    pub today: NaiveDate,
    /// One person's "not for us" makes a game rarer instead of leaving it out.
    pub meh_soft: bool,
    pub solo_rare: bool,
    /// Titles everyone in the group owns on Steam, when two or more Steam libraries are linked.
    pub all_own: Option<HashSet<String>>,
}

impl Context {
    pub fn new(players: Vec<String>, today: NaiveDate) -> Self {
        Context {
            marks: json!({}),
            turn: players.first().cloned().unwrap_or_default(),
            players,
            plays: HashMap::new(),
            history: vec![],
            vetoes: HashSet::new(),
            ongoing: HashSet::new(),
            unavailable: HashMap::new(),
            today,
            meh_soft: false,
            solo_rare: false,
            all_own: None,
        }
    }

    fn mark(&self, t: &str, pid: &str) -> &str {
        self.marks.get(t).and_then(|m| m.get(pid)).and_then(Value::as_str).unwrap_or("")
    }

    fn marks_of(&self, t: &str) -> Vec<&str> {
        self.players.iter().map(|p| self.mark(t, p)).collect()
    }

    pub fn played_before(&self, g: &Json) -> bool {
        let t = s(g, "t");
        self.marks_of(t).contains(&"played") || self.plays.get(t).is_some_and(|p| p.count > 0)
    }

    fn days_since(&self, g: &Json) -> Option<i64> {
        let last = self.plays.get(s(g, "t"))?.last.as_deref()?;
        let d = crate::util::parse_date(last)?;
        Some((self.today - d).num_days().max(0))
    }

    fn anyone_meh(&self, t: &str) -> bool {
        self.marks_of(t).contains(&"meh")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filters {
    pub where_: String,
    pub len: String,
    pub gap: i64,
    pub n: Option<u32>,
    pub hide: bool,
    pub match_any: bool,
    pub two: bool,
    pub a: Vec<String>,
    pub b: Vec<String>,
    pub fresh: bool,
    pub one: bool,
    pub platform: String,
    /// Only games everyone owns on Steam (ignored until two Steam libraries are linked).
    pub owned: bool,
}

impl Filters {
    pub fn from_json(raw: &Value) -> Self {
        let pick = |k: &str, ok: &[&str], d: &str| -> String {
            let v = s(raw, k);
            if ok.contains(&v) { v.into() } else { d.into() }
        };
        let moods = |k: &str| -> Vec<String> { strs(raw, k).into_iter().filter(|m| crate::catalog::VIBES.contains(&m.as_str())).take(7).collect() };
        Filters {
            where_: pick("where", &["any", "couch", "online"], "any"),
            len: pick("len", &["any", "quick", "evening", "long"], "any"),
            gap: raw.get("gap").and_then(Value::as_i64).unwrap_or(1).clamp(1, 5),
            n: raw.get("n").and_then(Value::as_u64).filter(|n| (1..=crate::config::MAX_PLAYERS as u64).contains(n)).map(|n| n as u32),
            hide: b(raw, "hide"),
            match_any: s(raw, "match") == "any",
            two: b(raw, "two"),
            a: moods("A"),
            b: moods("B"),
            fresh: b(raw, "fresh"),
            one: b(raw, "one"),
            platform: s(raw, "platform").chars().take(24).collect(),
            owned: b(raw, "owned"),
        }
    }

    /// One-player games keep their full share only when the filters ask for them: One controller, or spooky.
    fn wants_solo(&self) -> bool {
        self.one || self.a.iter().any(|m| m == "spooky") || (self.two && self.b.iter().any(|m| m == "spooky"))
    }
}

pub fn for_filters(ctx: &Context, f: &Filters) -> Context {
    Context { solo_rare: !f.wants_solo(), ..ctx.clone() }
}

pub fn solo(g: &Json) -> bool {
    prange(s(g, "p")).1 <= 1
}

fn mood_ok(g: &Json, moods: &[String], any: bool) -> bool {
    if moods.is_empty() {
        return true;
    }
    let v = strs(g, "vibe");
    if any { moods.iter().any(|m| v.contains(m)) } else { moods.iter().all(|m| v.contains(m)) }
}

fn base_ok(g: &Json, f: &Filters, ctx: &Context) -> bool {
    if f.where_ != "any" && s(g, "where") != f.where_ {
        return false;
    }
    if f.len != "any" && s(g, "len") != f.len {
        return false;
    }
    if i(g, "gap") < f.gap {
        return false;
    }
    if let Some(n) = f.n {
        let (lo, hi) = prange(s(g, "p"));
        if n >= 3 && hi < n || n < 3 && !(lo <= n && n <= hi) {
            return false;
        }
    }
    let t = s(g, "t");
    if f.hide && (ctx.played_before(g) || ctx.marks_of(t).iter().any(|m| *m == "meh" || *m == "pass")) {
        return false;
    }
    if f.one && !b(g, "one") {
        return false;
    }
    if f.fresh && ctx.played_before(g) {
        return false;
    }
    if !f.platform.is_empty() && !s(g, "sys").eq_ignore_ascii_case(&f.platform) {
        return false;
    }
    if f.owned
        && let Some(all) = &ctx.all_own
        && !all.contains(t)
    {
        return false;
    }
    true
}

/// (games that fit the filters, a note). With two moods: games fitting both, else the second mood's.
pub fn matching<'a>(games: &'a [Json], f: &Filters, ctx: &Context) -> (Vec<&'a Json>, String) {
    let base: Vec<&Json> = games.iter().filter(|g| base_ok(g, f, ctx)).collect();
    if !f.two {
        return (base.into_iter().filter(|g| mood_ok(g, &f.a, f.match_any)).collect(), String::new());
    }
    let both: Vec<&Json> = base.iter().copied().filter(|g| mood_ok(g, &f.a, f.match_any) && mood_ok(g, &f.b, f.match_any)).collect();
    if !both.is_empty() || f.b.is_empty() {
        return (both, String::new());
    }
    (base.into_iter().filter(|g| mood_ok(g, &f.b, f.match_any)).collect(), "No game fits both moods, so the list follows the second one.".into())
}

/// Games eligible for the draw. Left out: "not for us" (unless Settings says rarer), today's vetoes, games that can't start right now.
pub fn pool<'a>(games: &'a [Json], f: &Filters, ctx: &Context) -> (Vec<&'a Json>, String) {
    let (m, note) = matching(games, f, ctx);
    (
        m.into_iter()
            .filter(|g| {
                let t = s(g, "t");
                (ctx.meh_soft || !ctx.anyone_meh(t)) && !ctx.vetoes.contains(t) && !ctx.unavailable.contains_key(t)
            })
            .collect(),
        note,
    )
}

pub fn skipped_for(games: &[Json], f: &Filters, ctx: &Context) -> Vec<Json> {
    matching(games, f, ctx).0.into_iter().filter_map(|g| ctx.unavailable.get(s(g, "t")).map(|r| json!({"title": s(g, "t"), "reason": r}))).collect()
}

pub fn rest_set(ctx: &Context, n: usize) -> HashSet<String> {
    let k = 3.min(n / 2);
    ctx.history.iter().rev().take(k).cloned().collect()
}

/// (key, multiplier, detail) for everything that changed this game's share. The weight is W_BASE times the product.
pub fn factors(g: &Json, ctx: &Context, rest: &HashSet<String>) -> Vec<(&'static str, f64, i64)> {
    let t = s(g, "t");
    let mut out = vec![];
    if ctx.played_before(g) {
        match ctx.days_since(g) {
            None => out.push(("played", W_PLAYED_UNKNOWN / W_BASE, -1)),
            Some(d) => out.push(("played", (W_PLAYED_MIN + W_PLAYED_SPAN * (d as f64 / W_PLAYED_DAYS).min(1.0)) / W_BASE, d)),
        }
    }
    let marks = ctx.marks_of(t);
    let count = |m: &str| marks.iter().filter(|x| **x == m).count() as i32;
    if count("loved") > 0 {
        out.push(("loved", W_LOVED.powi(count("loved")), count("loved") as i64));
    }
    if ctx.mark(t, &ctx.turn) == "loved" {
        out.push(("turn", W_TURN_LOVED, 0));
    }
    if count("curious") > 0 {
        out.push(("curious", W_CURIOUS.powi(count("curious")), count("curious") as i64));
    }
    if count("pass") > 0 {
        out.push(("pass", W_PASS.powi(count("pass")), count("pass") as i64));
    }
    if ctx.meh_soft && count("meh") > 0 {
        out.push(("meh", W_MEH.powi(count("meh")), count("meh") as i64));
    }
    if ctx.ongoing.contains(t) {
        out.push(("ongoing", W_ONGOING, 0));
    }
    if ctx.solo_rare && solo(g) {
        out.push(("solo", W_SOLO, 0));
    }
    if rest.contains(t) {
        out.push(("rest", W_REST, 0));
    }
    out
}

pub fn weight(g: &Json, ctx: &Context, rest: &HashSet<String>) -> f64 {
    factors(g, ctx, rest).iter().fold(W_BASE, |w, (_, m, _)| w * m)
}

/// The odds line plus the biggest reasons behind this game's share (largest effect first), from the same factors the weight used.
pub fn explain(g: &Json, ctx: &Context, prob: f64, n: usize, rest: &HashSet<String>, max_reasons: usize) -> Vec<String> {
    let mut out = vec![format!("Odds this spin: about {}% out of {n} possible games.", (prob * 100.0).round())];
    let mut fs = factors(g, ctx, rest);
    fs.sort_by(|a, b| b.1.ln().abs().total_cmp(&a.1.ln().abs()));
    for (key, mult, d) in fs.iter().take(max_reasons) {
        let line = match *key {
            "played" if *d < 0 => "You've played it before, so it had a smaller share.".to_string(),
            "played" => {
                let ago = match d {
                    0 => "today".into(),
                    1 => "yesterday".into(),
                    d => format!("{d} days ago"),
                };
                format!("Last played {ago}, which {} its share.", if (*d as f64) < W_PLAYED_DAYS { "lowers" } else { "no longer affects" })
            }
            "loved" if *d > 1 => format!("{d} of you loved it. That raised its share."),
            "curious" if *d > 1 => format!("{d} of you said it looks fun. That raised its share."),
            "pass" if *d > 1 => format!("{d} of you said it isn't really your thing. That lowered its share."),
            k => format!(
                "{}{}",
                table_text(k),
                if *mult > 1.0 {
                    " That raised its share."
                } else if *mult < 1.0 {
                    " That lowered its share."
                } else {
                    ""
                }
            ),
        };
        out.push(line);
    }
    if fs.is_empty() {
        out.push("Nothing changed its share: it was new and unmarked.".into());
    }
    out
}

fn draw_one<'a>(items: &[&'a Json], ws: &HashMap<String, f64>, rng: &mut dyn Rng) -> &'a Json {
    let total: f64 = items.iter().map(|g| ws[s(g, "t")]).sum();
    let mut r = rng.f64() * total;
    for g in items {
        r -= ws[s(g, "t")];
        if r < 0.0 {
            return g;
        }
    }
    items[items.len() - 1]
}

/// Weighted sample without replacement (Efraimidis-Spirakis).
fn weighted_sample<'a>(items: &[&'a Json], k: usize, ws: &HashMap<String, f64>, rng: &mut dyn Rng) -> Vec<&'a Json> {
    let mut keys: Vec<(f64, &Json)> = items.iter().map(|g| (-(rng.f64().max(1e-12)).ln() / ws[s(g, "t")], *g)).collect();
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.into_iter().take(k).map(|x| x.1).collect()
}

/// "Another" is another: leave out what this round already showed; once everything was shown, start over minus the last one.
pub fn without_seen<'a>(p: Vec<&'a Json>, seen: &[String]) -> (Vec<&'a Json>, usize) {
    let fresh: Vec<&Json> = p.iter().copied().filter(|g| !seen.iter().any(|x| x == s(g, "t"))).collect();
    if !fresh.is_empty() {
        let left = p.len() - fresh.len();
        return (fresh, left);
    }
    let last = seen.last().cloned().unwrap_or_default();
    let rest: Vec<&Json> = p.iter().copied().filter(|g| s(g, "t") != last).collect();
    if rest.is_empty() { (p, 0) } else { (rest, 1) }
}

fn weights_for(p: &[&Json], ctx: &Context) -> (HashMap<String, f64>, HashSet<String>) {
    let rest = rest_set(ctx, p.len());
    (p.iter().map(|g| (s(g, "t").to_string(), weight(g, ctx, &rest))).collect(), rest)
}

pub fn pick(games: &[Json], filters: &Value, ctx: &Context, rng: &mut dyn Rng, seen: &[String]) -> Json {
    let f = Filters::from_json(filters);
    let ctx = for_filters(ctx, &f);
    let (p, note) = pool(games, &f, &ctx);
    let skipped = skipped_for(games, &f, &ctx);
    if p.is_empty() {
        return json!({"ok": false, "message": "Nothing to pick from: no game matches the filters (or all were vetoed or can't be started). Loosen a filter or clear the vetoes.", "note": note, "skipped": skipped});
    }
    let (p, left_out) = if seen.is_empty() { (p, 0) } else { without_seen(p, &seen[seen.len().saturating_sub(60)..]) };
    let (ws, rest) = weights_for(&p, &ctx);
    let total: f64 = ws.values().sum();
    let winner = draw_one(&p, &ws, rng);
    let wt = s(winner, "t");
    let prob = ws[wt] / total;
    let others: Vec<&Json> = p.iter().copied().filter(|g| s(g, "t") != wt).collect();
    let mut wheel: Vec<&Json> = vec![winner];
    wheel.extend(weighted_sample(&others, WHEEL_SIZE - 1, &ws, rng));
    shuffle(&mut wheel, rng);
    let mut left = others.clone();
    let mut runners = vec![];
    for _ in 0..RUNNERS.min(left.len()) {
        let g = draw_one(&left, &ws, rng);
        runners.push(json!({"title": s(g, "t"), "prob": ws[s(g, "t")] / total}));
        left.retain(|x| s(x, "t") != s(g, "t"));
    }
    let mut reel: Vec<&str> = others.iter().map(|g| s(g, "t")).collect();
    shuffle(&mut reel, rng);
    reel.truncate(REEL_SIZE);
    let wheel_titles: Vec<&str> = wheel.iter().map(|g| s(g, "t")).collect();
    json!({
        "ok": true, "winner": wt, "index": wheel_titles.iter().position(|t| *t == wt), "wheel": wheel_titles,
        "land": (rng.f64() - 0.5) * 0.7, "turns": 5 + rng.below(3), "prob": prob, "pool": p.len(),
        "why": explain(winner, &ctx, prob, p.len(), &rest, 2), "note": note, "skipped": skipped, "runners": runners, "leftOut": left_out, "reel": reel,
    })
}

/// Three cards: k finalists drawn one after another without replacement, each with its share of the first draw.
pub fn finalists(games: &[Json], filters: &Value, ctx: &Context, k: usize, rng: &mut dyn Rng) -> Json {
    let f = Filters::from_json(filters);
    let ctx = for_filters(ctx, &f);
    let (p, note) = pool(games, &f, &ctx);
    if p.len() < 2 {
        return json!({"ok": false, "message": "Not enough games fit the filters for a round of three.", "note": note});
    }
    let (ws, rest) = weights_for(&p, &ctx);
    let total: f64 = ws.values().sum();
    let mut left = p.clone();
    let mut out = vec![];
    for _ in 0..k.min(left.len()) {
        let g = draw_one(&left, &ws, rng);
        let prob = ws[s(g, "t")] / total;
        out.push(json!({"title": s(g, "t"), "prob": prob, "why": explain(g, &ctx, prob, p.len(), &rest, 1)[1..].to_vec()}));
        left.retain(|x| s(x, "t") != s(g, "t"));
    }
    json!({"ok": true, "pool": p.len(), "note": note, "finalists": out})
}

pub fn preview_wheel(games: &[Json], filters: &Value, ctx: &Context, rng: &mut dyn Rng) -> Vec<String> {
    let f = Filters::from_json(filters);
    let ctx = for_filters(ctx, &f);
    let (p, _) = pool(games, &f, &ctx);
    let (ws, _) = weights_for(&p, &ctx);
    weighted_sample(&p, WHEEL_SIZE, &ws, rng).iter().map(|g| s(g, "t").to_string()).collect()
}

/// A short evening: a quick warm-up, a main game and a lighter wind-down, three different games drawn with the usual weights.
#[allow(clippy::type_complexity)]
pub fn plan_evening(games: &[Json], filters: &Value, ctx: &Context, rng: &mut dyn Rng) -> Json {
    let f = Filters::from_json(filters);
    let ctx = for_filters(ctx, &f);
    let (p, note) = pool(games, &f, &ctx);
    if p.len() < 2 {
        return json!({"ok": false, "message": "Not enough games fit the filters to plan an evening.", "note": note});
    }
    let (ws, _) = weights_for(&p, &ctx);
    let mut chosen: Vec<String> = vec![];
    let mut steps = vec![];
    let tests: [(&str, fn(&Json) -> bool); 3] = [
        ("Warm-up", |g| s(g, "len") == "quick"),
        ("Main game", |g| matches!(s(g, "len"), "evening" | "long")),
        ("Wind-down", |g| s(g, "len") != "long" && (s(g, "len") == "quick" || strs(g, "vibe").iter().any(|v| v == "cozy" || v == "silly"))),
    ];
    for (label, test) in tests {
        let mut cands: Vec<&Json> = p.iter().copied().filter(|g| test(g) && !chosen.iter().any(|c| c == s(g, "t"))).collect();
        if cands.is_empty() {
            cands = p.iter().copied().filter(|g| !chosen.iter().any(|c| c == s(g, "t"))).collect();
        }
        if !cands.is_empty() {
            let g = draw_one(&cands, &ws, rng);
            chosen.push(s(g, "t").into());
            steps.push(json!({"step": label, "title": s(g, "t")}));
        }
    }
    json!({"ok": true, "steps": steps, "note": note})
}

/// Games for a quick "does this look fun?" round: ones this player hasn't marked, spread across lengths and couch/online.
pub fn taste_set(games: &[Json], marks: &Json, pid: &str, n: usize, rng: &mut dyn Rng) -> Vec<String> {
    let mut fresh: Vec<&Json> = games.iter().filter(|g| marks.get(s(g, "t")).and_then(|m| m.get(pid)).is_none()).collect();
    shuffle(&mut fresh, rng);
    let mut buckets: Vec<((String, String), Vec<&Json>)> = vec![];
    for g in fresh {
        let k = (s(g, "len").to_string(), s(g, "where").to_string());
        match buckets.iter_mut().find(|b| b.0 == k) {
            Some(b) => b.1.push(g),
            None => buckets.push((k, vec![g])),
        }
    }
    buckets.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    let mut out = vec![];
    while out.len() < n && buckets.iter().any(|b| !b.1.is_empty()) {
        for b in buckets.iter_mut() {
            if out.len() < n
                && let Some(g) = b.1.pop()
            {
                out.push(s(g, "t").to_string());
            }
        }
    }
    shuffle(&mut out, rng);
    out
}

/// Games most like this one: shared moods, same mode and place, a close skill gap. Deterministic.
pub fn similar(games: &[Json], title: &str, n: usize) -> Vec<String> {
    let Some(base) = games.iter().find(|g| s(g, "t") == title) else { return vec![] };
    let bv: HashSet<String> = strs(base, "vibe").into_iter().collect();
    let mut scored: Vec<(f64, String)> = games
        .iter()
        .filter(|g| s(g, "t") != title)
        .map(|g| {
            let gv: HashSet<String> = strs(g, "vibe").into_iter().collect();
            let j = bv.intersection(&gv).count() as f64 / (bv.union(&gv).count().max(1)) as f64;
            let score = 3.0 * j
                + f64::from(s(g, "mode") == s(base, "mode"))
                + f64::from(s(g, "where") == s(base, "where"))
                + (1.0 - (i(g, "gap") - i(base, "gap")).abs() as f64 / 4.0);
            (-score, s(g, "t").to_string())
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(n).map(|x| x.1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests::game;
    use crate::util::SeedRng;

    const N: usize = 30000;

    fn ctx() -> Context {
        Context::new(vec!["p1".into(), "p2".into(), "p3".into()], NaiveDate::from_ymd_opt(2026, 10, 21).unwrap())
    }

    fn g(t: &str, kv: &[(&str, Value)]) -> Json {
        let mut x = game(t);
        for (k, v) in kv {
            x[*k] = v.clone();
        }
        x
    }

    fn games() -> Vec<Json> {
        let mut v: Vec<Json> =
            ["Loved", "Plain", "Pass", "Played", "Rested", "Ongoing", "Curious", "Other", "Group"].iter().map(|t| g(t, &[("len", json!("evening"))])).collect();
        v.push(g("Quick", &[]));
        v
    }

    fn busy() -> Context {
        let mut c = ctx();
        c.marks = json!({"Loved": {"p1": "loved", "p2": "loved"}, "Pass": {"p3": "pass"}, "Curious": {"p2": "curious"}, "Group": {"p1": "loved", "p2": "loved", "p3": "loved"}});
        c.plays.insert("Played".into(), Play { count: 3, last: Some("2026-10-19".into()) });
        c.history = vec!["Rested".into()];
        c.ongoing.insert("Ongoing".into());
        c
    }

    fn tally(filters: Value, seen: &[String], seed: u64, c: &Context, gs: &[Json]) -> (HashMap<String, usize>, HashMap<String, f64>) {
        let mut rng = SeedRng(seed);
        let (mut wins, mut prob) = (HashMap::new(), HashMap::new());
        for _ in 0..N {
            let r = pick(gs, &filters, c, &mut rng, seen);
            let w = s(&r, "winner").to_string();
            *wins.entry(w.clone()).or_insert(0) += 1;
            prob.insert(w, r["prob"].as_f64().unwrap());
        }
        (wins, prob)
    }

    fn check(wins: &HashMap<String, usize>, prob: &HashMap<String, f64>) {
        assert!((prob.values().sum::<f64>() - 1.0).abs() < 1e-9, "the reported shares must add up to 100%");
        for (t, p) in prob {
            let sigma = (p * (1.0 - p) / N as f64).sqrt();
            let got = wins[t] as f64 / N as f64;
            assert!((got - p).abs() < 4.5 * sigma + 1e-4, "{t}: reported {p:.4}, drew {got:.4}");
        }
    }

    #[test]
    fn reported_odds_are_real_odds() {
        let (w, p) = tally(json!({}), &[], 11, &busy(), &games());
        check(&w, &p);
    }

    #[test]
    fn after_another_the_odds_are_renormalised_and_still_true() {
        let (w, p) = tally(json!({}), &["Loved".into(), "Plain".into()], 12, &busy(), &games());
        assert!(!p.contains_key("Loved") && !p.contains_key("Plain"));
        check(&w, &p);
    }

    #[test]
    fn shares_follow_the_documented_multipliers() {
        let c = busy();
        let gs = games();
        let rest = rest_set(&c, gs.len());
        let w: HashMap<&str, f64> = gs.iter().map(|x| (s(x, "t"), weight(x, &c, &rest))).collect();
        let close = |a: f64, b: f64| assert!((a - b).abs() < 1e-9, "{a} != {b}");
        close(w["Plain"], W_BASE);
        close(w["Loved"], W_BASE * W_LOVED * W_LOVED * W_TURN_LOVED);
        close(w["Group"], W_BASE * W_LOVED.powi(3) * W_TURN_LOVED);
        close(w["Pass"], W_BASE * W_PASS);
        close(w["Curious"], W_BASE * W_CURIOUS);
        close(w["Ongoing"], W_BASE * W_ONGOING);
        close(w["Rested"], W_BASE * W_REST);
        assert!(w["Played"] < W_BASE && w["Played"] > W_PLAYED_MIN * 0.99);
    }

    #[test]
    fn not_for_us_leaves_out_or_lowers() {
        let mut c = ctx();
        c.marks = json!({"A": {"p3": "meh"}});
        let gs = vec![game("A"), game("B")];
        let f = Filters::from_json(&json!({}));
        assert_eq!(pool(&gs, &f, &c).0.len(), 1);
        c.meh_soft = true;
        assert_eq!(pool(&gs, &f, &c).0.len(), 2);
        assert!((weight(&gs[0], &c, &HashSet::new()) - W_BASE * W_MEH).abs() < 1e-9);
    }

    #[test]
    fn one_player_games_are_rarer_unless_asked_for() {
        let gs = vec![
            g("Solo", &[("p", json!("1")), ("one", json!(true)), ("vibe", json!(["spooky"]))]),
            g("Pair", &[("p", json!("1-2")), ("one", json!(true)), ("vibe", json!(["spooky"]))]),
        ];
        let (w, p) = tally(json!({}), &[], 4, &ctx(), &gs);
        let share = W_SOLO / (1.0 + W_SOLO);
        assert!((p["Solo"] - share).abs() < 1e-9);
        check(&w, &p);
        for f in [json!({"one": true}), json!({"A": ["spooky"]}), json!({"two": true, "A": ["silly"], "B": ["spooky"], "match": "any"})] {
            let c = for_filters(&ctx(), &Filters::from_json(&f));
            assert!((weight(&gs[0], &c, &HashSet::new()) - W_BASE).abs() < 1e-9, "{f}");
        }
    }

    #[test]
    fn filters_do_what_they_say() {
        let gs = vec![
            g("Couch4", &[("p", json!("1-4"))]),
            g("Online", &[("where", json!("online")), ("p", json!("2-8")), ("sys", json!("Switch"))]),
            g("Two", &[("p", json!("2")), ("len", json!("evening"))]),
        ];
        let c = ctx();
        let titles = |f: Value| -> Vec<String> { pool(&gs, &Filters::from_json(&f), &c).0.iter().map(|x| s(x, "t").to_string()).collect() };
        assert_eq!(titles(json!({"where": "online"})), vec!["Online"]);
        assert_eq!(titles(json!({"n": 2})), vec!["Couch4", "Online", "Two"]);
        assert_eq!(titles(json!({"n": 5})), vec!["Online"], "room for five");
        assert_eq!(titles(json!({"len": "evening"})), vec!["Two"]);
        assert_eq!(titles(json!({"platform": "switch"})), vec!["Online"]);
        assert_eq!(titles(json!({"n": 99})).len(), 3, "a nonsense player count is ignored");
    }

    #[test]
    fn the_wheel_reel_and_runners_are_sound() {
        let mut rng = SeedRng(3);
        for _ in 0..300 {
            let r = pick(&games(), &json!({}), &busy(), &mut rng, &[]);
            let wheel: Vec<&str> = r["wheel"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
            assert_eq!(wheel[r["index"].as_u64().unwrap() as usize], s(&r, "winner"));
            assert_eq!(wheel.iter().collect::<HashSet<_>>().len(), wheel.len());
            let reel: Vec<&str> = r["reel"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
            assert!(!reel.contains(&s(&r, "winner")));
            assert_eq!(reel.iter().collect::<HashSet<_>>().len(), reel.len());
        }
    }

    #[test]
    fn finalists_first_draw_follows_the_weights() {
        let c = busy();
        let gs = games();
        let f = Filters::from_json(&json!({}));
        let c2 = for_filters(&c, &f);
        let (p, _) = pool(&gs, &f, &c2);
        let (ws, _) = weights_for(&p, &c2);
        let total: f64 = ws.values().sum();
        let mut rng = SeedRng(5);
        let mut first: HashMap<String, usize> = HashMap::new();
        for _ in 0..N {
            let r = finalists(&gs, &json!({}), &c, 3, &mut rng);
            let fin = r["finalists"].as_array().unwrap();
            assert_eq!(fin.iter().map(|x| s(x, "title")).collect::<HashSet<_>>().len(), 3);
            *first.entry(s(&fin[0], "title").into()).or_insert(0) += 1;
        }
        for (t, w) in ws {
            let share = w / total;
            let got = *first.get(&t).unwrap_or(&0) as f64 / N as f64;
            assert!((got - share).abs() < 4.5 * (share * (1.0 - share) / N as f64).sqrt() + 1e-4, "{t}");
        }
    }

    #[test]
    fn explanation_names_the_biggest_reasons() {
        let mut c = ctx();
        c.marks = json!({"X": {"p1": "loved", "p2": "loved"}});
        c.ongoing.insert("X".into());
        c.turn = "p3".into();
        let why = explain(&game("X"), &c, 0.3, 10, &HashSet::new(), 2);
        assert_eq!(why.len(), 3);
        assert!(why[1].contains("2 of you loved it"), "{why:?}");
        assert!(why[2].contains("in progress"), "{why:?}");
    }

    #[test]
    fn another_leaves_out_what_was_shown() {
        let gs = games();
        let refs: Vec<&Json> = gs.iter().collect();
        let all: Vec<String> = gs.iter().map(|g| s(g, "t").to_string()).collect();
        let (p, n) = without_seen(refs.clone(), &all[..3]);
        assert_eq!((p.len(), n), (gs.len() - 3, 3));
        let (p, n) = without_seen(refs, &all);
        assert_eq!((p.len(), n), (gs.len() - 1, 1));
        assert!(!p.iter().any(|g| s(g, "t") == all.last().unwrap()));
    }

    #[test]
    fn taste_set_and_similar() {
        let gs = games();
        let mut rng = SeedRng(9);
        let t = taste_set(&gs, &json!({"Plain": {"p1": "loved"}}), "p1", 5, &mut rng);
        assert_eq!(t.len(), 5);
        assert!(!t.contains(&"Plain".to_string()));
        assert_eq!(similar(&gs, "Plain", 3).len(), 3);
        assert!(similar(&gs, "Nope", 3).is_empty());
    }

    #[test]
    fn plan_has_three_different_games() {
        let mut rng = SeedRng(2);
        let r = plan_evening(&games(), &json!({}), &busy(), &mut rng);
        let steps = r["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps.iter().map(|x| s(x, "title")).collect::<HashSet<_>>().len(), 3);
        assert_eq!(s(&steps[0], "title"), "Quick");
    }
}
