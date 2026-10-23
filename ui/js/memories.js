/* Looking back: "Our year" (a few slides, one at a time), the memories screen (cycles through your nights when the page is left alone),
 */
import { S, api, gameByTitle, h, mascot, nm, players, poster } from "./core.js";
import { VIBE_NAME, fmtMinutes, mediaUrl, nightLabel } from "./logic.js";
import { jumpToGame } from "./games.js";
import { go } from "./main.js";
import { readPads } from "./pad.js";

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const list = a => a.length <= 1 ? (a[0] || "") : a.slice(0, -1).join(", ") + " and " + a[a.length - 1];
const plural = (n, one, many) => n + " " + (n === 1 ? one : many);

// ---------- Our year

let yearShown = null, slideAt = 0;

function slides(y) {
  const out = [], hours = fmtMinutes(y.minutes);
  out.push({ big: plural(y.nights, "night", "nights"), head: y.year + " together", text: (hours ? hours + " of play" : "") + (hours ? " across " : "Across ") + plural(y.games, "game", "games") + "." });
  if (y.top.length) {
    const t = y.top[0], g = gameByTitle(t.title);
    out.push({ head: "Most played", art: g ? g : null, big: t.title, text: plural(t.nights, "night", "nights") + (t.minutes ? ", " + fmtMinutes(t.minutes) : "") + "." + (y.top.length > 1 ? " Then " + list(y.top.slice(1).map(x => x.title)) + "." : ""), jump: g ? t.title : null });
  }
  if (y.new.length) out.push({ head: "New to us", big: plural(y.new.length, "game", "games"), text: "Played for the first time: " + list(y.new.slice(0, 6)) + (y.new.length > 6 ? " and " + (y.new.length - 6) + " more" : "") + ".", posters: y.new.slice(0, 6) });
  if (y.finished.length) out.push({ head: "Finished", big: plural(y.finished.length, "game", "games"), text: list(y.finished) + ".", posters: y.finished.slice(0, 6) });
  if (y.allLiked.length) out.push({ head: players().length === 2 ? "You both liked" : "Everyone liked", big: plural(y.allLiked.length, "game", "games"), text: list(y.allLiked) + ".", posters: y.allLiked.slice(0, 6) });
  const chose = Object.entries(y.chose || {}).filter(([, n]) => n).sort((a, b) => b[1] - a[1]);
  if (chose.length && players().length > 1) out.push({ head: "Who chose", big: nm(chose[0][0]), text: chose.map(([id, n]) => nm(id) + " chose " + plural(n, "game", "games")).join(", ") + "." });
  if (y.moods.length) out.push({ head: "Your kind of night", big: VIBE_NAME[y.moods[0].vibe] || y.moods[0].vibe, text: "The mood that came up most" + (y.moods.length > 1 ? ", then " + list(y.moods.slice(1).map(m => VIBE_NAME[m.vibe] || m.vibe)) : "") + "." });
  out.push({ head: "Rhythm", big: y.busiest.month, text: "The busiest month, with " + plural(y.busiest.nights, "night", "nights") + "." + (y.weeksInARow > 1 ? " At best you played " + y.weeksInARow + " weeks in a row." : "") });
  out.push({ head: "First and latest", big: nightLabel(y.first.date), text: "It started with " + list(y.first.titles) + (y.last.date !== y.first.date ? ". Most recently, " + nightLabel(y.last.date).toLowerCase() + ": " + list(y.last.titles) : "") + "." });
  return out;
}

function slideEl(s, i, n) {
  return h("div", { class: "slide", role: "group", "aria-roledescription": "slide", "aria-label": (i + 1) + " of " + n },
    h("p", { class: "slidehead" }, s.head),
    s.art ? h("div", { class: "slideart" }, poster(s.art, null, "Box art for " + s.art.t)) : null,
    h("p", { class: "slidebig" }, s.big),
    h("p", {}, s.text),
    s.posters ? h("div", { class: "slideposters", "aria-hidden": "true" }, s.posters.map(t => { const g = gameByTitle(t); return g ? poster(g, null, "") : null; })) : null,
    s.jump ? h("button", { class: "link", type: "button", onclick: () => jumpToGame(s.jump) }, "Open " + s.jump) : null);
}

export async function yearSection(y) {
  const want = y || yearShown || new Date().getFullYear();
  const d = await api("/api/year?y=" + want);
  if (yearShown !== d.year) slideAt = 0;
  yearShown = d.year;
  const picker = d.years.length > 1 ? h("div", { class: "row", role: "group", "aria-label": "Which year" }, d.years.map(x => h("button", { type: "button", class: "chip", "aria-pressed": String(x === d.year), onclick: async e => { const box = e.currentTarget.closest(".yearbox"); box.replaceWith(await yearSection(x)); } }, String(x)))) : null;
  if (!d.nights) return h("div", { class: "yearbox" }, picker, h("div", { class: "panel empty" }, mascot(), h("div", {}, h("b", {}, "Nothing in " + d.year + " yet"),
    h("p", { class: "small" }, "Once you've played a few nights, this shows your year: the games you played most, the ones that were new to you, what you finished and when you played."),
    h("button", { class: "btn primary", type: "button", onclick: () => go("tonight") }, "Pick tonight's game"))));
  const all = slides(d), stage = h("div", { class: "slidestage", "aria-live": "polite" }), count = h("span", { class: "small slidecount" });
  const back = h("button", { class: "btn", type: "button", "aria-label": "Previous" }, "Back"), next = h("button", { class: "btn primary", type: "button", "aria-label": "Next" }, "Next");
  const show = (i, dir) => {
    slideAt = Math.max(0, Math.min(all.length - 1, i));
    const el = slideEl(all[slideAt], slideAt, all.length);
    if (dir && !reduced()) el.classList.add(dir > 0 ? "in-next" : "in-prev");
    stage.replaceChildren(el);
    count.textContent = (slideAt + 1) + " of " + all.length;
    back.disabled = slideAt === 0; next.textContent = slideAt === all.length - 1 ? "Start again" : "Next";
  };
  back.onclick = () => show(slideAt - 1, -1);
  next.onclick = () => show(slideAt === all.length - 1 ? 0 : slideAt + 1, 1);
  const box = h("div", { class: "yearbox", onkeydown: e => { if (e.target.closest("input,textarea,select")) return; if (e.key === "ArrowRight") { e.preventDefault(); next.click(); } else if (e.key === "ArrowLeft" && !back.disabled) { e.preventDefault(); back.click(); } } },
    picker, h("div", { class: "panel yearpanel" }, stage, h("div", { class: "row slidenav" }, back, count, next)));
  show(slideAt);
  return box;
}

// ---------- the memories screen

const IDLE_KEY = "gn-memories", IDLE_MINUTES = 8, STEP_MS = 9000;
let lastActive = Date.now(), cards = [], at = 0, timer = null, padPoll = null;

export function memoriesOn() { try { return localStorage.getItem(IDLE_KEY) !== "off"; } catch (e) { return true; } }
export function setMemoriesOn(on) { try { localStorage.setItem(IDLE_KEY, on ? "on" : "off"); } catch (e) { /* storage blocked: stays on for this visit */ } }

function line(m) {
  const bits = [];
  if (m.minutes) bits.push(fmtMinutes(m.minutes));
  if (m.outcome === "finished") bits.push("finished it"); else if (m.outcome === "again") bits.push("wanted more");
  if (m.allLiked) bits.push(players().length === 2 ? "you both liked it" : "everyone liked it");
  if (m.chose) bits.push(nm(m.chose) + "'s pick");
  return bits.join(" · ");
}

function card(m) {
  const g = gameByTitle(m.title) || { t: m.title, cover: false };
  return h("figure", { class: "memory" },
    g.shot ? h("div", { class: "memback", style: "background-image:url(\"" + mediaUrl(g.t, "shot") + "\")", "aria-hidden": "true" }) : null,
    h("div", { class: "memcard" }, poster(g, null, "Box art for " + g.t),
      h("figcaption", {}, h("span", { class: "memwhen" }, m.date ? nightLabel(m.date) + (m.n ? " · Night " + m.n : "") : "On the shelf"), h("b", {}, g.t), h("span", {}, m.date ? line(m) : (g.why || "").split(". ")[0].replace(/\.$/, "") + "."))));
}

function shuffle(a) { for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [a[i], a[j]] = [a[j], a[i]]; } return a; }

function step() {
  const box = document.getElementById("mem"); if (!box || !box.open || !cards.length) return;
  at = (at + 1) % cards.length;
  const old = box.querySelector(".memory:not(.leaving)"), el = card(cards[at]);
  if (old && !reduced()) { old.classList.add("leaving"); old.setAttribute("aria-hidden", "true"); setTimeout(() => old.remove(), 1600); } else if (old) old.remove();
  box.append(el);
}

export function closeMemories() {
  const box = document.getElementById("mem");
  clearInterval(timer); clearInterval(padPoll); timer = padPoll = null;
  S.memories = false; lastActive = Date.now();
  if (box && box.open) box.close();
}

/** The memories screen: your nights, one at a time, slowly. Any key, click, wheel or controller button ends it. */
export async function openMemories() {
  if (document.getElementById("mem")?.open) return;
  let mine = [];
  try { mine = (await api("/api/memories")).memories; } catch (e) { return; }
  const withArt = mine.filter(m => { const g = gameByTitle(m.title); return g && g.cover; });
  const shelf = shuffle(S.boot.games.filter(g => g.cover && !withArt.some(m => m.title === g.t))).slice(0, Math.max(0, 12 - withArt.length)).map(g => ({ title: g.t }));
  cards = shuffle(withArt.slice(0, 30)).concat(shelf);
  if (!cards.length) return;
  let box = document.getElementById("mem");
  if (!box) {
    box = h("dialog", { id: "mem", class: "memories", "aria-label": "Memories" });
    box.addEventListener("cancel", e => { e.preventDefault(); closeMemories(); });
    ["pointerdown", "keydown", "wheel", "touchstart"].forEach(t => box.addEventListener(t, e => {
      e.preventDefault(); e.stopPropagation();
      if (t === "pointerdown" || t === "touchstart") {                          // the click that ends the screen must not land on whatever was under it (Surprise us, a tile)
        const swallow = ev => { ev.stopPropagation(); ev.preventDefault(); };
        addEventListener("click", swallow, { capture: true, once: true }); setTimeout(() => removeEventListener("click", swallow, { capture: true }), 700);
      }
      closeMemories();
    }, { passive: false }));
    box.addEventListener("pointermove", e => { if (Math.abs(e.movementX) + Math.abs(e.movementY) > 6) closeMemories(); });
    document.body.append(box);
  }
  at = 0;
  box.replaceChildren(h("p", { class: "memhint" }, "Press any key or button to come back"), card(cards[0]));
  S.memories = true;
  box.showModal();
  timer = setInterval(step, STEP_MS);
  const held = new Set((readPads()?.buttons || []).map((b, i) => b.pressed ? i : -1));
  padPoll = setInterval(() => { const p = readPads(); if (p && p.buttons.some((b, i) => b.pressed && !held.has(i))) closeMemories(); if (p) p.buttons.forEach((b, i) => { if (!b.pressed) held.delete(i); }); }, 120);
}

/** Left alone on Tonight's start screen for a while (no sheet open, nothing playing), the memories screen comes up by itself. */
export function startIdleWatch() {
  const wake = () => { lastActive = Date.now(); };
  ["pointerdown", "keydown", "wheel", "touchstart", "scroll"].forEach(t => addEventListener(t, wake, { passive: true, capture: true }));
  addEventListener("pointermove", e => { if (e.movementX || e.movementY) wake(); }, { passive: true, capture: true });
  setInterval(() => {
    const p = readPads(); if (p && (p.buttons.some(b => b.pressed) || p.axes.some(v => Math.abs(v) > 0.4))) wake();
    if (!memoriesOn() || S.memories || document.hidden || S.spinning || S.tab !== "tonight" || document.getElementById("dlg").open || [...document.querySelectorAll("video")].some(v => !v.paused && !v.classList.contains("hv"))) return;
    if (Date.now() - lastActive > IDLE_MINUTES * 60000) openMemories();
  }, 1000);
}
