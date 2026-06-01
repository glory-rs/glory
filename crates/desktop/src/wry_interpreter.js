(() => {
  const nodes = new Map();
  const listeners = new Map();

  const byId = (id) => {
    const node = nodes.get(id);
    if (!node) throw new Error(`Glory WryRenderer missing node ${id}`);
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

  const emitEvent = (id, name, event) => {
    const payload = {
      id,
      name,
      event: {
        type: event.type,
        bubbles: event.bubbles,
        cancelable: event.cancelable,
        defaultPrevented: event.defaultPrevented,
      },
    };
    if (typeof window.__gloryWryEvent === "function") {
      window.__gloryWryEvent(payload);
    }
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify({ GloryWryEvent: payload }));
    }
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
    } else {
      throw new Error(`Glory WryRenderer unknown command ${type}`);
    }
  };
})();
