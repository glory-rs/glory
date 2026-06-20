// Glory LiveView WebSocket client.
//
// Requires the command-stream interpreter to be loaded first. The interpreter
// provides window.__gloryApplyWryBatch and calls __gloryWryEvent /
// __gloryWryQuery when browser events or node query answers need to go back to
// the server.
(() => {
  const DEFAULT_PATH = "/__glory/liveview";

  const wsUrl = (path) => {
    if (/^wss?:\/\//.test(path)) return path;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${window.location.host}${path}`;
  };

  window.__gloryLiveViewConnect = (path = DEFAULT_PATH, options = {}) => {
    let socket = null;
    let closed = false;
    let reconnectMs = options.reconnectMs || 250;
    const maxReconnectMs = options.maxReconnectMs || 5000;

    const send = (message) => {
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(message));
      }
    };

    const apply = (commands) => {
      if (!commands || commands.length === 0) return;
      if (typeof window.__gloryApplyWryBatch !== "function") {
        throw new Error("Glory LiveView: command interpreter is not installed");
      }
      window.__gloryApplyWryBatch(commands);
    };

    const handle = (message) => {
      const payload = message.payload || {};
      if (message.type === "hello") {
        send({ type: "hello", payload: { protocol_version: payload.protocol_version || 1 } });
      } else if (message.type === "mount" || message.type === "patch") {
        apply(payload.commands);
      } else if (message.type === "ping") {
        send({ type: "pong" });
      } else if (message.type === "close") {
        closed = true;
        if (socket) socket.close();
      } else if (message.type === "error") {
        console.error("Glory LiveView:", payload.message || message);
      }
    };

    const connect = () => {
      socket = new WebSocket(wsUrl(path));
      socket.addEventListener("open", () => {
        reconnectMs = options.reconnectMs || 250;
        send({ type: "hello", payload: { protocol_version: 1 } });
      });
      socket.addEventListener("message", (event) => {
        try {
          handle(JSON.parse(event.data));
        } catch (err) {
          console.error("Glory LiveView: bad message", err);
        }
      });
      socket.addEventListener("close", () => {
        if (closed) return;
        const delay = reconnectMs;
        reconnectMs = Math.min(reconnectMs * 2, maxReconnectMs);
        window.setTimeout(connect, delay);
      });
    };

    window.__gloryWryEvent = (event) => send({ type: "event", payload: event });
    window.__gloryWryQuery = (query) => send({ type: "query", payload: query });
    connect();

    return {
      close() {
        closed = true;
        if (socket) socket.close();
      },
      send,
    };
  };
})();
