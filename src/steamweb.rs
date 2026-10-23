//! Steam accounts, for groups who play online together: each player's owned games, read with Steam's official Web API
//! (one free key, from steamcommunity.com/dev/apikey), so Game Night can show what everyone owns. Only when someone presses
//! Refresh. Each profile's "Game details" must be public in Steam's privacy settings; a private one is reported as such.
#![cfg_attr(not(feature = "scrape"), allow(dead_code))] // without online lookups these are compiled but unused

use serde_json::Value;

/// What someone typed for their Steam account.
#[derive(Debug, Clone, PartialEq)]
pub enum Profile {
    Id(u64),
    Vanity(String),
    Invalid,
}

/// A SteamID64 (`7656119...`), a profile link (`/profiles/<id>` or `/id/<name>`), or a custom profile name.
pub fn parse_profile(input: &str) -> Profile {
    let t = input.trim().trim_end_matches('/');
    let id = |x: &str| x.parse::<u64>().ok().filter(|n| x.len() == 17 && x.starts_with("7656119") && *n > 0).map(Profile::Id);
    if let Some(p) = id(t) {
        return p;
    }
    let rest = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")).unwrap_or(t);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    if let Some(r) = rest.strip_prefix("steamcommunity.com/profiles/") {
        return id(r.split('/').next().unwrap_or("")).unwrap_or(Profile::Invalid);
    }
    if let Some(r) = rest.strip_prefix("steamcommunity.com/id/") {
        let name = r.split('/').next().unwrap_or("");
        return vanity(name);
    }
    if rest.contains('/') || rest.contains('.') {
        return Profile::Invalid;
    }
    vanity(rest)
}

fn vanity(name: &str) -> Profile {
    // all digits is a mistyped SteamID64, not a profile name
    let digits = name.chars().all(|c| c.is_ascii_digit());
    if !digits && (2..=32).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        Profile::Vanity(name.into())
    } else {
        Profile::Invalid
    }
}

/// (app id, name) for every game in a `GetOwnedGames` reply. A reply without a games list means the profile's game details are private.
pub fn owned_from_reply(v: &Value) -> Result<Vec<(u64, String)>, String> {
    let Some(games) = v["response"]["games"].as_array() else {
        return Err("this profile's game details are private (Steam > Profile > Edit Profile > Privacy Settings > Game details: Public)".into());
    };
    Ok(games
        .iter()
        .filter_map(|g| Some((g["appid"].as_u64()?, crate::steam::clean_name(g["name"].as_str().unwrap_or("")))))
        .filter(|(_, n)| !n.is_empty())
        .collect())
}

/// How a game plays together, from Steam's store categories: (online at all, online co-op, same screen).
pub fn play_modes(details: &Value) -> (bool, bool, bool) {
    let cats: Vec<i64> = details["categories"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_i64()).collect();
    let has = |ids: &[i64]| ids.iter().any(|i| cats.contains(i));
    (has(&[36, 38]), has(&[38]), has(&[24, 37, 39, 44])) // 36 online PvP, 38 online co-op; 24/37/39/44 shared screen and Remote Play Together
}

#[cfg(feature = "scrape")]
mod net {
    use super::*;
    use crate::scrape::net::{enc, get_json};

    fn api(path: &str, key: &str, q: &str) -> Result<Value, String> {
        get_json(&format!("https://api.steampowered.com/{path}/?key={}&format=json{q}", enc(key)), None).map_err(|e| {
            if e.contains("403") || e.contains("401") {
                "Steam refused the Web API key (check it in Settings)".to_string()
            } else {
                format!("Steam didn't answer ({e})")
            }
        })
    }

    /// The SteamID64 for whatever was typed (a custom profile name is looked up).
    pub fn resolve(input: &str, key: &str) -> Result<u64, String> {
        match parse_profile(input) {
            Profile::Id(id) => Ok(id),
            Profile::Invalid => Err("that doesn't look like a Steam profile link, ID or name".into()),
            Profile::Vanity(name) => {
                let r = api("ISteamUser/ResolveVanityURL/v1", key, &format!("&vanityurl={}", enc(&name)))?;
                r["response"]["steamid"].as_str().and_then(|s| s.parse().ok()).ok_or(format!("no Steam profile called {name}"))
            }
        }
    }

    pub fn owned(steamid: u64, key: &str) -> Result<Vec<(u64, String)>, String> {
        owned_from_reply(&api("IPlayerService/GetOwnedGames/v1", key, &format!("&steamid={steamid}&include_appinfo=1&include_played_free_games=1"))?)
    }

    /// (online, online co-op, same screen) for one app, from the store's public details (no key).
    pub fn modes(appid: u64) -> Result<(bool, bool, bool), String> {
        let r = get_json(&format!("https://store.steampowered.com/api/appdetails?appids={appid}&filters=categories"), None)?;
        Ok(play_modes(&r[appid.to_string()]["data"]))
    }
}

#[cfg(feature = "scrape")]
pub use net::{modes, owned, resolve};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn profiles_in_every_usual_form() {
        let id = 76561197960287930u64;
        for t in ["76561197960287930", "https://steamcommunity.com/profiles/76561197960287930/", "steamcommunity.com/profiles/76561197960287930"] {
            assert_eq!(parse_profile(t), Profile::Id(id), "{t}");
        }
        assert_eq!(parse_profile("https://steamcommunity.com/id/gabelogannewell/"), Profile::Vanity("gabelogannewell".into()));
        assert_eq!(parse_profile("  some_name-1 "), Profile::Vanity("some_name-1".into()));
        for bad in ["", "x", "https://evil.example/id/x", "12345", "has space", "https://steamcommunity.com/profiles/123"] {
            assert_eq!(parse_profile(bad), Profile::Invalid, "{bad}");
        }
    }

    #[test]
    fn owned_games_and_private_profiles() {
        let r = json!({"response": {"game_count": 2, "games": [{"appid": 620, "name": "Portal 2"}, {"appid": 1426210, "name": "It Takes Two\u{2122}"}]}});
        assert_eq!(owned_from_reply(&r).unwrap(), vec![(620, "Portal 2".to_string()), (1426210, "It Takes Two".to_string())]);
        assert!(owned_from_reply(&json!({"response": {}})).unwrap_err().contains("private"));
    }

    #[test]
    fn play_modes_from_categories() {
        let cat = |ids: &[i64]| json!({"categories": ids.iter().map(|i| json!({"id": i})).collect::<Vec<_>>()});
        assert_eq!(play_modes(&cat(&[2, 9, 38])), (true, true, false));
        assert_eq!(play_modes(&cat(&[1, 36])), (true, false, false));
        assert_eq!(play_modes(&cat(&[2, 39, 24])), (false, false, true));
        assert_eq!(play_modes(&cat(&[2])), (false, false, false));
    }
}
