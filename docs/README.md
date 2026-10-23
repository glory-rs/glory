# Glory Docs

Start here when evaluating or extending Glory:

- [Semantic UI architecture report](semantic-ui-rendering-architecture-report-2026-10-21.md):
  breaking rewrite decisions, short component names, crate boundaries, CSS/Blitz/Taffy,
  toolkit tradeoffs, concrete platform/crate organization and implementation priorities.
  Start with §0 for the adopted names and decisions; §13.4 defines legacy retirement.
- [Semantic UI implementation evidence](semantic-ui-implementation.md):
  delivered behavior, reproducible checks and remaining release requirements.
- [Windowed-list baseline](semantic-ui-list-baseline-2026-10-21.md):
  bounded large-list behavior and the scope of browser/native evidence.
- [Semantic component contract audit](semantic-ui-contract.md):
  eight-component content, event/query/action targets, adapter conformance and
  explicit remaining R0 work.
- [Current browser framework comparison](performance.md): semantic Glory/Leptos/Dioxus
  full-row workloads, independent builds, actual results and explicit historical boundaries.
- [Bulk creation and Dioxus design](bulk-create-dioxus-2026-10-24.md): fixed DOM creation bridge,
  independent paired/cross-framework results, rejected experiments, full regression and static-plan priorities.
- [HTML mapping and DOM creation](performance.md#html-mapping-and-dom-creation--2026-10-23):
  finite native prototypes, inline Part handles, independent paired timings and full regression.
- [Scope cleanup and sibling unlink](performance.md#scope-cleanup-and-sibling-unlink--2026-10-23):
  bounded front retirement, fused indices, controlled paired results and full regression.
- [Owner/Binding retirement follow-up](performance.md#owner-and-binding-retirement-follow-up--2026-10-23):
  unified live Owner interests, paired timings, remaining costs and complete regression.
- [DOM retirement follow-up](performance.md#dom-retirement-follow-up--2026-10-23):
  parent-table scan removal, paired timings, final-source verification and full regression.
- [Bulk DOM optimization](bulk-dom-optimization-2026-10-23.md): Owner/Binding, metadata,
  comment/range costs, paired timings, identity/pixel checks and the existing anchor failure.
- [Fractional list anchor correction](anchor-residual-fix-2026-10-23.md): 0.375 px residual
  fix, 64/1024 real-height caches, settled pixel/node identity and complete regression evidence.
- [Engine forks](engine-forks.md): immutable glory-rs Blitz/Taffy Git dependencies,
  source provenance, fork tests and the prohibition on vendoring.

The documents above describe the active rewrite. The guides below retain
material from the previous architecture; check the implementation evidence before
using their API or platform-support claims.

- [Project completion and improvement report](project-improvement-report-2026-10-13.md):
  assessment dated 2026-10-13, verified gaps, priorities, and release criteria.
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
