# Contributing

Thanks for helping. A few things keep Game Night what it is:

- **One thing at a time on screen.** New features live behind a tap (a sheet, a section), not on the Tonight screen.
- **Honest odds.** Anything that changes how likely a game is goes in `picker::weight_table` and `picker::factors`, so "Why this one?" can explain it, with a statistics test.
- **Copy describes games, never coaches people.** The catalog refuses phrases that tell one player how to treat another (`catalog::tone_issues`).
- **Nothing household-specific.** No names, drives or folders in code; everything a person sets lives in `settings.json`.
- **Local first.** No network calls unless someone presses a button that says so.
- **Readable and calm.** Colours are tokens in `ui/style.css` and must pass WCAG AA; animations follow the motion rules in `docs/ARCHITECTURE.md` and switch off with reduced motion. Videos always start muted.

## Working on it

```bash
cargo test                 # Rust, including the odds statistics and a real server
node --test "tests/js/*.test.mjs"  # the page's DOM-free logic
cargo clippy --all-targets # no warnings, please
cargo fmt
GAMENIGHT_UI_DIR=ui cargo run -- --browser   # serve the page from disk while editing it
```

Adding a scraper: implement `scrape::Provider` and list it in `scrape::providers`. Adding a launch kind: extend `catalog::launch_issues` and `launch::plan`, with a test that it refuses what it shouldn't run.
