# Glory Docs

Start here when evaluating or extending Glory:

- [API guide](api-guide.md): builder API, reactivity, widgets, SSR, server
  functions, styling, and runnable example commands.
- [Fullstack notes](fullstack.md): server functions, request context, server
  state/cache, preloaded state, streaming helpers, and multipart helpers.
- [Server function adapter recipes](serverfn-adapter-recipes.md): Salvo, Axum,
  and Actix routes for streaming, SSE, uploads, and cookie sessions.
- [Server function native extractor decision](serverfn-extractor-decision.md):
  why `#[server]` remains adapter-neutral and native extractors stay in custom
  framework routes for now.
- [Styling notes](styling.md): scoped styles with `web::scoped_css`.
- [HTML/event coverage audit](html-event-coverage-audit.md): current parity
  against local Dioxus `packages/html`, including SVG/MathML and event gaps.
- [Desktop guide](desktop.md): desktop runtime configuration, protocol, menu,
  multi-window, and remote server functions.
- [Platform APIs](platform-apis.md): window configuration, menus, file-dialog
  integration points, node queries, mobile viewport lifecycle, and event payloads.
- [Mobile device validation](mobile-validation.md): Android/iOS runtime smoke
  procedure and `scripts/mobile-device-smoke.ps1` usage.
- [Mobile crate architecture decision](mobile-architecture-decision.md): why
  mobile stays as transparent host templates for now.
- [Mobile native bridge recipes](mobile-native-recipes.md): permissions,
  camera/gallery, and share-sheet integration patterns.
- [Devtools protocol](devtools.md): reactive graph snapshots, command queue
  snapshots, protocol messages, and static inspector panel rendering.
- [Reactivity copy handle decision](reactivity-copy-handles.md): current
  `Cage<T>`/`Bond<T>` handle tradeoffs and revisit triggers.
- [LiveView protocol](liveview.md): server-held command-stream session model
  and WebSocket message shapes.
- [Third-party renderer guide](renderer-author-guide.md): command stream,
  event, query, lifecycle, batching, and conformance rules for custom renderers.
- [Performance notes](performance.md): wasm size, Criterion benches, and local
  benchmark report scripts.
- [Wasm split evaluation](wasm-split-evaluation.md): current size thresholds
  and why split-wasm is deferred.
- [Release readiness](release-readiness.md): feature matrix and required checks
  before cutting a release.
- [Multi-platform rendering plan](multi-platform-rendering-plan.md): current
  renderer status and next platform stages.
