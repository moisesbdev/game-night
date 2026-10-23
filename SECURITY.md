# Security

Game Night runs a small web server on your own computer so its window (or your browser) can show the page. It is built to be safe to leave running:

- It binds to `127.0.0.1` only, and refuses requests whose Host isn't `127.0.0.1` or `localhost` (DNS rebinding).
- Every change (POST) must carry a token that only the served page knows, and a matching Origin when one is sent. Request bodies are capped.
- Pages get a strict Content-Security-Policy: no inline scripts, frames only from `youtube-nocookie.com`.
- Media is served by game title and kind, never by a path from the browser, and only from inside the folder it was found in.
- Play starts only games on the shelf, the way their entry says, without a shell; launcher links are limited to known game launchers.
- Online lookups only happen when someone presses Look it up, and images are only downloaded from the providers' own image hosts.

## Reporting a problem

Please report security issues privately through the repository's "Report a vulnerability" button (GitHub private advisories) rather than a public issue. Include what you did, what happened and the version (`gamenight --version`). You'll get an answer within a week.
