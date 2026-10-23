/* The Journal's "Together" and "Stats" sections: this week, your taste, games you haven't played in a while, most played, by month. */
import { S, api, h, nm, players } from "./core.js";
import { MARK_WORDS, VIBE_NAME, ago, daysSince } from "./logic.js";
import { jumpToGame } from "./games.js";
import { go } from "./main.js";

export function tastePanel(tt) {
  if (!tt.enough) return h("div", { class: "panel" }, h("b", {}, "Your taste"), h("p", { class: "small" }, "Rate a few games on the shelf, or run the quick taste test on Tonight, and this fills in with what each of you likes and what matches next."));
  const top = Math.max(1, ...tt.people.flatMap(p => p.vibes.map(v => v.n)));
  const col = p => h("div", { class: "taste-col" }, h("b", {}, nm(p.id)), h("div", { class: "small" }, p.liked + " liked · " + p.passed + " passed"),
    p.vibes.length ? p.vibes.map(v => h("div", { class: "barrow" }, h("span", {}, VIBE_NAME[v.vibe] || v.vibe), h("div", { class: "bar", style: "width:" + Math.round(100 * v.n / top) + "%" }), h("span", {}, v.n)))
      : h("div", { class: "small" }, "Nothing marked yet."));
  const chipTo = t => h("button", { class: "btn", type: "button", onclick: () => jumpToGame(t) }, t);
  return h("div", { class: "panel" }, h("b", {}, "Your taste"), h("p", { class: "small" }, "From " + tt.marked + " marked game" + (tt.marked === 1 ? "" : "s") + ". Liked counts \"loved it\" and \"looks fun\"."),
    h("div", { class: "taste" }, tt.people.map(col)),
    tt.all.length ? h("div", {}, h("h3", { class: "sub" }, "Everyone liked"), h("div", { class: "row" }, tt.all.map(chipTo))) : null,
    tt.split.length ? h("div", {}, h("h3", { class: "sub" }, "Opinions differ"), h("div", { class: "small" }, tt.split.map(s => h("div", {}, h("b", {}, s.title), ": " + Object.entries(s.marks).filter(([id, m]) => players().some(p => p.id === id) && MARK_WORDS[m]).map(([id, m]) => nm(id) + " " + MARK_WORDS[m]).join(", "))))) : null,
    tt.tryNext.length ? h("div", {}, h("h3", { class: "sub" }, "Untried games that match"), h("div", { class: "row" }, tt.tryNext.map(x => h("button", { class: "btn", type: "button", title: "Moods: " + x.matches.map(m => VIBE_NAME[m] || m).join(", "), onclick: () => jumpToGame(x.title) }, x.title)))) : null);
}

function weekPanel(rc) {
  return h("div", { class: "panel" }, h("b", {}, "This week"),
    rc.total || rc.minutes ? h("div", {}, rc.minutes ? h("p", {}, "Time in games this week: " + (rc.minutes >= 60 ? Math.floor(rc.minutes / 60) + " h " : "") + (rc.minutes % 60) + " min.") : null, rc.total ? h("p", {}, "Confirmed in Game Night: " + rc.confirmed.map(x => x.title + (x.times > 1 ? " ×" + x.times : "")).join(", ") + ".") : null,
      null)
      : h("p", { class: "small" }, "Nothing recorded since " + rc.since + "."),
    null);
}

export function dustOff() {
  const list = S.boot.games.filter(g => g.launchable).map(g => ({ g, d: g.plays ? daysSince(g.plays.last) : null, played: !!(g.plays || g.marks.you === "played" || g.marks.them === "played") }))
    .sort((a, b) => (b.d ?? (b.played ? 500 : 9999)) - (a.d ?? (a.played ? 500 : 9999)) || a.g.t.localeCompare(b.g.t)).slice(0, 4);
  const never = S.boot.games.filter(g => !g.plays && g.marks.you !== "played" && g.marks.them !== "played").length;
  return h("div", { class: "panel" }, h("b", {}, "Haven't played in a while"), h("p", { class: "small" }, never + " of " + S.boot.games.length + " games haven't been played yet (as far as Game Night and your marks know)."),
    h("div", { class: "row" }, list.map(x => h("button", { class: "btn", type: "button", onclick: () => jumpToGame(x.g.t) }, x.g.t + (x.d == null ? (x.played ? "" : " · never") : " · " + ago(x.d))))));
}

export async function togetherSection() {
  const [rc, tt] = await Promise.all([api("/api/recap"), api("/api/taste")]);
  return h("div", {}, tastePanel(tt), weekPanel(rc), dustOff());
}

export async function statsSection() {
  const d = await api("/api/history"), max = Math.max(1, ...d.top.map(t => t.count));
  const months = Object.entries(d.byMonth), mmax = Math.max(1, ...months.map(m => m[1]));
  return h("div", {}, h("p", { class: "lede" }, d.total ? d.total + " game" + (d.total === 1 ? "" : "s") + " confirmed so far." : "Nothing confirmed yet. Press \"We'll play this\" or \"Play it now\" after a pick and it shows up here."),
    d.top.length ? h("div", { class: "panel" }, h("b", {}, "Most played"), d.top.map(t => h("div", { class: "barrow" }, h("span", {}, t.title), h("div", { class: "bar", style: "width:" + Math.round(100 * t.count / max) + "%" }), h("span", {}, t.count)))) : null,
    months.length ? h("div", { class: "panel" }, h("b", {}, "By month"), months.map(([m, n]) => h("div", { class: "barrow" }, h("span", {}, m), h("div", { class: "bar", style: "width:" + Math.round(100 * n / mmax) + "%" }), h("span", {}, n)))) : null,
    Object.keys(d.chose).length ? h("div", { class: "panel" }, h("b", {}, "Who chose"), h("p", {}, Object.entries(d.chose).filter(([id, n]) => id && n).map(([id, n]) => nm(id) + ": " + n).join(" · "))) : null,
    d.entries.length ? h("details", { class: "panel" }, h("summary", {}, "Recent picks"), h("table", {}, h("thead", {}, h("tr", {}, h("th", {}, "When"), h("th", {}, "Game"), h("th", {}, "Chose"))),
      h("tbody", {}, d.entries.slice(0, 40).map(e => h("tr", {}, h("td", {}, e.at.replace("T", " ").slice(0, 16)), h("td", {}, e.title), h("td", {}, e.chose ? nm(e.chose) : "")))))) : null);
}
