/* The Health section of More. */
import { S, api, h } from "./core.js";
import { buildTabs, render } from "./main.js";

export async function healthSection() {
  const d = await api("/api/health"); S.health = d; buildTabs();
  const badge = l => h("span", { class: "badge " + l }, { ok: "OK", info: "Note", warn: "Check", fail: "Problem" }[l]);
  return h("div", {}, h("p", { class: "lede" }, "Checked " + d.at.replace("T", " ") + ". This page only reads; nothing here changes your files."),
    h("div", { class: "panel" }, h("table", {}, h("tbody", {}, d.checks.map(c => h("tr", {}, h("td", {}, badge(c.level)), h("td", {}, h("b", {}, c.title)), h("td", {}, c.detail, c.advice ? h("div", { class: "small" }, c.advice) : null)))))),
    h("div", { class: "row" }, h("button", { class: "btn", type: "button", onclick: () => render() }, "Check again")));
}

