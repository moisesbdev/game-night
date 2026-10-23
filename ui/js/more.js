/* More: Health and Settings, one at a time. */
import { S, h, seg } from "./core.js";
import { SUBS } from "./logic.js";
import { healthSection } from "./health.js";
import { goSub } from "./main.js";
import { settingsSection } from "./settings.js";

export async function viewMore() {
  const sub = S.sub.more;
  const body = sub === "health" ? await healthSection() : await settingsSection();
  return h("div", {}, h("div", { class: "pagehead" }, h("h1", {}, "More"), seg("More sections", SUBS.more, sub, goSub("more"))), body);
}
