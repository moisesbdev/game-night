/* Saved filter sets (built-in and your own, kept in this browser only). */
import { S, h, toast } from "./core.js";
import { BUILTIN_PRESETS, cleanFilters } from "./logic.js";
import { go } from "./main.js";
import { refilterUI, schedulePreview } from "./tonight.js";

export function loadPresets() {
  try { const a = JSON.parse(localStorage.getItem("gn-presets") || "[]"); return Array.isArray(a) ? a.filter(p => p && typeof p.name === "string" && p.name.trim()).slice(0, 12).map(p => ({ name: p.name.slice(0, 30), f: cleanFilters(p.f) })) : []; } catch (e) { return []; }
}

export function savePresets(a) { try { localStorage.setItem("gn-presets", JSON.stringify(a)); return true; } catch (e) { toast("This browser blocks storage, so the filter couldn't be saved."); return false; } }

export function refreshPresets() {
  const el = document.getElementById("presets"); if (!el) return;
  const idx = [...el.querySelectorAll("button")].indexOf(document.activeElement), next = presetRow();       // re-rendering would drop keyboard focus to the page
  el.replaceWith(next);
  if (idx >= 0) { const bs = next.querySelectorAll("button:not(:disabled)"); const b = bs[Math.min(idx, bs.length - 1)]; if (b) b.focus(); }
}

export function applyPreset(p) {
  if (S.spinning) return;                                  // swapping the wheel mid-spin would leave the winner off it
  S.filters = cleanFilters(p.f); S.result = null;
  const box = document.getElementById("pickbox"); if (box) box.replaceChildren();
  refilterUI(); schedulePreview(); toast("Filters: " + p.name);
}

export function presetRow() {
  const custom = loadPresets();
  return h("div", { id: "presets", class: "row", role: "group", "aria-label": "Saved filters" }, h("span", { class: "t" }, "Saved"),
    BUILTIN_PRESETS.map(p => h("button", { class: "btn", type: "button", disabled: S.spinning, onclick: () => applyPreset(p) }, p.name)),
    custom.map((p, i) => h("span", { class: "presetchip" }, h("button", { class: "btn", type: "button", disabled: S.spinning, onclick: () => applyPreset(p) }, p.name),
      h("button", { class: "x", type: "button", disabled: S.spinning, "aria-label": "Delete saved filter " + p.name, onclick: () => { if (savePresets(custom.filter((_, j) => j !== i))) refreshPresets(); } }, "×"))),
    h("button", { class: "btn", type: "button", onclick: savePresetDialog }, "Save current…"));
}

export function savePresetDialog() {
  const dlg = document.getElementById("dlg"), name = h("input", { type: "text", id: "preset-name", maxlength: "30", placeholder: "Quick and silly" });
  const go = () => {
    const n = name.value.trim(); if (!n) { name.focus(); return; }
    const all = loadPresets().filter(p => p.name.toLowerCase() !== n.toLowerCase()); all.push({ name: n, f: cleanFilters(S.filters) });
    if (savePresets(all.slice(-12))) { dlg.close(); refreshPresets(); toast("Saved the filters as " + n + "."); }
  };
  name.addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); go(); } });
  dlg.replaceChildren(h("h2", {}, "Save these filters"), h("p", { class: "small" }, "Where, mood, time, players and skill gap as they are now. Kept in this browser; saving the same name again replaces it."),
    h("label", { class: "f", for: "preset-name" }, "Name"), name,
    h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: go }, "Save"), h("button", { class: "btn", type: "button", onclick: () => dlg.close() }, "Cancel")));
  dlg.showModal(); name.focus();
}
