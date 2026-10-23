//! The local web server the page talks to. It listens on 127.0.0.1 only and checks every request:
//! the Host must be 127.0.0.1 or localhost (no DNS rebinding), a POST's Origin must be this server, and every POST carries a
//! per-run token that only the served page knows. Bodies are capped. Media is served by catalog title and kind, never by path.

use crate::app::App;
use crate::util::{Json, s};
use serde_json::{Value, json};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use tiny_http::{Header, Method, Request, Response, Server};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

const MAX_BODY: usize = 256 * 1024;
const MAX_IMPORT: usize = 16 * 1024 * 1024;
const MEDIA_CHUNK: u64 = 8 * 1024 * 1024;
const CSP: &str = "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; media-src 'self'; frame-src https://www.youtube-nocookie.com; frame-ancestors 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

/// A page file. Built into the program; for working on the page, `GAMENIGHT_UI_DIR` serves it from a folder instead (same names only).
pub fn asset(name: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    let built = ASSETS.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)?;
    if let Some(dir) = std::env::var_os("GAMENIGHT_UI_DIR")
        && let Ok(b) = std::fs::read(std::path::Path::new(&dir).join(name))
    {
        return Some(b.into());
    }
    Some(built.into())
}

fn ctype(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes().to_vec(), v.as_bytes().to_vec()).unwrap()
}

fn respond(req: Request, code: u16, body: Vec<u8>, ct: &str, extra: &[(&str, String)]) {
    let mut r = Response::from_data(body)
        .with_status_code(code)
        .with_header(header("Content-Type", ct))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header("Content-Security-Policy", CSP))
        .with_header(header("Cross-Origin-Resource-Policy", "same-origin"))
        .with_header(header("Referrer-Policy", "no-referrer"));
    for (k, v) in extra {
        r = r.with_header(header(k, v));
    }
    let _ = req.respond(r);
}

fn json_reply(req: Request, code: u16, v: &Json) {
    respond(req, code, serde_json::to_vec(v).unwrap_or_default(), "application/json; charset=utf-8", &[("Cache-Control", "no-store".into())]);
}

fn err(req: Request, code: u16, msg: &str) {
    json_reply(req, code, &json!({"error": msg}));
}

pub fn decode(q: &str) -> String {
    let b = q.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                Ok(x) => {
                    out.push(x);
                    i += 2;
                }
                Err(_) => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query(url: &str) -> Vec<(String, String)> {
    url.split_once('?').map(|(_, q)| q.split('&').filter_map(|kv| kv.split_once('=').map(|(k, v)| (decode(k), decode(v)))).collect()).unwrap_or_default()
}

fn q<'a>(qs: &'a [(String, String)], k: &str) -> &'a str {
    qs.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

fn get_header(req: &Request, name: &'static str) -> Option<String> {
    req.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string())
}

fn host_ok(req: &Request) -> bool {
    let h = get_header(req, "Host").unwrap_or_default();
    let host = h.rsplit_once(':').map(|(a, _)| a).unwrap_or(&h).trim_matches(['[', ']']);
    host == "127.0.0.1" || host == "localhost"
}

fn origin_ok(req: &Request, port: u16) -> bool {
    match get_header(req, "Origin") {
        None => true,
        Some(o) => o == format!("http://127.0.0.1:{port}") || o == format!("http://localhost:{port}"),
    }
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A media file with byte ranges, so a video can be scrubbed. One reply never carries more than MEDIA_CHUNK bytes.
fn serve_file(req: Request, path: &std::path::Path) {
    let ct = crate::media::content_type(path);
    let Ok(mut f) = std::fs::File::open(path) else { return err(req, 404, "not found") };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut extra = vec![("Cache-Control", "private, max-age=3600".to_string()), ("Accept-Ranges", "bytes".to_string())];
    let range = get_header(&req, "Range").unwrap_or_default();
    if let Some(spec) = range.trim().strip_prefix("bytes=").filter(|r| !r.contains(',') && r.len() <= 40) {
        let (a, b) = spec.split_once('-').unwrap_or(("", ""));
        let (start, end) = match (a.parse::<u64>().ok(), b.parse::<u64>().ok()) {
            (Some(st), e) => (st, e.unwrap_or(size.saturating_sub(1))),
            (None, Some(n)) => (size.saturating_sub(n), size.saturating_sub(1)),
            _ => return err(req, 416, "bad range"),
        };
        let end = end.min(size.saturating_sub(1)).min(start.saturating_add(MEDIA_CHUNK - 1));
        if start >= size || start > end {
            return respond(req, 416, vec![], "text/plain", &[("Content-Range", format!("bytes */{size}"))]);
        }
        let mut buf = vec![0u8; (end - start + 1) as usize];
        if f.seek(SeekFrom::Start(start)).is_err() || f.read_exact(&mut buf).is_err() {
            return err(req, 500, "read failed");
        }
        extra.push(("Content-Range", format!("bytes {start}-{end}/{size}")));
        return respond(req, 206, buf, ct, &extra);
    }
    let mut buf = vec![];
    if f.take(64 << 20).read_to_end(&mut buf).is_err() {
        return err(req, 500, "read failed");
    }
    respond(req, 200, buf, ct, &extra)
}

fn strings(v: &Value, k: &str) -> Vec<String> {
    crate::util::strs(v, k)
}

fn post(app: &App, path: &str, d: &Value) -> Result<Json, (u16, String)> {
    let e = |m: String| (400u16, m);
    let t = s(d, "title");
    let r = match path {
        "/api/marks" => app.set_mark(t, s(d, "who"), d.get("mark").and_then(Value::as_str)),
        "/api/pick" => app.pick(d.get("filters").unwrap_or(&json!({})), &strings(d, "seen")),
        "/api/preview" => app.preview(d.get("filters").unwrap_or(&json!({}))),
        "/api/finalists" => app.finalists(d.get("filters").unwrap_or(&json!({}))),
        "/api/plan" => app.plan(d.get("filters").unwrap_or(&json!({}))),
        "/api/taste" => app.taste_set(s(d, "who"), d.get("n").and_then(Value::as_u64).unwrap_or(10) as usize),
        "/api/similar" => app.similar(t),
        "/api/confirm" => app.confirm(t),
        "/api/veto" => app.veto(t),
        "/api/clear-vetoes" => app.clear_vetoes(),
        "/api/turn" => app.set_turn(s(d, "who")),
        "/api/progress" => app.set_progress(t, s(d, "note"), d.get("ongoing").and_then(Value::as_bool).unwrap_or(false)),
        "/api/launch" => app.launch(t, d.get("dry").and_then(Value::as_bool).unwrap_or(false)),
        "/api/session/end" => app.end_session(s(d, "id")),
        "/api/session/started" => app.session_started(s(d, "id"), d.get("ok").and_then(Value::as_bool).unwrap_or(false)),
        "/api/session/rate" => app.rate_session(s(d, "id"), s(d, "who"), s(d, "rating")),
        "/api/session/outcome" => app.session_outcome(s(d, "id"), s(d, "outcome")),
        "/api/session/dismiss" => app.dismiss_session(s(d, "id")),
        "/api/wishlist" => app.wishlist_edit(d),
        "/api/games" => app.save_game(
            d.get("game").unwrap_or(&json!({})),
            d.get("create").and_then(Value::as_bool).unwrap_or(false),
            d.get("oldTitle").and_then(Value::as_str),
        ),
        "/api/games/delete" => app.delete_game(t),
        "/api/settings" => app.save_settings(d),
        "/api/import" => app.import(d),
        "/api/sample" => app.add_sample(),
        "/api/library/import" => app.import_library(d.get("picks").unwrap_or(&json!([]))),
        "/api/scrape" => app.scrape(t, s(d, "provider"), d.get("forceArt").and_then(Value::as_bool).unwrap_or(false)),
        "/api/refresh" => {
            app.refresh();
            Ok(json!({"ok": true}))
        }
        _ => return Err((404, "not found".into())),
    };
    r.map_err(e)
}

fn get(app: &App, path: &str, qs: &[(String, String)]) -> Option<Result<Json, String>> {
    Some(match path {
        "/api/bootstrap" => app.bootstrap(),
        "/api/history" => app.history(),
        "/api/health" => app.health(),
        "/api/export" => app.export(),
        "/api/recap" => app.recap(q(qs, "days").parse().unwrap_or(7)),
        "/api/weights" => Ok(json!({"table": crate::picker::weight_table()})),
        "/api/now" => app.now(),
        "/api/taste" => app.taste(),
        "/api/nights" => app.nights(),
        "/api/memories" => app.memories(),
        "/api/year" => app.year(q(qs, "y").parse().ok()),
        "/api/library" => app.library(),
        "/api/providers" => app.providers(),
        _ => return None,
    })
}

fn handle(app: &App, token: &str, port: u16, mut req: Request) {
    if !host_ok(&req) {
        return err(req, 403, "bad host");
    }
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    match req.method() {
        Method::Get | Method::Head => {
            if path == "/" || path == "/index.html" {
                let html = String::from_utf8_lossy(&asset("index.html").unwrap_or_default()).replace("__TOKEN__", token);
                return respond(req, 200, html.into_bytes(), "text/html; charset=utf-8", &[("Cache-Control", "no-store".into())]);
            }
            let name = path.trim_start_matches('/');
            let flat = |prefix: &str| {
                name.strip_prefix(prefix)
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') && !n.contains(".."))
            };
            if (name == "style.css" || name == "favicon.svg" || flat("js/") || flat("fonts/"))
                && let Some(b) = asset(name)
            {
                return respond(req, 200, b.to_vec(), ctype(name), &[("Cache-Control", "no-cache".into())]);
            }
            let qs = query(&url);
            if path == "/api/media" || path == "/api/cover" {
                let kind = if path == "/api/cover" { "cover" } else { q(&qs, "kind") };
                return match app.media_path(q(&qs, "title"), kind) {
                    Some(p) => serve_file(req, &p),
                    None => err(req, 404, "no media"),
                };
            }
            match get(app, &path, &qs) {
                Some(Ok(v)) => json_reply(req, 200, &v),
                Some(Err(e)) => err(req, 400, &e),
                None => err(req, 404, "not found"),
            }
        }
        Method::Post => {
            if !origin_ok(&req, port) {
                return err(req, 403, "bad origin");
            }
            let sent = get_header(&req, "X-GameNight-Token").unwrap_or_default();
            if !same(sent.as_bytes(), token.as_bytes()) {
                return err(req, 403, "missing or wrong token (reload the page)");
            }
            let cap = if path == "/api/import" { MAX_IMPORT } else { MAX_BODY };
            if req.body_length().is_some_and(|n| n > cap) {
                return err(req, 413, "too large");
            }
            let mut body = vec![];
            if req.as_reader().take(cap as u64 + 1).read_to_end(&mut body).is_err() || body.len() > cap {
                return err(req, 413, "too large");
            }
            let d: Value = if body.is_empty() {
                json!({})
            } else {
                match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(_) => return err(req, 400, "the request isn't JSON"),
                }
            };
            match post(app, &path, &d) {
                Ok(v) => json_reply(req, 200, &v),
                Err((code, m)) => err(req, code, &m),
            }
        }
        _ => err(req, 405, "method not allowed"),
    }
}

pub fn new_token() -> String {
    let mut b = [0u8; 24];
    getrandom::fill(&mut b).expect("random source");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub struct Running {
    pub port: u16,
    #[cfg_attr(not(test), allow(dead_code))] // tests stop it; the program simply exits
    pub server: Arc<Server>,
}

/// Start serving on 127.0.0.1 (port 0 picks a free one). Each request gets its own short-lived thread.
pub fn start(app: Arc<App>, port: u16) -> Result<Running, String> {
    let server = Arc::new(Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?);
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(port);
    let token = new_token();
    let srv = server.clone();
    std::thread::spawn(move || {
        for req in srv.incoming_requests() {
            let (app, token) = (app.clone(), token.clone());
            std::thread::spawn(move || handle(&app, &token, port, req));
        }
    });
    Ok(Running { port, server })
}

/// Is a Game Night already answering on this port? (Then a second start just opens a window to it.)
pub fn already_running(port: u16) -> bool {
    use std::io::Write;
    let Ok(mut c) = std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), std::time::Duration::from_millis(400)) else { return false };
    let _ = c.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let _ = c.write_all(format!("GET /api/weights HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes());
    let mut buf = String::new();
    let _ = c.read_to_string(&mut buf);
    buf.contains("\"table\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn raw(port: u16, req: &str) -> String {
        let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        c.write_all(req.as_bytes()).unwrap();
        let mut buf = vec![];
        let _ = c.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn token_of(page: &str) -> String {
        let i = page.find("name=\"gn-token\" content=\"").unwrap() + 25;
        page[i..i + 48].to_string()
    }

    fn post_req(port: u16, path: &str, body: &str, token: &str, origin: Option<&str>) -> String {
        let o = origin.map(|o| format!("Origin: {o}\r\n")).unwrap_or_default();
        raw(
            port,
            &format!(
                "POST {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n{o}X-GameNight-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    #[test]
    fn the_server_guards_and_answers() {
        let d = std::env::temp_dir().join(format!("gn-srv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let app = Arc::new(App::new(d.clone()));
        *app.runner.lock().unwrap() = Some(Box::new(|_| Ok(None)));
        let run = start(app.clone(), 0).unwrap();
        let port = run.port;
        let page = raw(port, &format!("GET / HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert!(page.contains("200 OK") && page.contains("Content-Security-Policy"), "{page}");
        let tok = token_of(&page);
        assert!(raw(port, "GET / HTTP/1.0\r\nHost: evil.example\r\n\r\n").contains("403"), "DNS rebinding is refused");
        assert!(raw(port, &format!("GET /js/../../Cargo.toml HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n")).contains("404"));
        assert!(raw(port, &format!("GET /js/main.js HTTP/1.0\r\nHost: localhost:{port}\r\n\r\n")).contains("200 OK"));
        let game = r#"{"game": {"t": "A", "sys": "PC", "where": "couch", "p": "1-4", "mode": "Party", "gap": 4, "vibe": ["silly"], "len": "quick", "why": "x. y", "launch": {"steam": 620}}, "create": true}"#;
        assert!(post_req(port, "/api/games", game, "wrong", None).contains("403"), "the token is required");
        assert!(post_req(port, "/api/games", game, &tok, Some("https://evil.example")).contains("403"), "other sites are refused");
        assert!(post_req(port, "/api/games", game, &tok, Some(&format!("http://127.0.0.1:{port}"))).contains("200 OK"));
        assert!(post_req(port, "/api/marks", r#"{"title": "A", "who": "p9", "mark": "loved"}"#, &tok, None).contains("unknown player"));
        assert!(post_req(port, "/api/marks", r#"{"title": "A", "who": "p2", "mark": "loved"}"#, &tok, None).contains("loved"));
        let pick = post_req(port, "/api/pick", r#"{"filters": {}}"#, &tok, None);
        assert!(pick.contains("\"winner\":\"A\""), "{pick}");
        let big = "x".repeat(MAX_BODY + 10);
        assert!(post_req(port, "/api/pick", &big, &tok, None).contains("413"));
        let started = post_req(port, "/api/launch", r#"{"title": "A"}"#, &tok, None);
        assert!(started.contains("\"started\":true"), "{started}");
        assert!(raw(port, &format!("GET /api/now HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n")).contains("\"active\":{"));
        assert!(raw(port, &format!("GET /api/media?title=A&kind=../x HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n")).contains("404"));
        for p in [
            "/api/bootstrap",
            "/api/health",
            "/api/nights",
            "/api/year?y=2026",
            "/api/memories",
            "/api/library",
            "/api/providers",
            "/api/taste",
            "/api/history",
            "/api/export",
        ] {
            assert!(raw(port, &format!("GET {p} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n")).contains("200 OK"), "{p}");
        }
        assert!(already_running(port));
        run.server.unblock();
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn decoding() {
        assert_eq!(decode("It%27s+Two%20%E2%9C%93"), "It's Two ✓");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }
}
