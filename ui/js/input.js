/* Couch mode, the seasonal look and the keyboard shortcut. The controller lives in pad.js. */
import { S, h, svg } from "./core.js";
import { season } from "./logic.js";
import { render } from "./main.js";
import { seasonExtras } from "./spooky.js";
import "./pad.js";

/** The seasonal look (Halloween all through October) unless it was switched off on this browser. Checked again every hour, so it ends on 1 November by itself. */
export function seasonOn() { try { return localStorage.getItem("gn-season") !== "off"; } catch (e) { return true; } }
export function setSeasonOn(on) { try { localStorage.setItem("gn-season", on ? "on" : "off"); } catch (e) { /* storage blocked */ } applySeason(); }

export function applySeason() {
  const look = seasonOn() ? season() : null, was = document.body.dataset.season || null;
  document.body.classList.toggle("halloween", look === "halloween");
  if (look) document.body.dataset.season = look; else delete document.body.dataset.season;
  let bats = document.getElementById("bats");
  if (look === "halloween" && !bats) {                 // three small bats crossing the sky now and then: decoration only, off with reduced motion
    const bat = () => svg("svg", { viewBox: "0 0 40 18", "aria-hidden": "true", focusable: "false" },
      svg("path", { d: "M20 6c-1.2-2-2.8-2-4 0-2.4-3.2-6.5-4.2-9.5-2.5 2.2.6 3.2 2.5 2.6 4.6-2.4-1.4-5.4-.9-7.1 1.2 3.4-.1 5.6 1.6 6.4 4.4 1.7-1.6 4.2-2 6.3-.9.4 1.6 2 3.3 5.3 3.3s4.9-1.7 5.3-3.3c2.1-1.1 4.6-.7 6.3.9.8-2.8 3-4.5 6.4-4.4-1.7-2.1-4.7-2.6-7.1-1.2-.6-2.1.4-4 2.6-4.6-3-1.7-7.1-.7-9.5 2.5-1.2-2-2.8-2-4 0Z" }));
    bats = h("div", { id: "bats", class: "bats", "aria-hidden": "true" }, [0, 1, 2].map(i => h("span", { style: "--b:" + i }, bat())));
    document.body.append(bats);
  } else if (look !== "halloween" && bats) bats.remove();
  seasonExtras(look === "halloween");
  return look !== was;
}
setInterval(() => { if (applySeason() && S.boot) render(); }, 3600000);


export function setCouch(on) { document.body.classList.toggle("couch", on); try { localStorage.setItem("gn-couch", on ? "1" : "0"); } catch (e) { /* ignore */ } }

try { if (localStorage.getItem("gn-couch") === "1") document.body.classList.add("couch"); } catch (e) { /* ignore */ }

document.addEventListener("keydown", e => {
  if (e.defaultPrevented || e.repeat || e.ctrlKey || e.metaKey || e.altKey || S.tab !== "tonight" || document.getElementById("dlg").open) return;
  const t = e.target;
  if (t && t !== document.body && /^(INPUT|TEXTAREA|SELECT|BUTTON|A|SUMMARY)$/.test(t.tagName)) return;      // never steal a key from a control
  if (e.key === " ") { const b = document.getElementById("spin"); if (b && !b.disabled) { e.preventDefault(); b.click(); } }
});
