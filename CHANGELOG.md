# Changelog

## Unreleased

- **Library detectors, shown together on first run.** An empty shelf now greets you with what's on this computer ("Found: Steam 27 · ES-DE 62 · GOG 8 · Epic 1") and one button to bring it in. Each detector finds its launcher by itself and reads its own records:
  - **ES-DE**: `~/ES-DE`, Flatpak, or a portable install on any drive; every system's gamelist with names, descriptions, players, genres (as moods) and dates; its downloaded art folder is added automatically. Steam/Windows/Epic/GOG systems are skipped (their own detectors cover them).
  - **Epic Games**: the launcher's install manifests (DLC and unfinished installs left out); Play opens the game through Epic.
  - **GOG Galaxy**: the registry's GOG games (DLC left out); Play starts the game's program.
  - **RetroArch**: its playlists, wherever RetroArch lives; Play starts the game with its core.
  - **Heroic** (Epic and GOG) and **Lutris**: their installed games; Play goes through them.
- Shelf > Add from library shows one section per source, grouped by platform, with select-all per source and per platform.
- **Steam is found on any drive.** Besides the registry and the usual places, every connected drive (and /mnt, /media, /run/media, /Volumes) is checked for Steam libraries, including ones Steam's own list doesn't mention and second installs. Looked up at most once a minute.
- **Game folders without typing paths.** Settings lists folders on your drives that hold games (Epic, GOG, Xbox app, EA, Ubisoft, Riot, the Desktop's shortcuts, ~/Games, Heroic), each with how many games it found, one tap to add; **Browse...** opens the system's folder picker (Windows and macOS; zenity or kdialog on Linux). Art folders and the Steam folder have Browse too.
- **TheGamesDB and ScreenScraper** join the Steam store and SteamGridDB as lookup sources. Look it up and "Add and look them up" try every source that's set up, in order, each filling only what's missing. Keys never go back to the page.

## 1.0.0

The first public release: Game Night as one small program for Windows, Linux and macOS.

- **Groups of one to eight.** Names in Settings; marks, favourites, "not for us", the turn, the after-game ratings, the taste summary and the journal all work for the whole group. A couple sees "Just the two of us"; four friends see "All 4 of us".
- **Steam library import.** Finds Steam through the registry on Windows and in the usual places on Linux (native, Flatpak, Snap) and macOS; reads every library folder; skips Proton, runtimes and redistributables; uses Steam's cached box art. Play starts Steam games with `steam://rungameid`.
- **Game folders.** Any folders you choose are scanned for shortcuts, programs, `.desktop` files and AppImages, one program per game folder, installers and crash handlers skipped.
- **Look it up.** Steam store details (year, developer, genres, Metacritic, controller support, a short description) and art; SteamGridDB art with your own key. Imported games are drafts until someone checks them; a lookup fills a draft's placeholders and never overwrites what anyone wrote.
- **Launching** by Steam id, launcher link, shortcut file or direct command (never a shell), with sessions that end by themselves when the game closes.
- Everything from the home version it grew out of: the explainable weighted draw (statistics-tested), quick tiles, filters, three cards, evening plan, coin toss, taste test, trailers and co-op clips (muted, only when pressed), the journal, Our year, memories, controller support with an on-screen keyboard, couch mode, the Halloween look, WCAG AA contrast.
- A native window through the system web view (WebView2, WebKitGTK, WKWebView), or `--browser` / `--headless`.
