/* The Shelf: every game as a poster (details open in a sheet), the wishlist, and the library check, one section at a time. */
import { S, api, chip, h, icon, poster, reload, seg, toast } from "./core.js";
import { LEN_NAME, SUBS, daysSince, playersLabel } from "./logic.js";
import { editGame, gameSheet } from "./games.js";
import { librarySection, wishlistSection } from "./library.js";
import { go, goSub, render } from "./main.js";
import { mediaUrl } from "./logic.js";

const canHover = () => matchMedia("(hover: hover) and (pointer: fine)").matches && !matchMedia("(prefers-reduced-motion: reduce)").matches;

/** A little life on the shelf: the poster tilts toward the pointer, and after a short hover the game's own preview plays, muted (from your own media folders; nothing leaves the computer). */
let lastScroll = 0;                                  // hover previews stream video from the server: never start one while the page is moving under a still pointer
addEventListener("scroll", () => { lastScroll = performance.now(); document.querySelectorAll(".hv").forEach(v => v.remove()); }, { passive: true });

function liveTile(btn, g) {
  if (!canHover()) return;
  const poster = btn.querySelector(".poster"); let timer = null, vid = null;
  const stop = () => { clearTimeout(timer); if (vid) { vid.remove(); vid = null; } poster.style.removeProperty("--rx"); poster.style.removeProperty("--ry"); };
  btn.addEventListener("pointermove", e => {
    const r = poster.getBoundingClientRect(), x = (e.clientX - r.left) / r.width - 0.5, y = (e.clientY - r.top) / r.height - 0.5;
    poster.style.setProperty("--ry", (x * 10).toFixed(1) + "deg"); poster.style.setProperty("--rx", (-y * 10).toFixed(1) + "deg");
  });
  btn.addEventListener("pointerenter", () => {
    if (!g.video) return;
    timer = setTimeout(() => {
      if (performance.now() - lastScroll < 500) return;
      vid = h("video", { class: "hv", loop: true, playsinline: true, preload: "auto", src: mediaUrl(g.t, "video"), "aria-hidden": "true", tabindex: "-1" }); vid.muted = true;
      poster.append(vid); vid.play().then(() => { if (vid) vid.classList.add("on"); }).catch(() => {});
    }, 450);
  });
  btn.addEventListener("pointerleave", stop); btn.addEventListener("blur", stop);
}

let fillShelf = () => {};

function filtersActive() { return (S.platform ? 1 : 0) + (S.drafts ? 1 : 0) + (S.onlyLaunch ? 1 : 0) + (S.sort !== "title" ? 1 : 0); }

function openShelfFilters() {
  const dlg = document.getElementById("dlg"); dlg.className = "sheet";
  const select = (label, key, options) => h("div", {}, h("label", { class: "f", for: "sf-" + key }, label),
    h("select", { id: "sf-" + key, onchange: e => { S[key] = e.target.value; fillShelf(); } }, options.map(([v, l]) => h("option", { value: v, selected: S[key] === v }, l))));
  const only = chip("Can start from here", S.onlyLaunch, () => { S.onlyLaunch = !S.onlyLaunch; only.setAttribute("aria-pressed", String(S.onlyLaunch)); fillShelf(); });
  const drafts = chip("Still to check", S.drafts, () => { S.drafts = !S.drafts; drafts.setAttribute("aria-pressed", String(S.drafts)); fillShelf(); });
  dlg.replaceChildren(h("h2", {}, "Filter and sort"),
    select("Platform", "platform", [["", "All platforms"], ...(S.boot.platforms || []).map(p => [p, p])]),
    select("Sort", "sort", [["title", "A to Z"], ["gap", "Skill gap, most even first"], ["played", "Longest since played"]]),
    h("div", { class: "row" }, only, drafts),
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: () => dlg.close() }, "Done"),
      h("button", { class: "btn", type: "button", onclick: () => { S.platform = ""; S.drafts = false; S.onlyLaunch = false; S.sort = "title"; fillShelf(); openShelfFilters(); } }, "Reset")));
  dlg.onclose = () => { dlg.className = ""; dlg.onclose = null; const b = document.getElementById("shelf-filters"); if (b) b.replaceWith(filtersButton()); };
  dlg.showModal();
}

function filtersButton() {
  const n = filtersActive();
  return h("button", { class: "btn", id: "shelf-filters", type: "button", onclick: openShelfFilters }, icon("sliders"), n ? "Filters · " + n : "Filters");
}

function gameTile(g, i) {
  const bits = [playersLabel(g.p), LEN_NAME[g.len]].join(" · ");
  const tile = h("button", { class: "stile", type: "button", style: "--i:" + Math.min(i || 0, 18), onclick: () => gameSheet(g), "aria-label": g.t + ". " + bits + (g.hasLaunch && !g.launchable ? ". Can't be started right now" : "") }, poster(g, null, ""),
    h("span", { class: "stitle" }, g.t, g.hasLaunch && !g.launchable ? h("i", { class: "warn-dot", title: g.launchWhy }) : null), h("span", { class: "smeta" }, bits));
  liveTile(tile, g);
  return tile;
}

function gamesSection() {
  const search = h("input", { type: "search", placeholder: "Search games", "aria-label": "Search games", value: S.search, oninput: e => { S.search = e.target.value; fillShelf(); } });
  const grid = h("div", { class: "shelf" }), count = h("p", { class: "small", role: "status", "aria-live": "polite" });
  fillShelf = () => {
    const q = S.search.trim().toLowerCase();
    let list = S.boot.games.filter(g => (!S.platform || g.sys === S.platform) && (!q || (g.t + " " + g.why + " " + g.vibe.join(" ") + " " + ((g.info && g.info.genres) || []).join(" ") + " " + ((g.info && g.info.tags) || []).join(" ")).toLowerCase().includes(q)) &&
      (!S.onlyLaunch || g.launchable) && (!S.drafts || g.draft));
    list = list.slice().sort((a, b) => S.sort === "gap" ? b.gap - a.gap || a.t.localeCompare(b.t) : S.sort === "played" ? ((daysSince(b.plays && b.plays.last) ?? 9999) - (daysSince(a.plays && a.plays.last) ?? 9999)) || a.t.localeCompare(b.t) : a.t.localeCompare(b.t));
    const refill = grid.classList.contains("settled");
    grid.replaceChildren(...list.map((g, i) => gameTile(g, i))); grid.classList.add("settled");
    if (refill && grid.animate && !matchMedia("(prefers-reduced-motion: reduce)").matches) grid.animate([{ opacity: 0.55 }, { opacity: 1 }], { duration: 160, easing: "ease-out" });      // a soft settle, not a blink
    count.textContent = list.length === S.boot.games.length ? S.boot.games.length + " games" : list.length + " of " + S.boot.games.length + " games";
    if (!list.length) grid.append(h("p", { class: "lede" }, "Nothing matches. Try a different word or clear the filters."));
  };
  const wrap = h("div", {}, h("div", { class: "toolbar" }, h("div", { class: "searchbox" }, icon("search"), search), filtersButton(), h("button", { class: "btn", type: "button", onclick: () => editGame(null) }, icon("plus"), "Add a game")), count, grid);
  fillShelf();
  return wrap;
}

export async function viewShelf() {
  const sub = S.sub.shelf, b = S.boot;
  const body = sub === "games" ? gamesSection() : sub === "wishlist" ? wishlistSection() : await librarySection();
  return h("div", {}, h("div", { class: "pagehead" }, h("h1", {}, "The shelf"), seg("Shelf sections", SUBS.shelf, sub, goSub("shelf"))),
    !b.games.length && sub === "games" ? h("p", { class: "banner" }, "The shelf is empty. ", h("button", { class: "link", type: "button", onclick: () => go("shelf", "library") }, "Add games from your Steam library or game folders"), ", add one by hand, or ",
      h("button", { class: "link", type: "button", onclick: async () => { const r = await api("/api/sample", {}); await reload(); toast("Added " + r.added + " sample games."); render(); } }, "start with 20 well-known games"), ".") : null,
    body);
}
