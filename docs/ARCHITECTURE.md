# Architecture

```
window (wry: WebView2 / WebKitGTK / WKWebView)  or your browser
        │  http://127.0.0.1:<port>/  (page files built into the program)
        ▼
server.rs ── Host/Origin/token checks, body caps, CSP, byte ranges for media
        │
app.rs ──┬─ catalog.rs   catalog.json: validation, tone check, safe edits, backups, wishlist
         ├─ store.rs     state.json: marks per player, history, turn, vetoes, notes
         ├─ sessions.rs  sessions.json: what was played, for how long, how it went; process watching
         ├─ picker.rs    filters, weights, the draw, finalists, plan, taste set, similar
         ├─ journal.rs   nights, Our year, memories, taste (pure)
         ├─ launch.rs    Steam id, launcher link, shortcut file, direct command (no shell)
         ├─ media.rs     art and previews: scraped, art folders (slug or ES-DE layout), Steam's cache
         ├─ steam.rs     Steam libraries, installed games, cached art (local files only)
         ├─ folders.rs   game folders: shortcuts, programs, .desktop files, AppImages
         ├─ scrape.rs    Provider trait: Steam store, SteamGridDB (only when asked)
         ├─ health.rs    read-only checks
         └─ config.rs    data folder, settings.json, players
```

Standard JSON files, written atomically (temp file, sync, rename). Each store changes a copy and only makes it live once it is on disk. A file that exists but can't be read is never overwritten.

## The page (`ui/`, ES modules, no build step)

Four places in a dock (Tonight, Shelf, Journal, More); a place with several parts shows one at a time. `logic.js` holds the DOM-free helpers and is tested with `node --test`. `core.js` holds shared state, the element builder, `api()` and the player helpers (`players()`, `nm(id)`, `rater()`); the rest are one module per place or feature (`tonight.js`, `shelf.js`, `games.js`, `library.js`, `journal.js`, `nights.js`, `history.js`, `memories.js`, `more.js`, `settings.js`, `health.js`, `watch.js`, `session.js`, `pad.js`, `input.js`, `spooky.js`, `wheel.js`, `presets.js`).

## Decisions

- **The server picks; the page dresses it up.** One implementation of the draw, covered by statistics tests. The page animates the reveal and shows the odds the server reports.
- **A group, not a pair.** Players are `{id, name}` in settings; marks and history are keyed by id, so renaming someone keeps everything and removing someone leaves their history in the journal.
- **Drafts.** Games imported from a library are usable at once but marked `draft` until someone saves them in the editor. A lookup may replace a draft's placeholders; it never overwrites a checked game's fields.
- **A game with no launch is still a game.** Console games are drawn like any other. A game whose launch is set but broken (a missing program, a disconnected drive) is left out of the draw, and the page says why.
- **Local first.** No network unless someone presses Trailer or Look it up. Images only from the providers' image hosts.
- **Readable by construction.** Colour tokens at the top of `style.css`; the home version's contrast tests apply to every theme including the Halloween one.

## Motion

1. Nothing vanishes; things change into each other (the Tonight stage cross-fades through a ghost layer).
2. Entrances are for what is new; a refresh is never an arrival.
3. Exits are quieter than entrances.
4. One thing moves at a time (seasonal extras replace the usual mover instead of adding one).
5. Never destroy what someone is using (changing who is rating doesn't rebuild a sheet with a playing trailer).
6. Reduced motion switches everything off.
7. Sound never starts by itself.
