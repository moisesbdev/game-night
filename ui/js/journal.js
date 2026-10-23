/* The Journal: your nights, what you like together, and the numbers, one section at a time. */
import { S, h, seg } from "./core.js";
import { SUBS } from "./logic.js";
import { statsSection, togetherSection } from "./history.js";
import { goSub } from "./main.js";
import { yearSection } from "./memories.js";
import { nightsSection } from "./nights.js";

export async function viewJournal() {
  const sub = S.sub.journal;
  const body = sub === "nights" ? await nightsSection() : sub === "year" ? await yearSection() : sub === "together" ? await togetherSection() : await statsSection();
  return h("div", {}, h("div", { class: "pagehead" }, h("h1", {}, "The journal"), seg("Journal sections", SUBS.journal, sub, goSub("journal"))), body);
}
