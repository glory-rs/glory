(() => {
  const nodes = new Map();
  const byId = (id) => {
    const node = nodes.get(id);
    if (!node) throw new Error(`Glory WryRenderer missing node ${id}`);
    return node;
  };
  window.__gloryApplyWryCommand = (cmd) => {
    switch (cmd.type ?? Object.keys(cmd)[0]) {
      default: {
        const value = cmd[cmd.type] ?? cmd;
        if (cmd.Create || cmd.type === "Create") {
          const data = cmd.Create ?? value;
          nodes.set(data.id, document.createElement(data.name));
        } else if (cmd.SetAttribute || cmd.type === "SetAttribute") {
          const data = cmd.SetAttribute ?? value;
          byId(data.id).setAttribute(data.name, data.value);
        } else if (cmd.SetText || cmd.type === "SetText") {
          const data = cmd.SetText ?? value;
          byId(data.id).textContent = data.value;
        } else if (cmd.Insert || cmd.type === "Insert") {
          const data = cmd.Insert ?? value;
          byId(data.parent).appendChild(byId(data.child));
        }
      }
    }
  };
})();
