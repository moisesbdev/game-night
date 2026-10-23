/* The spinner wheel: canvas drawing only. */
import { S, cssVar } from "./core.js";
import { COLORS, WHEEL_INK } from "./logic.js";

let cv, cx, SZ = 420, DPR = 1;
const FONT = '"Nunito","Segoe UI Variable Text","Segoe UI",system-ui,sans-serif';

export function resizeWheel() {
  cv = document.getElementById("wheel"); if (!cv) return; cx = cv.getContext("2d");
  const w = Math.round(cv.getBoundingClientRect().width) || 420; DPR = window.devicePixelRatio || 1; SZ = w;
  cv.width = Math.round(w * DPR); cv.height = Math.round(w * DPR); cx.setTransform(DPR, 0, 0, DPR, 0, 0); drawWheel();
  if (document.fonts && document.fonts.load) document.fonts.load('800 16px "Nunito"').then(() => drawWheel(), () => {});     // redraw once the bundled font is ready
}

window.addEventListener("resize", () => { if (S.tab === "tonight") resizeWheel(); });

export function fit(text, maxW, size) { cx.font = "800 " + size + "px " + FONT; while (size > 13 && cx.measureText(text).width > maxW) { size--; cx.font = "800 " + size + "px " + FONT; } let t = text; while (t.length > 3 && cx.measureText(t).width > maxW) t = t.slice(0, -2); return t === text ? t : t.trimEnd() + "…"; }

export function drawWheel() {
  if (!cx) return;
  const R = SZ / 2 - 8, hub = SZ * 0.06; cx.clearRect(0, 0, SZ, SZ);
  const list = S.wheel;
  if (!list.length) { cx.fillStyle = getComputedStyle(document.body).color; cx.font = "700 18px " + FONT; cx.textAlign = "center"; cx.fillText("No games match the filters", SZ / 2, SZ / 2); cv.setAttribute("aria-label", "Spinner wheel: no games match the filters"); return; }
  const n = list.length, seg = 2 * Math.PI / n;
  for (let i = 0; i < n; i++) {
    const a0 = S.angle + i * seg, a1 = a0 + seg;
    cx.beginPath(); cx.moveTo(SZ / 2, SZ / 2); cx.arc(SZ / 2, SZ / 2, R, a0, a1); cx.closePath(); cx.fillStyle = COLORS[i % COLORS.length]; cx.fill(); cx.strokeStyle = "#ffffff"; cx.lineWidth = 4; cx.lineJoin = "round"; cx.stroke();
    cx.save(); cx.translate(SZ / 2, SZ / 2); cx.rotate(a0 + seg / 2); cx.textAlign = "right"; cx.textBaseline = "middle"; cx.fillStyle = WHEEL_INK;
    cx.fillText(fit(list[i], R - hub - SZ * 0.08 - 10, Math.round(SZ * (n > 8 ? 0.046 : 0.052))), R - 14, 0); cx.restore();
  }
  cx.beginPath(); cx.arc(SZ / 2, SZ / 2, R + 2, 0, 2 * Math.PI); cx.strokeStyle = cssVar("--accent", "#0a7c82"); cx.lineWidth = 7; cx.stroke();
  for (let i = 0; i < 24; i++) {                       // little "bulbs" around the rim
    const a = i * Math.PI / 12, bx = SZ / 2 + Math.cos(a) * (R + 2), by = SZ / 2 + Math.sin(a) * (R + 2);
    cx.beginPath(); cx.arc(bx, by, 2.6, 0, 2 * Math.PI); cx.fillStyle = i % 2 ? "#ffffff" : cssVar("--accent-bg", "#d3f1ed"); cx.fill();
  }
  cx.beginPath(); cx.arc(SZ / 2, SZ / 2, hub * 1.45, 0, 2 * Math.PI); cx.fillStyle = "#ffffff"; cx.fill(); cx.strokeStyle = cssVar("--accent", "#0a7c82"); cx.lineWidth = 4; cx.stroke();
  cx.beginPath();                                        // a four-point star in the hub
  for (let i = 0; i < 8; i++) { const a = i * Math.PI / 4 - Math.PI / 2, r = i % 2 ? hub * 0.42 : hub * 1.0; cx.lineTo(SZ / 2 + Math.cos(a) * r, SZ / 2 + Math.sin(a) * r); }
  cx.closePath(); cx.fillStyle = cssVar("--accent", "#0a7c82"); cx.fill();  cv.setAttribute("aria-label", "Spinner wheel showing " + n + " games: " + list.join(", "));
}
