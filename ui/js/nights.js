/* The Journal's "Nights" section: a journal of your evenings, built from what Game Night recorded (sessions and confirmed picks). */
import { api, h, mascot, nm, players, poster } from "./core.js";
import { fmtMinutes, fmtSeconds, nightLabel, ratingWords } from "./logic.js";
import { countUp } from "./core.js";
import { go } from "./main.js";
import { openMemories } from "./memories.js";

const OUTCOME = { finished: "Finished it", again: "Again soon", done: "Done for now" };

function tile(g) {
  const time = fmtMinutes(g.minutes) || fmtSeconds(g.esdeSeconds);
  const bits = [time, OUTCOME[g.outcome]].filter(Boolean).join(" · ");
  const rated = ratingWords(g.ratings, players());
  return h("article", { class: "tile" }, poster({ t: g.title, cover: g.cover }, null, g.cover ? "Box art for " + g.title : ""),
    h("b", {}, g.title), bits ? h("div", { class: "small" }, bits) : (g.source === "confirmed" ? h("div", { class: "small" }, "Picked, no play time recorded") : null),
    rated ? h("div", { class: "small" }, rated) : null, g.chose ? h("div", { class: "small" }, "Chosen by " + nm(g.chose)) : null);
}

function nightSummary(n) {
  const games = n.games.length, bits = [games + (games === 1 ? " game" : " games")];
  if (n.minutes) bits.push(fmtMinutes(n.minutes));
  const chose = Object.entries(n.chose || {}).filter(([, c]) => c);
  if (chose.length && players().length > 1) bits.push(chose.map(([id, c]) => nm(id) + " chose " + c).join(", "));
  return bits.join(" · ");
}

export async function nightsSection() {
  const d = await api("/api/nights"), s = d.stats;
  const lede = s.nights
    ? `${s.nights} night${s.nights === 1 ? "" : "s"} so far${s.minutes ? ", " + fmtMinutes(s.minutes) + " of play" : ""} across ${s.games} game${s.games === 1 ? "" : "s"}${s.finished ? ", " + s.finished + " finished" : ""}.`
    : "A journal of your evenings: what you played, for how long, who chose and how it went.";
  const body = d.nights.length
    ? d.nights.map(n => h("section", { class: "night", "aria-label": nightLabel(n.date) },
      h("div", { class: "nighthead" }, h("h2", {}, nightLabel(n.date)), n.n ? h("span", { class: "tag" }, "Night " + n.n) : null, h("span", { class: "small" }, nightSummary(n))),
      h("div", { class: "tiles" }, n.games.map(tile))))
    : [h("div", { class: "panel empty" }, mascot(), h("div", {}, h("b", {}, "No nights yet"), h("p", { class: "small" }, "Play something from Game Night, or press \"We'll play this\" after a spin, and your first night shows up here with the games, the time and how each one went."),
      h("button", { class: "btn primary", type: "button", onclick: () => go("tonight") }, "Pick tonight's game")))];
  const stat = (n, label, fmt, i) => h("div", { class: "stat", style: "--i:" + i, role: "group", "aria-label": (fmt ? fmt(n) : n) + " " + label },
    countUp(h("b", { "aria-hidden": "true" }), n, fmt || String), h("span", { "aria-hidden": "true" }, label));
  const strip = s.nights ? h("div", { class: "statstrip" }, [stat(s.nights, s.nights === 1 ? "night" : "nights", null, 0), s.minutes ? stat(s.minutes, "of play", fmtMinutes, 1) : null,
    stat(s.games, s.games === 1 ? "game" : "games", null, 2), s.finished ? stat(s.finished, "finished", null, 3) : null].filter(Boolean)) : null;
  const look = h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: () => openMemories() }, "Memories"), h("span", { class: "small" }, "Your nights, one at a time, full screen. It also comes up by itself when Tonight is left alone for a while."));
  return h("div", {}, strip, h("p", { class: "lede" }, lede), look, body);
}
