function createRichEditor({ main, event, origin, alive }) {
  const utf8 = new TextEncoder();
  const empty = () => ({ text: "", bold: false, italic: false, code: false, link: null });
  const signature = (blocks) =>
    JSON.stringify(
      blocks.map((b) => [b.tag, b.runs.map((r) => [r.text, r.bold, r.italic, r.code, r.link])]),
    );
  const equalMarks = (a, b) =>
    a.bold === b.bold && a.italic === b.italic && a.code === b.code && a.link === b.link;
  function read(root) {
    let nodes = 0,
      runs = 0,
      bytes = 0,
      metadata = 0;
    const roots = [],
      blocks = [];
    function inline(node, marks, out, depth) {
      if (++nodes > 8192 || depth > 32) throw Error("rich node budget");
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node.data;
        if (text.length > 65536 || /[\0\r\n]/.test(text)) throw Error("invalid rich text");
        if (!text.length) return;
        bytes += utf8.encode(text).length;
        if (bytes > 65536 || ++runs > 2048) throw Error("rich document budget");
        if (out.length && equalMarks(out[out.length - 1], marks)) out[out.length - 1].text += text;
        else out.push({ ...marks, text });
        return;
      }
      switch (node.tagName) {
        case "BR":
          return;
        case "B":
        case "STRONG":
          marks = { ...marks, bold: true };
          break;
        case "I":
        case "EM":
          marks = { ...marks, italic: true };
          break;
        case "CODE":
          marks = { ...marks, code: true };
          break;
        case "SPAN":
          if (node.hasAttribute("style")) throw Error("unsupported styled span");
          break;
        case "A": {
          const link = node.getAttribute("href");
          if (
            !link ||
            link.length > 2048 ||
            utf8.encode(link).length > 2048 ||
            !/^(https?:|mailto:).+/i.test(link) ||
            /[\x00-\x20\x7f]/.test(link)
          )
            throw Error("invalid rich link");
          metadata += utf8.encode(link).length;
          if (metadata > 131072) throw Error("rich link budget");
          marks = { ...marks, link };
          break;
        }
        default:
          throw Error("unsupported rich DOM");
      }
      for (const child of node.childNodes) inline(child, marks, out, depth + 1);
    }
    for (const n of root.childNodes) {
      if (blocks.length >= 128) throw Error("rich block budget");
      const out = [],
        tag = n.tagName?.toLowerCase(),
        block = tag && /^(p|div|h[1-6]|blockquote)$/.test(tag);
      if (block) for (const child of n.childNodes) inline(child, empty(), out, 0);
      else inline(n, empty(), out, 0);
      blocks.push({ tag: block && tag !== "div" ? tag : "p", runs: out.length ? out : [empty()] });
      roots.push(n);
    }
    if (!blocks.length) {
      blocks.push({ tag: "p", runs: [empty()] });
      roots.push(root);
    }
    return { blocks, roots };
  }
  function selection(root, roots) {
    const sel = root.ownerDocument.getSelection();
    function point(node, index) {
      if (!node || !root.contains(node)) return null;
      if (node === root) {
        const block = Math.min(index, roots.length - 1);
        return { block, offset: index === roots.length ? roots[block].textContent.length : 0 };
      }
      const block = roots.findIndex((r) => r.contains(node));
      if (block < 0) return null;
      const range = root.ownerDocument.createRange();
      range.selectNodeContents(roots[block]);
      range.setEnd(node, index);
      return { block, offset: range.toString().length };
    }
    const anchor = point(sel?.anchorNode, sel?.anchorOffset),
      focus = point(sel?.focusNode, sel?.focusOffset);
    return anchor && focus ? { anchor, focus } : null;
  }
  function snapshot(e, requireSelection = false) {
    const root = main(e),
      { blocks, roots } = read(root);
    const activeSelection = selection(root, roots);
    if (requireSelection && !activeSelection) throw Error("selection unavailable");
    const sel = activeSelection ||
      e.richSelection || { anchor: { block: 0, offset: 0 }, focus: { block: 0, offset: 0 } };
    return { blocks, selection: sel, composing: e.composing, revision: e.revision };
  }
  function select(e, sel) {
    const root = main(e),
      { roots } = read(root);
    function target(point) {
      const block = roots[point.block];
      if (!block) return null;
      const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT);
      let node = block.nodeType === Node.TEXT_NODE ? block : walker.nextNode(),
        offset = point.offset;
      while (node) {
        if (offset <= node.length) return [node, offset];
        offset -= node.length;
        node = walker.nextNode();
      }
      return offset === 0 ? [block, block.childNodes.length] : null;
    }
    const a = target(sel.anchor),
      b = target(sel.focus);
    if (!a || !b) return;
    document.getSelection().setBaseAndExtent(a[0], a[1], b[0], b[1]);
    e.richSelection = sel;
  }
  function render(e, blocks) {
    const root = main(e),
      frag = document.createDocumentFragment();
    for (const b of blocks) {
      const block = document.createElement(b.tag);
      for (const r of b.runs) {
        let parent = block;
        for (const [tag, on] of [
          ["a", r.link],
          ["strong", r.bold],
          ["em", r.italic],
          ["code", r.code],
        ]) {
          if (!on) continue;
          const mark = document.createElement(tag);
          if (tag === "a") mark.setAttribute("href", r.link);
          parent.append(mark);
          parent = mark;
        }
        if (r.text) parent.append(document.createTextNode(r.text));
      }
      if (!block.hasChildNodes()) block.append(document.createElement("br"));
      frag.append(block);
    }
    root.replaceChildren(frag);
  }
  function property(e, blocks) {
    const next = signature(blocks);
    e.richAccepted = blocks;
    if (e.composing) {
      e.richPending = blocks;
      return;
    }
    if (e.richProposal === next && e.richDirty) return;
    let saved = null;
    try {
      const current = read(main(e));
      if (main(e).hasChildNodes() && signature(current.blocks) === next) return;
      saved = selection(main(e), current.roots);
    } catch {}
    render(e, blocks);
    e.richDirty = false;
    e.richSelection = null;
    if (saved) select(e, saved);
  }
  function reject(e) {
    if (e.richAccepted) render(e, e.richAccepted);
    e.richSelection = null;
    if (e.interests.has("RichRejected")) event(e, { kind: "rich_rejected", revision: e.revision });
  }
  function observe(e, editing) {
    if (!alive(e)) return;
    if (e.richProposal) {
      if (editing) e.richDirty = true;
      return;
    }
    let value;
    try {
      value = snapshot(e, !editing);
    } catch {
      if (editing) reject(e);
      return;
    }
    e.richSelection = value.selection;
    if (editing && e.interests.has("RichEdit")) {
      e.richProposal = signature(value.blocks);
      event(e, { kind: "rich_edit", snapshot: value });
    } else if (!editing && e.interests.has("RichSelection")) {
      const key = JSON.stringify([e.revision, value.selection]);
      if (key !== e.richSelectionKey) {
        e.richSelectionKey = key;
        event(e, { kind: "rich_selection", snapshot: value });
      }
    }
  }
  function done(e, cmd) {
    const dirty = e.richDirty;
    e.richProposal = null;
    e.richDirty = false;
    if (cmd.retired) return;
    if (!e.composing) e.revision = cmd.revision;
    if (cmd.reset) property(e, cmd.value);
    else if (dirty) observe(e, true);
    else if (document.activeElement === main(e)) observe(e, false);
  }
  function receive(e, raw) {
    if (e.edit !== "Rich") return false;
    const root = main(e);
    if (raw.type === "focusin" || raw.type === "focusout") return false;
    if (!root.isContentEditable) return false;
    switch (raw.type) {
      case "input":
        e.composing ||= !!raw.isComposing;
        observe(e, true);
        return true;
      case "compositionstart":
        e.composing = true;
        event(e, { kind: "composition", active: true });
        return true;
      case "compositionend":
        e.composing = false;
        if (e.richPending) {
          const blocks = e.richPending;
          e.richPending = null;
          property(e, blocks);
        }
        e.revision = e.requested;
        event(e, { kind: "composition", active: false });
        observe(e, true);
        return true;
      case "beforeinput": {
        if (raw.inputType === "historyUndo" || raw.inputType === "historyRedo") {
          raw.preventDefault();
          if (e.interests.has("RichCommand"))
            event(e, {
              kind: "rich_command",
              command: raw.inputType === "historyUndo" ? "undo" : "redo",
              revision: e.revision,
            });
        } else if (
          [
            "insertFromDrop",
            "insertOrderedList",
            "insertUnorderedList",
            "formatUnderline",
            "formatStrikeThrough",
            "formatBold",
            "formatItalic",
          ].includes(raw.inputType)
        ) {
          raw.preventDefault();
        }
        return true;
      }
      case "paste": {
        raw.preventDefault();
        const text = raw.clipboardData?.getData("text/plain");
        if (text === undefined || text.length > 65536 || utf8.encode(text).length > 65536) {
          reject(e);
          return true;
        }
        // insertText sends plain text through the native editor's line breaking
        // and selection machinery. Clipboard markup never enters the document.
        if (!document.execCommand("insertText", false, text)) reject(e);
        return true;
      }
      case "drop":
        raw.preventDefault();
        return true;
      case "click":
        if (raw.target.closest?.("a")) raw.preventDefault();
        return false;
      case "keydown": {
        if (
          raw.isComposing ||
          e.composing ||
          raw.altKey ||
          raw.repeat ||
          !(raw.ctrlKey || raw.metaKey)
        )
          return false;
        const command = {
          z: raw.shiftKey ? "redo" : "undo",
          y: "redo",
          b: "bold",
          i: "italic",
          "\x60": "code",
        }[raw.key.toLowerCase()];
        if (!command || !e.interests.has("RichCommand")) return false;
        raw.preventDefault();
        event(e, { kind: "rich_command", command, revision: e.revision });
        return true;
      }
      default:
        return false;
    }
  }
  document.addEventListener("selectionchange", () => {
    const e = origin(document.activeElement);
    if (e?.edit === "Rich") observe(e, false);
  });
  return { receive, done, snapshot, select, property };
}
