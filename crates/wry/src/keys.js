function createKeyboard({ bindings, event, origin, observed, control, readValue, canonical }) {
  const printable = (key) => [...key].length === 1 && !/[\p{Cc}\p{White_Space}\p{Cs}]/u.test(key);
  function key(raw, entry) {
    const el = raw.target;
    if (
      raw.defaultPrevented ||
      raw.isComposing ||
      entry.composing ||
      raw.ctrlKey ||
      raw.metaKey ||
      el !== document.activeElement ||
      el !== control(entry) ||
      el.closest("[hidden],[inert],[aria-disabled=true]") ||
      el.disabled
    )
      return false;
    const data = el.dataset;
    const flags = Number(raw.shiftKey) + 2 * Number(raw.repeat) + 4 * Number(raw.altKey);
    const stroke = flags + raw.key;
    const use = (profile, receiver = entry) => {
      if (!receiver) return false;
      const binding = bindings[profile]?.[stroke];
      if (binding === undefined) return false;
      raw.preventDefault();
      event(receiver, {
        kind: "key_binding",
        binding,
        ...(entry !== receiver ? { from: entry.id } : {}),
      });
      return true;
    };
    const search = (tree = null) => {
      if (raw.altKey || !printable(raw.key)) return false;
      raw.preventDefault();
      const at_millis = Math.floor(performance.now());
      if (tree) event(tree, { kind: "tree_search", from: entry.id, text: raw.key, at_millis });
      else event(entry, { kind: "typeahead", text: raw.key, at_millis });
      return true;
    };

    if (
      el.hasAttribute("data-gly-tags") &&
      entry.interests.has("Tags") &&
      !el.readOnly &&
      !raw.repeat &&
      !raw.altKey &&
      !raw.shiftKey
    ) {
      const value = readValue(entry);
      const command = bindings[`tags-${data.glyTags}-${!value}`]?.[raw.key];
      if (typeof command === "string") {
        raw.preventDefault();
        sendTags(entry, command === "commit" ? { command, value } : { command });
        return true;
      }
    }
    if (data.glyCombobox && entry.interests.has("Choice")) {
      const expanded = el.getAttribute("aria-expanded") === "true";
      if (!el.readOnly && use(`choice-${data.glyCombobox}-${expanded}-${!el.value}`)) return true;
      if (
        data.glyCombobox === "select" &&
        el.hasAttribute("data-gly-typeahead") &&
        entry.interests.has("Typeahead") &&
        search()
      )
        return true;
    }
    if (el.hasAttribute("data-gly-tree-item")) {
      const tree = el.closest("[role=tree]"),
        receiver = origin(tree);
      if (tree?.dataset.glyTreeNavigation && receiver?.interests.has("TreeNavigate")) {
        if (use("tree-" + tree.dataset.glyTreeNavigation, receiver)) return true;
        if (tree.dataset.glyTreeTypeahead === "true" && search(receiver)) return true;
      }
    }
    if (el.tagName === "BUTTON") {
      const grid = el.closest("[role=grid]"),
        receiver = origin(grid);
      if (
        grid?.dataset.glyGridNavigation &&
        receiver?.interests.has("GridNavigate") &&
        use("grid-" + grid.dataset.glyGridNavigation, receiver)
      )
        return true;
    }
    if (entry.interests.has("Range") && el.getAttribute("aria-readonly") !== "true") {
      const axis = el.getAttribute("aria-orientation") || "horizontal";
      if (el.hasAttribute("data-gly-slider") && use(`range-${axis}-${el.dir || "ltr"}`))
        return true;
      if (el.hasAttribute("data-gly-splitter") && use("splitter-" + axis)) return true;
    }
    if (
      el.hasAttribute("data-gly-spinbutton") &&
      entry.interests.has("Spin") &&
      !el.readOnly &&
      el.getAttribute("aria-readonly") !== "true" &&
      use(`spin-${el.hasAttribute("aria-valuemin")}-${el.hasAttribute("aria-valuemax")}`)
    )
      return true;
    if (
      data.glyScrollKeyboard &&
      entry.interests.has("ScrollKeyboard") &&
      use("scroll-" + data.glyScrollKeyboard)
    )
      return true;

    const menu = el.closest("[role=menu]");
    if (data.glyMenuSearch && origin(menu)?.interests.has("Menu") && use("menu-search"))
      return true;
    if (entry.edit === "Text" || entry.edit === "Select") return false;
    const panel = el.closest("[data-gly-navigation-panel=content]");
    const trigger = data.glyNavigationPanel === "trigger";
    if (
      (trigger || (panel && el.hasAttribute("data-gly-navigation-item"))) &&
      observed(entry, "NavigationPanel")
    ) {
      const dir = panel?.dir || el.closest("[data-gly-navigation]")?.dir || "ltr";
      if (use(`panel-${dir}-${trigger}-${!!panel}`)) return true;
    }
    const menuTrigger =
      el.getAttribute("aria-haspopup") === "menu" &&
      (!el.hasAttribute("data-gly-menu-item") || el.hasAttribute("data-gly-menubar-item"));
    if (menuTrigger ? entry.interests.has("Menu") : origin(menu)?.interests.has("Menu")) {
      const dir = menu?.dir || el.closest("[role=menubar]")?.dir || "ltr";
      if (use(`menu-${dir}-${menuTrigger}`)) return true;
    }
    if (
      !flags &&
      raw.key === "Enter" &&
      entry.edit === "Check" &&
      el.matches("[role=menuitemcheckbox],[role=menuitemradio]")
    ) {
      raw.preventDefault();
      el.click();
      return true;
    }
    const boundary = el.closest("[data-gly-navigation]");
    if (
      (el === boundary || el.hasAttribute("data-gly-navigation-item")) &&
      origin(boundary)?.interests.has("Navigate") &&
      use(`navigate-${boundary.dataset.glyNavigation}-${boundary.dir || "ltr"}`)
    )
      return true;
    return !!(
      boundary?.hasAttribute("data-gly-typeahead") &&
      origin(boundary)?.interests.has("Typeahead") &&
      (el === boundary || el.hasAttribute("data-gly-navigation-item")) &&
      search()
    );
  }

  function menuSearchPointer(target) {
    const editor = document.activeElement,
      menu = editor?.closest("[role=menu]");
    return !!(
      menu &&
      editor.hasAttribute("data-gly-menu-search") &&
      target.closest("[data-gly-menu-item]") &&
      target.closest("[role=menu]") === menu
    );
  }
  function pointer(raw) {
    if (
      raw.isPrimary &&
      raw.button === 0 &&
      (raw.target.closest("[data-gly-option-owner],[data-gly-menubar-item]") ||
        menuSearchPointer(raw.target))
    ) {
      raw.preventDefault();
      return true;
    }
    return false;
  }
  function tagBudget(value, max) {
    return value.length <= max && new TextEncoder().encode(value).length <= max;
  }
  function sendTags(entry, body) {
    if (body.value !== undefined && !tagBudget(body.value, 4096))
      body = { command: "input_budget" };
    event(entry, { kind: "tags", revision: entry.revision, data: body });
  }
  function paste(raw, entry) {
    const el = raw.target;
    if (
      raw.defaultPrevented ||
      !raw.clipboardData ||
      raw.isComposing ||
      entry.composing ||
      document.activeElement !== el ||
      el !== control(entry) ||
      !el.hasAttribute("data-gly-tags") ||
      !entry.interests.has("Tags") ||
      el.disabled ||
      el.readOnly ||
      el.closest("[inert],[aria-disabled=true]")
    )
      return;
    const rawText = raw.clipboardData.getData("text/plain");
    const text = rawText.length <= 16384 ? canonical(rawText) : null;
    if (text === null || !tagBudget(text, 16384)) {
      raw.preventDefault();
      sendTags(entry, { command: "clipboard_budget" });
      return;
    }
    const policy = el.dataset.glyTags;
    if (
      !text.includes("\n") &&
      !(policy !== "enter" && text.includes(",")) &&
      !(policy === "enter-comma-semicolon" && text.includes(";"))
    )
      return;
    raw.preventDefault();
    sendTags(entry, {
      command: "paste",
      value: readValue(entry),
      text,
      start: el.selectionStart,
      end: el.selectionEnd,
    });
  }
  function revealOption(option) {
    const owner = option.closest("[role=listbox]");
    if (
      !owner ||
      owner.id !== option.dataset.glyOptionOwner ||
      !owner.isConnected ||
      owner.closest("[hidden],[data-gly-hidden]")
    )
      return;
    const box = option.getBoundingClientRect(),
      top = owner.getBoundingClientRect().top + owner.clientTop;
    const bottom = top + owner.clientHeight;
    const delta = box.top < top ? box.top - top : Math.max(0, box.bottom - bottom);
    if (delta) owner.scrollTop = Math.max(0, owner.scrollTop + delta);
  }
  return { key, pointer, menuSearchPointer, paste, revealOption };
}
