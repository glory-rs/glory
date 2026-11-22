function createPlots({ origin, parent, main, alive, event }) {
  // Capture belongs to the chart subscription, independent of HTML pointer
  // capture used by sliders and drags. Local coordinates stay in CSS pixels.
  const slots = new Map();
  function owner(target) {
    for (let entry = origin(target); entry; entry = parent(entry)) {
      if (entry.interests.has("PlotPointer")) return alive(entry) ? entry : null;
    }
    return null;
  }
  function publish(slot, phase, x, y, box) {
    event(slot.owner, {
      kind: "plot_pointer",
      pointer_id: slot.id,
      device: slot.device,
      phase,
      x: x - box.x,
      y: y - box.y,
      width: box.width,
      height: box.height,
    });
  }
  function finish(slot, phase) {
    if (slot.sample) {
      const { x, y, box } = slot.sample;
      publish(slot, phase, x, y, box);
    }
    slots.delete(slot.id);
  }
  function refresh() {
    for (const slot of slots.values()) {
      const { owner, sample } = slot;
      if (!owner.interests.has("PlotPointer") || !alive(owner)) {
        finish(slot, slot.down ? "cancel" : "leave");
      } else if (sample) {
        const box = main(owner).getBoundingClientRect();
        if (["x", "y", "width", "height"].some((key) => box[key] !== sample.box[key])) {
          finish(slot, slot.down ? "cancel" : "leave");
        }
      }
    }
  }
  function receive(raw) {
    const id = raw.pointerId,
      device = raw.pointerType;
    if (!["mouse", "pen", "touch"].includes(device)) return;
    let slot = slots.get(id);
    if (slot && slot.device !== device) {
      finish(slot, "cancel");
      slot = null;
    }
    if (raw.type === "pointercancel" || raw.type === "lostpointercapture") {
      if (slot) finish(slot, "cancel");
      return;
    }
    if (raw.type === "pointerout") {
      if (slot && !slot.down && owner(raw.relatedTarget) !== slot.owner) finish(slot, "leave");
      return;
    }
    if (raw.type === "pointerdown" && raw.button !== 0) return;
    const target = slot?.down ? slot.owner : owner(raw.target);
    if (slot && target !== slot.owner) {
      finish(slot, "leave");
      slot = null;
    }
    if (!target || !alive(target)) return;
    if (!slot) {
      if (
        slots.size >= 32 ||
        raw.type === "pointerup" ||
        (device === "touch" && raw.type !== "pointerdown")
      )
        return;
      slot = { id, device, owner: target, down: false };
      slots.set(id, slot);
    }
    const phase = { pointerdown: "down", pointerup: "up", pointermove: "move" }[raw.type];
    const x = raw.clientX,
      y = raw.clientY,
      box = main(target).getBoundingClientRect();
    if (!phase || !box.width || !box.height) return;
    slot.sample = { x, y, box };
    if (phase === "down") slot.down = true;
    publish(slot, phase, x, y, box);
    if (phase === "up") {
      slot.down = false;
      if (device === "touch" || owner(raw.target) !== target) finish(slot, "leave");
    }
  }
  function cancelAll() {
    for (const slot of slots.values()) finish(slot, slot.down ? "cancel" : "leave");
  }
  for (const type of [
    "pointerdown",
    "pointermove",
    "pointerup",
    "pointerout",
    "pointercancel",
    "lostpointercapture",
  ]) {
    window.addEventListener(type, receive, true);
  }
  window.addEventListener("blur", cancelAll);
  window.addEventListener("resize", cancelAll);
  window.addEventListener("scroll", cancelAll, true);
  return { refresh };
}
