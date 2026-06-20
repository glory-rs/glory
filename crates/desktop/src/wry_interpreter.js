// Glory command-stream interpreter.
//
// Applies serialized renderer `Command` batches to the real DOM and posts
// serialized `EventData` payloads back to the host. The Rust reference
// implementation of these semantics is `glory_core::renderer::command_dom`;
// keep the two in sync (notably: Insert always *moves*, SetText clears
// children, node id 0 is the host root = document.body).
(() => {
  const nodes = new Map();
  const listeners = new Map();

  const byId = (id) => {
    if (id === 0) return document.body;
    const node = nodes.get(id);
    if (!node) throw new Error(`Glory interpreter: missing node ${id}`);
    return node;
  };

  const decode = (cmd) => {
    if (cmd.type) return [cmd.type, cmd[cmd.type] ?? cmd];
    const type = Object.keys(cmd)[0];
    return [type, cmd[type]];
  };

  const positionName = (position) => {
    if (typeof position === "string") return position;
    return Object.keys(position)[0];
  };

  const positionValue = (position) => {
    if (typeof position === "string") return undefined;
    return position[positionName(position)];
  };

  const classParts = (value) => String(value).split(/\s+/).filter(Boolean);

  const insertChild = (parent, child, position) => {
    const name = positionName(position);
    const value = positionValue(position);
    if (name === "Head") {
      parent.insertBefore(child, parent.firstChild);
    } else if (name === "Before") {
      parent.insertBefore(child, byId(value));
    } else if (name === "After") {
      const anchor = byId(value);
      parent.insertBefore(child, anchor.nextSibling);
    } else {
      parent.appendChild(child);
    }
  };

  const post = (message) => {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(message));
    }
  };

  // Builds the serializable `EventData` mirror of a DOM event. Form state
  // is snapshotted here because the host cannot read the live node
  // synchronously across IPC.
  const buildEventData = (id, name, event) => {
    const data = { name, node_id: id };
    if (typeof event.clientX === "number") {
      data.pointer = {
        client_x: event.clientX,
        client_y: event.clientY,
        button: typeof event.button === "number" ? event.button : 0,
        buttons: typeof event.buttons === "number" ? event.buttons : 0,
      };
    }
    if (typeof event.key === "string") {
      data.keyboard = {
        key: event.key,
        code: event.code || "",
        alt: !!event.altKey,
        ctrl: !!event.ctrlKey,
        shift: !!event.shiftKey,
        meta: !!event.metaKey,
      };
    }
    if (event.touches || event.changedTouches) {
      // Touch events: surface the primary point as PointerData and the
      // full point list through `extra.touches`.
      const points = [...(event.touches?.length ? event.touches : event.changedTouches || [])];
      if (points.length > 0) {
        data.pointer = data.pointer || {
          client_x: points[0].clientX,
          client_y: points[0].clientY,
          button: 0,
          buttons: 1,
        };
        data.extra = {
          touches: points.map((t) => ({ id: t.identifier, client_x: t.clientX, client_y: t.clientY })),
        };
      }
    }
    const target = event.target;
    if (target && ("value" in target || "checked" in target)) {
      data.target = {
        value: "value" in target ? String(target.value) : null,
        checked: typeof target.checked === "boolean" ? target.checked : null,
      };
    }
    return data;
  };

  const emitEvent = (id, name, event) => {
    const payload = buildEventData(id, name, event);
    if (typeof window.__gloryWryEvent === "function") {
      window.__gloryWryEvent(payload);
    }
    post({ GloryWryEvent: payload });
  };

  window.__gloryApplyWryCommand = (cmd) => {
    const [type, data] = decode(cmd);
    if (type === "Create") {
      nodes.set(data.id, document.createElement(data.name));
    } else if (type === "SetAttribute") {
      byId(data.id).setAttribute(data.name, data.value);
    } else if (type === "RemoveAttribute") {
      byId(data.id).removeAttribute(data.name);
    } else if (type === "SetProperty") {
      byId(data.id)[data.name] = data.value;
    } else if (type === "RemoveProperty") {
      byId(data.id)[data.name] = undefined;
      try {
        delete byId(data.id)[data.name];
      } catch (_) {}
    } else if (type === "AddClass") {
      byId(data.id).classList.add(...classParts(data.value));
    } else if (type === "RemoveClass") {
      byId(data.id).classList.remove(...classParts(data.value));
    } else if (type === "SetText") {
      byId(data.id).textContent = data.value;
    } else if (type === "SetHtml") {
      byId(data.id).innerHTML = data.value;
    } else if (type === "Insert") {
      insertChild(byId(data.parent), byId(data.child), data.position);
    } else if (type === "Remove") {
      const parent = byId(data.parent);
      const child = byId(data.child);
      if (child.parentNode === parent) parent.removeChild(child);
      else child.remove();
    } else if (type === "AttachEvent") {
      const key = `${data.id}:${data.name}`;
      if (listeners.has(key)) {
        byId(data.id).removeEventListener(data.name, listeners.get(key));
      }
      const handler = (event) => emitEvent(data.id, data.name, event);
      listeners.set(key, handler);
      byId(data.id).addEventListener(data.name, handler);
    } else if (type === "Query") {
      const respond = (result) => {
        const payload = { token: data.token, result };
        if (typeof window.__gloryWryQuery === "function") {
          window.__gloryWryQuery(payload);
        }
        post({ GloryWryQuery: payload });
      };
      let node;
      try {
        node = byId(data.id);
      } catch (_) {
        respond({ Err: "NodeGone" });
        return;
      }
      if (data.kind === "BoundingRect") {
        const rect = node.getBoundingClientRect();
        respond({ Ok: { Rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height } } });
      } else if (data.kind === "Value") {
        respond({ Ok: { Value: "value" in node ? String(node.value) : "" } });
      } else if (data.kind === "ScrollOffset") {
        respond({ Ok: { ScrollOffset: { x: node.scrollLeft, y: node.scrollTop } } });
      } else {
        respond({ Err: "Unsupported" });
      }
    } else if (type === "DetachEvent") {
      const key = `${data.id}:${data.name}`;
      if (listeners.has(key)) {
        byId(data.id).removeEventListener(data.name, listeners.get(key));
        listeners.delete(key);
      }
      // The widget owning this node is gone; release the DOM handle too so
      // long sessions don't accumulate dead nodes in the map.
      const node = nodes.get(data.id);
      if (node && !node.isConnected) nodes.delete(data.id);
    } else {
      throw new Error(`Glory interpreter: unknown command ${type}`);
    }
  };

  window.__gloryApplyWryBatch = (commands) => {
    for (const cmd of commands) {
      window.__gloryApplyWryCommand(cmd);
    }
  };

  // Runs host-supplied JavaScript and posts the JSON-serialized result back
  // as `{ GloryWryEval: { id, ok, value } }`. The source is wrapped in an
  // async function body so callers can `await` and return via a trailing
  // expression. Any thrown error or non-serializable result yields ok=false
  // with a human-readable message in `value`.
  window.__gloryWryEval = (id, source) => {
    const finish = (ok, value) => post({ GloryWryEval: { id, ok, value } });
    let result;
    try {
      const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
      const fn = new AsyncFunction(source);
      result = fn();
    } catch (err) {
      finish(false, String(err && err.stack ? err.stack : err));
      return;
    }
    Promise.resolve(result).then(
      (value) => {
        let json;
        try {
          json = JSON.stringify(value === undefined ? null : value);
          if (json === undefined) json = "null";
        } catch (err) {
          finish(false, String(err && err.stack ? err.stack : err));
          return;
        }
        finish(true, json);
      },
      (err) => finish(false, String(err && err.stack ? err.stack : err)),
    );
  };

  const announceReady = () => post({ GloryWryReady: true });
  if (document.readyState === "loading") {
    window.addEventListener("DOMContentLoaded", announceReady);
  } else {
    announceReady();
  }
})();
