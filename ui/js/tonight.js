/* The Tonight view: one calm screen. A big "Surprise us" button and a few feelings; everything else (filters, the wheel, three cards, the plan, the taste test) is one tap away. */
import { S, api, chip, gameByTitle, h, icon, mascot, nm, pids, players, poster, reload, settings, toast } from "./core.js";
import { LEN_HELP, LEN_NAME, MOODS, QUICK_PICKS, ago, cleanFilters, daysSince, describeFilters, fmtMinutes, fmtSeconds, greeting, nightKey, paceLine, playersLabel, quickLabel, quickOrder, resolveFilters, tilesFor, watchFirst } from "./logic.js";
import { buildTurn, go, render } from "./main.js";
import { celebrateSpooky } from "./spooky.js";
import { foundSummary } from "./library.js";
import { presetRow } from "./presets.js";
import { drawWheel, resizeWheel } from "./wheel.js";
import { gameSheet } from "./games.js";
import { canWatch } from "./watch.js";

let previewTimer;
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const wait = ms => new Promise(r => setTimeout(r, ms));

// ---------- filters (shown in a sheet)
export function schedulePreview() { clearTimeout(previewTimer); previewTimer = setTimeout(refreshPreview, 120); }

export async function refreshPreview() {
  const p = await api("/api/preview", { filters: S.filters });
  S.wheel = p.wheel; S.matching = p.matching.length; S.pool = p.pool; S.note = p.note;
  const text = S.matching === S.boot.games.length && !S.note ? S.boot.games.length + " games in the draw" : S.matching + " of " + S.boot.games.length + " games fit" + (S.note ? ". " + S.note : "");
  document.querySelectorAll(".count").forEach(c => { c.textContent = text; });
  if (!S.spinning) drawWheel();
}

export function setFilter(k, v) { S.filters[k] = v; S.result = null; S.seen = []; refilterUI(); schedulePreview(); }

export function toggleMood(which, m) { const a = S.filters[which]; const i = a.indexOf(m); i < 0 ? a.push(m) : a.splice(i, 1); setFilter(which, a); }

export function refilterUI() {
  const f = document.getElementById("filters"); if (!f) return;
  const idx = [...f.querySelectorAll("button")].indexOf(document.activeElement), next = buildFilters();      // keep keyboard focus on the same chip after the re-render
  f.replaceWith(next);
  if (idx >= 0) { const b = next.querySelectorAll("button")[idx]; if (b) b.focus(); }
}

export function buildFilters() {
  const F = S.filters, row = (label, ...kids) => h("div", { class: "row", role: "group", "aria-label": label }, h("span", { class: "t" }, label), kids);
  const set = (k, v) => () => setFilter(k, v);
  return h("fieldset", { id: "filters", disabled: S.spinning },
    S.boot.modes ? null : row("Where", chip("Any", F.where === "any", set("where", "any")), chip("Same couch, one screen", F.where === "couch", set("where", "couch")), chip("Online, two computers", F.where === "online", set("where", "online"))),
    row(F.two ? "First mood" : "Mood", MOODS.map(([v, l]) => chip(l, F.A.includes(v), () => toggleMood("A", v)))),
    F.two ? row("Second mood", MOODS.map(([v, l]) => chip(l, F.B.includes(v), () => toggleMood("B", v)))) : null,
    row("Moods", chip("Must fit all picked", F.match === "all", set("match", "all")), chip("Any of them", F.match === "any", set("match", "any")), chip("Two different moods", F.two, set("two", !F.two))),
    row("Time", chip("Any", F.len === "any", set("len", "any")), chip("Quick (" + LEN_HELP.quick + ")", F.len === "quick", set("len", "quick")), chip("An evening (" + LEN_HELP.evening + ")", F.len === "evening", set("len", "evening")), chip("Long-running (" + LEN_HELP.long + ")", F.len === "long", set("len", "long"))),
    row("Players", chip("Any", F.n === "any", set("n", "any")), Array.from({ length: Math.max(4, pids().length) }, (_, i) => i + 1).map(n => chip(n === 1 ? "1 player" : n === 2 ? "2 players" : "Room for " + n, F.n === n, set("n", n)))),
    (S.boot.platforms || []).length > 1 ? row("Platform", chip("Any", !F.platform, set("platform", "")), S.boot.platforms.map(p => chip(p, F.platform === p, set("platform", p)))) : null,
    row("Skill gap", chip("Any", F.gap === 1, set("gap", 1)), chip("3+ dots", F.gap === 3, set("gap", 3)), chip("4+ dots", F.gap === 4, set("gap", 4)), chip("5 dots (skill barely matters)", F.gap === 5, set("gap", 5))),
    row("Show", chip("Hide ones played or marked not for us", F.hide, set("hide", !F.hide)), chip("Only games new to us", F.fresh, set("fresh", !F.fresh)), chip("One controller (one plays, the others watch)", F.one, set("one", !F.one)),
      S.boot.steamShared && !S.boot.modes ? chip("Games we all own on Steam", F.owned, set("owned", !F.owned)) : null));
}

export function openFilters() {
  const dlg = document.getElementById("dlg");
  dlg.className = "sheet";
  dlg.replaceChildren(h("h2", {}, "Narrow it down"), h("p", { class: "small" }, "Everything you set here applies to the next pick."), presetRow(), buildFilters(),
    h("p", { class: "small count", role: "status", "aria-live": "polite" }, ""),
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: () => dlg.close() }, "Done"),
      h("button", { class: "btn", type: "button", onclick: () => { S.filters = cleanFilters(); S.result = null; openFilters(); schedulePreview(); } }, "Reset")));
  dlg.onclose = () => { dlg.className = ""; dlg.onclose = null; if (S.tab === "tonight" && S.home === "start") renderHome(); };
  dlg.showModal(); schedulePreview();
}

// ---------- the screen
/** Puts something on the stage. Nothing vanishes: the old content is moved to a ghost layer that fades out behind the new content, which is in the page immediately
 *  (so focus, bubbles and everything else work as before). `quiet` swaps with no fade, for a refresh of what is already there. */
function setStage(node, opts) {
  const home = document.getElementById("home"); if (!home) return home;
  const old = [...home.childNodes];
  if (reduced() || (opts && opts.quiet) || !old.length || !home.animate) { home.replaceChildren(node); return home; }
  const r = home.getBoundingClientRect(), ghost = h("div", { class: "stage-ghost", "aria-hidden": "true", style: "width:" + r.width + "px;height:" + r.height + "px" });
  ghost.inert = true; ghost.append(...old);
  home.replaceChildren(node); home.append(ghost);
  const gone = () => ghost.remove(), a = ghost.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(.985)" }], { duration: 200, easing: "ease-out", fill: "forwards" });
  a.onfinish = gone; a.oncancel = gone; setTimeout(gone, 450);                  // and if the window never produces frames, the ghost still goes
  return home;
}

function backToStart() { S.home = "start"; S.result = null; S.seen = []; renderHome(); }

function pastDay(g) { return g.plays ? daysSince(g.plays.last) : null; }

/** Games marked "in progress" as small buttons: one tap launches and records it. */
function continuePills() {
  const list = S.boot.games.filter(g => g.progress && g.progress.ongoing && g.launchable).sort((a, b) => (pastDay(a) ?? 9999) - (pastDay(b) ?? 9999)).slice(0, 2);
  return list.map(g => h("button", { class: "pill", type: "button", title: g.progress.note || "Pick up where you left off", onclick: async ev => {
    const b = ev.currentTarget; b.disabled = true;
    try { const l = await api("/api/launch", { title: g.t }); toast(l.message); if (l.started) await confirmPlay(g, true); } catch (e) { toast(e.message); }
    b.disabled = false;
  } }, icon("back"), "Continue " + g.t));
}

function soFarLine(nt) {
  const n = nt && nt.nights && nt.nights[0];
  if (!n || n.date !== nightKey()) return null;
  const played = n.games.filter(g => g.minutes || g.esde);
  if (!played.length) return null;
  const how = g => fmtMinutes(g.minutes) || fmtSeconds(g.esdeSeconds) || "";
  return h("p", { class: "small sofar" }, "Tonight so far: " + played.map(g => g.title + " (" + how(g) + ")").join(" · ") + " ", h("button", { class: "link", type: "button", onclick: () => go("journal", "nights") }, "Open the journal"));
}

/** The empty shelf's welcome: what the library detectors found on this computer, one tap to bring it in, or the sample games. */
function welcomeView() {
  const found = h("p", { class: "lede count", role: "status" }, "Looking for your games on this computer...");
  const card = h("div", { class: "hero welcome" }, mascot("hero-mascot"), h("p", { class: "kicker" }, greeting()), h("h1", {}, "Let's find your games"), found,
    h("div", { class: "row", style: "justify-content:center" },
      h("button", { class: "btn primary big", type: "button", onclick: () => go("shelf", "library") }, "Choose what to bring in"),
      h("button", { class: "btn", type: "button", onclick: async () => { const r = await api("/api/sample", {}); await reload(); toast("Added " + r.added + " sample games."); renderHome(); } }, "Start with 20 well-known games")),
    h("p", { class: "small" }, "Then set your names in More > Settings: a couple, a few friends, up to eight."));
  api("/api/library").then(d => {
    const sum = foundSummary(d.sources);
    found.textContent = sum ? "Found: " + sum + "." : "Nothing found on this computer's launchers yet. Add a game folder in Settings, or add games by hand on the Shelf.";
  }).catch(() => { found.textContent = ""; });
  return card;
}

/** How you're playing tonight, when both groups exist: one tap switches who's playing, their turn, and which games are drawn. */
function modeSwitch() {
  if (!S.boot.modes) return null;
  const pick = async m => {
    if (m === S.boot.mode || S.spinning) return;
    try { await api("/api/mode", { mode: m }); await reload(); buildTurn(); S.result = null; S.seen = []; renderHome(); schedulePreview(); toast(m === "online" ? "Online tonight: " + players().map(p => p.name).join(", ") : "On the couch tonight: " + players().map(p => p.name).join(", ")); }
    catch (e) { toast(e.message); }
  };
  return h("div", { class: "modeswitch", role: "group", "aria-label": "How you're playing tonight" },
    [["couch", "On the couch"], ["online", "Online"]].map(([m, l]) => h("button", { type: "button", class: "chip", "aria-pressed": String(S.boot.mode === m), onclick: () => pick(m) }, l)));
}

function heroView(mode) {
  if (!S.boot.games.length) return welcomeView();
  const b = S.boot, narrowed = describeFilters(S.filters);
  const tiles = quickOrder(tilesFor(QUICK_PICKS, b.mode), document.body.dataset.season).map((q, i) => h("button", { class: "qtile", type: "button", "data-id": q.id, style: "--i:" + i, disabled: S.spinning, onclick: () => spin(resolveFilters(q.f, pids().length)) }, h("span", { class: "qicon" }, icon(q.icon)), h("span", {}, quickLabel(q, pids().length))));
  return h("div", { class: "hero" + (mode ? " " + mode : "") },
    mascot("hero-mascot"),
    modeSwitch(),
    h("p", { class: "kicker" }, greeting()),
    h("h1", {}, "What shall we play?"),
    h("p", { class: "lede count" }, b.games.length + " games in the draw"),
    h("button", { class: "btn primary big", id: "spin", type: "button", disabled: S.spinning, onclick: () => spin() }, "Surprise us"),
    narrowed.length ? h("p", { class: "narrowed" }, "Narrowed to " + narrowed.join(" · ") + " ", h("button", { class: "link", type: "button", onclick: () => { S.filters = cleanFilters(); renderHome(); schedulePreview(); } }, "Clear")) : null,
    h("p", { class: "orline" }, "or pick a feeling"),
    h("div", { class: "qgrid" }, tiles),
    h("div", { class: "quiet" },
      h("button", { class: "link", type: "button", onclick: openFilters }, icon("sliders"), "Narrow it down"),
      h("details", { class: "ways" }, h("summary", {}, "More ways to choose"),
        h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: threeCards }, "Three cards"), h("button", { class: "btn", type: "button", onclick: planEvening }, "Plan the evening"),
          h("button", { class: "btn", type: "button", onclick: () => tasteTest() }, "Quick taste test"), h("button", { class: "btn", type: "button", onclick: openWheel }, "Spin the wheel"), h("button", { class: "btn", type: "button", onclick: coinToss }, icon("coin"), "Flip a coin"),
          b.vetoes.length ? h("button", { class: "btn", type: "button", onclick: clearVetoes }, "Clear " + b.vetoes.length + " veto" + (b.vetoes.length > 1 ? "es" : "")) : null))),
    h("div", { class: "contextrow" }, continuePills()),
    soFarLine(S.nights),
    h("p", { id: "padhint", class: "small" + (S.pad ? "" : " hide") }, "Controller: D-pad or stick moves · A selects · B goes back · X another · Y watch · LB and RB change place · Start surprises us."));
}

/** Draws the stage. Arriving on Tonight plays the full entrance once; coming back to the start screen later fades in calmly; redrawing the screen that is already there
 *  (closing the filters, clearing a veto) swaps it in place with no animation at all, so nothing blinks. */
export function renderHome() {
  if (S.home === "result" && S.result) { showResult(S.result); return; }
  const home = document.getElementById("home"), again = !!(home && home.querySelector(".hero"));
  const mode = again ? "still" : S.arriving ? "" : "calm";
  S.arriving = false;
  setStage(heroView(mode), { quiet: again });
}

export async function viewTonight() {
  const b = S.boot;
  S.nights = await api("/api/nights").catch(() => null);
  return h("div", {}, b.notices && b.notices.length ? h("div", { class: "notice", role: "status" }, b.notices.map(n => h("div", {}, n))) : null, h("section", { id: "home", class: "home", "aria-live": "polite" }));
}

/** Called by the router once the Tonight view is on the page (the screen inside #home is drawn here, not earlier). */
export function afterTonight() { S.arriving = true; renderHome(); schedulePreview(); }

// ---------- ways of choosing
export async function threeCards() {
  let r; try { r = await api("/api/finalists", { filters: S.filters }); } catch (e) { setStage(h("div", { class: "errlist", role: "alert" }, e.message)); return; }
  if (!r.ok) { setStage(emptyCard(r.message)); return; }
  const ids = pids(), other = ids[(ids.indexOf(S.boot.turn) + 1) % ids.length] || S.boot.turn;
  const order = [other, S.boot.turn];                 // the next player strikes first, then whoever's pick it is (it balances the boost the chooser's favourites already got)
  let left = r.finalists.slice(), step = 0;
  const draw = () => {
    if (left.length === 1) {
      const w = left[0];
      showResult({ winner: w.title, prob: w.prob, why: ["Three games were drawn with the same odds as a spin, and two were removed. This one had about a " + Math.round(w.prob * 100) + "% share of the first draw.", ...w.why], runners: [], skipped: r.skipped });
      return;
    }
    setStage(h("div", { class: "stagecard solo" }, h("h2", {}, nm(order[step]) + ", remove one."),
      h("p", { class: "small" }, "Three games, each with its real share of the draw. Then " + nm(order[1 - step]) + " removes one and the last one stays."),
      h("div", { class: "threecards" }, left.map(f => { const g = gameByTitle(f.title);
        return h("button", { class: "pickcard", type: "button", onclick: () => { left = left.filter(x => x !== f); step++; draw(); }, "aria-label": "Remove " + f.title },
          poster(g || { t: f.title, cover: false }, null, ""), h("b", {}, f.title), h("span", { class: "small" }, "about " + Math.round(f.prob * 100) + "% of the draw"), g && g.time ? h("span", { class: "small" }, g.time) : null); })),
      h("button", { class: "link", type: "button", onclick: backToStart }, "Back")));
  };
  draw();
}

function emptyCard(message) {
  return h("div", { class: "stagecard solo" }, mascot("hero-mascot"), h("h2", {}, "Nothing fits"), h("p", { class: "lede" }, message),
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: openFilters }, "Change the filters"), h("button", { class: "link", type: "button", onclick: backToStart }, "Back")));
}

export async function tasteTest() {
  const dlg = document.getElementById("dlg"); dlg.className = "";
  const ask = h("div", {});
  dlg.replaceChildren(h("h2", { style: "margin-top:0" }, "Quick taste test"), h("p", { class: "small" }, "About two minutes. Games show one at a time: say if it looks fun. These are guesses, so they count for less than a real \"loved it\"."),
    h("p", {}, "Who's answering?"), h("div", { class: "row" }, players().map(p => h("button", { class: "btn primary", type: "button", onclick: () => run(p.id) }, p.name)), h("button", { class: "btn", type: "button", onclick: () => dlg.close() }, "Cancel")), ask);
  dlg.showModal();
  async function run(who) {
    let r; try { r = await api("/api/taste", { who, n: 10 }); } catch (e) { toast(e.message); return; }
    const list = r.games; let i = 0; const tally = { curious: 0, pass: 0, skip: 0 };
    const answer = async mark => {
      const g = list[i];
      if (mark) { try { await api("/api/marks", { title: g.title, who, mark }); } catch (e) { toast(e.message); } tally[mark]++; } else tally.skip++;
      i++; show();
    };
    function show() {
      if (i >= list.length) {
        dlg.replaceChildren(h("h2", { style: "margin-top:0" }, "Thanks"), h("p", {}, tally.curious + " looked fun, " + tally.pass + " not really, " + tally.skip + " not sure. The fun ones come up a bit more often."),
          h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", "data-pad": "a", onclick: () => run(who) }, "Ten more"), h("button", { class: "btn", type: "button", "data-pad": "b", onclick: async () => { dlg.close(); await reload(); render(); } }, "Done")));
        return;
      }
      const g = list[i];
      dlg.replaceChildren(h("p", { class: "small" }, nm(who) + " · " + (i + 1) + " of " + list.length),
        h("div", { class: "tasteq" }, poster({ t: g.title, cover: g.cover }, null, ""),
          h("div", {}, h("h2", { style: "margin:0 0 .4rem" }, g.title), h("p", {}, g.line))),
        h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", "data-pad": "a", onclick: () => answer("curious") }, "Looks fun"), h("button", { class: "btn", type: "button", "data-pad": "b", onclick: () => answer("pass") }, "Not really"),
          h("button", { class: "btn", type: "button", "data-pad": "x", onclick: () => answer(null) }, "Not sure"), h("button", { class: "btn", type: "button", onclick: async () => { dlg.close(); await reload(); render(); } }, "Stop"),
          h("span", { class: "small" }, S.pad ? "A looks fun · B not really · X not sure" : "")));
    }
    show();
  }
}

export async function planEvening() {
  let r; try { r = await api("/api/plan", { filters: S.filters }); } catch (e) { setStage(h("div", { class: "errlist", role: "alert" }, e.message)); return; }
  if (!r.ok) { setStage(emptyCard(r.message)); return; }
  setStage(h("div", { class: "stagecard solo" }, h("h2", {}, "A short evening"),
    h("ol", { class: "plan" }, r.steps.map(s => { const g = gameByTitle(s.title); return h("li", {}, h("b", {}, s.step), g ? poster(g, null, "") : null, h("span", {}, s.title, g ? h("span", { class: "small" }, " " + (paceLine(g.mySessions) || LEN_NAME[g.len].toLowerCase())) : null)); })),
    h("p", { class: "small" }, "Drawn with the same weights as a pick. Use \"Surprise us\" if you'd rather have one game."),
    h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: planEvening }, "Another plan"), h("button", { class: "link", type: "button", onclick: backToStart }, "Back"))));
}

/** Who goes first (or takes player one): a coin that flips and lands on someone. Pure chance from the browser's secure random source; nothing is counted or remembered.
 *  Two players get a face each; a bigger group's coin lands face up on whoever was drawn. */
export async function coinToss() {
  const ids = pids(), pickOne = n => { const b = new Uint32Array(1); let x; do { crypto.getRandomValues(b); x = b[0]; } while (x >= Math.floor(4294967296 / n) * n); return x % n; };
  const winner = ids[pickOne(ids.length || 1)] || "", two = ids.length === 2, initials = id => nm(id).slice(0, 2).toUpperCase();
  const front = two ? ids[0] : winner, back = two ? ids[1] : winner;
  const coin = h("div", { class: "coin", "aria-hidden": "true" }, h("div", { class: "face front" }, initials(front)), h("div", { class: "face back" }, initials(back)));
  const up = winner === front;
  coin.style.setProperty("--end", (up ? 1800 : 1980) + "deg");
  const result = h("p", { class: "lede coinresult hide", role: "status" }, nm(winner) + " goes first.");
  setStage(h("div", { class: "stagecard solo" }, h("p", { class: "kicker" }, "Flip a coin"), h("div", { class: "cointable" }, coin), result,
    h("div", { class: "row", style: "justify-content:center" }, h("button", { class: "btn", type: "button", onclick: coinToss }, "Flip again"), h("button", { class: "link", type: "button", onclick: backToStart }, icon("back"), "Back"))));
  if (reduced()) { coin.style.transform = "rotateY(" + (up ? 0 : 180) + "deg)"; result.classList.remove("hide"); return; }
  coin.classList.add("flipping");
  await wait(1700); result.classList.remove("hide");
}

export async function setTurn(who) { const r = await api("/api/turn", { who }); S.boot.turn = r.turn; buildTurn(); toast(nm(r.turn) + " has the pick."); }

export async function clearVetoes() { await api("/api/clear-vetoes", {}); S.boot.vetoes = []; renderHome(); toast("Vetoes cleared."); }

export function setSpinning(v) {
  S.spinning = v; const f = document.getElementById("filters"); if (f) f.disabled = v;
  document.querySelectorAll("#spin, .qtile").forEach(b => { b.disabled = v; });
  document.querySelectorAll("#presets button").forEach(b => { b.disabled = v; });
}

/** The moon in the corner hops when a pick lands. */
function cheer() {
  const m = document.querySelector(".brand .mascot"); if (!m || reduced()) return;
  m.classList.remove("cheer"); void m.getBoundingClientRect(); m.classList.add("cheer"); setTimeout(() => m.classList.remove("cheer"), 1000);
}

/** A few bubbles drift up from the card (skipped when the system asks for reduced motion). */
function celebrate(el) {
  if (!el || reduced()) return;
  const tints = ["var(--accent)", "var(--sky)", "var(--mint)", "var(--sand)"], box = h("div", { class: "confetti", "aria-hidden": "true" });
  if (!celebrateSpooky(box)) for (let i = 0; i < 12; i++) {
    const s = h("span", {}), size = 8 + Math.round(Math.random() * 16), spread = (Math.random() - 0.5) * 260;
    s.style.setProperty("--s", size + "px"); s.style.setProperty("--bc", tints[i % tints.length]);
    s.style.setProperty("--dx", Math.round(spread) + "px"); s.style.setProperty("--dy", -(90 + Math.round(Math.random() * 110)) + "px"); s.style.animationDelay = (i * 45) + "ms";
    box.append(s);
  }
  el.append(box); setTimeout(() => box.remove(), 2000);
}

/** The pick. `preset` (a QUICK_PICKS filter set) replaces the current filters first. The server decides; the page only dresses it up. */
/** `again` is "Another": the same round, so what it already showed is left out (the server starts over once everything has been shown). */
export async function spin(preset, again) {
  if (S.spinning) return;
  if (preset) S.filters = cleanFilters(preset);
  S.seen = again ? (S.seen || []) : [];
  setSpinning(true);
  let r; try { r = await api("/api/pick", { filters: S.filters, seen: S.seen }); } catch (e) { setSpinning(false); setStage(h("div", { class: "errlist", role: "alert" }, e.message)); return; }
  if (!r.ok) { setSpinning(false); setStage(emptyCard(r.message)); return; }
  S.result = r; S.wheel = r.wheel; S.home = "dealing"; S.seen.push(r.winner);
  if (!reduced()) await deal(r.reel && r.reel.length ? r.reel : r.wheel, r.winner);
  setSpinning(false); S.home = "result"; showResult(r);
}

/** The reveal: names flick past, slowing down, and land on the winner. */
async function deal(titles, winner) {
  const others = titles.filter(t => t !== winner), line = h("div", { class: "dealt", "aria-hidden": "true" }, " ");
  setStage(h("div", { class: "stagecard solo dealing" }, mascot("hero-mascot thinking"), h("p", { class: "kicker" }, "Choosing"), line));
  const tick = text => { line.textContent = text; line.classList.remove("tick"); void line.offsetWidth; line.classList.add("tick"); };      // restarts the little slide-in on every name
  const delays = [70, 70, 75, 85, 95, 110, 130, 160, 200, 250, 320, 420];
  for (let i = 0; i < delays.length; i++) { tick(others.length ? others[i % others.length] : winner); await wait(delays[i]); }      // the server already shuffled them: no repeats unless the pool is tiny
  tick(winner); line.classList.add("landed"); await wait(620);
}

// ---------- the wheel, for anyone who misses it (a sheet)
export function openWheel() {
  const dlg = document.getElementById("dlg"); dlg.className = "sheet wheelsheet";
  dlg.replaceChildren(h("h2", {}, "Spin the wheel"), h("p", { class: "small" }, "It shows 12 of the games the filters allow; the real odds are listed under the result."),
    h("div", { class: "wheelbox" }, h("canvas", { id: "wheel", role: "img", "aria-label": "Spinner wheel" }), h("div", { class: "pointer", "aria-hidden": "true" })),
    h("div", { class: "row", style: "justify-content:center" }, h("button", { class: "btn primary", id: "wheel-spin", type: "button", onclick: spinWheel }, "Spin"), h("button", { class: "btn", type: "button", onclick: () => dlg.close() }, "Close")));
  dlg.onclose = () => { dlg.className = ""; dlg.onclose = null; };
  dlg.showModal(); resizeWheel(); schedulePreview();
}

async function spinWheel() {
  if (S.spinning) return;
  const dlg = document.getElementById("dlg"), btn = document.getElementById("wheel-spin");
  let r; try { r = await api("/api/pick", { filters: S.filters }); } catch (e) { toast(e.message); return; }
  if (!r.ok) { S.wheel = []; drawWheel(); toast(r.message); return; }
  S.wheel = r.wheel; S.result = r; (S.seen = S.seen || []).push(r.winner);
  const n = r.wheel.length, seg = 2 * Math.PI / n, tau = 2 * Math.PI;
  const want = (((1.5 * Math.PI - (r.index + 0.5) * seg - r.land * seg) % tau) + tau) % tau, cur = ((S.angle % tau) + tau) % tau;
  const target = S.angle + tau * r.turns + (((want - cur) % tau) + tau) % tau;
  const done = () => { S.angle = target % tau; drawWheel(); setSpinning(false); if (btn) btn.disabled = false; dlg.close(); S.home = "result"; showResult(r); };
  if (reduced()) { drawWheel(); done(); return; }
  setSpinning(true); if (btn) btn.disabled = true;
  const a0 = S.angle, t0 = performance.now(), dur = 4200;
  (function step(now) { const q = Math.min(1, (now - t0) / dur), e = 1 - Math.pow(1 - q, 3); S.angle = a0 + (target - a0) * e; drawWheel(); if (q < 1) requestAnimationFrame(step); else done(); })(t0);
}

// ---------- starting a game, and the result card
export function resumeCard(g) {
  return new Promise(resolve => {
    const dlg = document.getElementById("dlg"), d = g.plays ? daysSince(g.plays.last) : null; dlg.className = "";
    const done = v => { dlg.onclose = null; if (dlg.open) dlg.close(); resolve(v); };
    dlg.replaceChildren(h("div", {}, h("h2", { style: "margin-top:0" }, "Picking up " + g.t),
      poster(g, null, ""),
      h("p", {}, h("b", {}, "Last time: "), g.progress && g.progress.note ? g.progress.note : "no note left."),
      d != null ? h("p", { class: "small" }, "Last played " + ago(d) + ".") : null,
      h("div", { class: "row", style: "clear:both" }, h("button", { class: "btn primary", type: "button", "data-pad": "a", onclick: () => done(true) }, "Continue"),
        h("button", { class: "btn", type: "button", "data-pad": "b", onclick: () => done(false) }, "Cancel"),
        h("span", { class: "small" }, S.pad ? "A continues, B cancels." : ""))));
    dlg.onclose = () => resolve(false);
    dlg.showModal();
  });
}

export async function startGame(g) {
  if (g.progress && (g.progress.ongoing || g.progress.note)) { if (!(await resumeCard(g))) return null; }
  return api("/api/launch", { title: g.t });
}

export function showResult(r) {
  const g = gameByTitle(r.winner); if (!g) { backToStart(); return; }
  S.home = "result"; S.result = r;
  const msg = h("div", { class: "note", role: "status" });
  const play = h("button", { class: "btn primary", type: "button", onclick: async () => {
    play.disabled = true;
    try {
      const l = await startGame(g);
      if (!l) { play.disabled = false; return; }
      msg.textContent = l.message;
      if (l.started) { await confirmPlay(g, true); return; }
      if (r.runners && r.runners.length) {
        msg.append(h("div", { class: "row" }, h("span", { class: "small" }, "It didn't start. Same draw, next in line:"), r.runners.map(x => h("button", { class: "btn", type: "button", onclick: () => showResult({ winner: x.title, prob: x.prob, why: ["Runner-up from the same draw: about " + Math.round(x.prob * 100) + "% chance."], runners: r.runners.filter(y => y.title !== x.title) }) }, x.title + " (about " + Math.round(x.prob * 100) + "%)"))));
      }
    } catch (e) { msg.textContent = e.message; }
    play.disabled = false;
  } }, "Play it now");
  const confirm = h("button", { class: g.launchable ? "btn" : "btn primary", type: "button", "data-pad-default": "", onclick: () => confirmPlay(g, false) }, "We'll play this");
  const veto = h("button", { class: "btn", type: "button", onclick: async () => { await api("/api/veto", { title: g.t }); S.boot.vetoes.push(g.t); toast(g.t + " is out for tonight."); spin(undefined, true); } }, "Not tonight");
  const facts = [playersLabel(g.p), g.mode, LEN_NAME[g.len], g.one ? "one controller" : null, paceLine(g.mySessions)].filter(Boolean).map(x => h("span", { class: "tag" }, x));
  const card = h("article", { class: "stagecard result" }, poster(g, null, "Box art for " + g.t),
    h("div", { class: "resbody" }, h("p", { class: "kicker" }, "Tonight, how about"), h("h2", { class: "title" }, g.t), h("div", { class: "tags" }, facts), h("p", { class: "why" }, g.why),
      h("div", { class: "row actions2" }, g.launchable ? play : null, confirm, h("button", { class: "btn", type: "button", onclick: () => spin(undefined, true) }, "Another"), veto),
      msg,
      h("details", { class: "why-details" }, h("summary", {}, "Why this one?"),
        h("p", {}, h("span", { class: "lab" }, "Tip  "), g.role), h("p", { class: "warn" }, "Watch out: " + g.watch), g.time ? h("p", { class: "small" }, "Length: " + g.time) : null,
        h("p", { class: "small" }, r.why.join(" ")),
        r.leftOut ? h("p", { class: "small" }, "Not repeating the " + r.leftOut + " game" + (r.leftOut > 1 ? "s" : "") + " already shown this round, so the odds above are among what is left.") : null,
        g.launchable ? null : h("p", { class: "small" }, g.hasLaunch ? "Can't start it from here right now: " + g.launchWhy : "Start it on " + g.sys + " yourself; Game Night has no way to start it set."),
        r.skipped && r.skipped.length ? h("p", { class: "small" }, r.skipped.length + " game" + (r.skipped.length > 1 ? "s were" : " was") + " left out because " + (r.skipped.length > 1 ? "they" : "it") + " can't be started right now: " + r.skipped.slice(0, 3).map(x => x.title + " (" + x.reason + ")").join("; ") + (r.skipped.length > 3 ? "..." : "")) : null),
      h("div", { class: "row" }, canWatch(g) ? h("button", { class: "link", type: "button", onclick: () => gameSheet(g, { watch: watchFirst(g, settings().trailersEnabled !== false) }) }, icon("play"), "Watch it first") : null,
        h("button", { class: "link", type: "button", onclick: backToStart }, icon("back"), "Back to the start"))));
  setStage(card); celebrate(card); cheer();
  if (document.body.classList.contains("padnav")) confirm.focus({ preventScroll: true });       // controller: A is "We'll play this", never an accidental launch
}

export async function confirmPlay(g, launched) {
  const r = await api("/api/confirm", { title: g.t });
  S.boot.turn = r.turn; S.boot.history.push(r.entry); buildTurn();
  S.home = "start"; S.result = null;
  setStage(h("div", { class: "stagecard solo" }, mascot("hero-mascot"), h("p", { class: "kicker" }, launched ? "Started" : "Have fun"), h("h2", {}, g.t),
    h("p", { class: "lede" }, (launched || g.launchable ? "" : "Start it on " + g.sys + ". ") + (pids().length > 1 ? "Next pick: " + nm(r.turn) + "." : "")),
    h("button", { class: "link", type: "button", onclick: backToStart }, "Back to the start")));
}
