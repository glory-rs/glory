# LiveView Salvo Example

Server-held command-stream rendering over a WebSocket. The Salvo app serves an
HTML shell at `/` and mounts the LiveView socket at `/__glory/liveview`.

```sh
cargo run --manifest-path examples/liveview-salvo/Cargo.toml
```

Open <http://127.0.0.1:8080/>.
