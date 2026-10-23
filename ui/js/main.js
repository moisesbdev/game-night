/* Game Night UI. The server decides picks and owns all state; these modules render, animate and send small requests.
   Entry point: the shell (top bar and dock), routing and start-up. See docs/ARCHITECTURE.md. */
import { S, api, h, icon, nm, pids, reload, toast } from "./core.js";
import { viewJournal } from "./journal.js";
import { SUBS, TABS, legacyRoute } from "./logic.js";
import { viewMore } from "./more.js";
import { applySeason } from "./input.js";
import { startIdleWatch } from "./memories.js";
import { startPolling } from "./session.js";
import { viewShelf } from "./shelf.js";
import { afterTonight, setTurn, viewTonight } from "./tonight.js";

const VIEWS = { tonight: viewTonight, shelf: viewShelf, journal: viewJournal, more: viewMore };

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
let dockInd = null, dockPlaced = false;

/** Slides the highlight under the selected dock item (the first placement is instant; later ones animate). */
function placeDockIndicator() {
  const nav = document.getElementById("tabs"), sel = nav && nav.querySelector("[aria-current=page]");
  if (!nav || !sel || !dockInd) return;
  dockInd.style.width = sel.offsetWidth + "px"; dockInd.style.transform = "translateX(" + sel.offsetLeft + "px)";
  if (!dockPlaced) { dockPlaced = true; setTimeout(() => dockInd.classList.add("ready"), 60); }
}
window.addEventListener("resize", () => { if (dockInd) { dockInd.classList.remove("ready"); dockPlaced = false; placeDockIndicator(); } });

/** The dock: four places, one of them selected. A dot on More means Health has something to look at. */
export function buildTabs() {
  const nav = document.getElementById("tabs");
  if (!dockInd) dockInd = h("span", { class: "ind", "aria-hidden": "true" });
  nav.querySelectorAll("button").forEach(b => b.remove());
  if (!dockInd.isConnected) nav.prepend(dockInd);
  TABS.forEach(([id, label, iconName]) => {
    const dot = id === "more" && S.health && ["warn", "fail"].includes(S.health.worst) ? h("span", { class: "dot " + S.health.worst, title: "Health needs a look" }) : null;
    nav.append(h("button", { type: "button", id: "tab-" + id, "aria-label": label + (dot ? " (needs a look)" : ""), "aria-current": S.tab === id ? "page" : null, onclick: () => go(id) }, icon(iconName), h("span", { class: "lbl" }, label), dot));
  });
  placeDockIndicator();
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(placeDockIndicator);
  nav.onkeydown = e => {
    const i = TABS.findIndex(t => t[0] === S.tab);
    if (e.key === "ArrowRight") go(TABS[(i + 1) % TABS.length][0], null, true);
    else if (e.key === "ArrowLeft") go(TABS[(i + TABS.length - 1) % TABS.length][0], null, true);
  };
  buildTurn();
}

/** Whose pick it is, as a small pill at the top; pressing it hands the pick over. */
export function buildTurn() {
  const el = document.getElementById("turn"); if (!el || !S.boot) return;
  const ids = pids(), next = ids[(ids.indexOf(S.boot.turn) + 1) % ids.length];
  if (ids.length < 2) { el.replaceChildren(); return; }                                   // one player: nobody to hand the pick to
  el.replaceChildren(h("button", { type: "button", class: "turn", title: "Hand the pick over", "aria-label": nm(S.boot.turn) + "'s pick. Press to hand it to " + nm(next),
    onclick: () => setTurn(next) }, h("span", { class: "who" }, nm(S.boot.turn) + "'s pick"), icon("swap")));
}

/** Show a place (and optionally one of its sections). Writes the address so a reload comes back to the same spot. */
export function go(id, sub, focusTab) {
  if (!VIEWS[id]) return;
  S.tab = id;
  if (sub && SUBS[id] && SUBS[id].some(s => s[0] === sub)) S.sub[id] = sub;
  const def = SUBS[id] ? SUBS[id][0][0] : null, hash = "#" + id + (def && S.sub[id] !== def ? "/" + S.sub[id] : "");
  if (location.hash !== hash) history.replaceState(null, "", hash);
  buildTabs(); const drawn = render(true);
  { const b = document.getElementById("tab-" + id); if (b && !reduced()) { b.classList.add("pop"); setTimeout(() => b.classList.remove("pop"), 700); } } if (focusTab) document.getElementById("tab-" + id).focus();
  window.scrollTo({ top: 0, behavior: "instant" });
  return drawn;
}

/** A section switcher for a place: `seg(..., goSub("shelf"))`. After a keyboard move the focus goes back to the (re-drawn) selected option. */
export function goSub(tab) {
  return (id, kbd) => { const drawn = go(tab, id); if (kbd) Promise.resolve(drawn).then(() => { const b = document.querySelector("#view .seg [aria-selected=true]"); if (b) b.focus(); }); };
}

/** Placeholder blocks shown only when a view takes a moment to arrive. */
function skeleton() {
  return h("div", { class: "skeleton", "aria-hidden": "true" }, h("div", { class: "sk sk-title" }), h("div", { class: "sk sk-line" }), h("div", { class: "sk sk-line short" }), h("div", { class: "sk-grid" }, [0, 1, 2, 3, 4, 5].map(() => h("div", { class: "sk sk-card" }))));
}

/** Draws the current place. `animate` (set when you move between places) slides the change in with the browser's view transitions, unless motion is reduced. */
let renderSeq = 0;      // only the newest render may draw: a slow earlier one (Health, say) must not land under a place chosen after it
export async function render(animate) {
  const v = document.getElementById("view"), mine = ++renderSeq;
  let arrived = false; const wait = setTimeout(() => { if (!arrived && mine === renderSeq) v.replaceChildren(skeleton()); }, 160);
  try {
    const node = await VIEWS[S.tab](); arrived = true; clearTimeout(wait);
    if (mine !== renderSeq) return;
    let applied = false;
    const apply = () => { if (applied || mine !== renderSeq) return; applied = true; v.replaceChildren(node); if (S.tab === "tonight") afterTonight(); };
    if (animate && document.startViewTransition && !reduced()) {
      // The transition draws the new place when the browser gets to it. If the window isn't producing frames (hidden, minimised, busy), draw it ourselves after a moment: the animation must never be what shows the page.
      try {
        const t = document.startViewTransition(apply);
        for (const p of [t.ready, t.finished, t.updateCallbackDone]) p.catch(() => {});        // a skipped transition rejects these; that is not an error worth showing
        await Promise.race([t.updateCallbackDone.catch(() => {}), new Promise(r => setTimeout(r, 700))]);
      } catch (e) { /* fall through */ }
      apply();
    } else apply();
  } catch (e) { arrived = true; clearTimeout(wait); if (mine === renderSeq) v.replaceChildren(h("div", { class: "errlist", role: "alert" }, "Something went wrong: " + e.message)); }
}

// every sheet is named by its first heading, so a screen reader announces what opened
{ const dlg = document.getElementById("dlg"); let n = 0;
  if (dlg) new MutationObserver(() => { const hd = dlg.querySelector("h2, h1"); if (hd) { hd.id = hd.id || "dlg-title-" + (++n); dlg.setAttribute("aria-labelledby", hd.id); } else dlg.removeAttribute("aria-labelledby"); }).observe(dlg, { childList: true, subtree: true }); }

window.addEventListener("unhandledrejection", e => { toast((e.reason && e.reason.message) || "That didn't work."); e.preventDefault(); });

(async function init() {
  try { await reload(); } catch (e) { document.getElementById("view").textContent = "Couldn't load: " + e.message; return; }
  const [tabPart, subPart] = location.hash.slice(1).split("/"), route = legacyRoute(tabPart);
  if (route) { S.tab = route[0]; const sub = subPart || route[1]; if (sub && SUBS[S.tab] && SUBS[S.tab].some(s => s[0] === sub)) S.sub[S.tab] = sub; }
  applySeason(); buildTabs(); render(); startPolling(); startIdleWatch();
  if (S.boot.warnings && S.boot.warnings.length) document.getElementById("view").before(h("div", { class: "errlist", role: "alert", style: "max-width:960px;margin:8px auto" }, S.boot.warnings.map(w => h("div", {}, w))));
  api("/api/health").then(d => { S.health = d; buildTabs(); }).catch(() => {});
})();
