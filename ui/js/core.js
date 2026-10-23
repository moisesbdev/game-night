/* Shared state and the small DOM and network helpers every view uses. */
import { cleanFilters, monogram, posterColor } from "./logic.js";

export const TOKEN = document.querySelector('meta[name="gn-token"]').content;

export function cssVar(n, fallback) { return getComputedStyle(document.documentElement).getPropertyValue(n).trim() || fallback; }

export function poster(g, tags, alt) {
  return h("div", { class: "poster", style: "--c:" + posterColor(g.t) }, h("span", { class: "mono", "aria-hidden": "true" }, monogram(g.t)),
    g.cover ? h("img", { class: "art", src: "/api/cover?title=" + encodeURIComponent(g.t), alt: alt || "", loading: "lazy" }) : null, tags && tags.length ? h("div", { class: "ptags" }, tags) : null);
}

export const S = {
  boot: null, tab: "tonight", sub: { shelf: "games", journal: "nights", more: "health" }, rater: "", spinning: false, home: "start", wheel: [], angle: 0, result: null, health: null,
  filters: cleanFilters(),
  matching: 0, pool: 0, note: "", search: "", sys: "all", libf: "all", sort: "title", onlyLaunch: false, platform: "",
};

try { S.rater = localStorage.getItem("gn-rater") || ""; } catch (e) { /* storage may be blocked */ }

export function h(tag, attrs, ...kids) {
  const e = document.createElement(tag);
  for (const k in attrs || {}) {
    const v = attrs[k];
    if (v === false || v == null) continue;
    if (k === "class") e.className = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const c of kids.flat(Infinity)) { if (c == null || c === false) continue; e.append(c.nodeType ? c : document.createTextNode(String(c))); }
  return e;
}

const SVG_NS = "http://www.w3.org/2000/svg";
export function svg(tag, attrs, ...kids) {
  const e = document.createElementNS(SVG_NS, tag);
  for (const k in attrs || {}) e.setAttribute(k, attrs[k]);
  for (const c of kids) if (c) e.append(c);
  return e;
}

/** Counts a number up from zero (instantly when motion is reduced). Put it in an element hidden from screen readers and give the group its real text. */
export function countUp(el, to, fmt = String) {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches || !(to > 0)) { el.textContent = fmt(to); return el; }
  el.textContent = fmt(0);
  const t0 = performance.now(), dur = 800, id = setInterval(() => {
    const q = Math.min(1, (performance.now() - t0) / dur);
    el.textContent = fmt(Math.round(to * (1 - Math.pow(1 - q, 3))));
    if (q >= 1 || !el.isConnected) clearInterval(id);
  }, 30);
  return el;
}

/** The little moon: decoration only (hidden from screen readers). */
export function mascot(extra) {
  const ink = "#12343b", line = { stroke: ink, "stroke-width": "2.5", "stroke-linejoin": "round", "stroke-linecap": "round" };
  return svg("svg", { class: "mascot" + (extra ? " " + extra : ""), viewBox: "0 0 64 64", "aria-hidden": "true", focusable: "false" },
    svg("circle", { cx: "32", cy: "34", r: "22", fill: "#9fe3dc", ...line }),
    svg("circle", { cx: "45", cy: "25", r: "3.4", fill: "#7fd3cb" }), svg("circle", { cx: "19", cy: "47", r: "2.8", fill: "#7fd3cb" }), svg("circle", { cx: "42", cy: "49", r: "1.9", fill: "#7fd3cb" }),
    svg("path", { d: "M22 33 Q25.5 37 29 33", fill: "none", ...line, "stroke-width": "2.2" }), svg("path", { d: "M35 33 Q38.5 37 42 33", fill: "none", ...line, "stroke-width": "2.2" }),
    svg("path", { d: "M28 42 Q32 46 36 42", fill: "none", ...line, "stroke-width": "2.2" }),
    svg("path", { class: "twinkle", d: "M52 7 l1.5 3.5 l3.5 1.5 l-3.5 1.5 l-1.5 3.5 l-1.5 -3.5 l-3.5 -1.5 l3.5 -1.5 z", fill: "#f2c46b" }), svg("circle", { class: "twinkle late", cx: "9", cy: "17", r: "1.7", fill: "#f2c46b" }),
    svg("g", { class: "hat" }, svg("path", { d: "M21 13.5 C24 6 28 1 38 0 C35 4 37 9 41 13.5 Z", fill: "#2a1f33", stroke: ink, "stroke-width": "2", "stroke-linejoin": "round" }),      // the witch hat: shown only in the Halloween look
      svg("path", { d: "M22.6 10.6 Q31 12.6 39.6 10.6 L40.4 13 Q31 15 21.8 13 Z", fill: "#f4a259" }), svg("ellipse", { cx: "31", cy: "14.5", rx: "16", ry: "3.4", fill: "#2a1f33", stroke: ink, "stroke-width": "2" })));
}

/** Small line icons for the tabs (stroke follows the text colour). Decoration only. */
const ICONS = {
  moon: [["path", { d: "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z" }], ["path", { d: "M17.5 3.5v3M16 5h3" }]],
  pad: [["path", { d: "M7 9h10a4 4 0 0 1 3.9 4.9l-.6 2.6a2 2 0 0 1-3.5.8L15 15H9l-1.8 2.3a2 2 0 0 1-3.5-.8l-.6-2.6A4 4 0 0 1 7 9Z" }], ["path", { d: "M8.5 11.5v3M7 13h3M16 12.5h.01M18 14h.01" }]],
  book: [["path", { d: "M4 5.5C6.5 4.5 9.5 4.5 12 6c2.5-1.5 5.5-1.5 8-.5V19c-2.5-1-5.5-1-8 .5-2.5-1.5-5.5-1.5-8-.5V5.5Z" }], ["path", { d: "M12 6v13.5" }]],
  clock: [["circle", { cx: "12", cy: "12", r: "8" }], ["path", { d: "M12 7.5V12l3 2" }]],
  shelf: [["path", { d: "M5 4v15M10 4v15M15.2 5.2l3.6 13.6M3.5 20h17" }]],
  pulse: [["path", { d: "M3 12h4l2-5 4 10 2-5h6" }]],
  sliders: [["path", { d: "M4 7h8M18 7h2M4 17h2M12 17h8" }], ["circle", { cx: "15", cy: "7", r: "2" }], ["circle", { cx: "9", cy: "17", r: "2" }]],
  leaf: [["path", { d: "M5 19c0-8 5-13 14-14 0 9-5 14-13 14Z" }], ["path", { d: "M5 19l7-7" }]],
  bolt: [["path", { d: "M13 3 5 13.5h6L10 21l8-10.5h-6L13 3Z" }]],
  pair: [["circle", { cx: "8.5", cy: "9", r: "3" }], ["circle", { cx: "16.5", cy: "10", r: "2.5" }], ["path", { d: "M3 19c.5-3 2.6-5 5.5-5s5 2 5.5 5M14.5 14.6c.7-.4 1.4-.6 2-.6 2.2 0 3.700 1.600 4 4.500" }]],
  smile: [["circle", { cx: "12", cy: "12", r: "8.5" }], ["path", { d: "M8.5 14c1 1.600 2.200 2.300 3.500 2.300s2.500-.7 3.500-2.300M9.200 9.800h.01M14.800 9.800h.01" }]],
  ghost: [["path", { d: "M6 20V11a6 6 0 0 1 12 0v9l-2-1.500-2 1.500-2-1.500-2 1.500-2-1.500L6 20Z" }], ["path", { d: "M9.500 11h.01M14.500 11h.01" }]],
  flag: [["path", { d: "M6 21V4M6 5h11l-2 4 2 4H6" }]],
  volume: [["path", { d: "M4 9.500v5h3.500L12 18.500v-13L7.500 9.500H4Z" }], ["path", { d: "M15.500 9a4 4 0 0 1 0 6M18 6.500a7.500 7.500 0 0 1 0 11" }]],
  volumeOff: [["path", { d: "M4 9.500v5h3.500L12 18.500v-13L7.500 9.500H4Z" }], ["path", { d: "m16 9.500 5 5M21 9.500l-5 5" }]],
  pause: [["path", { d: "M8 5.500v13M16 5.500v13" }]],
  expand: [["path", { d: "M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" }]],
  eye: [["path", { d: "M2.500 12S6 5.500 12 5.500 21.500 12 21.500 12 18 18.500 12 18.500 2.500 12 2.500 12Z" }], ["circle", { cx: "12", cy: "12", r: "3" }]],
  spark: [["path", { d: "M12 3l1.800 5.200L19 10l-5.200 1.800L12 17l-1.800-5.200L5 10l5.200-1.800L12 3Z" }], ["path", { d: "M19 16l.800 2.200L22 19l-2.200.800L19 22l-.800-2.200L16 19l2.200-.800L19 16Z" }]],
  coin: [["circle", { cx: "12", cy: "12", r: "8.500" }], ["path", { d: "M12 7.500v9M9.500 10c0-1 1-1.600 2.500-1.600s2.500.600 2.500 1.700c0 2.200-5 1.200-5 3.500 0 1.100 1 1.800 2.500 1.800s2.500-.700 2.500-1.800" }]],
  swap: [["path", { d: "M5 8h13l-3-3M19 16H6l3 3" }]],
  search: [["circle", { cx: "11", cy: "11", r: "6.500" }], ["path", { d: "m16 16 4 4" }]],
  plus: [["path", { d: "M12 5v14M5 12h14" }]],
  close: [["path", { d: "m6 6 12 12M18 6 6 18" }]],
  back: [["path", { d: "M15 5l-7 7 7 7" }]],
  play: [["path", { d: "M8 5.500v13l11-6.500-11-6.500Z" }]],
};
export function icon(name) {
  return svg("svg", { class: "ico", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", "stroke-width": "1.9", "stroke-linecap": "round", "stroke-linejoin": "round", "aria-hidden": "true", focusable: "false" },
    ...(ICONS[name] || []).map(([tag, attrs]) => svg(tag, attrs)));
}
/** A segmented control: one row of options with one selected; `onPick(id)` runs on a click or an arrow key. */
export function seg(label, options, current, onPick) {
  const bar = h("div", { class: "seg", role: "tablist", "aria-label": label });
  options.forEach(([id, text]) => bar.append(h("button", { type: "button", role: "tab", "aria-selected": String(id === current), tabindex: id === current ? "0" : "-1", "data-id": id, onclick: () => onPick(id) }, text)));
  bar.addEventListener("keydown", e => {
    const i = options.findIndex(o => o[0] === current), dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (dir) { e.preventDefault(); onPick(options[(i + dir + options.length) % options.length][0], true); }
  });
  return bar;
}

export async function api(path, body) {
  const opt = body === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-GameNight-Token": TOKEN }, body: JSON.stringify(body) };
  let r;
  try { r = await fetch(path, opt); } catch (e) { showOffline(true); throw new Error("Game Night isn't running. Start it again."); }
  showOffline(false);
  let d; try { d = await r.json(); } catch (e) { d = { error: "unreadable response" }; }
  if (!r.ok) throw new Error(d.error || ("HTTP " + r.status));
  return d;
}

export function showOffline(on) {
  let b = document.getElementById("offline");
  if (!on) { if (b) b.remove(); return; }
  if (b) return;
  b = h("div", { id: "offline", class: "errlist", role: "alert", style: "position:sticky;top:60px;z-index:4" }, "Game Night stopped answering (its window may have been closed). Start it again, then ",
    h("button", { class: "btn", type: "button", onclick: () => location.reload() }, "reload this page"), ".");
  document.getElementById("view").before(b);
}

let toastTimer;

/** A passing message. It lives in the browser's top layer (a popover), so it shows above an open sheet too, where an ordinary fixed element would be hidden behind it. */
export function toast(msg) {
  const t = document.getElementById("toast"); t.textContent = msg; clearTimeout(toastTimer);
  try { if (t.showPopover) { if (t.matches(":popover-open")) t.hidePopover(); t.showPopover(); } } catch (e) { /* an older browser: the plain fixed element still shows outside sheets */ }
  void t.offsetWidth; t.classList.add("show");
  toastTimer = setTimeout(() => { t.classList.remove("show"); toastTimer = setTimeout(() => { try { if (t.hidePopover && t.matches(":popover-open")) t.hidePopover(); } catch (e) { /* already hidden */ } }, 400); }, 3800);
}

/** The group: [{id, name}] from Settings (1 to 8 players). */
export const players = () => (S.boot && S.boot.players) || [];
export const pids = () => players().map(p => p.id);
/** A player's name by id; someone no longer in the group shows as "a past player". */
export const nm = id => (players().find(p => p.id === id) || { name: "a past player" }).name;
/** Whoever is rating on this device: the remembered player if still in the group, else the first. */
export function rater() { if (!pids().includes(S.rater)) S.rater = pids()[0] || ""; return S.rater; }
export const settings = () => (S.boot && S.boot.settings) || {};

export function chip(label, pressed, on, extra) { return h("button", { type: "button", class: "chip", "aria-pressed": String(!!pressed), onclick: on, ...(extra || {}) }, label); }

export function dots(n) { return h("span", { class: "dots", role: "img", "aria-label": "Skill gap " + n + " of 5" }, [1, 2, 3, 4, 5].map(i => h("i", { class: i <= n ? "on" : "" }))); }

export function gameByTitle(t) { return S.boot.games.find(g => g.t === t); }

export async function reload() { S.boot = await api("/api/bootstrap"); }

