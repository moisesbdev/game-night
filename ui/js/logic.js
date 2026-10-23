/* Pure helpers and constants: no DOM, no network, so they can be tested with `node --test`. */

/** The four places, as [id, label, icon name]; the icon (drawn in core.js) is decoration only and hidden from screen readers. */
export const TABS = [["tonight", "Tonight", "moon"], ["shelf", "Shelf", "pad"], ["journal", "Journal", "book"], ["more", "More", "sliders"]];

/** What each place can show, one section at a time: [id, label]. The first one is the default. */
export const SUBS = {
  shelf: [["games", "Games"], ["library", "Add from library"], ["wishlist", "Wishlist"]],
  journal: [["nights", "Nights"], ["year", "Our year"], ["together", "Together"], ["stats", "Stats"]],
  more: [["health", "Health"], ["settings", "Settings"]],
};

/** Old addresses (#games, #nights, ...) still work: -> [tab, section]. */
export function legacyRoute(hash) {
  const map = { tonight: ["tonight", null], games: ["shelf", "games"], library: ["shelf", "library"], nights: ["journal", "nights"], history: ["journal", "stats"], health: ["more", "health"], settings: ["more", "settings"] };
  if (Object.prototype.hasOwnProperty.call(map, hash)) return map[hash];        // not `map[hash]`: "#__proto__" or "#constructor" would find the object's own members
  if (TABS.some(t => t[0] === hash)) return [hash, null];
  return null;
}

export const MOODS = [["cozy", "Cozy"], ["silly", "Silly"], ["action", "Action"], ["brain", "Puzzle / brain"], ["story", "Story"], ["spooky", "Spooky"], ["compete", "A little rivalry"]];

export const VIBE_NAME = { cozy: "cozy", silly: "silly", action: "action", brain: "puzzle", story: "story", spooky: "spooky", compete: "rivalry" };

export const LEN_NAME = { quick: "Quick", evening: "An evening", long: "Long-running" };

export const LEN_HELP = { quick: "rounds of about 5 to 20 minutes", evening: "one sitting of 1 to 3 hours", long: "8+ hours spread over many nights" };

/** Platform names offered when typing one (any text is allowed). */
export const PLATFORMS = ["PC", "Steam Deck", "Switch", "Switch 2", "PlayStation 5", "PlayStation 4", "Xbox Series", "Xbox One", "Mobile", "Board game"];

export const MODES = ["True co-op", "Co-op", "Co-op / party", "Co-op / versus", "Party", "Race / teams", "Team race", "Team vs CPU", "Versus", "Versus / co-op", "Helper seat", "Take turns"];

/** Soft sea-glass slice and poster colours (teals with sand, coral and lavender); labels on them use WHEEL_INK (contrast is tested). */
export const COLORS = ["#bff0ea", "#9fe3dc", "#c9f1de", "#c5e6f5", "#d4f3d8", "#f6e7c8", "#fbd9c6", "#d9d8f5", "#f8efb8", "#f9cfc9", "#b4e6e6", "#cfeaf0"];

export const WHEEL_INK = "#12343b";

export function posterColor(t) { let n = 0; for (const c of t) n = (n * 31 + c.charCodeAt(0)) >>> 0; return COLORS[n % COLORS.length]; }

export function monogram(t) { const w = t.replace(/[^A-Za-z0-9 ]/g, "").split(/\s+/).filter(x => x && !/^(the|of|and|a)$/i.test(x)); return (w.slice(0, 3).map(x => x[0]).join("") || t.slice(0, 2)).toUpperCase(); }


export function ago(d) { return d === 0 ? "today" : d === 1 ? "yesterday" : d + " days ago"; }

export function daysSince(last) { if (!last) return null; const m = last.match(/^(\d{4})-(\d{2})-(\d{2})$/); if (!m) return null; const n = new Date(); return Math.max(0, Math.round((Date.UTC(n.getFullYear(), n.getMonth(), n.getDate()) - Date.UTC(+m[1], +m[2] - 1, +m[3])) / 864e5)); }

export const DEFAULT_FILTERS = { where: "any", len: "any", gap: 1, n: "any", hide: false, match: "all", two: false, fresh: false, one: false, platform: "", owned: false, A: [], B: [] };

export const BUILTIN_PRESETS = [{ name: "Quick round", f: { len: "quick" } }, { name: "One couch, two players", f: { where: "couch", n: 2 } }, { name: "Spooky night", f: { A: ["spooky"] } }];

export function cleanFilters(f) {
  const out = JSON.parse(JSON.stringify(DEFAULT_FILTERS)); if (!f || typeof f !== "object") return out;
  const moods = MOODS.map(m => m[0]);
  if (["any", "couch", "online"].includes(f.where)) out.where = f.where;
  if (["any", "quick", "evening", "long"].includes(f.len)) out.len = f.len;
  if ([1, 3, 4, 5].includes(f.gap)) out.gap = f.gap;
  if (Number.isInteger(f.n) && f.n >= 1 && f.n <= 8) out.n = f.n;
  if (["all", "any"].includes(f.match)) out.match = f.match;
  if (typeof f.platform === "string") out.platform = f.platform.slice(0, 24);
  for (const k of ["hide", "two", "fresh", "one", "owned"]) if (typeof f[k] === "boolean") out[k] = f[k];
  for (const k of ["A", "B"]) if (Array.isArray(f[k])) out[k] = f[k].filter(m => moods.includes(m));
  return out;
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"], MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** The evening a moment belongs to, as YYYY-MM-DD: anything before 05:00 counts as the evening before (same rule as the server's journal). */
export function nightKey(now = new Date()) {
  const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - (now.getHours() < 5 ? 1 : 0));
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** "Tonight", "Last night", or "Wed 21 Oct" (with the year when it isn't this year). */
export function nightLabel(date, now = new Date()) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date || ""); if (!m) return String(date || "");
  const [y, mo, d] = [+m[1], +m[2], +m[3]], today = nightKey(now).split("-").map(Number);
  const diff = Math.round((Date.UTC(today[0], today[1] - 1, today[2]) - Date.UTC(y, mo - 1, d)) / 864e5);
  if (diff === 0) return "Tonight";
  if (diff === 1) return "Last night";
  return `${DAYS[new Date(Date.UTC(y, mo - 1, d)).getUTCDay()]} ${d} ${MONTHS[mo - 1]}${y !== today[0] ? " " + y : ""}`;
}

/** 95 -> "1 h 35 min", 60 -> "1 h", 25 -> "25 min", null/0 -> "". */
export function fmtMinutes(m) {
  if (!m || m < 1) return "";
  const h = Math.floor(m / 60), r = Math.round(m % 60);
  return h ? (r ? `${h} h ${r} min` : `${h} h`) : `${r} min`;
}

/** Seconds as a duration: 45 -> "under a minute", 1409 -> "23 min", 0/null -> "". */
export function fmtSeconds(s) {
  if (!s || s < 1) return "";
  return s < 60 ? "under a minute" : fmtMinutes(Math.round(s / 60));
}

/** {p1:'up', p2:'down'} + players [{id, name}] -> "Sam liked it · Alex not really" (skips are left out). */
export function ratingWords(ratings, players) {
  const word = { up: "liked it", down: "not really" };
  return players.filter(p => ratings && word[ratings[p.id]]).map(p => `${p.name} ${word[ratings[p.id]]}`).join(" · ");
}

/** A quick tile's filters for this group: "group" in a player count means everyone in it. */
export function resolveFilters(f, groupSize) {
  return f && f.n === "group" ? { ...f, n: Math.max(1, Math.min(8, groupSize || 2)) } : f;
}

/** A quick tile's label for this group ("All of us" reads "Just the two of us" for a couple and "Just me" alone). */
export function quickLabel(q, groupSize) {
  if (q.id !== "pair") return q.label;
  return groupSize === 1 ? "Just me" : groupSize === 2 ? "Just the two of us" : "All " + groupSize + " of us";
}

/** One-tap feelings on the Tonight screen: pressing one sets these filters and picks right away. `icon` is a name drawn in core.js. */
export const QUICK_PICKS = [
  { id: "cozy", label: "Something cozy", icon: "leaf", f: { A: ["cozy"] } },
  { id: "quick", label: "A quick round", icon: "bolt", f: { len: "quick" } },
  { id: "pair", label: "All of us", icon: "pair", f: { where: "couch", n: "group" } },
  { id: "silly", label: "Something silly", icon: "smile", f: { A: ["silly"] } },
  { id: "spooky", label: "A little spooky", icon: "ghost", f: { A: ["spooky"] } },
  { id: "rivalry", label: "A little rivalry", icon: "flag", f: { A: ["compete"] } },
  { id: "fresh", label: "Something new to us", icon: "spark", f: { fresh: true } },
  { id: "one", label: "One controller", icon: "eye", f: { one: true } },
  { id: "evening", label: "A proper evening", icon: "clock", f: { len: "evening" } },
];

/** The filters in plain words, for the small line under the main button: ["Cozy", "Quick rounds", ...]. Empty when nothing is narrowed. */
export function describeFilters(f) {
  const F = cleanFilters(f), out = [], mood = m => (MOODS.find(x => x[0] === m) || [m, m])[1];
  if (F.where === "couch") out.push("same couch"); else if (F.where === "online") out.push("online");
  const moods = [...new Set([...F.A, ...(F.two ? F.B : [])])].map(mood);
  if (moods.length) out.push(moods.join(F.match === "any" || F.two ? " or " : " + "));
  if (F.len !== "any") out.push({ quick: "quick rounds", evening: "an evening", long: "long-running" }[F.len]);
  if (F.n === 1) out.push("one player"); else if (F.n === 2) out.push("two players"); else if (F.n >= 3) out.push("room for " + F.n);
  if (F.gap > 1) out.push("skill gap " + F.gap + "+");
  if (F.hide) out.push("hiding played ones");
  if (F.fresh) out.push("new to us");
  if (F.one) out.push("one controller");
  if (F.platform) out.push("on " + F.platform);
  if (F.owned) out.push("games you all own");
  return out;
}
/** The seasonal look for a date: "halloween" all through October, otherwise null. */
export function season(now = new Date()) { return now.getMonth() === 9 ? "halloween" : null; }

/** Online nights have no use for "One controller": that tile becomes this one (the grid stays three across). */
export const ONLINE_TILE = { id: "long", label: "Something long-running", icon: "clock", f: { len: "long" } };

/** The quick tiles for how you're playing: online swaps "One controller" for ONLINE_TILE. */
export function tilesFor(picks, mode) {
  return mode === "online" ? picks.map(q => (q.id === "one" ? ONLINE_TILE : q)) : picks;
}

/** The quick tiles in the order to show them: in the Halloween look the spooky one comes first. */
export function quickOrder(picks, look) {
  if (look !== "halloween") return picks;
  return [...picks.filter(q => q.id === "spooky"), ...picks.filter(q => q.id !== "spooky")];
}

/** "Good morning" / "Good afternoon" / "Good evening" / "Late night" for the Tonight screen. */
export function greeting(now = new Date()) {
  const hr = now.getHours();
  return hr < 5 ? "Late night" : hr < 12 ? "Good morning" : hr < 18 ? "Good afternoon" : "Good evening";
}
/** A YouTube video id is exactly 11 letters, digits, "_" or "-". */
export function isVideoId(s) { return typeof s === "string" && /^[A-Za-z0-9_-]{11}$/.test(s); }

/** The privacy-friendly embed address for a video id (youtube-nocookie.com), or null for anything that isn't a plain id. */
export function trailerEmbedUrl(id) {
  return isVideoId(id) ? "https://www.youtube-nocookie.com/embed/" + id + "?autoplay=1&mute=1&rel=0&modestbranding=1&playsinline=1&enablejsapi=1" : null;
}

/** The normal YouTube page for a video id (the way out if the embedded player shows an error), or null for anything that isn't a plain id. */
export function watchUrl(id) { return isVideoId(id) ? "https://www.youtube.com/watch?v=" + id : null; }

/** A YouTube search for people playing a game together (for games without a saved co-op clip). */
/** Controller navigation: which of `rects` ({x, y, w, h}) is nearest in direction `dir` ("up" | "down" | "left" | "right") from rects[from]?
 *  Things that share a row (or column) with the current one win over things merely closer; -1 when nothing lies that way.
 *  With no current one (from < 0) it picks the top-left-most. */
export function padStep(rects, from, dir) {
  if (!rects.length) return -1;
  if (from < 0 || from >= rects.length) { let b = 0; rects.forEach((r, i) => { if (r.y < rects[b].y - 4 || (Math.abs(r.y - rects[b].y) <= 4 && r.x < rects[b].x)) b = i; }); return b; }
  const c = rects[from], cx = c.x + c.w / 2, cy = c.y + c.h / 2, horiz = dir === "left" || dir === "right";
  let best = -1, bestScore = Infinity;
  rects.forEach((r, i) => {
    if (i === from) return;
    const dx = r.x + r.w / 2 - cx, dy = r.y + r.h / 2 - cy;
    const along = dir === "right" ? dx : dir === "left" ? -dx : dir === "down" ? dy : -dy;
    if (along <= 1) return;
    const across = horiz ? Math.abs(dy) : Math.abs(dx);
    const overlap = horiz ? Math.min(c.y + c.h, r.y + r.h) - Math.max(c.y, r.y) : Math.min(c.x + c.w, r.x + r.w) - Math.max(c.x, r.x);
    const score = along + across * (overlap > 0 ? 1 : 4);
    if (score < bestScore) { bestScore = score; best = i; }
  });
  return best;
}

/** "1 player", "2 players", "1-4 players": the player count as people say it. */
export function playersLabel(p) {
  return p === "1" ? "1 player" : p + " players";
}

/** Typing from the controller's on-screen keyboard: what `value` becomes when `key` is pressed with the caret at `start` (and `end`, if text is selected).
 *  `key` is a character, "Backspace", "Clear" or "Enter". opts: max (maxlength), multiline (Enter allowed), numeric (digits, "." and "-" only).
 *  A null caret (number boxes have none) means the end. Returns { value, caret }. */
export function applyKey(value, start, end, key, opts) {
  opts = opts || {};
  value = String(value == null ? "" : value);
  const clamp = n => Math.min(Math.max(n, 0), value.length);
  let a = start == null ? value.length : clamp(+start), b = end == null ? a : clamp(+end);
  if (b < a) [a, b] = [b, a];
  if (key === "Clear") return { value: "", caret: 0 };
  if (key === "Backspace") {
    if (a !== b) return { value: value.slice(0, a) + value.slice(b), caret: a };
    return a === 0 ? { value, caret: 0 } : { value: value.slice(0, a - 1) + value.slice(a), caret: a - 1 };
  }
  const text = key === "Enter" ? "\n" : String(key);
  if (!opts.multiline && text.includes("\n")) return { value, caret: a };
  if (opts.numeric && !/^[0-9.\-]+$/.test(text)) return { value, caret: a };
  const max = opts.max > 0 ? opts.max : Infinity, room = max - (value.length - (b - a));
  if (room <= 0) return { value, caret: a };
  const ins = text.slice(0, room);
  return { value: value.slice(0, a) + ins + value.slice(b), caret: a + ins.length };
}

/** The "About this game" lines from a game's `info` (what the databases say). Plain strings, only the parts that exist. */
export function aboutLines(info) {
  if (!info || typeof info !== "object") return [];
  const out = [], hrs = n => (Math.round(n * 10) / 10).toString().replace(/\.0$/, "") + " h";
  const head = [info.year, info.developer, info.publisher && info.publisher !== info.developer ? "published by " + info.publisher : null].filter(Boolean).join(" · ");
  if (head) out.push(head);
  if (info.rating) out.push("Age rating: " + info.rating + (info.descriptors && info.descriptors.length ? " (" + info.descriptors.join(", ") + ")" : ""));
  if (info.genres && info.genres.length) out.push("Genre: " + info.genres.join(", "));
  const h = info.hltb || {}, parts = [["main story", h.main], ["with extras", h.extras], ["everything", h.full]].filter(([, v]) => typeof v === "number" && v > 0).map(([l, v]) => hrs(v) + " " + l);
  if (parts.length) out.push("How long: " + parts.join(", ") + " (HowLongToBeat)");
  const s = info.scores || {}, sc = [typeof s.metacritic === "number" ? "Metacritic " + s.metacritic : null, typeof s.steamPct === "number" ? s.steamPct + "% positive on Steam" : null].filter(Boolean);
  if (sc.length) out.push("Scores: " + sc.join(" · "));
  if (info.controller) out.push("Controller: " + ({ full: "full support", partial: "partial support", none: "not supported" })[info.controller]);
  if (info.localPlay) out.push("Playing together: " + info.localPlay);
  return out;
}

/** The short names of the places `info.sources` came from ("store.steampowered.com", ...). */
export function sourceNames(info) {
  const out = [];
  for (const u of (info && info.sources) || []) { try { const h = new URL(u).hostname.replace(/^www\./, ""); if (!out.includes(h)) out.push(h); } catch (e) { /* skip */ } }
  return out;
}

/** What "Watch it first" opens: the trailer; if the game has none, its co-op gameplay clip; then the local preview. YouTube ones only while they are switched on. */
export function watchFirst(g, youtubeOn) {
  if (youtubeOn && isVideoId(g.trailer)) return "trailer";
  if (youtubeOn && isVideoId(g.coop)) return "coop";
  return g.video ? "video" : null;
}

export function coopSearchUrl(title) { return "https://www.youtube.com/results?search_query=" + encodeURIComponent(String(title || "") + " local co-op gameplay"); }

/** "usually 40 to 60 minutes" -> "Usually 40 to 60 min for us"; from the {low, high} range of the nights you actually played it. null when there is no range yet. */
export function paceLine(s) {
  if (!s || !(s.low > 0)) return null;
  return "Usually " + (s.high && s.high !== s.low ? s.low + " to " + s.high : String(s.low)) + " min for us";
}

/** A YouTube search for a game's trailer, for games without a saved one (opens in a new tab when pressed). */
export function trailerSearchUrl(title) { return "https://www.youtube.com/results?search_query=" + encodeURIComponent(String(title || "") + " official trailer"); }

/** Address of a game's screenshot, title screen or preview video, served by title and kind (never by path). */
export function mediaUrl(title, kind) { return "/api/media?title=" + encodeURIComponent(title) + "&kind=" + encodeURIComponent(kind); }
export const MARK_WORDS = { loved: "loved it", curious: "looks fun", pass: "not really", meh: "not for us" };
