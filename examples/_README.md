# Examples

Each subdirectory is its own crate (the workspace excludes `examples/`, see
the top-level `Cargo.toml`), so you'll typically build them with
`glory-cli` from inside the example directory rather than from the
workspace root. Examples cover progressively richer combinations of
features so newcomers can find the smallest one that demonstrates the
feature they care about.

## Index

| Example | Target | Features | What it shows |
|---|---|---|---|
| [`counter`](counter/) | wasm32 / CSR | `glory/web-csr` | Smallest possible app. Single `Cage<i32>`, `+1` / `-1` / clear, `input` two-way binding, `Bond` derived text. |
| [`forms-showcase`](forms-showcase/) | wasm32 / CSR | `glory/web-csr` | Form controls and state patterns: controlled/uncontrolled text, checkbox, radio, select, file input, readonly/disabled-style state. |
| [`counters`](counters/) | wasm32 / CSR | `glory/web-csr` | List of counters demonstrating `Each` keyed reordering (add / remove / update entries). |
| [`each-bench`](each-bench/) | native / SSR | `glory-core/web-ssr` | Standalone timing harness for `Each` reorder workloads (reverse, shuffle, head/tail insert, clear). |
| [`todomvc`](todomvc/) | wasm32 / CSR | `glory/web-csr`, `web-sys/Storage` | Full TodoMVC clone. `Each` over `Cage<Vec<TodoItem>>`, derived counts via `Bond`, `Switch` for active/completed filter views, `localStorage` persistence. |
| [`router-basic`](router-basic/) | wasm32 / CSR | `glory/routing`, `glory/web-csr` | Client-side routing only (no server). History API navigation, path params, nested routes. |
| [`ssr-simple-salvo`](ssr-simple-salvo/) | server + wasm32 | `glory/salvo` (server) + `glory/web-csr` (client) | Minimum SSR + hydration. Same component compiled twice, served by Salvo and hydrated in the browser. |
| [`ssr-modes-salvo`](ssr-modes-salvo/) | server + wasm32 | `glory/salvo` + `glory/web-csr`, `dep:gloo-net` | Different SSR strategies (in-order / async / streaming-ish) sharing one component tree. |
| [`hackernews-salvo`](hackernews-salvo/) | server + wasm32 | `glory/salvo` + `glory/server-fn` + `glory/web-csr` | Realistic fullstack app: HN clone with routing, `#[glory::server]` data functions (no hand-written API routes), hydrated nav. |
| [`todomvc-fullstack`](todomvc-fullstack/) | server + wasm32 | `glory/salvo` + `glory/server-fn` + `glory/web-csr` | TodoMVC-style fullstack flow: SSR first render, server functions for list/add/toggle/clear, request-context cookie display. |
| [`desktop-counter`](desktop-counter/) | native window | `glory-core/backend-command` + `glory-desktop/runtime` | The web counter widget code running in a wry desktop window via the command stream; IPC events round-trip into the reactive layer. |
| [`tailwind-salvo`](tailwind-salvo/) | server + wasm32 | `glory/salvo` + `glory/web-csr`, Tailwind CDN/build | SSR + Tailwind CSS integration. |
| [`_test-size`](_test-size/) | wasm32 / CSR | minimal | Tiny program kept around for tracking bundle size. Not interesting as a tutorial. |

## Pick-by-task cheat sheet

- **Just want to learn the builder API** → `counter`.
- **List rendering / `Each`** → `counters`, then `todomvc`.
- **Routing without a server** → `router-basic`.
- **SSR + hydration from scratch** → `ssr-simple-salvo`.
- **Real-world fullstack template (SSR + server functions)** → `hackernews-salvo`.
- **Server mutations / form-ish TodoMVC flow** → `todomvc-fullstack`.
- **Desktop app from the same widget code** → `desktop-counter`.
- **Two-way input / form patterns** → `forms-showcase`, then `counter`
  (minimal input binding) and `todomvc` (add / edit / commit).

## Running

CSR-only examples:

```sh
cd examples/counter
glory-cli serve
```

SSR + CSR examples need both features to compile into two separate
artefacts; consult the example's own `README.md` (if present) or its
`Cargo.toml` `[features]` block for the exact commands.

> `glory-cli` itself lives in [../crates/cli](../crates/cli) and is a
> separate binary; install it from this repo with
> `cargo install --path crates/cli`.

## Notes for AI agents

- The `examples/` folder is **excluded from the workspace** (`exclude =
  ["examples"]` in the root `Cargo.toml`). Examples have their own
  `Cargo.lock` and use the `path = "../../crates/..."` dependency form.
  Workspace-level `cargo check` / `cargo test` does NOT cover them; run
  the per-example commands when verifying changes that affect the public
  API.
- Don't add framework features by editing an example. New framework APIs
  belong in `crates/core` (or the relevant crate); examples should be
  consumers only.
- When adding a new example, also add a row above and update the
  pick-by-task cheat sheet.
