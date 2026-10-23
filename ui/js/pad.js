/* The controller. Nothing here runs unless a controller is plugged in and pressed.
   D-pad or left stick: move to the nearest thing in that direction.  A: select.  B: back (closes the player, then the sheet, then steps back).
   X: Another / Surprise us.  Y: Watch.  LB / RB: change place.  LT / RT: change section.  Start: Surprise us from anywhere.  Right stick: scroll.
   Dropdowns open as a list of options and text or number boxes open an on-screen keyboard (X backspace, Y space, B done).
   Pressing anything on the controller shows a clear focus ring and a legend; touching the mouse or keyboard hides them again. */
import { S, h, toast } from "./core.js";
import { TABS, applyKey, padStep } from "./logic.js";
import { go } from "./main.js";
import { spin } from "./tonight.js";

const FOCUSABLE = 'button, a[href], input:not([type="hidden"]), select, textarea, summary, [tabindex]:not([tabindex="-1"])';
const DIR_BUTTONS = { 12: "up", 13: "down", 14: "left", 15: "right" };
const FIRST_REPEAT = 380, NEXT_REPEAT = 140, DEADZONE = 0.55;

const prev = {};                                  // button index -> was it down on the last frame
const held = { dir: null, since: 0, last: 0 };    // the direction being held, for key-repeat
let legendKey = "", lastScroll = 0;
addEventListener("scroll", () => { lastScroll = performance.now(); }, { passive: true, capture: true });

const dlg = () => document.getElementById("dlg");
const edge = (p, i) => { const on = !!(p.buttons[i] && p.buttons[i].pressed), was = !!prev[i]; prev[i] = on; return on && !was; };
const q = (sel, root = document) => root.querySelector(sel);
const byText = (sel, re, root = document) => [...root.querySelectorAll(sel)].find(b => re.test(b.textContent.trim()));

function usable(el) {
  if (el.disabled || el.classList.contains("skip") || el.closest("[inert], .hide, [hidden]")) return false;
  const r = el.getBoundingClientRect();
  if (r.width < 2 || r.height < 2) return false;
  const s = getComputedStyle(el);
  return s.visibility !== "hidden" && s.display !== "none";
}

/** Everything the controller can land on right now: inside the open list or keyboard if there is one, else the open sheet, else the whole page. */
function candidates() {
  const d = dlg(), root = layerOpen() || (d.open ? d : document);
  return [...root.querySelectorAll(FOCUSABLE)].filter(usable);
}

/** Where focus goes when it has nowhere: the thing you most likely want next. Never "Play it now" on a fresh result (an extra A press must not launch a game). */
function defaultTarget(list) {
  return list.find(el => el.matches("[data-pad-default]")) || list.find(el => el.matches("#spin, #dlg .btn.primary, [data-pad='a']")) || list.find(el => el.closest("#view")) || list[0];
}

/** Focus a thing. `scroll` is true only for a deliberate move (a D-pad press); the background housekeeping passes false so it can never drag the page. */
function focusEl(el, scroll = true) {
  if (!el) return;
  el.focus({ preventScroll: true });
  if (scroll) el.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
}

// ---------- lists and an on-screen keyboard, so dropdowns and text boxes work from the couch
// Both are popovers appended INSIDE the open sheet (a modal sheet makes everything outside it unreachable), or to the page when no sheet is open.
let layer = null;                                   // { kind: "menu" | "keys", el, origin, ... }
const layerOpen = () => (layer && layer.el.isConnected ? layer.el : null);

function openLayer(kind, el, origin, extra) {
  closeLayers(false);
  (dlg().open ? dlg() : document.body).append(el);
  try { el.showPopover(); } catch (e) { /* an older browser: the element still shows */ }
  layer = { kind, el, origin, ...extra };
}

function closeLayers(refocus) {
  if (!layer) return;
  const { el, origin } = layer; layer = null;
  try { if (el.matches(":popover-open")) el.hidePopover(); } catch (e) { /* already gone */ }
  el.remove();
  if (refocus && origin && origin.isConnected) focusEl(origin);
}
dlg().addEventListener("close", () => closeLayers(false));

const TEXT_TYPES = new Set(["", "text", "search", "number", "email", "url", "tel", "password"]);
const isTextLike = el => el.tagName === "TEXTAREA" || (el.tagName === "INPUT" && TEXT_TYPES.has(el.type));
const labelOf = el => { const l = el.id && document.querySelector('label[for="' + el.id + '"]'); return (l && l.textContent.trim()) || el.getAttribute("aria-label") || el.placeholder || ""; };

/** A <select> as a list of buttons: the native dropdown can't be driven from a controller. */
function openChooser(sel) {
  const pick = o => () => { sel.value = o.value; sel.dispatchEvent(new Event("input", { bubbles: true })); sel.dispatchEvent(new Event("change", { bubbles: true })); closeLayers(true); };
  const items = [...sel.options].map(o => h("button", { class: "padopt", type: "button", role: "menuitemradio", "aria-checked": String(o.value === sel.value), onclick: pick(o) }, o.textContent));
  const title = labelOf(sel) || "Choose one";
  openLayer("menu", h("div", { class: "padmenu", popover: "manual", role: "menu", "aria-label": title }, h("div", { class: "padmenu-title" }, title), items), sel);
  focusEl(items[Math.max(0, sel.selectedIndex)] || items[0]);
}

const KEY_ROWS = [["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"], [..."qwertyuiop"], [..."asdfghjkl'"], ["Shift", ..."zxcvbnm", ".", ","], [..."-:!?&()+/\""]];
const NUM_ROWS = [["1", "2", "3"], ["4", "5", "6"], ["7", "8", "9"], [".", "0", "Backspace"]];

/** An on-screen keyboard for text, search and number boxes. X is backspace, Y is space, B or Start is done. */
function openKeyboard(el) {
  const numeric = el.type === "number", multiline = el.tagName === "TEXTAREA", opts = { max: el.maxLength > 0 ? el.maxLength : 0, multiline, numeric };
  const st = { shift: !numeric && !el.value };
  const shown = h("div", { class: "kpreview", "aria-live": "polite" });
  const paint = () => { shown.textContent = (labelOf(el) ? labelOf(el) + ": " : "") + el.value + "\u258f"; };
  try { const n = el.value.length; el.setSelectionRange(n, n); } catch (e) { /* number boxes have no caret */ }
  const type = key => {
    const r = applyKey(el.value, el.selectionStart, el.selectionEnd, key, opts);
    if (r.value !== el.value) { el.value = r.value; el.dispatchEvent(new Event("input", { bubbles: true })); }
    try { el.setSelectionRange(r.caret, r.caret); } catch (e) { /* no caret */ }
    paint();
  };
  const letters = [];
  const press = k => {
    if (k === "Done") { closeLayers(true); return; }
    if (k === "Shift") { st.shift = !st.shift; relabel(); return; }
    if (k === "Space") { type(" "); return; }
    const isLetter = /^[a-z]$/.test(k);
    type(isLetter && st.shift ? k.toUpperCase() : k);
    if (isLetter && st.shift) { st.shift = false; relabel(); }
  };
  const caption = k => (k === "Backspace" ? "\u232b" : k === "Shift" ? "\u21e7" : k);
  const shiftKey = h("button", { class: "pk", type: "button", "aria-label": "Shift", "aria-pressed": String(st.shift), onclick: () => press("Shift") }, "\u21e7");
  const key = k => k === "Shift" ? shiftKey : h("button", { class: "pk", type: "button", "aria-label": k === "Backspace" ? "Backspace" : k, "data-k": k, onclick: () => press(k) }, caption(k));
  const rows = (numeric ? NUM_ROWS : KEY_ROWS).map(r => h("div", { class: "krow" }, r.map(k => { const b = key(k); if (/^[a-z]$/.test(k)) letters.push(b); return b; })));
  function relabel() { shiftKey.setAttribute("aria-pressed", String(st.shift)); letters.forEach(b => { const k = b.dataset.k; b.textContent = st.shift ? k.toUpperCase() : k; }); }
  const bottom = h("div", { class: "krow" },
    numeric ? null : h("button", { class: "pk wide", type: "button", onclick: () => press("Space") }, "Space"),
    numeric ? null : h("button", { class: "pk", type: "button", "aria-label": "Backspace", onclick: () => press("Backspace") }, "\u232b"),
    multiline ? h("button", { class: "pk", type: "button", onclick: () => press("Enter") }, "Enter") : null,
    h("button", { class: "pk done", type: "button", onclick: () => press("Done") }, "Done"));
  relabel(); paint();
  openLayer("keys", h("div", { class: "padkeys", popover: "manual", role: "group", "aria-label": "On-screen keyboard" }, shown, rows, bottom), el, { press });
  focusEl(numeric ? q('[data-k="5"]', layer.el) : q('[data-k="h"]', layer.el) || q(".pk", layer.el));
}

function move(dir) {
  const list = candidates(); if (!list.length) return;
  const i = list.indexOf(document.activeElement);
  if (i < 0) { focusEl(defaultTarget(list)); return; }
  const rects = list.map(el => { const r = el.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  const j = padStep(rects, i, dir);
  if (j >= 0) focusEl(list[j]);
  else if (dir === "up" || dir === "down") scroller().scrollBy(0, dir === "down" ? 200 : -200);
}

const scroller = () => (dlg().open ? dlg() : window);

function select() {
  const d = dlg();
  const quick = !layerOpen() && d.open && q("[data-pad='a']", d); if (quick) { quick.click(); return; }      // sheets with their own A/B/X answers (taste test, "pick up where you left off")
  const list = candidates(), a = document.activeElement;
  if (!a || !list.includes(a)) { focusEl(defaultTarget(list)); return; }
  if (a.tagName === "SELECT") { openChooser(a); return; }
  if (isTextLike(a)) { openKeyboard(a); return; }
  a.click();
}

function back() {
  if (layerOpen()) { closeLayers(true); return; }
  const d = dlg();
  if (d.open) {
    const quick = q("[data-pad='b']", d); if (quick) { quick.click(); return; }
    const closePlayer = q(".watch.playing .watchbar button", d); if (closePlayer) { closePlayer.click(); return; }      // first the player, then the sheet
    d.close(); return;
  }
  const home = byText("#home .link", /^Back/);
  if (home) home.click(); else if (S.tab !== "tonight") go("tonight");
}

function another() {
  if (dlg().open || S.spinning || layerOpen()) return;
  if (S.tab !== "tonight") { go("tonight"); return; }
  const again = byText("#home button", /^Another$/);
  if (again) { again.click(); return; }
  const s = document.getElementById("spin"); if (s && !s.disabled) s.click();
}

function surprise() {
  if (S.spinning) return;
  if (layer && layer.kind === "keys") { layer.press("Done"); return; }
  if (layerOpen()) { closeLayers(true); return; }
  const d = dlg(); if (d.open) d.close();
  if (S.tab === "tonight") { spin(); return; }
  Promise.resolve(go("tonight")).then(() => spin());
}

/** A player control in the open sheet (the buttons under the player), if a player is up. */
function playerAct(name) { const b = q('#dlg .watch.playing [data-act="' + name + '"]'); if (b) b.click(); return !!b; }

function watch() {
  const d = dlg();
  if (d.open) { const b = q(".watch:not(.playing) button.btn", d); if (b) b.click(); return; }
  const w = byText("#home .link", /^Watch it first/); if (w) w.click();
}

function places(step) {
  if (dlg().open || S.spinning || layerOpen()) return;
  const i = TABS.findIndex(t => t[0] === S.tab);
  go(TABS[(i + step + TABS.length) % TABS.length][0]);
}

function sections(step) {
  if (dlg().open || layerOpen()) return;
  const tabs = [...document.querySelectorAll("#view .seg [role=tab]")]; if (tabs.length < 2) return;
  const i = Math.max(0, tabs.findIndex(t => t.getAttribute("aria-selected") === "true"));
  tabs[(i + step + tabs.length) % tabs.length].click();
}

function heldDirection(p, now) {
  let dir = null;
  for (const k in DIR_BUTTONS) if (p.buttons[k] && p.buttons[k].pressed) dir = DIR_BUTTONS[k];
  if (!dir) {
    const x = p.axes[0] || 0, y = p.axes[1] || 0;
    if (Math.max(Math.abs(x), Math.abs(y)) > DEADZONE) dir = Math.abs(x) > Math.abs(y) ? (x > 0 ? "right" : "left") : (y > 0 ? "down" : "up");
  }
  if (!dir) { held.dir = null; return null; }
  if (dir !== held.dir) { held.dir = dir; held.since = held.last = now; return dir; }
  if (now - held.since > FIRST_REPEAT && now - held.last > NEXT_REPEAT) { held.last = now; return dir; }
  return null;
}

/** Every connected controller as one: a button is down if it is down on any of them, a stick is whichever is pushed furthest. (Streaming and remapping tools add virtual pads next to the real one.) */
export function readPads() {
  const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter(p => p && p.connected) : [];
  if (!pads.length) return null;
  const buttons = [], axes = [0, 0, 0, 0];
  for (const p of pads) {
    p.buttons.forEach((b, i) => { const on = !!(b.pressed || (b.value || 0) > 0.5); buttons[i] = { pressed: on || !!(buttons[i] && buttons[i].pressed) }; });
    p.axes.forEach((v, i) => { if (i < 4 && Math.abs(v) > Math.abs(axes[i])) axes[i] = v; });
  }
  return { buttons, axes, names: pads.map(p => p.id) };
}

function frame() {
  const now = performance.now(), p = readPads();
  if (p && S.memories) { p.buttons.forEach((_, i) => edge(p, i)); return; }          // the memories screen ends on any button itself; nothing behind it should react to that press
  if (p) {
    if (!S.pad) connected();
    const dir = heldDirection(p, now);
    const a = edge(p, 0), b = edge(p, 1), x = edge(p, 2), y = edge(p, 3), lb = edge(p, 4), rb = edge(p, 5), lt = edge(p, 6), rt = edge(p, 7), start = edge(p, 9);
    const any = dir || a || b || x || y || lb || rb || lt || rt || start;
    if (any) document.body.classList.add("padnav");
    if (dir) move(dir);
    else if (a) select();
    else if (b) back();
    else if (x) { if (layer && layer.kind === "keys") layer.press("Backspace"); else if (!layerOpen()) { const quick = dlg().open && q("[data-pad='x']", dlg()); if (quick) quick.click(); else if (!playerAct("mute")) another(); } }
    else if (y) { if (layer && layer.kind === "keys") layer.press("Space"); else if (!layerOpen()) { const quick = dlg().open && q("[data-pad='y']", dlg()); if (quick) quick.click(); else if (!playerAct("pause")) watch(); } }
    else if (lb) places(-1);
    else if (rb) places(1);
    else if (lt) sections(-1);
    else if (rt) sections(1);
    else if (start) surprise();
    const ry = p.axes[3] || 0; if (Math.abs(ry) > 0.2) scroller().scrollBy(0, ry * 24);
  }
}

function connected() {
  S.pad = true;
  const e = document.getElementById("padhint"); if (e) e.classList.remove("hide");
  toast("Controller connected.");
}

/** The legend: what each button does right now. Rebuilt only when the situation changes. */
function legend() {
  let box = document.getElementById("padlegend");
  if (!box) { box = h("div", { id: "padlegend", class: "padlegend", "aria-hidden": "true" }); document.body.append(box); }
  const d = dlg(), items = [["A", "Select"]];
  if (layer && layer.kind === "keys") { items.length = 0; items.push(["A", "Type"], ["X", "Backspace"], ["Y", "Space"], ["B", "Done"]); }
  else if (layerOpen()) { items.length = 0; items.push(["A", "Choose"], ["B", "Close"]); }
  else if (d.open) {
    items.push(["B", q(".watch.playing", d) ? "Close player" : "Close"]);
    if (q(".watch.playing", d)) items.push(["X", "Sound"], ["Y", "Pause"]);
    else if (q(".watch button.btn", d)) items.push(["Y", "Watch"]);
  } else {
    items.push(["B", "Back"]);
    if (S.tab === "tonight") items.push(["X", byText("#home button", /^Another$/) ? "Another" : "Surprise us"]);
    if (byText("#home .link", /^Watch it first/)) items.push(["Y", "Watch"]);
    items.push(["LB RB", "Places"]);
    if (document.querySelectorAll("#view .seg [role=tab]").length > 1) items.push(["LT RT", "Sections"]);
    items.push(["Start", "Surprise us"]);
  }
  const key = items.map(i => i.join(":")).join("|");
  if (key === legendKey) return;
  legendKey = key;
  box.replaceChildren(...items.map(([k, t]) => h("span", { class: "padkey" }, h("kbd", {}, k), t)));
}

/** Windows hands controller input only to the page that has focus. Say so, instead of looking dead. */
function wakeNote() {
  let box = document.getElementById("padwake");
  const need = S.pad && !document.hasFocus();
  if (!need) { if (box) box.remove(); return; }
  if (!box) { box = h("div", { id: "padwake", class: "padlegend padwake", role: "status" }, "Controller found. Click anywhere on this page once so it can hear the buttons."); document.body.append(box); }
}
setInterval(wakeNote, 700);

const BUTTON_NAMES = ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "Back", "Start", "Left stick", "Right stick", "D-pad up", "D-pad down", "D-pad left", "D-pad right", "Home"];

/** A live readout of what the browser hears from the controller, for when something doesn't respond. */
export function openPadTest() {
  const d = dlg(); d.className = "";
  const state = h("p", { class: "padtest", role: "status", "aria-live": "off" }, "Press any button on the controller.");
  const names = h("p", { class: "small" });
  d.replaceChildren(h("h2", {}, "Controller test"), names, state,
    h("p", { class: "small" }, "If nothing shows here, click this window once and press a button again. Windows only sends controller input to the focused window."),
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: () => d.close() }, "Close")));
  const timer = setInterval(() => {
    const p = readPads();
    names.textContent = (document.hasFocus() ? "" : "This window isn't focused. ") + (p ? "Hears: " + p.names.join("; ") : "No controller found yet. Press a button on it.");
    if (!p) return;
    const down = p.buttons.map((b, i) => b.pressed ? (BUTTON_NAMES[i] || "Button " + i) : null).filter(Boolean);
    const stick = [["Left stick", p.axes[0], p.axes[1]], ["Right stick", p.axes[2], p.axes[3]]].filter(([, x, y]) => Math.hypot(x, y) > 0.3).map(([n, x, y]) => n + " (" + x.toFixed(1) + ", " + y.toFixed(1) + ")");
    state.textContent = down.concat(stick).join(" + ") || "Nothing pressed.";
  }, 80);
  d.onclose = () => { clearInterval(timer); d.className = ""; d.onclose = null; };
  d.showModal();
}

/** While the controller is in use: keep a focus ring somewhere sensible (a result replaces the button you were on) and the legend true. */
setInterval(() => {
  if (!document.body.classList.contains("padnav") || S.spinning) return;       // not mid-reveal: the result takes the focus when it lands
  legend();
  const a = document.activeElement;
  if (performance.now() - lastScroll < 900) return;                          // never touch focus while the page is being scrolled
  if (!a || a === document.body || !a.isConnected || !usable(a)) { const list = candidates(); if (list.length) focusEl(defaultTarget(list), false); }
}, 250);

/** Mouse or keyboard takes over again. (Movement with no distance is the browser re-checking what is under a still pointer: ignore it.) */
const mouseBack = () => document.body.classList.remove("padnav");
window.addEventListener("pointerdown", mouseBack, true);
window.addEventListener("wheel", mouseBack, { passive: true, capture: true });          // a wheel (or a touch) is the mouse taking over, even when the pointer itself is still
window.addEventListener("touchstart", mouseBack, { passive: true, capture: true });
window.addEventListener("pointermove", e => { if (e.movementX || e.movementY) mouseBack(); }, true);
window.addEventListener("keydown", e => { if (!e.repeat) mouseBack(); if (layer && layer.kind === "keys" && e.key.length === 1) closeLayers(true); }, true);       // typing on a real keyboard: hand the box back

window.addEventListener("gamepadconnected", () => { if (!S.pad) connected(); });
window.addEventListener("gamepaddisconnected", () => {
  S.pad = false; document.body.classList.remove("padnav");
  toast("Controller disconnected. The mouse and keyboard still work.");
  const e = document.getElementById("padhint"); if (e) e.classList.add("hide");
});

setInterval(() => { try { frame(); } catch (e) { console.error("controller:", e); } }, 16);
