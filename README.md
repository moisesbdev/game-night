# Game Night

**Pick what to play tonight, for couples and friends.**

You've got a few hundred games between you and an evening together, and twenty minutes go by scrolling. Game Night picks one, shows you why, and remembers how the night went. It brings in your Steam library and any game folders, works with a controller from the sofa, and runs on your own computer: one small program, no account, no cloud.

- **Made for the people on the couch.** Set up the group once: a couple, a few friends, a housemate who drops by, up to eight. Everyone's marks count, the pick goes round the room, and anyone can say "not for us".
- **Your Steam library, in one tap.** Game Night finds Steam on its own: the registry and the usual places, plus a look at every drive for Steam libraries (Windows, Linux native/Flatpak/Snap and macOS). It lists what's installed and adds what you pick, and Play starts the game through Steam.
- **Your other games, without typing paths.** Settings suggests the folders on your drives that hold games (Epic, GOG, Xbox app, EA, Ubisoft, Riot, your Desktop's shortcuts, `~/Games`, Heroic), each with how many it found, one tap to add, or **Browse...** with your system's own folder picker. Console games can sit on the shelf too: they're drawn, you just press start yourself.
- **Box art and details from four sources.** **Look it up** tries the Steam store (no account needed), TheGamesDB and ScreenScraper (consoles and retro games) and SteamGridDB (more art), in that order, filling only what's missing.
- **Fair, and it shows its work.** The pick is a weighted draw from your operating system's secure random source. Loved games come up more, recently played ones rest, one-player games are rarer unless you ask for them, and "Why this one?" shows the real odds and the reasons. Statistics tests check the reported odds against tens of thousands of draws.
- **One question on screen.** "What shall we play?", one big button, and nine one-tap feelings: cozy, a quick round, all of us, silly, a little spooky, a little rivalry, something new to us, one controller, a proper evening. Filters, three cards to strike from, an evening plan, a coin toss and a quick taste test are one tap away.
- **Watch before you commit.** Your own preview videos, or the game's trailer and a couch co-op clip from YouTube, only when you press, always muted.
- **A journal of your nights.** Who chose, how long you played, who liked it, what you finished. "Our year" turns it into a few slides, and a memories screen shows your nights one at a time when the app is left alone.
- **Couch-ready.** Full controller support (D-pad navigation, A/B/X/Y, an on-screen keyboard, dropdowns as lists), a couch mode with larger text, and WCAG AA contrast in light and dark themes. In October it dresses up for Halloween.

## Get it

Download the latest release for your system from the Releases page and run it. Game Night opens in its own window.

| System | File | Notes |
|---|---|---|
| Windows 10 and 11 | `gamenight-windows-x86_64.zip` | Uses the WebView2 runtime that comes with Windows. |
| Linux | `gamenight-linux-x86_64.tar.gz` | Needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), installed on most desktops. |
| Linux, no window | `gamenight-linux-x86_64-headless.tar.gz` | Serves the page for any browser (`--headless`), for a home server or a handheld. |
| macOS (Apple silicon) | `gamenight-macos-aarch64.tar.gz` | Unsigned; right-click > Open the first time. |

Start it with `--browser` to use your usual browser instead of its own window, or `--headless` to run only the server and open `http://127.0.0.1:8770/` yourself.

## Your first evening

1. **More > Settings > Who's playing**: type your names. Two for a couple, more for friends.
2. **Shelf > Add from library**: tick the games you'd play together and press **Add and look them up**. Steam is found by itself; for other games, add the folders Settings suggests (or Browse to one). Prefer to start small? The empty shelf offers twenty well-known games to try the picker with.
3. **Tonight**: press **Surprise us**, or pick a feeling. "We'll play this" passes the pick to the next person.

New games arrive marked "to check", and they're drawn straight away. Open one and press **Edit** to say how it plays together: on one couch or online, how many players, and how much skill matters.

## How picks work

Every game that fits the filters starts with the same share. The share changes when:

| What happened | Effect |
|---|---|
| Someone loved it | x1.6 for each person who did |
| It's a favourite of whoever's pick it is | x1.5 |
| Someone said "looks fun" / "not really" | x1.3 / x0.6 each |
| Someone marked it "not for us" | left out (or x0.3, in Settings) |
| It's in progress | x1.8 |
| You played it lately | down to x0.4 right after, back to normal after about two months |
| It's one of the last three picks | x0.15 |
| It's strictly one-player | x0.3, full share under One controller or spooky |

The names that flick past and the wheel are decoration. The winner is drawn first, from the whole pool, and the odds you see are the odds it was drawn with. "Another" never hands you the same game twice in a round.

## Privacy and safety

- It listens on `127.0.0.1` only. Every request must come from the page it served (Host and Origin checks, a per-run token for every change, size limits, a strict content security policy).
- It goes online only when you press **Trailer** (a muted, sandboxed `youtube-nocookie.com` frame) or **Look it up** (the Steam store's public pages, plus TheGamesDB, ScreenScraper and SteamGridDB once you add their keys). Switch trailers off in Settings and it never contacts YouTube. Images are only downloaded from those services' own image hosts.
- Play starts only games on your shelf, only the way their entry says: a Steam app id, a known launcher link (`steam://`, `heroic://`, `lutris:`, `itch://`, Epic, GOG, Ubisoft), a shortcut file, or a program run directly, never through a shell. Settings can switch Play off.
- Nothing about any one household is built in: no names, drives or folders.

## Your data

Everything lives in one folder: `%APPDATA%\GameNight` on Windows, `~/.local/share/game-night` on Linux, `~/Library/Application Support/GameNight` on macOS. Put a file named `portable.txt` next to the program to keep it in `./data` instead, or pass `--data-dir`.

- `catalog.json` is your shelf: plain JSON, validated on every save, with dated backups in `backups/`. A damaged file is set aside and the newest good backup is used.
- `state.json` holds marks, history and whose pick it is, with a daily backup (ten kept).
- `sessions.json` holds play sessions; `settings.json` holds settings; `media/` holds art saved by **Look it up**.
- Settings > Export makes one file with all of it; Import brings it back on another computer.

### Lookup sources

| Source | Needs | Good for |
|---|---|---|
| Steam store | nothing | PC games on Steam: details, box art, screenshots, how it's played together |
| TheGamesDB | a free API key (request on forums.thegamesdb.net) | consoles and older games |
| ScreenScraper | software credentials (request on the screenscraper.fr forum) and, optionally, your account for a bigger daily quota | the widest console and retro coverage |
| SteamGridDB | a free API key (steamgriddb.com, Preferences > API) | extra box art |

Keys are entered in Settings and stay in your data folder. A source that isn't set up is simply skipped.

### Art you already have

Settings > Art folders accepts folders named after each game (`covers/it-takes-two.png`, `screenshots/...`, `videos/...`) or an ES-DE `downloaded_media` folder. Steam's own cached box art is used automatically for Steam games.

## Build it

```bash
cargo build --release
```

- **Windows**: the stable Rust toolchain (MSVC or GNU). The window uses WebView2.
- **Linux**: `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev` (or your distribution's equivalent).
- **No window, no system libraries**: `cargo build --release --no-default-features --features scrape` builds the server-only program.
- **No network code at all**: `cargo build --release --no-default-features` leaves out the scrapers too.

Tests: `cargo test` (Rust, including the statistics) and `node --test "tests/js/*.test.mjs"` (the page's logic). While working on the page, set `GAMENIGHT_UI_DIR=ui` to serve it from disk instead of the copy built into the program.

## Licence

MIT. The bundled fonts, Fraunces and Nunito, are under the SIL Open Font License (see `ui/fonts`).
