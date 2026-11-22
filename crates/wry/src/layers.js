function createLayers({ origin, event, post, createMotion }) {
  const motion = createMotion({ origin, event });
  const panels = new Set(),
    scopeNodes = new Set(),
    motionNodes = new Set();
  const open = new Map(),
    scopes = new Map(),
    observed = new Set();
  function track(el, include = true) {
    if (!el?.hasAttribute) return;
    const member = (collection, enabled) => {
      if (include && enabled) collection.add(el);
      else collection.delete(el);
    };
    member(panels, el.hasAttribute("data-gly-layer"));
    member(scopeNodes, el.hasAttribute("data-gly-focus-scope"));
    member(
      motionNodes,
      ["translation", "collapse", "presence"].some((name) => el.hasAttribute("data-gly-" + name)),
    );
  }
  const observer = new ResizeObserver(schedule);
  const blocked = new Set();
  let mount,
    sequence = 0n,
    frame = 0,
    busy = false,
    scrollLock;
  const named = (name) => (name ? document.getElementById(name) : null);
  function namedControl(name) {
    const el = named(name);
    const target = el && origin(el);
    return target?.parts.get(target.control || "main") || el;
  }
  const visible = (el) =>
    el?.isConnected &&
    !el.closest("[hidden],[inert],[data-gly-hidden]") &&
    el.getClientRects().length &&
    getComputedStyle(el).visibility !== "hidden";
  const top = () => [...open.values()].at(-1);
  function candidates(panel, excludeMenus = false) {
    const result = [],
      pending = [panel];
    let inspected = 0;
    while (pending.length) {
      const el = pending.pop();
      if (++inspected > 65536) throw Error("focus traversal exceeds mounted node budget");
      if (el.hidden || el.inert || (excludeMenus && el.getAttribute("role") === "menu")) continue;
      if (el.tabIndex >= 0 && !el.matches(":disabled,[aria-disabled=true]") && visible(el))
        result.push(el);
      for (let child = el.lastElementChild; child; child = child.previousElementSibling)
        pending.push(child);
    }
    return result;
  }
  const focus = (el) => {
    if (visible(el)) el.focus({ preventScroll: true });
  };
  function first(panel, initial) {
    const preferred = namedControl(initial);
    return (
      (panel.contains(preferred) && visible(preferred) && preferred) ||
      candidates(panel)[0] ||
      panel
    );
  }
  function tab(raw, panel) {
    const items = candidates(panel),
      index = items.indexOf(document.activeElement);
    const next =
      index < 0
        ? raw.shiftKey
          ? items.length - 1
          : 0
        : (index + (raw.shiftKey ? -1 : 1) + items.length) % items.length;
    raw.preventDefault();
    focus(items[next] || panel);
  }
  function schedule() {
    if (!frame && mount)
      frame = requestAnimationFrame(() => {
        frame = 0;
        sync(mount);
      });
  }
  function dismiss(item, reason) {
    if (!item || item.dismissed || !item.entry.interests.has("Dismiss")) return;
    item.dismissed = true;
    event(item.entry, { kind: "dismiss", reason });
  }
  function position(command) {
    const item = [...open.values()].find((item) => item.entry.id === command.id);
    if (!item || item.request !== command.request) return;
    const panel = item.panel;
    panel.style.setProperty("--gly-layer-x", command.x + "px");
    panel.style.setProperty("--gly-layer-y", command.y + "px");
    if (command.arrow) {
      panel.dataset.glyAnchorResolved = command.arrow.side;
      panel.style.setProperty("--gly-anchor-arrow-cross", command.arrow.cross + "px");
      panel.style.setProperty("--gly-anchor-arrow-size", panel.dataset.glyAnchorArrowSize + "px");
      panel.style.setProperty(
        "--gly-anchor-arrow-radius",
        panel.dataset.glyAnchorArrowRadius + "px",
      );
    } else {
      delete panel.dataset.glyAnchorResolved;
      panel.style.removeProperty("--gly-anchor-arrow-cross");
    }
  }
  let grip;
  const visual = () =>
    window.visualViewport || {
      width: innerWidth,
      height: innerHeight,
      offsetLeft: 0,
      offsetTop: 0,
    };
  const horizontal = (panel) => ["left", "right"].includes(panel.dataset.glyLayer);
  function sizeSheet(panel) {
    const v = visual(),
      d = panel.dataset;
    const available = horizontal(panel) ? v.width : v.height;
    const fraction = Number(d.glySheet.split(",")[Number(d.glySheetCurrent)]);
    const pixels = Math.min(available, Number(d.glySheetMax)) * fraction;
    for (const [name, value] of [
      ["x", v.offsetLeft],
      ["y", v.offsetTop],
      ["width", v.width],
      ["height", v.height],
    ])
      panel.style.setProperty("--gly-sheet-" + name, value + "px");
    if (
      grip?.panel === panel &&
      (grip.available !== available || grip.revision !== d.glySheetRevision)
    )
      endGrip(false);
    if (grip?.panel !== panel) {
      panel.style.setProperty("--gly-sheet-extent", pixels + "px");
      panel.style.setProperty("--gly-sheet-settle", d.glySheetSettle + "ms");
    }
  }
  function endGrip(release) {
    const current = grip;
    grip = null;
    if (!current) return;
    const { handle, panel, id, entry, available, revision, extent, moved } = current;
    if (handle.hasPointerCapture(id)) handle.releasePointerCapture(id);
    if (
      release &&
      moved &&
      entry.interests.has("Sheet") &&
      panel.dataset.glySheetRevision === revision
    )
      event(entry, { kind: "sheet", revision, viewport: available, extent });
    if (panel.isConnected && panel.hasAttribute("data-gly-sheet")) sizeSheet(panel);
  }
  function trackGrip(raw) {
    if (raw.type === "pointerdown") {
      const handle = raw.target.closest?.("[data-gly-sheet-grip]");
      const panel = handle?.closest("[data-gly-sheet]");
      if (!panel || !visible(handle) || !raw.isPrimary || raw.button !== 0) return;
      endGrip(false);
      const along = horizontal(panel),
        v = visual(),
        rect = panel.getBoundingClientRect();
      grip = {
        handle,
        panel,
        id: raw.pointerId,
        entry: origin(panel),
        revision: panel.dataset.glySheetRevision,
        x: raw.clientX,
        y: raw.clientY,
        baseline: along ? rect.width : rect.height,
        available: along ? v.width : v.height,
      };
      handle.setPointerCapture(raw.pointerId);
      return;
    }
    if (!grip || grip.id !== raw.pointerId) return;
    if (
      raw.type === "lostpointercapture" ||
      raw.type === "pointercancel" ||
      !visible(grip.handle)
    ) {
      endGrip(false);
      return;
    }
    const { panel, x, y, baseline, available } = grip;
    const delta = {
      left: raw.clientX - x,
      right: x - raw.clientX,
      top: raw.clientY - y,
      bottom: y - raw.clientY,
    }[panel.dataset.glyLayer];
    if (Math.abs(delta) >= 6) grip.moved = true;
    if (grip.moved) {
      grip.extent = Math.max(
        0,
        Math.min(baseline + delta, available, Number(panel.dataset.glySheetMax)),
      );
      panel.style.setProperty("--gly-sheet-settle", "0ms");
      panel.style.setProperty("--gly-sheet-extent", grip.extent + "px");
      raw.preventDefault();
    }
    if (raw.type === "pointerup") endGrip(true);
  }
  function exitMenu(panel, backwards) {
    const context = panel.dataset.glyLayer === "context-point";
    const returned =
      namedControl(panel.dataset.glyLayerReturn) ||
      (context && namedControl(panel.dataset.glyLayerAnchor));
    if (!returned) return;
    const bar =
      returned.hasAttribute("data-gly-menubar-item") && returned.closest("[role=menubar]");
    const trigger = bar || returned,
      scope = modalPanels()[0] || document.body;
    const stops = candidates(scope, true);
    const match = (el) => el === trigger || ((bar || context) && trigger.contains(el));
    const index = backwards ? stops.findIndex(match) : stops.findLastIndex(match);
    if (index < 0) return;
    const next = stops[index + (backwards ? -1 : 1)];
    if (next) focus(next);
    else document.activeElement?.blur();
  }
  function raise(panel) {
    if (!open.has(panel) || panel.closest("[inert]")) return;
    const related = new Set([panel]);
    for (const { panel: child, anchor } of open.values())
      if ([...related].some((parent) => parent.contains(child) || parent.contains(anchor)))
        related.add(child);
    const active = document.activeElement;
    busy = true;
    try {
      for (const child of related) {
        const item = open.get(child);
        open.delete(child);
        open.set(child, item);
        if (child.matches(":popover-open")) {
          child.hidePopover();
          child.showPopover();
        }
      }
      if (document.activeElement !== active) focus(active);
    } finally {
      busy = false;
    }
  }
  function measure(item) {
    const { panel, entry, anchor } = item,
      data = panel.dataset;
    if (panel.hasAttribute("data-gly-sheet")) sizeSheet(panel);
    if (!["anchor", "context-point", "viewport"].includes(data.glyLayer)) return;
    const bounds = anchor?.getBoundingClientRect();
    const rect = bounds ? [bounds.x, bounds.y, bounds.width, bounds.height] : [0, 0, 0, 0];
    if (bounds) panel.style.setProperty("--gly-anchor-width", bounds.width + "px");
    const box = panel.getBoundingClientRect();
    const size = [box.width, box.height],
      viewport = [innerWidth, innerHeight];
    const signature = JSON.stringify([
      rect,
      size,
      viewport,
      ...Object.keys(data)
        .filter(
          (key) =>
            /^(glyLayer|glyAnchor|glyViewport|glyContext)/.test(key) && key !== "glyAnchorResolved",
        )
        .map((key) => [key, data[key]]),
    ]);
    if (item.signature !== signature) {
      item.request = (++sequence).toString();
      item.signature = signature;
      post({
        type: "layer_bounds",
        node: entry.id,
        request: item.request,
        anchor: rect,
        panel: size,
        viewport,
      });
    }
  }
  function restore(panel, previous, key) {
    const dest = namedControl(panel.dataset[key]) || previous;
    if (
      panel.contains(document.activeElement) ||
      !document.activeElement ||
      document.activeElement === document.body
    )
      focus(dest);
  }
  function releaseBlocks() {
    for (const el of blocked) if (!el.hasAttribute("data-gly-inert")) el.inert = false;
    blocked.clear();
  }
  function modalPanels() {
    const items = [...open.values()],
      index = items.findLastIndex((item) => item.panel.getAttribute("aria-modal") === "true");
    if (index < 0) return [];
    const allowed = [items[index].panel];
    for (const { panel, anchor } of items.slice(index + 1)) {
      if (allowed.some((parent) => parent.contains(panel) || parent.contains(anchor)))
        allowed.push(panel);
    }
    return allowed;
  }
  function lock(allowed) {
    if (!allowed.length) {
      if (scrollLock !== undefined) {
        if (document.documentElement.style.overflow === "hidden")
          document.documentElement.style.overflow = scrollLock;
        scrollLock = undefined;
      }
      return;
    }
    if (scrollLock === undefined) {
      scrollLock = document.documentElement.style.overflow;
      document.documentElement.style.overflow = "hidden";
    }
    function block(node) {
      if (allowed.includes(node)) return;
      if (allowed.some((el) => node.contains(el))) {
        for (const child of node.children) block(child);
      } else if (!node.inert) {
        node.inert = true;
        blocked.add(node);
      }
    }
    block(mount);
  }
  function currentScope(test) {
    let chosen;
    for (const panel of scopes.keys()) {
      if (visible(panel) && test(panel) && (!chosen || !panel.contains(chosen))) chosen = panel;
    }
    return chosen;
  }
  function syncScopes(active) {
    for (const [panel, record] of [...scopes].reverse()) {
      if (active.has(panel)) continue;
      scopes.delete(panel);
      if (record.restore)
        restore(panel, namedControl(record.returnName) || record.previous, "glyFocusReturn");
    }
    let autofocus;
    for (const panel of active) {
      let record = scopes.get(panel);
      if (!record) {
        record = { previous: document.activeElement, focused: false };
        scopes.set(panel, record);
        if (
          panel.dataset.glyFocusAuto === "true" &&
          visible(panel) &&
          (!autofocus || !panel.contains(autofocus))
        )
          autofocus = panel;
      }
      record.restore = panel.dataset.glyFocusRestore === "true";
      record.returnName = panel.dataset.glyFocusReturn;
    }
    if (autofocus) focus(first(autofocus, autofocus.dataset.glyFocusInitial));
    const owner = currentScope(
      (panel) => panel.dataset.glyFocusContain === "true" && scopes.get(panel).focused,
    );
    if (owner && !scopeOwns(owner, document.activeElement))
      focus(first(owner, owner.dataset.glyFocusInitial));
  }
  function sync(root) {
    if (busy) return;
    mount = root;
    if (grip && !visible(grip.handle)) endGrip(false);
    busy = true;
    try {
      releaseBlocks();
      const active = new Set(
        [...panels].filter(
          (el) => el.isConnected && !el.closest("[hidden],[inert],[data-gly-hidden]"),
        ),
      );
      for (const [panel, item] of [...open].reverse()) {
        if (active.has(panel)) continue;
        open.delete(panel);
        if (panel.matches(":popover-open")) panel.hidePopover();
        if (!item.preserve)
          restore(
            panel,
            (panel.dataset.glyLayer === "context-point" ? item.anchor : item.previous) ||
              item.anchor,
            "glyLayerReturn",
          );
      }
      const watches = new Set();
      for (const panel of active) {
        const anchor = named(panel.dataset.glyLayerAnchor);
        let item = open.get(panel);
        const opening = !item;
        if (opening) {
          item = { entry: origin(panel), panel, previous: document.activeElement, anchor };
          open.set(panel, item);
        }
        item.anchor = anchor;
        item.preserve = panel.hasAttribute("data-gly-layer-preserve");
        if (panel.dataset.glyLayerAnchor && !visible(anchor)) {
          dismiss(item, "anchor_lost");
          if (panel.matches(":popover-open")) panel.hidePopover();
          continue;
        }
        if (!panel.matches(":popover-open")) {
          if (panel.hasAttribute("data-gly-sheet")) sizeSheet(panel);
          panel.showPopover();
        }
        watches.add(panel);
        if (anchor) watches.add(anchor);
        measure(item);
        if (opening && !item.preserve) focus(first(panel, panel.dataset.glyLayerInitial));
      }
      for (const el of observed)
        if (!watches.has(el)) {
          observer.unobserve(el);
          observed.delete(el);
        }
      for (const el of watches)
        if (!observed.has(el)) {
          observer.observe(el);
          observed.add(el);
        }
      // Preserve the underlying focus scopes' leases while an overlay temporarily blocks them.
      const activeScopes = new Set(
        [...scopeNodes].filter((el) => el.dataset.glyFocusScope === "true" && visible(el)),
      );
      lock(modalPanels());
      syncScopes(activeScopes);
      motion.sync(root, motionNodes);
    } finally {
      busy = false;
    }
  }
  function scopeOwns(scope, target) {
    return (
      scope.contains(target) ||
      [...open.values()].some(
        ({ panel, anchor }) => panel.contains(target) && scope.contains(anchor),
      )
    );
  }
  document.addEventListener(
    "focusin",
    (raw) => {
      if (busy) return;
      const modal = modalPanels();
      if (modal.length && !modal.some((panel) => panel.contains(raw.target))) {
        focus(first(modal.at(-1), modal.at(-1).dataset.glyLayerInitial));
        return;
      }
      const scope = currentScope((panel) => panel.dataset.glyFocusContain === "true");
      if (scope && !scopeOwns(scope, raw.target)) {
        focus(first(scope, scope.dataset.glyFocusInitial));
        return;
      }
      for (const [panel, record] of scopes) record.focused = scopeOwns(panel, raw.target);
      const item = top();
      if (item?.panel.dataset.glyLayer === "context-point" && !item.panel.contains(raw.target))
        dismiss(item, "outside");
    },
    true,
  );
  document.addEventListener(
    "pointerdown",
    (raw) => {
      const item = top();
      if (raw.button !== 0 || !raw.isPrimary) return;
      const scope = currentScope((panel) => panel.dataset.glyFocusContain === "true");
      if (
        scope &&
        !scopeOwns(scope, raw.target) &&
        raw.target.closest?.("button,input,textarea,select,a[href],[tabindex]")
      )
        raw.preventDefault();
      if (!item) return;
      if (
        item.panel.dataset.glyLayerOutside === "true" &&
        !item.panel.contains(raw.target) &&
        !item.anchor?.contains(raw.target) &&
        raw.target.closest?.("[aria-controls]")?.getAttribute("aria-controls") !== item.panel.id
      )
        dismiss(item, "outside");
      const modal = modalPanels();
      if (modal.length && !modal.some((el) => el.contains(raw.target))) {
        raw.preventDefault();
        raw.stopImmediatePropagation();
      }
    },
    true,
  );
  window.addEventListener(
    "keydown",
    (raw) => {
      if (raw.isComposing || raw.ctrlKey || raw.altKey || raw.metaKey) return;
      const item = top();
      if (raw.key === "Escape" && !raw.repeat && item) {
        raw.preventDefault();
        raw.stopImmediatePropagation();
        if (item.panel.dataset.glyLayerEscape === "true") dismiss(item, "escape");
      } else if (raw.key === "Tab") {
        const scope = currentScope(
          (el) => el.dataset.glyFocusLoop === "true" && scopeOwns(el, raw.target),
        );
        if (scope && scope.contains(raw.target)) tab(raw, scope);
        else if (modalPanels().length && item?.panel.getAttribute("role") !== "menu")
          tab(raw, item.panel);
      }
    },
    true,
  );
  for (const type of [
    "pointerdown",
    "pointermove",
    "pointerup",
    "pointercancel",
    "lostpointercapture",
  ])
    document.addEventListener(type, trackGrip, true);
  window.addEventListener("blur", () => endGrip(false));
  window.visualViewport?.addEventListener("resize", schedule);
  window.visualViewport?.addEventListener("scroll", schedule);
  // Background WebViews may change activeElement without dispatching document focusin.
  // Reconcile the current containment lease when the system returns window focus.
  window.addEventListener("focus", () => {
    const modal = modalPanels();
    if (modal.length && !modal.some((el) => el.contains(document.activeElement))) {
      const panel = modal.at(-1);
      focus(first(panel, panel.dataset.glyLayerInitial));
    } else {
      const panel = currentScope((el) => el.dataset.glyFocusContain === "true");
      if (panel && !scopeOwns(panel, document.activeElement))
        focus(first(panel, panel.dataset.glyFocusInitial));
    }
  });
  window.addEventListener("resize", schedule);
  document.addEventListener(
    "scroll",
    () => {
      if (open.size) schedule();
    },
    true,
  );
  return {
    sync,
    position,
    raise,
    track,
    exitMenu,
    contains: (element) => !!top()?.panel.contains(element),
    dismissOtherContext(target) {
      const item = top();
      if (item?.panel.dataset.glyLayer === "context-point" && item.anchor !== target)
        dismiss(item, "outside");
    },
  };
}
