/* Now playing, session polling and the 'how was it?' prompt. */
import { S, api, gameByTitle, h, nm, reload, toast } from "./core.js";
import { render } from "./main.js";

let nowTimer, promptOpen = false;

export function renderNow(n) {
  const el = document.getElementById("nowplaying"); if (!el) return;
  if (!n.active) { el.replaceChildren(); el.classList.add("hide"); return; }
  el.classList.remove("hide");
  el.replaceChildren(h("span", {}, "Now playing: "), h("b", {}, n.active.title), h("span", {}, " · " + n.active.minutesSoFar + " min "),
    h("button", { class: "btn", type: "button", onclick: async () => { try { await api("/api/session/end", { id: n.active.id }); } catch (e) { toast(e.message); } pollNow(); } }, "We're done"));
}

export async function pollNow() {
  if (document.visibilityState !== "visible") return;
  let n; try { n = await api("/api/now"); } catch (e) { return; }
  renderNow(n);
  if (n.askEnabled && n.pending.length && !promptOpen && !document.getElementById("dlg").open && document.hasFocus()) postGame(n.pending[0], n.ask);
}

export function startPolling() { clearInterval(nowTimer); nowTimer = setInterval(pollNow, 5000); pollNow(); document.addEventListener("visibilitychange", pollNow); window.addEventListener("focus", pollNow); }

export function postGame(sess, ask) {
  promptOpen = true;
  const dlg = document.getElementById("dlg"); let finished = false;
  const close = async () => { finished = true; if (dlg.open) dlg.close(); promptOpen = false; await reload(); if (["tonight", "shelf", "journal"].includes(S.tab)) render(); };
  dlg.onclose = async () => { dlg.onclose = null; promptOpen = false; if (!finished) { try { await api("/api/session/dismiss", { id: sess.id }); } catch (e) { /* ignore */ } } };
  const btns = list => h("div", { class: "row" }, list.map(([label, pad, fn, primary]) => h("button", { class: "btn" + (primary ? " primary" : ""), type: "button", "data-pad": pad, onclick: fn }, label)));
  const show = (title, ...kids) => { dlg.replaceChildren(h("div", {}, h("h2", { style: "margin-top:0" }, title), kids)); if (!dlg.open) dlg.showModal(); };
  const hint = S.pad ? h("p", { class: "small" }, "A, B or X on the controller.") : null;
  if (sess.check) {
    show("Did " + sess.title + " start properly?", h("p", { class: "small" }, "It ended after under two minutes, so it may not have started."),
      btns([["Yes, it ran", "a", async () => { await api("/api/session/started", { id: sess.id, ok: true }); await api("/api/session/dismiss", { id: sess.id }); close(); }, true],
            ["No, it didn't", "b", async () => { await api("/api/session/started", { id: sess.id, ok: false }); toast("Taken off tonight's history."); close(); }]]), hint);
    return;
  }
  const people = ask.length ? ask : [];
  let i = 0;
  const ratePerson = () => {
    if (i >= people.length) return outcome();
    const who = people[i];
    show(nm(who) + ", how was " + sess.title + "?", btns([["👍 Liked it", "a", async () => { await api("/api/session/rate", { id: sess.id, who, rating: "up" }); i++; ratePerson(); }, true],
      ["👎 Not really", "b", async () => { await api("/api/session/rate", { id: sess.id, who, rating: "down" }); i++; ratePerson(); }],
      ["Skip", "x", async () => { await api("/api/session/rate", { id: sess.id, who, rating: "skip" }); i++; ratePerson(); }]]), hint);
  };
  const outcome = () => show("And " + sess.title + "?", h("p", { class: "small" }, sess.minutes + " minutes tonight."),
    btns([["Again soon", "a", async () => { await api("/api/session/outcome", { id: sess.id, outcome: "again" }); note(true); }, true],
          ["Done for now", "b", async () => { await api("/api/session/outcome", { id: sess.id, outcome: "done" }); close(); }],
          ["Finished it!", "x", async () => { await api("/api/session/outcome", { id: sess.id, outcome: "finished" }); close(); }]]), hint);
  const note = ongoing => {
    const g = gameByTitle(sess.title), box = h("textarea", { maxlength: "280", "aria-label": "Note for next time" }, g && g.progress ? g.progress.note : "");
    show("Leave a note for next time?", h("p", { class: "small" }, "Where you got to. It shows up before you start this game again."), box,
      btns([["Save note", "a", async () => { await api("/api/progress", { title: sess.title, note: box.value, ongoing }); close(); }, true], ["No note", "b", () => close()]]), hint);
  };
  ratePerson();
}

