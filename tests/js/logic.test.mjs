// Tests for the pure UI logic (no DOM). Run with: node --test tests/js
import test from "node:test";
import assert from "node:assert/strict";
import { BUILTIN_PRESETS, COLORS, DEFAULT_FILTERS, MOODS, QUICK_PICKS, SUBS, TABS, ago, cleanFilters, coopSearchUrl, daysSince, describeFilters, fmtMinutes, fmtSeconds, isVideoId, legacyRoute, mediaUrl, monogram, nightKey, nightLabel, aboutLines, applyKey, paceLine, playersLabel, padStep, posterColor, sourceNames, watchFirst, ratingWords, trailerEmbedUrl, trailerSearchUrl, watchUrl, season, quickOrder, quickLabel, resolveFilters } from "../../ui/js/logic.js";

test("nightKey: before 5 a.m. still belongs to the evening before", () => {
  assert.equal(nightKey(new Date(2026, 9, 21, 20, 0)), "2026-10-21");
  assert.equal(nightKey(new Date(2026, 9, 22, 0, 30)), "2026-10-21");
  assert.equal(nightKey(new Date(2026, 9, 22, 4, 59)), "2026-10-21");
  assert.equal(nightKey(new Date(2026, 9, 22, 5, 0)), "2026-10-22");
  assert.equal(nightKey(new Date(2026, 0, 1, 2, 0)), "2025-12-31");
});

test("nightLabel names tonight, last night, then the date", () => {
  const now = new Date(2026, 9, 22, 21, 0);
  assert.equal(nightLabel("2026-10-22", now), "Tonight");
  assert.equal(nightLabel("2026-10-21", now), "Last night");
  assert.equal(nightLabel("2026-10-17", now), "Sat 17 Oct");
  assert.equal(nightLabel("2025-12-31", now), "Wed 31 Dec 2025");
  assert.equal(nightLabel("2026-10-22", new Date(2026, 9, 23, 1, 0)), "Tonight", "1 a.m. is still the same evening");
  assert.equal(nightLabel("nonsense", now), "nonsense");
  assert.equal(nightLabel(undefined, now), "");
});

test("fmtMinutes", () => {
  assert.equal(fmtMinutes(25), "25 min");
  assert.equal(fmtMinutes(60), "1 h");
  assert.equal(fmtMinutes(95), "1 h 35 min");
  for (const none of [0, null, undefined, -5, 0.2]) assert.equal(fmtMinutes(none), "", String(none));
});

test("trailer helpers only ever build addresses from plain ids", () => {
  assert.equal(isVideoId("dQw4w9WgXcQ"), true);
  for (const bad of ["", "short", "dQw4w9WgXcQx", "dQw4w9WgXc!", "https://youtu.be/dQw4w9WgXcQ", null, undefined, 12345678901, ["dQw4w9WgXcQ"]]) assert.equal(isVideoId(bad), false, String(bad));
  assert.equal(trailerEmbedUrl("dQw4w9WgXcQ"), "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&mute=1&rel=0&modestbranding=1&playsinline=1&enablejsapi=1");
  assert.ok(trailerEmbedUrl("dQw4w9WgXcQ").includes("mute=1"), "trailers must start silent");
  assert.equal(trailerEmbedUrl("x\"><script>"), null, "anything that is not an id gets no address at all");
  assert.equal(trailerEmbedUrl("dQw4w9WgXcQ/../../x"), null);
  assert.ok(trailerEmbedUrl("dQw4w9WgXcQ").startsWith("https://www.youtube-nocookie.com/embed/"));
  assert.equal(trailerSearchUrl("Pikmin 4"), "https://www.youtube.com/results?search_query=Pikmin%204%20official%20trailer");
  assert.equal(trailerSearchUrl("A&B=C#x"), "https://www.youtube.com/results?search_query=A%26B%3DC%23x%20official%20trailer");
  assert.equal(mediaUrl("Kirby & Co", "video"), "/api/media?title=Kirby%20%26%20Co&kind=video");
  assert.equal(mediaUrl("../../x", "shot"), "/api/media?title=..%2F..%2Fx&kind=shot");
});

test("controller steps go to the nearest thing that way, preferring the same row or column", () => {
  const r = (x, y, w = 100, h = 40) => ({ x, y, w, h });
  // a 3 x 2 grid, then a wide button underneath
  const grid = [r(0, 0), r(120, 0), r(240, 0), r(0, 60), r(120, 60), r(240, 60), r(0, 140, 340)];
  assert.equal(padStep(grid, 0, "right"), 1);
  assert.equal(padStep(grid, 1, "right"), 2);
  assert.equal(padStep(grid, 2, "right"), -1, "nothing further right");
  assert.equal(padStep(grid, 1, "down"), 4);
  assert.equal(padStep(grid, 5, "down"), 6, "the wide button below");
  assert.equal(padStep(grid, 6, "up"), 4, "back up from the wide button lands on the tile above its centre");
  assert.equal(padStep(grid, 4, "left"), 3);
  assert.equal(padStep(grid, 0, "up"), -1);
  // the same row beats a closer thing on another row
  const rows = [r(0, 0), r(300, 0), r(110, 50)];
  assert.equal(padStep(rows, 0, "right"), 1, "300 away on the same row beats 110 away on the next one");
  // nothing focused yet: the top-left-most
  assert.equal(padStep([r(200, 100), r(10, 30), r(50, 0)], -1, "down"), 2, "the highest one");
  assert.equal(padStep([r(200, 2), r(10, 0)], -1, "down"), 1, "within a few pixels counts as one row: the leftmost");
  assert.equal(padStep([], 0, "down"), -1);
});

test("watch first: trailer, then co-op clip, then the local preview", () => {
  const id = "abcdefghijk", id2 = "lmnopqrstuv";
  assert.equal(watchFirst({ trailer: id, coop: id2, video: true }, true), "trailer");
  assert.equal(watchFirst({ coop: id2, video: true }, true), "coop");
  assert.equal(watchFirst({ video: true }, true), "video");
  assert.equal(watchFirst({ trailer: id, coop: id2, video: true }, false), "video", "YouTube is off");
  assert.equal(watchFirst({ trailer: id, coop: id2 }, false), null);
  assert.equal(watchFirst({ trailer: "not an id" }, true), null);
});

test("on-screen keyboard editing", () => {
  assert.deepEqual(applyKey("", 0, 0, "H"), { value: "H", caret: 1 });
  assert.deepEqual(applyKey("Hi", 2, 2, "!"), { value: "Hi!", caret: 3 });
  assert.deepEqual(applyKey("Hi", null, null, "!"), { value: "Hi!", caret: 3 }, "no caret means the end");
  assert.deepEqual(applyKey("Hello", 1, 4, "a"), { value: "Hao", caret: 2 }, "typing replaces a selection");
  assert.deepEqual(applyKey("Hello", 5, 5, "Backspace"), { value: "Hell", caret: 4 });
  assert.deepEqual(applyKey("Hello", 1, 3, "Backspace"), { value: "Hlo", caret: 1 }, "backspace removes a selection");
  assert.deepEqual(applyKey("Hello", 0, 0, "Backspace"), { value: "Hello", caret: 0 }, "nothing before the caret");
  assert.deepEqual(applyKey("Hello", 3, 3, "Clear"), { value: "", caret: 0 });
  assert.deepEqual(applyKey("abc", 3, 3, "d", { max: 3 }), { value: "abc", caret: 3 }, "maxlength is respected");
  assert.deepEqual(applyKey("abc", 1, 3, "xyz", { max: 3 }), { value: "axy", caret: 3 }, "a selection frees room");
  assert.deepEqual(applyKey("a", 1, 1, "Enter"), { value: "a", caret: 1 }, "no newline in a one-line box");
  assert.deepEqual(applyKey("a", 1, 1, "Enter", { multiline: true }), { value: "a\n", caret: 2 });
  assert.deepEqual(applyKey("5", null, null, "x", { numeric: true }), { value: "5", caret: 1 }, "numbers only");
  assert.deepEqual(applyKey("5", null, null, ".", { numeric: true }), { value: "5.", caret: 2 });
  assert.deepEqual(applyKey(null, null, null, "7", { numeric: true }), { value: "7", caret: 1 });
  assert.deepEqual(applyKey("abc", 99, -4, "z"), { value: "z", caret: 1 }, "wild positions are clamped, and a reversed selection still selects the text between them");
  assert.deepEqual(applyKey("abc", 99, 99, "z"), { value: "abcz", caret: 4 }, "a caret past the end is the end");
});

test("about lines say only what is known", () => {
  assert.deepEqual(aboutLines(null), []);
  assert.deepEqual(aboutLines({}), []);
  const lines = aboutLines({ year: 2014, developer: "Creative Assembly", publisher: "Sega", rating: "ESRB M", descriptors: ["blood and gore", "violence"], genres: ["survival horror"], hltb: { main: 8, extras: 12.5, full: null }, scores: { metacritic: 81, steamPct: 94 }, controller: "full", localPlay: "One player" });
  assert.deepEqual(lines, ["2014 · Creative Assembly · published by Sega", "Age rating: ESRB M (blood and gore, violence)", "Genre: survival horror", "How long: 8 h main story, 12.5 h with extras (HowLongToBeat)", "Scores: Metacritic 81 · 94% positive on Steam", "Controller: full support", "Playing together: One player"]);
  assert.deepEqual(aboutLines({ developer: "X", publisher: "X" }), ["X"], "a publisher equal to the developer is not repeated");
  assert.deepEqual(sourceNames({ sources: ["https://store.steampowered.com/app/1", "https://www.nintendo.com/x", "https://store.steampowered.com/app/2", "not a url"] }), ["store.steampowered.com", "nintendo.com"]);
});

test("player counts read naturally", () => {
  assert.equal(playersLabel("1"), "1 player");
  assert.equal(playersLabel("2"), "2 players");
  assert.equal(playersLabel("1-4"), "1-4 players");
});

test("pace and search helpers", () => {
  assert.equal(paceLine({ low: 40, high: 60, n: 5 }), "Usually 40 to 60 min for us");
  assert.equal(paceLine({ low: 45, high: 45, n: 3 }), "Usually 45 min for us");
  assert.equal(paceLine({ low: 30 }), "Usually 30 min for us");
  for (const none of [null, undefined, {}, { low: 0, high: 0 }, { low: -5 }]) assert.equal(paceLine(none), null, JSON.stringify(none));
  assert.equal(coopSearchUrl("Pikmin 4"), "https://www.youtube.com/results?search_query=Pikmin%204%20local%20co-op%20gameplay");
  assert.equal(coopSearchUrl("A&B=C"), "https://www.youtube.com/results?search_query=A%26B%3DC%20local%20co-op%20gameplay");
});

test("watchUrl is only ever built from a plain id", () => {
  assert.equal(watchUrl("dQw4w9WgXcQ"), "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
  for (const bad of ["", "short", "dQw4w9WgXcQ&x=1", "dQw4w9WgXcQ/../x", "javascript:alert(1)", null, undefined, 5]) assert.equal(watchUrl(bad), null, String(bad));
});

test("fmtSeconds speaks ES-DE's short play times", () => {
  assert.equal(fmtSeconds(16), "under a minute");
  assert.equal(fmtSeconds(59), "under a minute");
  assert.equal(fmtSeconds(60), "1 min");
  assert.equal(fmtSeconds(1409), "23 min");
  assert.equal(fmtSeconds(5400), "1 h 30 min");
  for (const none of [0, null, undefined, -1, 0.5]) assert.equal(fmtSeconds(none), "", String(none));
});

test("ratingWords names each player and leaves out skips and unknown values", () => {
  const group = [{ id: "p1", name: "Sam" }, { id: "p2", name: "Alex" }, { id: "p3", name: "Kai" }];
  assert.equal(ratingWords({ p1: "up", p2: "down", p3: "up" }, group), "Sam liked it · Alex not really · Kai liked it");
  assert.equal(ratingWords({ p1: "skip", p2: "up" }, group), "Alex liked it");
  assert.equal(ratingWords({ p1: "love", p9: "up" }, group), "", "unknown values and past players are left out");
  assert.equal(ratingWords(undefined, group), "");
});

test("cleanFilters with nothing gives a fresh default each time", () => {
  const a = cleanFilters(), b = cleanFilters(null);
  assert.deepEqual(a, DEFAULT_FILTERS);
  a.A.push("cozy");
  assert.deepEqual(b.A, [], "defaults must not be shared between calls");
  assert.deepEqual(DEFAULT_FILTERS.A, []);
});

test("cleanFilters keeps valid values and drops hostile or unknown ones", () => {
  const f = cleanFilters({ where: "couch", len: "quick", gap: 4, n: 2, match: "any", platform: "Switch", hide: "yes", two: 1, A: ["cozy", "nope", {}, "silly"], B: "spooky", extra: "<script>" });
  assert.equal(f.where, "couch");
  assert.equal(f.len, "quick");
  assert.equal(f.gap, 4);
  assert.equal(f.n, 2);
  assert.equal(f.match, "any");
  assert.equal(f.platform, "Switch");
  assert.equal(f.hide, false, "non-booleans fall back");
  assert.equal(f.two, false);
  assert.deepEqual(f.A, ["cozy", "silly"]);
  assert.deepEqual(f.B, []);
  assert.equal("extra" in f, false);
  for (const bad of [{ where: "<b>" }, { len: "forever" }, { gap: 99 }, { gap: "4" }, { n: 9 }, { n: 0 }, { n: "4" }, { match: "most" }, "string", 5, []]) {
    assert.deepEqual(cleanFilters(bad), DEFAULT_FILTERS, JSON.stringify(bad));
  }
});

test("built-in presets are valid as written", () => {
  for (const p of BUILTIN_PRESETS) {
    const clean = cleanFilters(p.f);
    for (const k of Object.keys(p.f)) assert.deepEqual(clean[k], p.f[k], `${p.name}: ${k} was changed by cleaning`);
  }
});

test("daysSince and ago", () => {
  const today = new Date(), iso = d => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const back = n => iso(new Date(today.getFullYear(), today.getMonth(), today.getDate() - n));
  assert.equal(daysSince(back(0)), 0);
  assert.equal(daysSince(back(1)), 1);
  assert.equal(daysSince(back(30)), 30);
  assert.equal(daysSince("2999-01-01"), 0, "a future date never goes negative");
  for (const bad of [null, undefined, "", "yesterday", "2026-1-5", "20261005"]) assert.equal(daysSince(bad), null, String(bad));
  assert.equal(ago(0), "today");
  assert.equal(ago(1), "yesterday");
  assert.equal(ago(5), "5 days ago");
});

test("monogram drops filler words and punctuation and never comes back empty", () => {
  assert.equal(monogram("A Hat in Time"), "HIT");
  assert.equal(monogram("The Legend of Zelda"), "LZ");
  assert.equal(monogram("Mario Kart 8 Deluxe"), "MK8");
  assert.equal(monogram("Portal 2 (co-op)"), "P2C");
  assert.equal(monogram("it takes two"), "ITT");
  assert.equal(monogram("A of the"), "A ");
  assert.ok(monogram("!!!").length > 0);
});

test("posterColor is stable and always one of the palette colours", () => {
  for (const t of ["", "A", "Overcooked! All You Can Eat", "日本語のゲーム", "x".repeat(500)]) {
    assert.equal(posterColor(t), posterColor(t));
    assert.ok(COLORS.includes(posterColor(t)), t);
  }
  assert.ok(new Set(["Kirby", "Portal", "Hades", "Celeste", "SOMA", "Outlast"].map(posterColor)).size >= 3, "titles should spread over several colours");
});

test("quick picks are valid filters and each one narrows something", () => {
  assert.ok(QUICK_PICKS.length >= 4 && QUICK_PICKS.length <= 9, "a handful, not a wall (three rows of three at most)");
  assert.equal(QUICK_PICKS.length % 3, 0, "the grid is three across, so it should fill whole rows");
  assert.equal(new Set(QUICK_PICKS.map(q => q.id)).size, QUICK_PICKS.length);
  for (const q of QUICK_PICKS) {
    const f = resolveFilters(q.f, 3), clean = cleanFilters(f);                       // a group of three, as the page would resolve it
    for (const k of Object.keys(f)) assert.deepEqual(clean[k], f[k], `${q.id}: ${k} would be changed by cleaning`);
    assert.ok(describeFilters(q.f).length > 0, `${q.id} should show up in the summary`);
    assert.ok(q.label && q.icon);
  }
});

test("describeFilters speaks plainly and says nothing when nothing is narrowed", () => {
  assert.deepEqual(describeFilters(), []);
  assert.deepEqual(describeFilters(DEFAULT_FILTERS), []);
  assert.deepEqual(describeFilters({ len: "quick", where: "couch", n: 2 }), ["same couch", "quick rounds", "two players"]);
  assert.deepEqual(describeFilters({ n: 5 }), ["room for 5"]);
  assert.deepEqual(describeFilters({ platform: "Switch" }), ["on Switch"]);
  assert.deepEqual(describeFilters({ A: ["cozy", "silly"] }), ["Cozy + Silly"]);
  assert.deepEqual(describeFilters({ A: ["cozy", "silly"], match: "any" }), ["Cozy or Silly"]);
  assert.deepEqual(describeFilters({ two: true, A: ["cozy"], B: ["spooky"] }), ["Cozy or Spooky"]);
  assert.deepEqual(describeFilters({ hide: true, gap: 4 }), ["skill gap 4+", "hiding played ones"]);
  assert.deepEqual(describeFilters({ fresh: true }), ["new to us"]);
  assert.deepEqual(describeFilters({ one: true }), ["one controller"]);
  assert.equal(cleanFilters({ one: "yes" }).one, false);
  assert.equal(cleanFilters({ one: true }).one, true);
  assert.equal(cleanFilters({ fresh: "yes" }).fresh, false, "only a real boolean turns it on");
  assert.equal(cleanFilters({ fresh: true }).fresh, true);
  assert.deepEqual(describeFilters({ len: "forever", where: "<b>", A: ["nope"] }), [], "junk is ignored");
});

test("old addresses still lead somewhere", () => {
  assert.deepEqual(legacyRoute("games"), ["shelf", "games"]);
  assert.deepEqual(legacyRoute("library"), ["shelf", "library"]);
  assert.deepEqual(legacyRoute("nights"), ["journal", "nights"]);
  assert.deepEqual(legacyRoute("history"), ["journal", "stats"]);
  assert.deepEqual(legacyRoute("health"), ["more", "health"]);
  assert.deepEqual(legacyRoute("settings"), ["more", "settings"]);
  assert.deepEqual(legacyRoute("shelf"), ["shelf", null]);
  for (const bad of ["", "nope", undefined, "__proto__", "constructor"]) assert.equal(legacyRoute(bad), null, String(bad));
});

test("tabs and moods have the shape the views rely on", () => {
  assert.deepEqual(TABS.map(t => t[0]), ["tonight", "shelf", "journal", "more"]);
  assert.ok(TABS.every(t => t.length === 3 && t[2].length > 0), "every tab has a label and an icon");
  for (const [tab, subs] of Object.entries(SUBS)) {
    assert.ok(TABS.some(t => t[0] === tab), tab + " must be a real place");
    assert.ok(subs.length >= 2 && subs.every(s => s.length === 2));
  }
  assert.ok(MOODS.every(m => m.length === 2 && typeof m[0] === "string" && typeof m[1] === "string"));
});

test("season: Halloween all through October and never otherwise", () => {
  assert.equal(season(new Date(2026, 9, 1, 0, 0)), "halloween");
  assert.equal(season(new Date(2026, 9, 31, 23, 59)), "halloween");
  assert.equal(season(new Date(2026, 10, 1, 0, 0)), null);
  assert.equal(season(new Date(2026, 8, 30, 23, 59)), null);
});

test("quickOrder: the spooky tile leads in the Halloween look and nothing is lost", () => {
  const h = quickOrder(QUICK_PICKS, "halloween");
  assert.equal(h[0].id, "spooky");
  assert.equal(h.length, QUICK_PICKS.length);
  assert.deepEqual(new Set(h.map(q => q.id)), new Set(QUICK_PICKS.map(q => q.id)));
  assert.deepEqual(quickOrder(QUICK_PICKS, undefined), QUICK_PICKS);
});

test("the everyone tile fits the group, and says so plainly", () => {
  const pair = QUICK_PICKS.find(q => q.id === "pair");
  assert.deepEqual(resolveFilters(pair.f, 4), { where: "couch", n: 4 });
  assert.deepEqual(resolveFilters(pair.f, 99), { where: "couch", n: 8 }, "never more than eight");
  assert.deepEqual(resolveFilters({ len: "quick" }, 4), { len: "quick" }, "other tiles are untouched");
  assert.equal(quickLabel(pair, 1), "Just me");
  assert.equal(quickLabel(pair, 2), "Just the two of us");
  assert.equal(quickLabel(pair, 5), "All 5 of us");
  assert.equal(quickLabel(QUICK_PICKS[0], 5), QUICK_PICKS[0].label);
  assert.equal(cleanFilters(resolveFilters(pair.f, 3)).n, 3);
});
