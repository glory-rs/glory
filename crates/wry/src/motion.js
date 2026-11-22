function createMotion({ origin, event }) {
  const completion = new Map(),
    translations = new Map();
  const reduced = matchMedia("(prefers-reduced-motion: reduce)");
  const blocked = (el) =>
    !el.isConnected || !!el.closest("[hidden],[inert],[data-gly-hidden],[aria-disabled=true]");
  let swipe,
    suppressClick = false,
    mount,
    tracked;

  function complete(el, kind, token) {
    if (completion.get(el) !== token || token.done) return;
    token.done = true;
    if (kind === "presence") el.setAttribute("data-gly-presence-settled", "");
    event(token.entry, { kind: kind + "_complete", revision: token.revision });
  }
  function watch(el) {
    const presence = el.hasAttribute("data-gly-presence");
    const kind = presence ? "presence" : "collapse";
    const revision = el.getAttribute("data-gly-" + kind + "-revision");
    const entry = origin(el);
    if (!entry?.interests.has(presence ? "PresenceComplete" : "CollapseComplete")) return;
    let token = completion.get(el);
    if (token?.revision === revision && token.kind === kind) return;
    token = { entry, revision, kind, done: false };
    completion.set(el, token);
    if (presence)
      el.toggleAttribute(
        "data-gly-presence-settled",
        revision === "0" &&
          (el.dataset.glyPresence === "closed" || !el.hasAttribute("data-gly-presence-appear")),
      );
    requestAnimationFrame(() => {
      if (completion.get(el) !== token || token.done) return;
      const properties = presence ? ["opacity", "transform", "display"] : ["height"];
      const animations = (el.getAnimations?.() || []).filter((a) =>
        properties.includes(a.transitionProperty),
      );
      if (!animations.length) complete(el, kind, token);
      else
        Promise.allSettled(animations.map((a) => a.finished)).then(() => complete(el, kind, token));
    });
  }
  function translate(el) {
    const d = el.dataset,
      prior = translations.get(el);
    const enabled =
      !reduced.matches &&
      el.isConnected &&
      !el.closest("[hidden],[data-gly-hidden]") &&
      el.getClientRects().length > 0;
    const kind = d.glyTranslation,
      duration = Number(d.glyTranslationDuration);
    const to = "translate(" + d.glyTranslationToX + "px," + d.glyTranslationToY + "px)";
    const from = "translate(" + d.glyTranslationFromX + "px," + d.glyTranslationFromY + "px)";
    const key = [kind, duration, from, to, enabled].join(";");
    const paused = d.glyTranslationPaused === "true";
    if (prior?.key === key) {
      if (prior.animation && prior.paused !== paused) {
        prior.paused = paused;
        if (paused) prior.animation.pause();
        else if (prior.animation.playState !== "finished") prior.animation.play();
      }
      return;
    }
    const start = prior?.animation ? getComputedStyle(el).transform : prior?.resting;
    prior?.animation?.cancel();
    let animation;
    const loop = kind === "loop";
    if (enabled && duration && (loop ? from !== to : prior?.enabled && start !== to)) {
      animation = el.animate([{ transform: loop ? from : start }, { transform: to }], {
        duration,
        iterations: loop ? Infinity : 1,
        easing: "linear",
      });
      if (paused) animation.pause();
    }
    translations.set(el, { key, paused, enabled, animation, resting: loop ? from : to });
  }
  function notifySwipe(s, phase, x, y) {
    event(s.entry, { kind: "swipe", phase, revision: s.revision, x, y });
  }
  function finish(phase) {
    const s = swipe;
    swipe = null;
    if (!s) return;
    if (s.started) notifySwipe(s, phase, s.x, s.y);
    if (s.el.hasPointerCapture(s.id)) s.el.releasePointerCapture(s.id);
  }
  function pointer(raw) {
    if (raw.type === "pointerdown") {
      finish("cancel");
      if (!raw.isPrimary || raw.button !== 0) return;
      const el = raw.target.closest?.("[data-gly-swipe-axis]");
      const entry = el && origin(el);
      if (!el || !entry?.interests.has("Swipe") || blocked(el)) return;
      swipe = {
        el,
        entry,
        axis: el.dataset.glySwipeAxis,
        revision: el.dataset.glySwipeRevision,
        id: raw.pointerId,
        startX: raw.clientX,
        startY: raw.clientY,
        x: raw.clientX,
        y: raw.clientY,
        started: false,
      };
      suppressClick = false;
      return;
    }
    const s = swipe;
    if (!s || raw.pointerId !== s.id) return;
    s.x = raw.clientX;
    s.y = raw.clientY;
    if (raw.type === "pointercancel" || raw.type === "lostpointercapture" || blocked(s.el)) {
      finish("cancel");
    } else if (raw.type === "pointermove") {
      if (!s.started) {
        const dx = Math.abs(s.x - s.startX),
          dy = Math.abs(s.y - s.startY);
        if (Math.hypot(dx, dy) < 6) return;
        const along = s.axis === "horizontal" ? dx : dy,
          cross = s.axis === "horizontal" ? dy : dx;
        if (cross > along) {
          finish("cancel");
          return;
        }
        if (along < 6) return;
        s.el.setPointerCapture(s.id);
        s.started = true;
        notifySwipe(s, "start", s.startX, s.startY);
      }
      raw.preventDefault();
      notifySwipe(s, "move", s.x, s.y);
    } else if (raw.type === "pointerup") {
      if (s.started) {
        raw.preventDefault();
        suppressClick = true;
      }
      finish("drop");
    }
  }
  function sync(root, nodes) {
    mount = root;
    tracked = nodes;
    if (swipe && (blocked(swipe.el) || swipe.el.dataset.glySwipeRevision !== swipe.revision))
      finish("cancel");
    for (const [el, t] of completion) {
      if (!mount.contains(el) || !el.hasAttribute("data-gly-" + t.kind)) {
        if (!el.isConnected) complete(el, t.kind, t);
        completion.delete(el);
      }
    }
    for (const [el, record] of translations) {
      if (!mount.contains(el) || !el.hasAttribute("data-gly-translation")) {
        record.animation?.cancel();
        translations.delete(el);
      }
    }
    for (const el of nodes) {
      if (!el.isConnected) continue;
      if (el.hasAttribute("data-gly-translation")) translate(el);
      if (el.hasAttribute("data-gly-presence") || el.hasAttribute("data-gly-collapse")) watch(el);
    }
  }
  for (const type of [
    "pointerdown",
    "pointermove",
    "pointerup",
    "pointercancel",
    "lostpointercapture",
  ])
    document.addEventListener(type, pointer, true);
  document.addEventListener(
    "click",
    (raw) => {
      if (suppressClick) {
        suppressClick = false;
        raw.preventDefault();
        raw.stopImmediatePropagation();
      }
    },
    true,
  );
  for (const type of ["resize", "blur"]) window.addEventListener(type, () => finish("cancel"));
  document.addEventListener("scroll", () => finish("cancel"), true);
  reduced.addEventListener("change", () => {
    if (mount) sync(mount, tracked);
  });
  return { sync };
}
