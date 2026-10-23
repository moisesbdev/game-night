/* Halloween motion (only in the Halloween look, all of it off with reduced motion): ghosts in the pick's celebration, a cobweb that draws itself in once,
   and a small ghost that now and then peeks up from behind the dock. Decoration only: hidden from screen readers, never in the way of a click. */
import { S, h, svg } from "./core.js";

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
export const spookyOn = () => document.body.classList.contains("halloween") && !reduced();

/** A little ghost (its colour is the text colour, so it suits both modes). */
export function ghost(cls) {
  return svg("svg", { class: "ghost" + (cls ? " " + cls : ""), viewBox: "0 0 32 36", "aria-hidden": "true", focusable: "false" },
    svg("path", { class: "body", d: "M16 2C8.5 2 4 7.6 4 14.5V33l4-3 4 3 4-3 4 3 4-3 4 3V14.5C28 7.6 23.5 2 16 2Z" }),
    svg("ellipse", { class: "eye", cx: "12", cy: "15", rx: "2", ry: "2.8" }), svg("ellipse", { class: "eye", cx: "20", cy: "15", rx: "2", ry: "2.8" }));
}

/** The pick's celebration in October: a few ghosts float up and sway among the bubbles. Returns false when it isn't the season (the page uses its usual bubbles). */
export function celebrateSpooky(box) {
  if (!spookyOn()) return false;
  const tints = ["var(--accent)", "#8f6bd1", "var(--accent-hi)", "#c58be8"];
  for (let i = 0; i < 12; i++) {
    const g = i % 3 === 0, size = g ? 22 + Math.round(Math.random() * 12) : 8 + Math.round(Math.random() * 12), spread = (Math.random() - 0.5) * 280;
    const s = h("span", { class: g ? "gh" : "" }, g ? ghost() : null);
    s.style.setProperty("--s", size + "px"); s.style.setProperty("--bc", tints[i % tints.length]);
    s.style.setProperty("--dx", Math.round(spread) + "px"); s.style.setProperty("--dy", -(100 + Math.round(Math.random() * 120)) + "px"); s.style.animationDelay = (i * 55) + "ms";
    box.append(s);
  }
  return true;
}

/** A cobweb in the top corner that draws itself in once, when the page opens in the season. It stays still afterwards. */
function cobweb() {
  if (document.getElementById("cobweb")) return;
  const line = { fill: "none", stroke: "currentColor", "stroke-width": "1.2", "stroke-linecap": "round", pathLength: "1" };
  const angles = [0, 15, 30, 45, 60, 75, 90].map(a => a * Math.PI / 180);                 // spokes fan out from the corner
  const at = (a, r) => [r * Math.cos(a), r * Math.sin(a)];
  const ring = r => angles.map((a, i) => {                                                 // each thread sags a little toward the corner between two spokes
    const [x, y] = at(a, r);
    if (!i) return `M${x.toFixed(1)} ${y.toFixed(1)}`;
    const [cx, cy] = at((a + angles[i - 1]) / 2, r * 0.86);
    return `Q${cx.toFixed(1)} ${cy.toFixed(1)} ${x.toFixed(1)} ${y.toFixed(1)}`;
  }).join(" ");
  const web = svg("svg", { id: "cobweb", class: "cobweb", viewBox: "0 0 120 120", "aria-hidden": "true", focusable: "false" },
    ...angles.map(a => { const [x, y] = at(a, 120); return svg("path", { ...line, d: `M0 0 L${x.toFixed(1)} ${y.toFixed(1)}` }); }),
    ...[30, 58, 88].map((r, i) => svg("path", { ...line, class: "ring", style: "--r:" + i, d: ring(r) })));
  document.body.append(web);
}

/** Now and then (about every 70 to 110 seconds, only on Tonight's start screen with nothing open and the page in view) a small ghost rises from behind the dock, looks around and sinks back. */
let peekTimer = null;
function schedulePeek() {
  clearTimeout(peekTimer);
  peekTimer = setTimeout(() => {
    const dock = document.getElementById("tabs");
    const quiet = spookyOn() && !document.hidden && S.tab === "tonight" && !S.spinning && !S.memories && !document.getElementById("dlg").open && dock && !document.querySelector(".peek");
    if (quiet) {
      const r = dock.getBoundingClientRect(), p = h("div", { class: "peek", "aria-hidden": "true" }, ghost());
      p.style.left = Math.round(r.left + r.width * (0.15 + Math.random() * 0.7) - 22) + "px"; p.style.top = Math.round(r.top - 34) + "px";
      document.body.append(p); setTimeout(() => p.remove(), 4200);
    }
    schedulePeek();
  }, 70000 + Math.random() * 40000);
}

/** Called whenever the look is applied: starts or stops the extras. */
export function seasonExtras(on) {
  if (on && !reduced()) { cobweb(); if (!peekTimer) schedulePeek(); }
  else { clearTimeout(peekTimer); peekTimer = null; document.querySelectorAll("#cobweb, .peek").forEach(e => e.remove()); }
}
