# realtime-notify

Realtime notifications over WebSocket: the client connects with the serverfn `WebSocketEndpoint` / `use_websocket` hook and renders server pushes; the Salvo server pushes a notification every second.

```bash
# server (pushes notifications, serves /ws/notify)
cargo run --no-default-features --features web-ssr
# client bundle check
cargo check --no-default-features --features web-csr
```
