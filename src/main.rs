//! Game Night: pick what to play tonight, for couples and friends. One small program: a local server and a native window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod catalog;
mod config;
mod detect;
mod folders;
mod health;
mod journal;
mod launch;
mod media;
mod picker;
mod scrape;
mod server;
mod sessions;
mod steam;
mod store;
mod util;
#[cfg(feature = "window")]
mod window;

use std::sync::Arc;

const HELP: &str = "Game Night - pick what to play tonight

USAGE: gamenight [options]

  --browser          open in your web browser instead of its own window
  --headless         only run the server (open http://127.0.0.1:<port>/ yourself)
  --port <n>         port to listen on (default 8770, or the one in Settings)
  --data-dir <dir>   where to keep the catalog and your data (or set GAMENIGHT_DATA)
  --version          print the version
  --help             this text

It listens on 127.0.0.1 only. Data folder by default: %APPDATA%\\GameNight on Windows,
~/.local/share/game-night on Linux, ~/Library/Application Support/GameNight on macOS.
A file named portable.txt next to the program keeps the data in ./data instead.";

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("gamenight {}", config::VERSION);
        return;
    }
    let data = config::data_dir(arg(&args, "--data-dir").as_deref());
    let app = Arc::new(app::App::new(data));
    let port: u16 = arg(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(app.settings().port);
    let headless = args.iter().any(|a| a == "--headless");
    let browser = args.iter().any(|a| a == "--browser") || !cfg!(feature = "window");

    let port = if server::already_running(port) {
        // one Game Night per data folder: a second start just shows the one that is running
        open(port, headless, browser, &app.paths.data);
        return;
    } else {
        match server::start(app.clone(), port) {
            Ok(r) => r.port,
            Err(_) => match server::start(app.clone(), 0) {
                Ok(r) => r.port,
                Err(e) => {
                    eprintln!("Game Night couldn't start its server: {e}");
                    std::process::exit(1);
                }
            },
        }
    };
    eprintln!("Game Night {} on http://127.0.0.1:{port}/ (data: {})", config::VERSION, app.paths.data.display());
    open(port, headless, browser, &app.paths.data);
    if headless || browser {
        loop {
            std::thread::park();
        }
    }
}

/// Show the page: in its own window (the web view keeps its cache in the data folder), else the browser.
fn open(port: u16, headless: bool, browser: bool, data: &std::path::Path) {
    let url = format!("http://127.0.0.1:{port}/");
    if headless {
        return;
    }
    #[cfg(feature = "window")]
    if !browser {
        if let Err(e) = window::run(&url, &data.join("webview")) {
            eprintln!("No window ({e}); opening the browser instead.");
            let _ = launch::open_url(&url);
            loop {
                std::thread::park();
            }
        }
        std::process::exit(0);
    }
    let _ = (browser, data);
    let _ = launch::open_url(&url);
}
