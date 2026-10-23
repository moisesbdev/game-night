/* One game, in detail: the sheet that opens from the shelf, plus notes, similar games and the add/edit dialog. */
import { S, api, chip, dots, gameByTitle, h, icon, nm, players, poster, rater, reload, toast } from "./core.js";
import { LEN_HELP, LEN_NAME, MODES, MOODS, PLATFORMS, VIBE_NAME, ago, aboutLines, daysSince, mediaUrl, playersLabel, sourceNames } from "./logic.js";
import { go, render } from "./main.js";
import { startGame } from "./tonight.js";
import { stopWatching, watchPanel } from "./watch.js";

export function markNote(g) { const t = { loved: "loved it", curious: "looks fun", pass: "not really", meh: "not for us", played: "played" }; return players().filter(p => g.marks[p.id]).map(p => p.name + ": " + t[g.marks[p.id]]).join(" · "); }

export function setRater(w) { S.rater = w; try { localStorage.setItem("gn-rater", w); } catch (e) { /* ignore */ } }

/** "About this game": what the databases say (year, rating, genres, tags, lengths, scores), collapsed so the sheet stays calm. */
function aboutSection(g) {
  const lines = aboutLines(g.info), tags = (g.info && g.info.tags) || [], src = sourceNames(g.info);
  if (!lines.length && !tags.length) return null;
  return h("details", { class: "about" }, h("summary", {}, "About this game"),
    lines.map(l => h("p", { class: "small" }, l)),
    tags.length ? h("div", { class: "tags" }, tags.map(t => h("span", { class: "tag" }, t))) : null,
    src.length ? h("p", { class: "small" }, "From " + src.join(", ") + (g.info.asOf ? " (checked " + g.info.asOf + ")" : "") + ".") : null);
}

/** The pictures found for this game (a screenshot and the title screen): small, and a press shows one large. */
function shotStrip(g) {
  const kinds = [g.shot ? ["shot", "Screenshot"] : null, g.titleShot ? ["title", "Title screen"] : null].filter(Boolean);
  if (!kinds.length) return null;
  const strip = h("div", { class: "shots", role: "group", "aria-label": "Pictures of " + g.t });
  kinds.forEach(([k, label]) => strip.append(h("button", { type: "button", class: "shot", "aria-pressed": "false", "aria-label": label + " of " + g.t + " (press to enlarge)", onclick: e => {
    const b = e.currentTarget, big = b.getAttribute("aria-pressed") !== "true";
    strip.querySelectorAll(".shot").forEach(x => x.setAttribute("aria-pressed", "false"));
    b.setAttribute("aria-pressed", String(big)); strip.classList.toggle("one", big);
  } }, h("img", { src: mediaUrl(g.t, k), alt: "", loading: "lazy" }))));
  return strip;
}

/** Everything about one game: what it is, how to play it together, what to watch for, your history with it, and the things you can do with it. */
export function gameSheet(g, opts) {
  const dlg = document.getElementById("dlg"); dlg.className = "sheet gamesheet";
  const marks = h("p", { class: "small" }, markNote(g));
  const pnote = h("p", { class: "small" }, g.progress && g.progress.note ? "Note: " + g.progress.note : "");
  const d = g.plays ? daysSince(g.plays.last) : null;
  const rate = h("div", { class: "row", role: "group", "aria-label": "Your rating" }, h("span", { class: "t" }, "Rating as"),
    players().map(p => chip(p.name, rater() === p.id, () => { setRater(p.id); paintRater(); }, { "data-w": p.id })));
  const buttons = h("div", { class: "row", role: "group", "aria-label": "Mark this game" });
  [["loved", "Loved it"], ["meh", "Not for us"], ["played", "Played"]].forEach(([k, l]) => {
    buttons.append(chip(l, g.marks[rater()] === k, async () => {
      const cur = g.marks[rater()] === k ? null : k;
      const r = await api("/api/marks", { title: g.t, who: rater(), mark: cur });
      g.marks = r.marks; buttons.querySelectorAll("[data-k]").forEach(x => x.setAttribute("aria-pressed", String(g.marks[rater()] === x.dataset.k))); marks.textContent = markNote(g);
    }, { "data-k": k, "aria-label": l + " (marked by " + nm(rater()) + "): " + g.t }));
  });
  const paintRater = () => {          // switching who is rating changes only the pressed states, never rebuilds the sheet
    rate.querySelectorAll("[data-w]").forEach(c => c.setAttribute("aria-pressed", String(c.dataset.w === rater())));
    buttons.querySelectorAll("[data-k]").forEach(x => { x.setAttribute("aria-pressed", String(g.marks[rater()] === x.dataset.k)); x.setAttribute("aria-label", x.textContent.trim() + " (marked by " + nm(rater()) + "): " + g.t); });
  };
  const tags = h("div", { class: "tags" }, [g.sys, g.where === "couch" ? "same couch" : "online", playersLabel(g.p), g.mode, LEN_NAME[g.len]].map(x => h("span", { class: "tag" }, x)), g.vibe.map(v => h("span", { class: "tag" }, VIBE_NAME[v])),
    g.draft ? h("span", { class: "tag flag", title: "Added from your library: check the details (Edit), or press Look it up" }, "to check") : null, g.one ? h("span", { class: "tag" }, "one controller") : null, g.progress && g.progress.ongoing ? h("span", { class: "tag" }, "in progress") : null);
  const playNote = h("p", { class: "small", role: "status" });
  dlg.replaceChildren(h("div", { class: "gs" }, poster(g, null, "Box art for " + g.t),
    h("div", { class: "gsbody" }, h("h2", {}, g.t), h("div", { class: "meta" }, "Skill gap", dots(g.gap)), tags,
      watchPanel(g, opts && opts.watch),
      shotStrip(g),
      h("p", {}, g.why), g.role ? h("p", {}, h("span", { class: "lab" }, "Tip  "), g.role) : null, g.watch ? h("p", { class: "warn" }, "Watch out: " + g.watch) : null,
      g.time ? h("p", { class: "small" }, "Length: " + g.time) : null,
      aboutSection(g),
      g.mySessions ? h("p", { class: "small" }, "Your nights: usually " + g.mySessions.low + (g.mySessions.high !== g.mySessions.low ? " to " + g.mySessions.high : "") + " minutes (" + g.mySessions.n + " nights)") : null,
      d != null ? h("p", { class: "small" }, "Last played " + ago(d)) : null,
      h("div", { class: "gsrate" }, rate, buttons, marks, pnote),
      playNote,
      h("div", { class: "row" }, g.launchable ? h("button", { class: "btn primary", type: "button", onclick: async ev => {
        const b = ev.currentTarget; if (b.disabled) return;
        b.disabled = true; playNote.textContent = "";                                   // one press, one launch: the button stays off until the answer is in
        try { const r = await startGame(g); if (r) { playNote.textContent = r.message; toast(r.message); } } catch (e) { playNote.textContent = e.message; }
        finally { setTimeout(() => { b.disabled = false; }, 4000); }
      } }, "Play") : null,
        h("button", { class: "btn", type: "button", onclick: () => notesDialog(g) }, "Notes"), h("button", { class: "btn", type: "button", onclick: () => similarDialog(g) }, "More like this"),
        h("button", { class: "btn", type: "button", onclick: () => editGame(g) }, "Edit"), S.boot.scrape ? h("button", { class: "btn", type: "button", onclick: () => lookUp(g, playNote) }, "Look it up") : null,
        h("button", { class: "link", type: "button", onclick: () => dlg.close() }, icon("close"), "Close")))));
  dlg.onclose = () => { stopWatching(dlg); dlg.className = ""; dlg.onclose = null; };
  if (!dlg.open) dlg.showModal();
}

export function notesDialog(g) {
  const dlg = document.getElementById("dlg"); dlg.className = "";
  const note = h("textarea", { id: "pn", maxlength: "280", "aria-label": "Note for " + g.t }, g.progress ? g.progress.note : "");
  const on = h("input", { type: "checkbox", id: "po", checked: !!(g.progress && g.progress.ongoing) });
  const err = h("div", { class: "hide errlist", role: "alert" });
  dlg.replaceChildren(h("h2", { style: "margin-top:0" }, "Notes: " + g.t), h("p", { class: "small" }, "Where you are, what's next. Shared by everyone. 280 characters."), note,
    h("label", { class: "small" }, on, " Save in progress (this game gets a bigger share when picking)"), err,
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: async () => {
      try { const r = await api("/api/progress", { title: g.t, note: note.value, ongoing: on.checked }); g.progress = r.progress; toast("Saved."); gameSheet(g); }
      catch (e) { err.classList.remove("hide"); err.textContent = e.message; }
    } }, "Save"), h("button", { class: "btn", type: "button", onclick: () => gameSheet(g) }, "Cancel")));
  if (!dlg.open) dlg.showModal();
}

export async function similarDialog(g) {
  const dlg = document.getElementById("dlg"), r = await api("/api/similar", { title: g.t }); dlg.className = "";
  dlg.replaceChildren(h("h2", { style: "margin-top:0" }, "More like " + g.t),
    h("div", { class: "similar" }, r.titles.map(t => { const o = gameByTitle(t); return h("button", { class: "pickcard row-card", type: "button", onclick: () => gameSheet(o) }, poster(o, null, ""), h("div", {}, h("b", {}, t), h("div", { class: "meta" }, o.where === "couch" ? "same couch" : "online", " · ", o.p + " players · ", o.mode), h("div", {}, o.why))); })),
    h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: () => gameSheet(g) }, "Back")));
  if (!dlg.open) dlg.showModal();
}

/** Open the shelf on one game. The shelf's sticky filters are cleared first so the game can't be hidden by a filter set earlier. */
export function jumpToGame(title) {
  S.search = title; S.onlyLaunch = false; S.platform = ""; S.drafts = false;
  go("shelf", "games");
}

/** Look a game up online (Steam store first, then SteamGridDB for art when a key is set). Fills in what's missing; never overwrites what you wrote. */
export async function lookUp(g, note) {
  note.textContent = "Looking it up...";
  const said = [];
  for (const provider of ["steam", "steamgriddb"]) {
    try {
      const r = await api("/api/scrape", { title: g.t, provider });
      said.push(r.source + ": " + (r.art.length ? "art saved" : "no new art") + (r.changed ? ", details filled in" : ""));
    } catch (e) { if (provider === "steam" || !/key/.test(e.message)) said.push((provider === "steam" ? "Steam store" : "SteamGridDB") + ": " + e.message); }
  }
  note.textContent = said.join(". ") + ".";
  await reload(); const fresh = gameByTitle(g.t); if (fresh) gameSheet(fresh); render();
}

/** How Play starts this game, as the editor shows it: [kind, value, folder]. */
function launchParts(l) {
  if (!l) return ["none", "", ""];
  if (l.steam) return ["steam", String(l.steam), ""];
  if (l.url) return ["url", l.url, ""];
  if (l.open) return ["open", l.open, ""];
  if (l.command) return ["command", l.command.map(a => /\s/.test(a) ? '"' + a + '"' : a).join(" "), l.cwd || ""];
  return ["none", "", ""];
}

/** `"C:\Games\x.exe" --flag "two words"` -> ["C:\Games\x.exe", "--flag", "two words"] (quotes group words; no shell is ever involved). */
export function splitCommand(text) {
  const out = []; let cur = "", q = false, any = false;
  for (const c of text.trim()) {
    if (c === '"') { q = !q; any = true; continue; }
    if (/\s/.test(c) && !q) { if (cur || any) out.push(cur); cur = ""; any = false; continue; }
    cur += c;
  }
  if (cur || any) out.push(cur);
  return out;
}

const DRAFT_WHY = "Added from your library; the details haven't been checked yet.";

/** `opts.wish`: a wishlist title to take off the list once this game is saved (and not before, so Cancel leaves the wish alone). */
export function editGame(g, prefill, opts) {
  const dlg = document.getElementById("dlg"), isNew = !g; dlg.className = "sheet";
  const v = g || Object.assign({ sys: "PC", where: "couch", p: "1-2", mode: "Co-op", gap: 3, vibe: ["cozy"], len: "evening", why: "", role: "", watch: "" }, prefill || {});
  const f = {};
  const inp = (name, label, val, extra) => h("div", {}, h("label", { class: "f", for: "f-" + name }, label), f[name] = h("input", { id: "f-" + name, type: "text", value: val == null ? "" : val, ...(extra || {}) }));
  const sel = (name, label, options, val) => h("div", {}, h("label", { class: "f", for: "f-" + name }, label), f[name] = h("select", { id: "f-" + name }, options.map(o => { const [value, text] = Array.isArray(o) ? o : [o, o]; return h("option", { value, selected: value == val }, text); })));
  const ta = (name, label, val) => h("div", {}, h("label", { class: "f", for: "f-" + name }, label), f[name] = h("textarea", { id: "f-" + name }, val || ""));
  const list = (id, items) => h("datalist", { id }, items.map(x => h("option", { value: x })));
  const vib = MOODS.map(([m, l]) => h("label", { class: "small", style: "margin-right:10px" }, f["v-" + m] = h("input", { type: "checkbox", checked: v.vibe.includes(m) }), " " + l));
  const err = h("div", { class: "hide errlist", role: "alert" });
  const one = h("label", { class: "small" }, f.one = h("input", { type: "checkbox", checked: !!v.one }), " Good with one controller (one plays, the others watch, or you pass it around)");
  const [lk, lv, lcwd] = launchParts(v.launch);
  const launchHelp = h("p", { class: "small" });
  const kinds = [["none", "Not from here (a console game, say)"], ["steam", "Steam: the game's app id"], ["url", "A launcher link (steam://, heroic://, lutris:, itch://, Epic, GOG, Ubisoft)"], ["open", "A shortcut file (.lnk, .url or .desktop)"], ["command", "A program, with any arguments"]];
  const help = { none: "Play buttons won't show; the game is still drawn.", steam: "The number in the game's Steam store address, like 620 for Portal 2.", url: "The link your launcher gives for the game.", open: "The full path to the shortcut file.", command: "The full path to the program, then its arguments. Put quotes around anything with spaces. It is started directly, never through a shell." };
  const paintLaunch = () => { const k = f.lkind.value; f.lval.closest("div").classList.toggle("hide", k === "none"); f.lcwd.closest("div").classList.toggle("hide", k !== "command"); launchHelp.textContent = help[k]; };
  dlg.replaceChildren(h("h2", { style: "margin-top:0" }, isNew ? "Add a game" : "Edit " + v.t),
    inp("t", "Title", v.t), inp("sys", "Platform, like PC, Switch or PlayStation 5", v.sys, { list: "f-sys-list" }), list("f-sys-list", [...new Set([...(S.boot.platforms || []), ...PLATFORMS])]),
    sel("where", "Where", [["couch", "Same couch, one screen"], ["online", "Online, separate screens"]], v.where), inp("p", "Players, like 2 or 1-4", v.p), inp("mode", "How you play together", v.mode, { list: "f-mode-list" }), list("f-mode-list", MODES),
    sel("gap", "Skill gap (5 = skill barely matters)", [1, 2, 3, 4, 5], v.gap), h("div", {}, h("label", { class: "f" }, "Moods"), h("div", {}, vib)),
    sel("len", "Length", [["quick", "Quick: " + LEN_HELP.quick], ["evening", "An evening: " + LEN_HELP.evening], ["long", "Long-running: " + LEN_HELP.long]], v.len), inp("time", "Length note, like \"about 8 hours to finish\" (optional)", v.time), one,
    ta("why", "What it is (one or two lines)", v.why === DRAFT_WHY ? "" : v.why), ta("role", "A tip about the game's own features (optional)", v.role), ta("watch", "Watch out for (optional)", v.watch),
    h("h3", { class: "sub" }, "Starting it"), sel("lkind", "How Play starts it", kinds, lk), inp("lval", "Value", lv), inp("lcwd", "Start in this folder (optional)", lcwd), launchHelp,
    inp("process", "Its program name, so the session ends when it closes (optional, like Game.exe)", v.process),
    h("h3", { class: "sub" }, "Watching it"),
    inp("trailer", "Trailer: a YouTube link or video id (optional)", v.trailer),
    inp("coop", "Co-op gameplay clip: a YouTube link or video id (optional)", v.coop), sel("coopKind", "That clip shows", [["local", "Playing together on one screen"], ["online", "Playing together online"]], v.coopKind || "local"),
    err,
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: save }, "Save"), h("button", { class: "btn", type: "button", onclick: () => dlg.close() }, "Cancel"),
      isNew ? null : h("button", { class: "link danger", type: "button", onclick: remove }, "Remove from the shelf")));
  f.lkind.addEventListener("change", paintLaunch); paintLaunch();
  dlg.onclose = () => { dlg.className = ""; dlg.onclose = null; };
  async function remove() {
    if (!confirm("Remove " + v.t + " from the shelf? Your marks and history for it stay in the journal.")) return;
    try { await api("/api/games/delete", { title: v.t }); dlg.close(); await reload(); toast(v.t + " removed."); render(); } catch (e) { err.classList.remove("hide"); err.textContent = e.message; }
  }
  async function save() {
    const game = Object.assign({}, isNew ? {} : { info: v.info, media: v.media, coopBy: v.coopBy }, { t: f.t.value.trim(), sys: f.sys.value.trim(), where: f.where.value, p: f.p.value.trim(), mode: f.mode.value.trim(), gap: +f.gap.value, len: f.len.value,
      vibe: MOODS.map(m => m[0]).filter(m => f["v-" + m].checked), why: f.why.value.trim(), role: f.role.value.trim(), watch: f.watch.value.trim() });
    if (f.time.value.trim()) game.time = f.time.value.trim();
    if (f.one.checked) game.one = true;
    if (f.process.value.trim()) game.process = f.process.value.trim();
    const k = f.lkind.value, val = f.lval.value.trim();
    if (k === "steam") game.launch = { steam: Number(val) };
    else if (k === "url") game.launch = { url: val };
    else if (k === "open") game.launch = { open: val };
    else if (k === "command") { game.launch = { command: splitCommand(val) }; if (f.lcwd.value.trim()) game.launch.cwd = f.lcwd.value.trim(); }
    game.trailer = f.trailer.value.trim();                  // empty removes it; the server turns a pasted link into the bare id
    game.coop = f.coop.value.trim(); if (game.coop) game.coopKind = f.coopKind.value;
    if (!(game.coop && game.coop === v.coop)) delete game.coopBy;                       // editing keeps who made its clip; a different clip drops it
    for (const key of Object.keys(game)) if (game[key] === undefined || game[key] === "") delete game[key];
    try {
      await api("/api/games", { game, create: isNew, oldTitle: isNew ? null : v.t });
      if (opts && opts.wish) { try { await api("/api/wishlist", { action: "remove", title: opts.wish }); } catch (e) { /* the game is saved; a leftover wish can be removed by hand */ } }
      dlg.close(); await reload(); toast("Saved " + game.t); render();
    }
    catch (e) { err.classList.remove("hide"); err.replaceChildren(...e.message.split("; ").map(m => h("div", {}, m))); }
  }
  dlg.showModal();
}
