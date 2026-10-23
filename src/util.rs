//! Small shared pieces: atomic JSON files, local time as ISO text, and the random source.

use chrono::{Local, NaiveDate, NaiveDateTime};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub type Json = Value;

/// Write JSON so a crash can never leave half a file: temp file, flush, sync, rename over the old one.
pub fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(serde_json::to_string_pretty(value).unwrap_or_default().as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Read JSON written by anything (a byte-order mark is tolerated). `Ok(None)` when the file doesn't exist.
pub fn read_json(path: &Path) -> Result<Option<Value>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
            serde_json::from_slice(b).map(Some).map_err(|e| e.to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Keep one copy per day of a file in `<dir>/backups`, and only the newest `keep` of them (only ever our own files).
pub fn daily_backup(path: &Path, prefix: &str, keep: usize) {
    if !path.exists() {
        return;
    }
    let dir = path.parent().unwrap_or(Path::new(".")).join("backups");
    let target = dir.join(format!("{prefix}-{}.json", today()));
    if target.exists() || fs::create_dir_all(&dir).is_err() || fs::copy(path, &target).is_err() {
        return;
    }
    let mut mine: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".json")))
                .collect()
        })
        .unwrap_or_default();
    mine.sort();
    if mine.len() > keep {
        for p in &mine[..mine.len() - keep] {
            let _ = fs::remove_file(p);
        }
    }
}

pub fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn today_date() -> NaiveDate {
    Local::now().date_naive()
}

pub fn parse_stamp(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s.get(..19)?, "%Y-%m-%dT%H:%M:%S").ok()
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()
}

/// Minutes between two ISO stamps (never negative).
pub fn minutes_between(a: &str, b: &str) -> i64 {
    match (parse_stamp(a), parse_stamp(b)) {
        (Some(x), Some(y)) => ((y - x).num_seconds().max(0) as f64 / 60.0).round() as i64,
        _ => 0,
    }
}

/// Where randomness comes from. The real draw uses the operating system's secure generator; tests use a seeded one.
pub trait Rng {
    fn next_u64(&mut self) -> u64;
    /// Uniform in [0, 1).
    fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform in 0..n (n > 0), without modulo bias.
    fn below(&mut self, n: usize) -> usize {
        let n = n as u64;
        let zone = u64::MAX - u64::MAX % n;
        loop {
            let x = self.next_u64();
            if x < zone {
                return (x % n) as usize;
            }
        }
    }
}

/// The operating system's secure random source.
pub struct OsRng;
impl Rng for OsRng {
    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        getrandom::fill(&mut b).expect("the operating system's random source is unavailable");
        u64::from_le_bytes(b)
    }
}

/// SplitMix64: small, fast and fully determined by its seed (for tests and statistics).
#[cfg(test)]
pub struct SeedRng(pub u64);
#[cfg(test)]
impl Rng for SeedRng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Fisher-Yates.
pub fn shuffle<T>(v: &mut [T], rng: &mut dyn Rng) {
    for i in (1..v.len()).rev() {
        let j = rng.below(i + 1);
        v.swap(i, j);
    }
}

// ---- reading loosely typed JSON (games are plain JSON objects, validated by catalog.rs)

pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
pub fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}
pub fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}
pub fn strs(v: &Value, k: &str) -> Vec<String> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// A URL-safe, lower-case name for a title ("Overcooked! 2" -> "overcooked-2"), used for media file names.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Overcooked! 2"), "overcooked-2");
        assert_eq!(slug("  It Takes Two "), "it-takes-two");
        assert_eq!(slug("Pokémon Let's Go"), "pok-mon-let-s-go");
    }

    #[test]
    fn below_is_in_range_and_covers_all() {
        let mut r = SeedRng(1);
        let mut seen = [0; 7];
        for _ in 0..7000 {
            seen[r.below(7)] += 1;
        }
        assert!(seen.iter().all(|&n| n > 800 && n < 1200), "{seen:?}");
    }

    #[test]
    fn json_round_trip_is_atomic_and_tolerates_a_bom() {
        let d = std::env::temp_dir().join(format!("gn-util-{}", std::process::id()));
        let p = d.join("x.json");
        write_json(&p, &serde_json::json!({"a": 1})).unwrap();
        assert_eq!(read_json(&p).unwrap().unwrap()["a"], 1);
        fs::write(&p, b"\xEF\xBB\xBF{\"a\":2}").unwrap();
        assert_eq!(read_json(&p).unwrap().unwrap()["a"], 2);
        assert!(read_json(&d.join("missing.json")).unwrap().is_none());
        let _ = fs::remove_dir_all(d);
    }
}
