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
    let resumeToken = options.resumeToken || null;
    // Keep the server's idle timer alive. Without this the connection is
    // dropped after `idle_timeout_secs` of no interaction (5 minutes by
    // default) even though the socket is open, and the client is told to
    // close, leaving a dead page with no explanation.
    const heartbeatMs = options.heartbeatMs || 30000;
    let heartbeat = null;
    // Events produced while the socket is down are queued rather than dropped,
    // so a click during the reconnect window is not silently lost.
    const pending = [];
    const maxPending = options.maxPendingEvents || 64;

    const stopHeartbeat = () => {
      if (heartbeat !== null) {
        window.clearInterval(heartbeat);
        heartbeat = null;
      }
    };

    const startHeartbeat = () => {
      stopHeartbeat();
      if (heartbeatMs > 0) {
        heartbeat = window.setInterval(() => send({ type: "ping" }), heartbeatMs);
      }
    };

    const send = (message) => {
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(message));
        return true;
      }
      return false;
    };

    const sendOrQueue = (message) => {
      if (send(message)) return;
      if (closed) return;
      if (pending.length >= maxPending) pending.shift();
      pending.push(message);
    };

    const flushPending = () => {
      while (pending.length > 0 && socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(pending.shift()));
      }
    };

    // A fresh session replays the whole tree from node 0. The page may still
    // be showing the previous session's DOM (an expired resume token, or a
    // server restart), and appending onto it would leave two copies of the UI
    // with colliding node ids.
    const resetTree = () => {
      if (typeof window.__gloryResetWryTree === "function") {
        window.__gloryResetWryTree();
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
        const hello = { protocol_version: payload.protocol_version || 1 };
        if (resumeToken) hello.resume_token = resumeToken;
        send({ type: "hello", payload: hello });
      } else if (message.type === "session" || message.type === "resumed") {
        if (message.type === "session") resetTree();
        resumeToken = payload.token || null;
        // Only a completed handshake proves the server is healthy, so the
        // backoff resets here rather than on `open` — otherwise a server that
        // rejects every handshake is retried in a tight loop forever.
        reconnectMs = options.reconnectMs || 250;
        startHeartbeat();
        flushPending();
        if (typeof options.onSessionToken === "function") {
          options.onSessionToken(resumeToken);
        }
      } else if (message.type === "mount" || message.type === "patch") {
        apply(payload.commands);
      } else if (message.type === "ping") {
        send({ type: "pong" });
      } else if (message.type === "close") {
        // The server dropped this session (expired, or past its max lifetime).
        // Forget the token and reconnect for a fresh one instead of leaving a
        // dead page behind.
        resumeToken = null;
        stopHeartbeat();
        if (socket) socket.close();
      } else if (message.type === "error") {
        console.error("Glory LiveView:", payload.message || message);
      }
    };

    const connect = () => {
      socket = new WebSocket(wsUrl(path));
      socket.addEventListener("open", () => {
        const payload = { protocol_version: 1 };
        if (resumeToken) payload.resume_token = resumeToken;
        send({ type: "hello", payload });
      });
      socket.addEventListener("message", (event) => {
        try {
          handle(JSON.parse(event.data));
        } catch (err) {
          console.error("Glory LiveView: bad message", err);
        }
      });
      socket.addEventListener("close", () => {
        stopHeartbeat();
        if (closed) return;
        const delay = reconnectMs;
        reconnectMs = Math.min(reconnectMs * 2, maxReconnectMs);
        window.setTimeout(connect, delay);
      });
    };

    window.__gloryWryEvent = (event) => sendOrQueue({ type: "event", payload: event });
    window.__gloryWryQuery = (query) => sendOrQueue({ type: "query", payload: query });
    connect();

    return {
      close() {
        closed = true;
        stopHeartbeat();
        // Tell the server so it can release the session immediately instead of
        // holding the widget tree until the idle timeout.
        send({ type: "close" });
        if (socket) socket.close();
      },
      resumeToken() {
        return resumeToken;
      },
      send,
    };
  };
})();
