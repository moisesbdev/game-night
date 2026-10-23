/* Watching a game: a preview video from your own media folders (local, instant, private), and the full trailer from YouTube (only when pressed, and only if allowed in Settings). */
import { S, h, icon, settings } from "./core.js";
import { coopSearchUrl, isVideoId, mediaUrl, trailerEmbedUrl, trailerSearchUrl, watchUrl } from "./logic.js";

const YT_ORIGIN = "https://www.youtube-nocookie.com";
const SANDBOX = "allow-scripts allow-same-origin allow-presentation allow-popups allow-popups-to-escape-sandbox";

/** Stops anything playing inside a container (a closed dialog keeps playing sound otherwise). */
export function stopWatching(root) {
  if (!root) return;
  root.querySelectorAll("video").forEach(v => { v.pause(); v.removeAttribute("src"); v.load(); v.remove(); });
  root.querySelectorAll("iframe").forEach(f => f.remove());
}

/** Whatever closes the sheet (its button, Escape, or code) stops the video first. A closed dialog is only hidden, so without this a playing video would carry on silently behind it. */
(function stopWhenTheSheetCloses() {
  const dlg = document.getElementById("dlg"); if (!dlg) return;
  for (const ev of ["cancel", "close"]) dlg.addEventListener(ev, () => stopWatching(dlg));
  const realClose = dlg.close.bind(dlg);
  dlg.close = (...args) => { stopWatching(dlg); return realClose(...args); };      // an own method on this one element; nothing global is touched
})();

function player(g, which) {
  if (which === "video") {
    const v = h("video", { controls: true, autoplay: true, muted: true, playsinline: true, preload: "metadata", src: mediaUrl(g.t, "video"), poster: g.shot ? mediaUrl(g.t, "shot") : null, "aria-label": "Preview of " + g.t });
    v.muted = true;                                  // always starts silent; the speaker button on the player turns the sound on
    return v;
  }
  const url = trailerEmbedUrl(which === "coop" ? g.coop : g.trailer);
  return h("iframe", { src: url, title: (which === "coop" ? "Co-op gameplay of " : "Trailer for ") + g.t, allow: "autoplay; encrypted-media; picture-in-picture; fullscreen", referrerpolicy: "strict-origin-when-cross-origin", sandbox: SANDBOX, loading: "lazy" });
}

/** Buttons under the player, so nobody has to click into the embed: sound, pause and full screen. The first unmute is gentle (half volume).
 *  A local preview is driven directly; the YouTube player is driven by the messages its embed API listens for (and tells us back when sound or play state changes). */
function controlRow(which, p, stage) {
  const state = { muted: true, playing: true };
  const mute = h("button", { class: "btn", type: "button", "data-act": "mute", title: "Sound on or off (M)" });
  const pause = h("button", { class: "btn", type: "button", "data-act": "pause", title: "Pause or play (K)" });
  const paint = () => {
    mute.replaceChildren(icon(state.muted ? "volumeOff" : "volume"), state.muted ? "Unmute" : "Mute");
    pause.replaceChildren(icon(state.playing ? "pause" : "play"), state.playing ? "Pause" : "Play");
  };
  if (which === "video") {
    mute.onclick = () => { if (p.muted) p.volume = 0.5; p.muted = !p.muted; };
    pause.onclick = () => { if (p.paused) p.play(); else p.pause(); };
    p.addEventListener("volumechange", () => { state.muted = p.muted; paint(); });
    p.addEventListener("play", () => { state.playing = true; paint(); });
    p.addEventListener("pause", () => { state.playing = false; paint(); });
  } else {
    const send = (func, args) => { try { p.contentWindow.postMessage(JSON.stringify({ event: "command", func, args: args || [] }), YT_ORIGIN); } catch (e) { /* not loaded yet */ } };
    mute.onclick = () => { if (state.muted) { send("setVolume", [50]); send("unMute"); } else send("mute"); state.muted = !state.muted; paint(); };
    pause.onclick = () => { send(state.playing ? "pauseVideo" : "playVideo"); state.playing = !state.playing; paint(); };
    const heard = ev => {
      if (!p.isConnected) { window.removeEventListener("message", heard); return; }
      if (ev.source !== p.contentWindow || ev.origin !== YT_ORIGIN) return;
      let d; try { d = JSON.parse(ev.data); } catch (e) { return; }
      const i = d && d.info; if (!i || typeof i !== "object") return;
      if (typeof i.muted === "boolean") state.muted = i.muted;
      if (typeof i.playerState === "number") state.playing = i.playerState === 1 || i.playerState === 3;
      paint();
    };
    window.addEventListener("message", heard);
    const listen = () => { try { p.contentWindow.postMessage(JSON.stringify({ event: "listening", id: 1, channel: "widget" }), YT_ORIGIN); } catch (e) { /* ignore */ } };
    p.addEventListener("load", () => { listen(); let n = 0; const t = setInterval(() => { listen(); if (++n > 6 || !p.isConnected) clearInterval(t); }, 500); });
  }
  paint();
  const row = h("div", { class: "row watchctl", role: "group", "aria-label": "Player controls" }, mute, pause);
  if (stage.requestFullscreen) row.append(h("button", { class: "btn", type: "button", "data-act": "full", title: "Full screen (F)", onclick: () => { if (document.fullscreenElement) document.exitFullscreen(); else stage.requestFullscreen(); } }, icon("expand"), "Full screen"));
  return row;
}

/** Keys while a player is open (and you are not typing): M sound, K pause, F full screen. */
document.addEventListener("keydown", e => {
  if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
  const dlg = document.getElementById("dlg"), box = dlg && dlg.open && dlg.querySelector(".watch.playing");
  if (!box || /^(INPUT|TEXTAREA|SELECT)$/.test((e.target && e.target.tagName) || "")) return;
  const act = { m: "mute", k: "pause", f: "full" }[e.key.toLowerCase()], b = act && box.querySelector('[data-act="' + act + '"]');
  if (b) { e.preventDefault(); b.click(); }
});

/** The "Watch" part of a game's sheet: buttons first; pressing one opens the player in place. `start` ("video" or "trailer") opens it straight away. */
export function watchPanel(g, start) {
  const canTrailer = isVideoId(g.trailer) && settings().trailersEnabled !== false;
  const canCoop = isVideoId(g.coop) && settings().trailersEnabled !== false;
  const trailersOff = (isVideoId(g.trailer) || isVideoId(g.coop)) && settings().trailersEnabled === false;
  const coopLabel = g.coopKind === "online" ? "Online co-op" : "Couch co-op";
  const box = h("div", { class: "watch" });
  const open = which => {
    stopWatching(box); box.classList.add("playing");          // the sheet turns into a theater while something plays (see .gamesheet:has(.watch.playing))
    const p = player(g, which), stage = h("div", { class: "stage169" }, p);
    if (which === "video") p.addEventListener("error", () => stage.replaceChildren(h("p", { class: "small cantplay", role: "alert" }, "This preview can't be played right now. If the drive holding your media is unplugged, plug it back in and reload.")));
    const offline = navigator.onLine === false && which !== "video" ? h("p", { class: "small", role: "status" }, "You're offline, so YouTube can't load. Preview still works.") : null;
    box.replaceChildren(...[stage, controlRow(which, p, stage), offline].filter(Boolean),
      h("div", { class: "row watchbar" }, which !== "video" ? h("span", { class: "small" }, "Muted." + (which === "coop" && g.coopBy ? " Gameplay by " + g.coopBy + "." : "") + " Loaded from YouTube just now; press Unmute for sound. You can switch YouTube videos off in More > Settings.") : h("span", { class: "small" }, "Muted. Press Unmute for sound. Played from your own media folder."),
        which !== "video" ? h("a", { class: "link", href: watchUrl(which === "coop" ? g.coop : g.trailer), target: "_blank", rel: "noopener noreferrer", title: "Opens YouTube in a new tab" }, "Not playing? Open on YouTube") : null,
        h("button", { class: "link", type: "button", onclick: () => { stopWatching(box); show(); } }, icon("close"), "Close")));
    queueMicrotask(() => {
      const d = box.closest("dialog"); if (d) d.scrollTop = 0;
      if (document.body.classList.contains("padnav")) { const c = box.querySelector(".watchbar button"); if (c) c.focus({ preventScroll: true }); }      // keep the controller on this page, not inside the video frame
    });       // the player is now at the top; show it
  };
  const show = () => {
    box.classList.remove("playing");
    const buttons = [];
    if (canTrailer) buttons.push(h("button", { class: "btn", type: "button", onclick: () => open("trailer") }, icon("play"), "Trailer"));
    if (canCoop) buttons.push(h("button", { class: "btn", type: "button", title: g.coopKind === "online" ? "Gameplay of two or more people playing together online" : "Gameplay of people playing together on one screen", onclick: () => open("coop") }, icon("play"), coopLabel));
    if (g.video) buttons.push(h("button", { class: "btn", type: "button", onclick: () => open("video") }, icon("play"), "Preview"));
    if (!canCoop && !g.coop && g.p !== "1" && settings().trailersEnabled !== false) buttons.push(h("a", { class: "link", href: coopSearchUrl(g.t), target: "_blank", rel: "noopener noreferrer", title: "Opens a YouTube search in a new tab" }, icon("search"), "Find co-op gameplay"));
    if (!buttons.length && settings().trailersEnabled !== false) buttons.push(h("a", { class: "link", href: trailerSearchUrl(g.t), target: "_blank", rel: "noopener noreferrer" }, icon("search"), "Find a trailer on YouTube"));
    box.replaceChildren(h("div", { class: "row", role: "group", "aria-label": "Watch " + g.t }, buttons,
      trailersOff ? h("span", { class: "small" }, "YouTube videos are switched off in Settings.") : null));
  };
  show();
  if (start === "video" && g.video) open("video"); else if (start === "coop" && canCoop) open("coop"); else if (start === "trailer" && canTrailer) open("trailer");
  return box;
}

/** Does this game have anything to watch (so the result card can offer it)? */
export function canWatch(g) { return !!g.video || ((isVideoId(g.trailer) || isVideoId(g.coop)) && settings().trailersEnabled !== false); }
