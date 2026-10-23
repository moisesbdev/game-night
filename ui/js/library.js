/* The Shelf's other two sections: adding games from your library (Steam and game folders), and the wishlist. */
import { S, api, chip, h, mascot, poster, reload, toast } from "./core.js";
import { editGame, lookUpAll, lookupSources } from "./games.js";
import { go, render } from "./main.js";

/** The latest price check, if one was recorded: "Lowest $19.99 at the eShop · usually $39.99 · checked 2026-10-22". */
function priceLine(p) {
  if (!p) return null;
  const bits = [p.regular ? "usually " + p.regular : "", p.recentLow ? "lowest lately " + p.recentLow : "", "checked " + p.asOf].filter(Boolean);
  return h("p", { class: "small price" }, h("b", {}, "Lowest " + p.low), " at " + p.where + " · " + bits.join(" · ") + (p.note ? ". " + p.note : ""));
}

export function wishlistSection() {
  const b = S.boot;
  return h("div", {}, h("div", { class: "row" }, h("p", { class: "lede", style: "margin:0" }, "Games worth getting, kept out of the picker until you have them."), h("button", { class: "btn primary", type: "button", onclick: () => wishDialog() }, "Add to the wishlist")),
    b.wishlist.length ? h("div", { class: "wishes" }, b.wishlist.map(w => h("article", { class: "wish" }, poster({ t: w.t, cover: false }, null, ""),
      h("div", { class: "wishbody" }, h("h3", {}, w.t), h("span", { class: "tag" }, w.sys), h("p", {}, w.why), priceLine(w.price),
        h("div", { class: "row" },
          h("button", { class: "btn", type: "button", title: "Take it off the wishlist and add it to the picker", onclick: () => editGame(null, { t: w.t, sys: w.sys }, { wish: w.t }) }, "Got it"),
          h("button", { class: "btn", type: "button", onclick: () => priceDialog(w) }, w.price ? "Update price" : "Note a price"),
          h("button", { class: "btn", type: "button", onclick: async () => { try { await api("/api/wishlist", { action: "remove", title: w.t }); await reload(); toast(w.t + " removed."); render(); } catch (e) { toast(e.message); } } }, "Remove"))))))
      : h("div", { class: "panel empty" }, mascot(), h("div", {}, h("b", {}, "Nothing on the wishlist"), h("p", { class: "small" }, "Add the games you'd like to try together."))));
}

function priceDialog(w) {
  const dlg = document.getElementById("dlg"); dlg.className = "";
  const p = w.price || {}, f = {};
  const inp = (k, label, val) => h("div", {}, h("label", { class: "f", for: "pr-" + k }, label), f[k] = h("input", { id: "pr-" + k, type: "text", value: val || "" }));
  const err = h("div", { class: "hide errlist", role: "alert" });
  dlg.replaceChildren(h("h2", { style: "margin-top:0" }, "Price: " + w.t), h("p", { class: "small" }, "What you found, so you can compare later. Game Night doesn't look prices up by itself."),
    inp("low", "Lowest price, like $19.99", p.low), inp("where", "Where", p.where), inp("regular", "Usual price (optional)", p.regular), inp("note", "Note, like \"sale ends Friday\" (optional)", p.note), inp("source", "Link (optional, https)", p.source), err,
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: async () => {
      const price = { low: f.low.value.trim(), where: f.where.value.trim(), asOf: S.boot.today };
      for (const k of ["regular", "note", "source"]) if (f[k].value.trim()) price[k] = f[k].value.trim();
      try { await api("/api/wishlist", { action: "price", title: w.t, price }); dlg.close(); await reload(); render(); } catch (e) { err.classList.remove("hide"); err.textContent = e.message; }
    } }, "Save"), w.price ? h("button", { class: "btn", type: "button", onclick: async () => { await api("/api/wishlist", { action: "price", title: w.t, price: null }); dlg.close(); await reload(); render(); } }, "Clear") : null,
      h("button", { class: "btn", type: "button", onclick: () => dlg.close() }, "Cancel")));
  dlg.showModal();
}

export function wishDialog() {
  const dlg = document.getElementById("dlg"); dlg.className = "";
  const t = h("input", { type: "text", id: "w-t", maxlength: "80" }), sys = h("input", { type: "text", id: "w-sys", maxlength: "24", value: "PC", list: "w-sys-list" }), why = h("textarea", { id: "w-why", maxlength: "240" });
  const err = h("div", { class: "hide errlist", role: "alert" });
  dlg.replaceChildren(h("h2", {}, "Add to the wishlist"), h("label", { class: "f", for: "w-t" }, "Title"), t, h("label", { class: "f", for: "w-sys" }, "Platform"), sys,
    h("datalist", { id: "w-sys-list" }, (S.boot.platforms || []).map(p => h("option", { value: p }))),
    h("label", { class: "f", for: "w-why" }, "What it is and why it fits your group (one line)"), why, err,
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: async () => {
      try { await api("/api/wishlist", { action: "add", entry: { t: t.value, sys: sys.value, why: why.value } }); dlg.close(); await reload(); toast("Added " + t.value.trim()); render(); }
      catch (e) { err.classList.remove("hide"); err.textContent = e.message; }
    } }, "Add"), h("button", { class: "btn", type: "button", onclick: () => dlg.close() }, "Cancel")));
  dlg.showModal(); t.focus();
}

/** "Steam 27 · ES-DE 62 · GOG 8" for the sources that found something not yet on the shelf. */
export function foundSummary(sources) {
  return sources.map(s => [s.label, s.games.filter(g => !g.inCatalog).length]).filter(([, n]) => n).map(([l, n]) => l + " " + n).join(" · ");
}

/** Everything the library detectors found, one section per source (grouped by platform where a source holds several). Pick some and
 *  they're added as drafts with what the source could tell; "Add and look them up" also asks the lookup sources. */
export async function librarySection() {
  const d = await api("/api/library");
  const picked = new Set();
  const status = h("p", { class: "small", role: "status", "aria-live": "polite" });
  const count = () => { status.textContent = picked.size ? picked.size + " selected" : ""; };
  const tick = (box, on) => box.querySelectorAll("input[type=checkbox]").forEach(c => { if (c.checked !== on) { c.checked = on; c.dispatchEvent(new Event("change")); } });
  const allChip = (box, label) => { const c = chip(label || "Select all", false, () => { const on = c.getAttribute("aria-pressed") !== "true"; c.setAttribute("aria-pressed", String(on)); tick(box, on); }); return c; };
  const item = x => h("li", {}, h("label", {},
    h("input", { type: "checkbox", onchange: e => { const k = x.source + "|" + x.key; if (e.target.checked) picked.add(k); else picked.delete(k); count(); } }),
    " " + x.title));
  const section = src => {
    const fresh = src.games.filter(x => !x.inCatalog), have = src.games.length - fresh.length;
    const head = h("summary", {}, h("b", {}, src.label), h("span", { class: "small" }, "  " + fresh.length + " new" + (have ? " · " + have + " already on the shelf" : "")));
    const box = h("details", { class: "panel libsource", open: fresh.length > 0 && fresh.length <= 60 }, head,
      src.note ? h("p", { class: "small" }, src.note) : null);
    if (!fresh.length) { box.append(h("p", { class: "small" }, "Everything here is already on the shelf.")); return box; }
    const groupOf = x => x.group || x.sys, platforms = [...new Set(fresh.map(groupOf))];
    if (platforms.length > 1) {
      box.append(h("div", { class: "row" }, allChip(box, "Select all " + fresh.length)));
      for (const p of platforms) {
        const games = fresh.filter(x => groupOf(x) === p), group = h("div", { class: "libgroup" });
        group.append(h("div", { class: "row" }, h("b", { class: "small" }, p + " (" + games.length + ")"), allChip(group, "Select these")), h("ul", { class: "libpick" }, games.map(item)));
        box.append(group);
      }
    } else box.append(h("div", { class: "row" }, allChip(box)), h("ul", { class: "libpick" }, fresh.map(item)));
    return box;
  };
  const add = async (lookup) => {
    if (!picked.size) { status.textContent = "Pick some games first."; return; }
    const picks = [...picked].map(k => { const [source, ...rest] = k.split("|"); return { source, key: rest.join("|") }; });
    const before = new Set(S.boot.games.map(g => g.t));
    try {
      const r = await api("/api/library/import", { picks });
      status.textContent = "Added " + r.added + " to the shelf." + (r.refused.length ? " Not added: " + r.refused.join("; ") : "");
      await reload();
      const added = S.boot.games.filter(g => !before.has(g.t));
      if (lookup && S.boot.scrape) {
        const sources = await lookupSources(), need = added.filter(g => !g.cover || /haven't been checked/.test(g.why));
        let n = 0;
        for (const g of need) { status.textContent = "Looking up " + (++n) + " of " + need.length + ": " + g.t; await lookUpAll(g.t, sources); }      // a game no source knows stays a draft to fill in by hand
        await reload();
        status.textContent = "Added " + r.added + (need.length ? " and looked up " + need.length : "") + ". Games marked \"to check\" still need a look at how they're played together.";
      }
      toast(status.textContent); render();
    } catch (e) { status.textContent = e.message; }
  };
  const found = d.sources.filter(s => s.foundAt.length), missing = d.sources.filter(s => !s.foundAt.length && s.id !== "folder");
  const summary = foundSummary(d.sources);
  return h("div", {},
    h("p", { class: "lede" }, summary ? "Found on this computer: " + summary + "." : "Nothing new found on this computer's launchers and folders."),
    h("p", { class: "small" }, "Tick what you'd play together. New games can be drawn straight away and are marked \"to check\" until someone looks over how they're played."),
    found.map(section),
    h("div", { class: "row" }, S.boot.scrape ? h("button", { class: "btn primary", type: "button", onclick: () => add(true) }, "Add and look them up") : null,
      h("button", { class: "btn" + (S.boot.scrape ? "" : " primary"), type: "button", onclick: () => add(false) }, "Add"), h("button", { class: "link", type: "button", onclick: () => go("more", "settings") }, "Game folders and Steam settings")),
    status,
    missing.length ? h("p", { class: "small" }, "Not on this computer: " + missing.map(s => s.label).join(", ") + ". Games anywhere else can come in through a game folder (Settings) or Add a game.") : null,
    S.boot.scrape ? h("p", { class: "small" }, "Looking up uses the Steam store's public pages (no account), plus TheGamesDB, ScreenScraper and SteamGridDB when you've added their keys in Settings, and only when you press it.") : null);
}
