# An experimental rust web front framework


Glory is a experimental rust web front framework modified from leptos.

## 🎯 Features

- Without hoops and props.
- Without virtual DOM.
- SSR and hydrate.

Your can find examples in [examples](./examples/) folder. 

Online example: [http://glory.rs:8000](http://glory.rs:8000).

## 🧭 How Glory compares

| | **Glory** | Leptos | Dioxus | Sycamore |
|---|---|---|---|---|
| Component surface | Rust builder pattern (`div().class().on(click, ...)`)| `view!` macro (RSX-like) | `rsx!` macro | `view!` macro |
| State primitive | `Cage<T>` (mutable) / `Bond<T>` (derived) / `Lotus<T>` (read-only union) | `RwSignal` / `Memo` | `Signal<T>` (Copy, generational-box) / `Memo` | `Signal` / `Memo` |
| `Copy` state handles | not yet (planned via generational-box) | depends on signal flavour | yes | yes |
| Update model | Fine-grained subscription per view; no VDOM | Fine-grained, no VDOM | VirtualDom + `WriteMutations` per renderer | Fine-grained, no VDOM |
| Targets today | Browser (CSR) + SSR (HTML / Salvo) | Browser + SSR + Axum/Actix + hydrate | Web / Desktop (webview) / Native (Blitz) / SSR / LiveView / Fullstack | Browser + SSR + hydrate |
| Multi-platform path | Renderer abstraction is on the roadmap; today CSR / SSR share the builder API but the `Node` type is split by `cfg` | SSR / hydrate / Axum first-class | One VDOM, many renderer crates | DOM and SSR backends |
| DSL / macro | Generated tag factories (`generate_tags!`), no JSX-like macro on purpose | Yes (`view!`) | Yes (`rsx!`) | Yes (`view!`) |

**When does Glory make sense?**
- You want a small, readable framework you can fully understand top-to-bottom.
- You prefer plain Rust (builder API) over macros and a `view!` DSL.
- You need CSR + Salvo-flavoured SSR today; multi-platform output is acceptable as a roadmap item rather than a shipping feature.

**When to pick something else?**
- You need a desktop or native binary today → Dioxus.
- You want the most polished SSR / streaming / Axum story right now → Leptos.
- You want minimal framework surface for a static-feeling SPA → Sycamore.

For a deeper architectural comparison (especially against Dioxus) and the
prioritised improvement backlog, see [`_report.md`](_report.md) and
[`_todos.md`](_todos.md). Contributor conventions live in
[`AGENTS.md`](AGENTS.md).

## 🩸 Contributing

Contributions are absolutely, positively welcome and encouraged! Contributions come in many forms. You could:

- Submit a feature request or bug report as an issue;
- Comment on issues that require feedback;
- Contribute code via pull requests;
- Publish Glory-related technical articles on blogs or technical platforms。

All pull requests are code reviewed and tested by the CI. Note that unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Salvo by you shall be dual licensed under the MIT License, without any additional terms or conditions.

## ☕ Supporters

Glory is an open source project. If you want to support Glory, you can ☕ [**buy a coffee here**](https://ko-fi.com/chrislearn).

## ⚠️ License

Glory is licensed under:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
