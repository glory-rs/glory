function createFiles({ post, main, origin, alive, enabled }) {
  let serial = 0n,
    batch = 0n,
    activePicker = null,
    pending = null,
    reads = 0;
  const selections = new Map(),
    encoder = new TextEncoder();
  function observe(e, revision, request, outcome) {
    const id = String(++batch);
    post({ type: "files", observation: { node: e.id, revision, request, batch: id, outcome } });
    return id;
  }
  function release(e) {
    const items = selections.get(e);
    if (items) for (const item of items.values()) if (item.url) URL.revokeObjectURL(item.url);
    selections.delete(e);
  }
  function clear(e) {
    release(e);
    if (pending?.entry === e) pending = null;
    if (activePicker?.entry === e) {
      activePicker.cleanup();
      activePicker = null;
    }
  }
  function select(e, revision, request, raw) {
    const picker = e.parts.get("picker");
    const reject = (reason) => observe(e, revision, request, { kind: "rejected", reason });
    if (pending) {
      reject("busy");
      return;
    }
    if (!raw?.length) {
      observe(e, revision, request, { kind: "cancelled" });
      return;
    }
    if (!alive(e) || main(e).disabled || revision !== e.revision) {
      reject("failed");
      return;
    }
    const options = picker.dataset;
    if (
      raw.length > Number(options.glyFileCount) ||
      raw.length > 32 ||
      (!picker.multiple && raw.length !== 1)
    ) {
      reject("count");
      return;
    }
    const selected = new Map(),
      files = [];
    const filters = picker.accept.split(",").filter(Boolean);
    let bytes = 0,
      metadata = 0;
    for (const file of raw) {
      const name = file.name,
        type = file.type.toLowerCase();
      if (
        !name ||
        name.length > 1024 ||
        encoder.encode(name).length > 1024 ||
        /[\x00\r]/.test(name) ||
        (type && (type.length > 127 || !/^[a-z0-9!#$&^_.+-]+\/[a-z0-9!#$&^_.+-]+$/.test(type)))
      ) {
        reject("metadata");
        return;
      }
      metadata += encoder.encode(name).length + type.length;
      if (metadata > 32768) {
        reject("metadata");
        return;
      }
      if (
        filters.length &&
        !filters.some((filter) =>
          filter.startsWith(".")
            ? name.toLowerCase().endsWith(filter)
            : filter.endsWith("/*")
              ? type.startsWith(filter.slice(0, -1))
              : type === filter,
        )
      ) {
        reject("type");
        return;
      }
      bytes += file.size;
      if (file.size > Number(options.glyFileBytes) || bytes > Number(options.glyFileTotal)) {
        reject("bytes");
        return;
      }
      const id = String(++serial);
      files.push({ serial: id, name, media_type: type, size: file.size });
      selected.set(id, {
        file,
        revision,
        url: null,
        metadata: encoder.encode(name).length + type.length,
      });
    }
    let count = selected.size;
    for (const [owner, items] of selections)
      if (owner !== e)
        for (const item of items.values()) {
          count++;
          bytes += item.file.size;
          metadata += item.metadata;
        }
    if (count > 128 || metadata > 131072 || bytes > 1073741824) {
      reject("busy");
      return;
    }
    const id = observe(e, revision, request, { kind: "selected", files });
    pending = { entry: e, id, selected };
  }
  function accept(id, accepted) {
    if (pending?.id !== id) return;
    const { entry, selected } = pending;
    pending = null;
    if (accepted) {
      release(entry);
      selections.set(entry, selected);
    }
  }
  function action(e, a) {
    if (!enabled || !e.parts.has("picker")) return false;
    if (a.kind === "clear_files") {
      clear(e);
      return true;
    }
    if (a.kind === "remove_file") {
      const items = selections.get(e),
        item = items?.get(a.serial);
      if (item?.url) URL.revokeObjectURL(item.url);
      items?.delete(a.serial);
      return true;
    }
    if (a.kind !== "pick_files") return false;
    const reject = (reason) => observe(e, a.revision, a.request, { kind: "rejected", reason });
    if (activePicker || pending) {
      reject("busy");
      return true;
    }
    const picker = e.parts.get("picker");
    if (!alive(e) || main(e).disabled || e.revision !== a.revision) {
      reject("failed");
      return true;
    }
    if (!picker.showPicker) {
      reject("unavailable");
      return true;
    }
    const cleanup = () => {
      picker.removeEventListener("change", done);
      picker.removeEventListener("cancel", done);
    };
    const done = (ev) => {
      cleanup();
      activePicker = null;
      select(e, a.revision, a.request, ev.type === "cancel" ? null : picker.files);
      picker.value = "";
    };
    activePicker = { entry: e, cleanup };
    picker.value = "";
    picker.addEventListener("change", done);
    picker.addEventListener("cancel", done);
    try {
      picker.showPicker();
    } catch {
      cleanup();
      activePicker = null;
      reject("unavailable");
    }
    return true;
  }
  function drop(raw) {
    if (!enabled || !raw.dataTransfer?.types.includes("Files")) return;
    const button = raw.target.closest?.("[data-gly-file-drop]"),
      e = origin(button);
    if (!e?.parts.has("picker") || !alive(e)) return;
    raw.preventDefault();
    if (button.disabled) return;
    if (raw.type === "drop") select(e, e.revision, null, raw.dataTransfer.files);
  }
  async function query(e, q) {
    const item = selections.get(e)?.get(q.serial);
    if (
      !enabled ||
      !item ||
      q.revision !== item.revision ||
      reads >= 4 ||
      q.length > 65536 ||
      q.offset + q.length > item.file.size
    )
      throw Error("file reference or range unavailable");
    if (q.kind === "file_preview_url") {
      item.url ||= URL.createObjectURL(item.file);
      return item.url;
    }
    reads++;
    try {
      const bytes = Array.from(
        new Uint8Array(await item.file.slice(q.offset, q.offset + q.length).arrayBuffer()),
      );
      if (selections.get(e)?.get(q.serial) !== item) throw Error("file reference retired");
      return { bytes };
    } finally {
      reads--;
    }
  }
  return { action, accept, clear, drop, query };
}
