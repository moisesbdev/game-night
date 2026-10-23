/* The Settings section of More: who's playing, how picks treat "not for us", where games and art live, and the look. */
import { S, api, h, reload, settings, toast } from "./core.js";
import { seasonOn, setCouch, setSeasonOn } from "./input.js";
import { openPadTest } from "./pad.js";
import { buildTurn, render } from "./main.js";
import { memoriesOn, openMemories, setMemoriesOn } from "./memories.js";

const MAX_PLAYERS = 8;

/** The group: one row per player (a name and a Remove button), up to eight. Ids stay stable so marks follow a renamed player. */
function playersPanel(draft) {
  const box = h("div", { class: "players" });
  const paint = () => {
    box.replaceChildren(...draft.map((p, i) => h("div", { class: "row" },
      h("label", { class: "sr", for: "pl-" + p.id }, "Player " + (i + 1)),
      h("input", { type: "text", id: "pl-" + p.id, value: p.name, maxlength: "24", oninput: e => { p.name = e.target.value; } }),
      draft.length > 1 ? h("button", { class: "link", type: "button", onclick: () => { draft.splice(i, 1); paint(); } }, "Remove") : null)),
      draft.length < MAX_PLAYERS ? h("button", { class: "btn", type: "button", onclick: () => {
        let n = 1; while (draft.some(p => p.id === "p" + n)) n++;
        draft.push({ id: "p" + n, name: "Player " + (draft.length + 1) }); paint(); box.querySelector("input:last-of-type")?.focus();
      } }, "Add a player") : h("p", { class: "small" }, "Eight is the most."));
  };
  paint();
  return box;
}

/** Folders where art lives: a path and a layout per row. */
function mediaPanel(draft) {
  const box = h("div", {});
  const paint = () => box.replaceChildren(...draft.map((m, i) => h("div", { class: "row" },
    h("input", { type: "text", value: m.path, "aria-label": "Art folder " + (i + 1), placeholder: "A folder with covers/, screenshots/, videos/", oninput: e => { m.path = e.target.value; } }),
    h("select", { "aria-label": "Layout of art folder " + (i + 1), onchange: e => { m.layout = e.target.value; } }, [["slug", "Named after each game"], ["esde", "ES-DE downloaded_media"]].map(([v, l]) => h("option", { value: v, selected: m.layout === v }, l))),
    h("button", { class: "link", type: "button", onclick: () => { draft.splice(i, 1); paint(); } }, "Remove"))),
    h("button", { class: "btn", type: "button", onclick: () => { draft.push({ path: "", layout: "slug" }); paint(); } }, "Add an art folder"));
  paint();
  return box;
}

export async function settingsSection() {
  const wt = (await api("/api/weights")).table;
  const b = S.boot, st = settings();
  const draft = { players: (b.players || []).map(p => ({ ...p })), media: (st.mediaDirs || []).map(m => ({ ...m })) };
  const box = (id, on) => h("input", { type: "checkbox", id, checked: on });
  const lc = box("launch", st.launchEnabled), askBox = box("ask", st.askAfterGame !== false), trailerBox = box("trailers", st.trailersEnabled !== false);
  const meh = h("select", { id: "meh" }, [["leave-out", "Leaves the game out of the draw"], ["rarer", "Makes it rarer (x0.3)"]].map(([v, l]) => h("option", { value: v, selected: st.notForUs === v }, l)));
  const gameDirs = h("textarea", { id: "gamedirs", rows: "3", placeholder: "One folder per line" }, (st.gameDirs || []).join("\n"));
  const steamDir = h("input", { type: "text", id: "steamdir", value: st.steamDir || "", placeholder: st.steamFound ? "Found: " + st.steamFound : "Not found; set its folder here" });
  const gridKey = h("input", { type: "password", id: "gridkey", value: st.steamgriddbKey === "set" ? "set" : "", autocomplete: "off", placeholder: "Optional" });
  const seasonBox = h("input", { type: "checkbox", id: "season", checked: seasonOn(), onchange: e => setSeasonOn(e.target.checked) });
  const memBox = h("input", { type: "checkbox", id: "memidle", checked: memoriesOn(), onchange: e => setMemoriesOn(e.target.checked) });
  const couchBox = h("input", { type: "checkbox", id: "couch", checked: document.body.classList.contains("couch"), onchange: e => setCouch(e.target.checked) });
  const file = h("input", { type: "file", accept: "application/json,.json", class: "hide", "aria-label": "Choose an export file", onchange: ev => {
    const f = ev.target.files[0]; if (!f) return; const rd = new FileReader();
    rd.onload = async () => { try { const r = await api("/api/import", JSON.parse(rd.result)); await reload(); toast("Imported " + r.imported + " marks and " + r.gamesAdded + " games."); render(); } catch (e) { toast("That file isn't an export: " + e.message); } };
    rd.readAsText(f); ev.target.value = "";
  } });
  const save = async () => {
    try {
      const r = await api("/api/settings", {
        players: draft.players.map(p => ({ id: p.id, name: p.name.trim() })), notForUs: meh.value, launchEnabled: lc.checked, askAfterGame: askBox.checked, trailersEnabled: trailerBox.checked,
        gameDirs: gameDirs.value.split(/\r?\n/).map(x => x.trim()).filter(Boolean), steamDir: steamDir.value.trim(), mediaDirs: draft.media.filter(m => m.path.trim()), steamgriddbKey: gridKey.value.trim(),
      });
      await reload(); buildTurn(); toast("Saved."); render();
      return r;
    } catch (e) { toast(e.message); }
  };
  return h("div", {},
    h("div", { class: "panel" }, h("b", {}, "Who's playing"), h("p", { class: "small" }, "A couple, a few friends, or just you: up to eight. Each person's marks and favourites count in the draw, and the pick goes round in this order."), playersPanel(draft.players),
      h("label", { class: "f", for: "meh" }, "When someone marks a game \"not for us\""), meh),
    h("div", { class: "panel" }, h("b", {}, "Where your games are"),
      h("label", { class: "f", for: "gamedirs" }, "Game folders: folders holding your games or their shortcuts (.lnk, .url, .exe on Windows; .desktop and AppImages on Linux). Shelf > Add from library lists what's in them."), gameDirs,
      h("label", { class: "f", for: "steamdir" }, "Steam folder (leave empty to use the one found automatically)"), steamDir,
      h("label", { class: "f" }, "Art folders: box art, screenshots and preview videos you already have"), mediaPanel(draft.media),
      h("label", { class: "f", for: "gridkey" }, "SteamGridDB API key, for more box art when you press Look it up (free at steamgriddb.com, under Preferences > API)"), gridKey),
    h("div", { class: "panel" }, h("b", {}, "Playing"), h("label", {}, lc, " Play buttons start games (only games on the shelf, the way their entry says)"), h("label", {}, askBox, " Ask how it went after a game (two taps each; it stops asking someone who skips three times in a row)"),
      h("label", {}, trailerBox, " The Trailer and Co-op buttons may load videos from YouTube (only when pressed, always muted)")),
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: save }, "Save settings")),
    h("div", { class: "panel" }, h("b", {}, "On this screen"), h("p", { class: "small" }, "These are kept by this browser or window only."),
      h("label", {}, couchBox, " Couch mode: larger text and buttons for reading from the sofa"),
      h("label", {}, seasonBox, " Seasonal look: Halloween all through October (pumpkin and plum, a witch hat, a few bats)"),
      h("label", {}, memBox, " Memories: when Tonight is left alone for 8 minutes, your nights show one at a time"),
      h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: openPadTest }, "Test the controller"), h("button", { class: "btn", type: "button", onclick: () => openMemories() }, "Show memories now"))),
    h("details", { class: "panel" }, h("summary", {}, "How picks work"), h("p", { class: "small" }, "Every pick multiplies a game's share by these. The odds under each result come from the same list."),
      h("table", {}, h("tbody", {}, wt.map(w => h("tr", {}, h("td", {}, h("b", {}, w.label)), h("td", {}, w.value), h("td", { class: "small" }, w.text)))))),
    h("h2", {}, "Your data"), h("p", { class: "small" }, "The shelf, your marks and history live in " + (st.dataDir || "the data folder") + ", with daily backups. Export makes one file you can keep or bring to another computer."),
    h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: async () => {
      const d = await api("/api/export"); const a = h("a", { href: URL.createObjectURL(new Blob([JSON.stringify(d, null, 2)], { type: "application/json" })), download: "game-night-export.json" }); document.body.append(a); a.click(); a.remove();
    } }, "Export"), h("button", { class: "btn", type: "button", onclick: () => file.click() }, "Import"), file,
      h("button", { class: "btn", type: "button", onclick: async () => { await api("/api/refresh", {}); await reload(); toast("Refreshed."); render(); } }, "Refresh")),
    h("p", { class: "small" }, "Game Night " + b.version + ". It runs on this computer only. It goes online only when you press Trailer or Look it up."));
}
