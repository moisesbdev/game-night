//! Looking back. The journal of evenings (nights), "Our year", the memories screen, and what the group likes (taste).
//! All pure: records in, facts out.
//!
//! A "night" is a calendar evening; anything before 05:00 belongs to the evening before. Per night each game appears once:
//! minutes add up, the last ratings and outcome win, and who chose it comes from the confirmed pick.

use crate::util::{Json, parse_stamp, s, strs};
use chrono::{Datelike, Duration, NaiveDate};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};

const CUTOFF_HOUR: u32 = 5;
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

pub fn night_of(stamp: &str) -> Option<String> {
    let dt = parse_stamp(stamp)?;
    let d = if chrono::Timelike::hour(&dt) < CUTOFF_HOUR { dt.date() - Duration::days(1) } else { dt.date() };
    Some(d.format("%Y-%m-%d").to_string())
}

/// Nights, newest first, each `{date, n, games: [{title, minutes, sessions, ratings, outcome, chose, source}], minutes, chose: {pid: n}, finished}`.
pub fn nights(sessions: &[Json], history: &[Json]) -> (Vec<Json>, Json) {
    let mut by: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    fn slot(by: &mut BTreeMap<String, Vec<Json>>, night: String, title: &str) -> (String, usize) {
        let games = by.entry(night.clone()).or_default();
        let i = games.iter().position(|g| s(g, "title") == title).unwrap_or_else(|| {
            games.push(
                json!({"title": title, "minutes": null, "sessions": 0, "ratings": {}, "outcome": null, "chose": null, "source": "confirmed", "start": ""}),
            );
            games.len() - 1
        });
        (night, i)
    }
    let mut sorted: Vec<&Json> = sessions.iter().collect();
    sorted.sort_by(|a, b| s(a, "start").cmp(s(b, "start")));
    let mut edits: Vec<(String, usize, Json)> = vec![];
    for x in sorted {
        let ok = !x["end"].is_null() && !x["failed"].as_bool().unwrap_or(false) && x["minutes"].as_i64().unwrap_or(0) >= crate::sessions::FAILED_UNDER_MINUTES;
        if let (true, Some(n)) = (ok, night_of(s(x, "start"))) {
            let (n, i) = slot(&mut by, n, s(x, "title"));
            edits.push((n, i, x.clone()));
        }
    }
    for (n, i, x) in edits {
        let g = &mut by.get_mut(&n).unwrap()[i];
        g["minutes"] = json!(g["minutes"].as_i64().unwrap_or(0) + x["minutes"].as_i64().unwrap_or(0));
        g["sessions"] = json!(g["sessions"].as_i64().unwrap_or(0) + 1);
        g["source"] = json!("played");
        if s(g, "start").is_empty() {
            g["start"] = json!(s(&x, "start"));
        }
        if x["ratings"].as_object().is_some_and(|o| !o.is_empty()) {
            g["ratings"] = x["ratings"].clone();
        }
        if !x["outcome"].is_null() {
            g["outcome"] = x["outcome"].clone();
        }
    }
    let mut picks: Vec<(String, usize, Json)> = vec![];
    for h in history {
        if let Some(n) = night_of(s(h, "at")) {
            let (n, i) = slot(&mut by, n, s(h, "title"));
            picks.push((n, i, h.clone()));
        }
    }
    for (n, i, h) in picks {
        let g = &mut by.get_mut(&n).unwrap()[i];
        if !s(&h, "chose").is_empty() {
            g["chose"] = json!(s(&h, "chose"));
        }
        if s(g, "start").is_empty() {
            g["start"] = json!(s(&h, "at"));
        }
    }
    let mut out: Vec<Json> = vec![];
    for (num, (date, mut games)) in by.into_iter().enumerate() {
        games.sort_by(|a, b| s(a, "start").cmp(s(b, "start")));
        let mut chose: BTreeMap<String, i64> = BTreeMap::new();
        for g in &games {
            if let Some(c) = g["chose"].as_str() {
                *chose.entry(c.into()).or_default() += 1;
            }
        }
        let minutes: i64 = games.iter().filter_map(|g| g["minutes"].as_i64()).sum();
        let finished: Vec<String> = games.iter().filter(|g| s(g, "outcome") == "finished").map(|g| s(g, "title").to_string()).collect();
        for g in games.iter_mut() {
            g.as_object_mut().unwrap().remove("start");
        }
        out.push(json!({"date": date, "n": num + 1, "games": games, "minutes": minutes, "chose": chose, "finished": finished}));
    }
    out.reverse();
    let every: HashSet<&str> = out.iter().flat_map(|n| n["games"].as_array().unwrap().iter().map(|g| s(g, "title"))).collect();
    let stats = json!({"nights": out.len(), "minutes": out.iter().map(|n| n["minutes"].as_i64().unwrap_or(0)).sum::<i64>(), "games": every.len(), "finished": out.iter().map(|n| n["finished"].as_array().unwrap().len()).sum::<usize>()});
    (out, stats)
}

fn liked_by_all(g: &Json, players: &[String]) -> bool {
    let r = &g["ratings"];
    players.len() > 1 && players.iter().all(|p| s(r, p) == "up")
}

/// One card per catalog game per night, newest first, for the memories screen.
pub fn memories(nights: &[Json], in_catalog: &dyn Fn(&str) -> bool, players: &[String], limit: usize) -> Vec<Json> {
    let mut out = vec![];
    for n in nights {
        for g in n["games"].as_array().into_iter().flatten() {
            if !in_catalog(s(g, "title")) {
                continue;
            }
            out.push(json!({"title": s(g, "title"), "date": s(n, "date"), "n": n["n"], "minutes": g["minutes"], "outcome": g["outcome"], "allLiked": liked_by_all(g, players), "chose": g["chose"]}));
            if out.len() >= limit {
                return out;
            }
        }
    }
    out
}

fn weeks_in_a_row(dates: &[NaiveDate]) -> usize {
    let mut weeks: Vec<NaiveDate> = dates.iter().map(|d| *d - Duration::days(d.weekday().num_days_from_monday() as i64)).collect();
    weeks.sort();
    weeks.dedup();
    let (mut best, mut run) = (0, 0);
    for (i, w) in weeks.iter().enumerate() {
        run = if i > 0 && *w - weeks[i - 1] == Duration::days(7) { run + 1 } else { 1 };
        best = best.max(run);
    }
    best
}

/// Everything "Our year" shows for calendar year `y`.
#[allow(clippy::type_complexity)] // the year's tallies, declared together
pub fn year(nights: &[Json], vibes_of: &dyn Fn(&str) -> Vec<String>, players: &[String], y: i32) -> Json {
    let mut years: Vec<i32> = nights.iter().filter_map(|n| s(n, "date").get(..4)?.parse().ok()).collect();
    years.sort_by(|a, b| b.cmp(a));
    years.dedup();
    let prefix = format!("{y:04}-");
    let mut mine: Vec<&Json> = nights.iter().filter(|n| s(n, "date").starts_with(&prefix)).collect();
    mine.sort_by(|a, b| s(a, "date").cmp(s(b, "date")));
    if mine.is_empty() {
        return json!({"year": y, "years": years, "nights": 0});
    }
    let mut first_seen: HashMap<&str, &str> = HashMap::new();
    let mut all: Vec<&Json> = nights.iter().collect();
    all.sort_by(|a, b| s(a, "date").cmp(s(b, "date")));
    for n in &all {
        for g in n["games"].as_array().unwrap() {
            first_seen.entry(s(g, "title")).or_insert(s(n, "date"));
        }
    }
    let (mut per_nights, mut per_minutes, mut chose, mut moods, mut months): (
        BTreeMap<&str, i64>,
        HashMap<&str, i64>,
        BTreeMap<String, i64>,
        HashMap<String, i64>,
        BTreeMap<u32, i64>,
    ) = Default::default();
    let (mut order, mut finished, mut all_liked): (Vec<&str>, Vec<&str>, Vec<&str>) = Default::default();
    for n in &mine {
        *months.entry(s(n, "date")[5..7].parse().unwrap_or(1)).or_default() += 1;
        for g in n["games"].as_array().unwrap() {
            let t = s(g, "title");
            if !per_nights.contains_key(t) {
                order.push(t);
            }
            *per_nights.entry(t).or_default() += 1;
            *per_minutes.entry(t).or_default() += g["minutes"].as_i64().unwrap_or(0);
            if let Some(c) = g["chose"].as_str() {
                *chose.entry(c.into()).or_default() += 1;
            }
            if s(g, "outcome") == "finished" && !finished.contains(&t) {
                finished.push(t);
            }
            if liked_by_all(g, players) && !all_liked.contains(&t) {
                all_liked.push(t);
            }
            for v in vibes_of(t) {
                *moods.entry(v).or_default() += 1;
            }
        }
    }
    let mut top: Vec<&str> = per_nights.keys().copied().collect();
    top.sort_by(|a, b| per_nights[b].cmp(&per_nights[a]).then(per_minutes[b].cmp(&per_minutes[a])).then(a.cmp(b)));
    let mut mood_list: Vec<(String, i64)> = moods.into_iter().collect();
    mood_list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let busiest = months.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(m, n)| (*m, *n)).unwrap_or((1, 0));
    let mut new: Vec<&str> = order.iter().copied().filter(|t| first_seen.get(t).is_some_and(|d| d.starts_with(&prefix))).collect();
    new.sort_by(|a, b| first_seen[a].cmp(first_seen[b]));
    let dates: Vec<NaiveDate> = mine.iter().filter_map(|n| crate::util::parse_date(s(n, "date"))).collect();
    let titles = |n: &Json| -> Vec<String> { n["games"].as_array().unwrap().iter().map(|g| s(g, "title").to_string()).collect() };
    json!({
        "year": y, "years": years, "nights": mine.len(), "minutes": mine.iter().map(|n| n["minutes"].as_i64().unwrap_or(0)).sum::<i64>(), "games": per_nights.len(),
        "top": top.iter().take(3).map(|t| json!({"title": t, "nights": per_nights[t], "minutes": per_minutes[t]})).collect::<Vec<_>>(),
        "new": new, "finished": finished, "allLiked": all_liked, "chose": chose,
        "moods": mood_list.iter().take(3).map(|(v, n)| json!({"vibe": v, "n": n})).collect::<Vec<_>>(),
        "busiest": {"month": MONTHS[busiest.0 as usize - 1], "nights": busiest.1}, "weeksInARow": weeks_in_a_row(&dates),
        "first": {"date": s(mine[0], "date"), "titles": titles(mine[0])}, "last": {"date": s(mine[mine.len() - 1], "date"), "titles": titles(mine[mine.len() - 1])},
    })
}

/// What each player tends to like, what everyone likes, where opinions split, and untried games that match.
pub fn taste(games: &[Json], marks: &Json, players: &[String], playable: Option<&HashSet<String>>) -> Json {
    const LIKE: [&str; 2] = ["loved", "curious"];
    const DISLIKE: [&str; 2] = ["pass", "meh"];
    let by: HashMap<&str, &Json> = games.iter().map(|g| (s(g, "t"), g)).collect();
    let mark = |t: &str, p: &str| -> String { s(&marks[t], p).to_string() };
    let known: Vec<&str> = marks.as_object().into_iter().flatten().map(|(t, _)| t.as_str()).filter(|t| by.contains_key(t)).collect();
    let marked = known
        .iter()
        .filter(|t| {
            players.iter().any(|p| {
                let m = mark(t, p);
                LIKE.contains(&m.as_str()) || DISLIKE.contains(&m.as_str())
            })
        })
        .count();
    let mut weights: HashMap<String, i64> = HashMap::new();
    let people: Vec<Json> = players
        .iter()
        .map(|p| {
            let liked: Vec<&str> = known.iter().copied().filter(|t| LIKE.contains(&mark(t, p).as_str())).collect();
            let passed = known.iter().filter(|t| DISLIKE.contains(&mark(t, p).as_str())).count();
            let mut v: HashMap<String, i64> = HashMap::new();
            for t in &liked {
                for x in strs(by[t], "vibe") {
                    *v.entry(x).or_default() += 1;
                }
            }
            let mut vs: Vec<(String, i64)> = v.into_iter().collect();
            vs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            vs.truncate(4);
            for (x, n) in &vs {
                *weights.entry(x.clone()).or_default() += n;
            }
            json!({"id": p, "liked": liked.len(), "passed": passed, "vibes": vs.iter().map(|(x, n)| json!({"vibe": x, "n": n})).collect::<Vec<_>>()})
        })
        .collect();
    let mut all: Vec<&str> = known.iter().copied().filter(|t| players.len() > 1 && players.iter().all(|p| LIKE.contains(&mark(t, p).as_str()))).collect();
    all.sort();
    for t in &all {
        for x in strs(by[t], "vibe") {
            *weights.entry(x).or_default() += 2;
        }
    }
    let mut split: Vec<Json> = known
        .iter()
        .filter(|t| players.iter().any(|p| LIKE.contains(&mark(t, p).as_str())) && players.iter().any(|p| DISLIKE.contains(&mark(t, p).as_str())))
        .map(|t| json!({"title": t, "marks": marks[*t]}))
        .collect();
    split.sort_by(|a, b| s(a, "title").cmp(s(b, "title")));
    let mut try_next: Vec<(i64, String, Vec<String>)> = games
        .iter()
        .filter(|g| {
            let t = s(g, "t");
            !players.iter().any(|p| !mark(t, p).is_empty()) && playable.is_none_or(|pl| pl.contains(t))
        })
        .filter_map(|g| {
            let mut shared: Vec<String> = strs(g, "vibe").into_iter().filter(|v| weights.get(v).copied().unwrap_or(0) > 0).collect();
            let score: i64 = shared.iter().map(|v| weights[v]).sum();
            shared.sort_by(|a, b| weights[b].cmp(&weights[a]).then(a.cmp(b)));
            (score > 0).then(|| (score, s(g, "t").to_string(), shared))
        })
        .collect();
    try_next.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    json!({"enough": marked >= 3, "marked": marked, "people": people, "all": all, "split": split,
           "tryNext": try_next.iter().take(5).map(|(sc, t, m)| json!({"title": t, "score": sc, "matches": m})).collect::<Vec<_>>()})
}

/// Turn a nights list into play counts and last dates per title (feeds the picker's "played recently").
pub fn plays_from(sessions_dates: &HashMap<String, (i64, String)>) -> HashMap<String, crate::picker::Play> {
    sessions_dates.iter().map(|(t, (n, d))| (t.clone(), crate::picker::Play { count: *n, last: Some(d.clone()).filter(|d| !d.is_empty()) })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sess(title: &str, start: &str, minutes: i64, ratings: Value, outcome: Value) -> Json {
        json!({"id": "x", "title": title, "start": start, "end": start, "minutes": minutes, "failed": false, "ratings": ratings, "outcome": outcome})
    }

    fn sample() -> Vec<Json> {
        let ses = vec![
            sess("Spooky", "2026-03-10T21:00:00", 50, json!({"p1": "up", "p2": "up"}), json!("finished")),
            sess("Cozy", "2026-03-03T20:00:00", 40, json!({}), json!("again")),
            sess("Cozy", "2026-02-24T20:00:00", 30, json!({"p1": "up"}), Value::Null),
            sess("Silly", "2026-02-25T01:30:00", 20, json!({"p1": "up", "p2": "up"}), Value::Null),
            sess("Silly", "2025-12-31T20:00:00", 25, json!({}), Value::Null),
            sess("Failed", "2026-03-10T22:00:00", 1, json!({}), Value::Null),
        ];
        let hist =
            vec![json!({"title": "Spooky", "at": "2026-03-10T20:55:00", "chose": "p2"}), json!({"title": "Cozy", "at": "2026-03-03T19:55:00", "chose": "p1"})];
        nights(&ses, &hist).0
    }

    #[test]
    fn nights_group_by_evening() {
        let n = sample();
        assert_eq!(n.len(), 4);
        assert_eq!(s(&n[0], "date"), "2026-03-10");
        assert_eq!(n[0]["games"].as_array().unwrap().len(), 1, "the one-minute failed launch isn't a night's game");
        let feb = n.iter().find(|x| s(x, "date") == "2026-02-24").unwrap();
        assert_eq!(feb["games"].as_array().unwrap().len(), 2, "01:30 belongs to the evening before");
        assert_eq!(n[3]["n"], 1);
        assert_eq!(n[0]["chose"]["p2"], 1);
    }

    #[test]
    fn our_year() {
        let vibes = |t: &str| -> Vec<String> {
            match t {
                "Cozy" => vec!["cozy".into()],
                "Spooky" => vec!["spooky".into(), "story".into()],
                "Silly" => vec!["silly".into(), "cozy".into()],
                _ => vec![],
            }
        };
        let players = vec!["p1".to_string(), "p2".into()];
        let y = year(&sample(), &vibes, &players, 2026);
        assert_eq!((y["nights"].as_i64(), y["minutes"].as_i64(), y["games"].as_i64()), (Some(3), Some(140), Some(3)));
        assert_eq!(y["years"], json!([2026, 2025]));
        assert_eq!(s(&y["top"][0], "title"), "Cozy");
        assert_eq!(y["new"], json!(["Cozy", "Spooky"]));
        assert_eq!(y["finished"], json!(["Spooky"]));
        assert_eq!(y["allLiked"], json!(["Silly", "Spooky"]));
        assert_eq!(y["moods"][0], json!({"vibe": "cozy", "n": 3}));
        assert_eq!(y["busiest"], json!({"month": "March", "nights": 2}));
        assert_eq!(y["weeksInARow"], 3);
        assert_eq!(year(&sample(), &vibes, &players, 2024)["nights"], 0);
    }

    #[test]
    fn weeks_cross_the_new_year() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        assert_eq!(weeks_in_a_row(&[d("2025-12-24"), d("2025-12-31"), d("2026-01-07"), d("2026-02-01")]), 3);
    }

    #[test]
    fn memories_only_for_catalog_games() {
        let m = memories(&sample(), &|t| t != "Silly", &["p1".into(), "p2".into()], 40);
        assert_eq!(m.iter().map(|x| s(x, "title")).collect::<Vec<_>>(), vec!["Spooky", "Cozy", "Cozy"]);
        assert!(m[0]["allLiked"].as_bool().unwrap());
    }

    #[test]
    fn taste_for_a_group() {
        let mk = |t: &str, v: &[&str]| json!({"t": t, "vibe": v});
        let games = vec![mk("A", &["cozy"]), mk("B", &["cozy", "silly"]), mk("C", &["action"]), mk("D", &["cozy"])];
        let marks = json!({"A": {"p1": "loved", "p2": "curious", "p3": "loved"}, "B": {"p1": "loved", "p2": "meh"}, "C": {"p3": "pass"}});
        let players = vec!["p1".to_string(), "p2".into(), "p3".into()];
        let t = taste(&games, &marks, &players, None);
        assert!(t["enough"].as_bool().unwrap());
        assert_eq!(t["all"], json!(["A"]));
        assert_eq!(s(&t["split"][0], "title"), "B");
        assert_eq!(s(&t["tryNext"][0], "title"), "D");
        assert_eq!(t["people"].as_array().unwrap().len(), 3);
    }
}
