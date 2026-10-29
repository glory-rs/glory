# Glory Docs

Start here when evaluating or extending Glory:

- [Routing implementation and acceptance](routing-implementation-2026-10-30.md):
  portable routing, retained Widget branches, Web/native/SSR integration,
  default dependency boundaries and verified versus deferred platform behavior.

- [Routing architecture proposal](routing-architecture-2026-10-30.md):
  legacy routing behavior, current Widget/Owner integration, typed routes,
  nested layout retention, history providers, SSR and staged acceptance.
  The broader roadmap; [current APIs and boundaries](../crates/routing/README.md)
  describe the implemented core and Web/native/SSR integrations.

- [Keyed API implementation and complete benchmark evidence](v2-display-completion-results-2026-10-29.md):
  delivered interfaces, full Rust/browser checks, alternating self-suite measurements,
  the separate standard table suite and remaining performance/capability limits.

- [Keyed display API and XML guide](v2-display-keyed-content-guide-2026-10-28.md):
  implemented Each/Show/Content, authoritative keyed values and field projections,
  component children/macros, boolean classes, display prelude and a normal application example.

- [v2 display API, DSL and performance completion plan](v2-display-completion-plan-2026-10-28.md):
  staged keyed data ownership, public lists and content ranges, XML control flow and children,
  concise canonical application, measured performance gates and component ergonomics.
  Implementation status and the original measurable exit conditions are kept separately.

- [Canonical benchmark migration to the display DSL](benchmark-display-dsl-migration-2026-10-28.md):
  Glory's default XML entry, handwritten feature comparison, removal of the duplicate
  html-template application, preserved probes and fresh legacy/DSL/Dioxus measurements.

- [Handwritten display API and XML DSL](v2-view-api-and-xml-dsl-2026-10-28.md):
  named Field/Click/Outlet declarations, value/handle/computed binding semantics,
  shared XML extraction, explicit component Props, opt-in macros and equivalent browser fixtures.

- [v2 sparse HTML template prototype and measurements](v2-html-template-prototype-results-2026-10-28.md):
  direct HTML instances, no per-element semantic Memory/Entry, same-graph and grouped-binding
  experiments, four alternating rounds against current v2 and Dioxus, fault/GC acceptance and scope limits.

- [v2 HTML display templates and higher-level widgets](v2-html-template-runtime-architecture-2026-10-28.md):
  user-directed HTML/CSS display contract, raw elements versus components,
  pinned Dioxus 0.7.10 implementation comparison, sparse instance records and migration criteria.
  The first limited browser runtime and XML frontend are implemented; platform migration remains.

- [v2 Plan submission preparation and journal optimization](v2-plan-submit-cost-optimization-2026-10-28.md):
  same-machine A/B against remote v2, compact host object tables and contiguous prefix journals;
  clear/10k Script gains, unchanged 1k results, wall regressions, and Blink layout GC evidence.

- [Glory v2 Plan host batch prototype](v2-plan-host-batch-prototype-2026-10-28.md):
  bounded JS creation/initialization/attachment/retirement, prefix confirmation,
  input-state barriers, independent A/B and Dioxus comparison, and baseline gate failures.

- [Glory v2 DSL and browser batching investigation](v2-dsl-and-host-batching-investigation-2026-10-28.md):
  XML-like compilation, data projections, Dioxus 0.7.10 source comparison,
  whole-batch JS execution, prefix/error boundaries and separate validation tracks.

- [Glory and Dioxus feature gaps](dioxus-feature-gap-analysis-2026-10-26.md):
  current source comparison, platform delivery boundaries and implementation priorities;
  SSR framework integration remains Salvo-only.

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
- [Latest stable framework comparison](framework-benchmark-latest-stable-2026-10-25.md):
  independent builds of current Glory, Leptos 0.8.22 and Dioxus 0.7.10, identity and pixel checks.
- [Why Dioxus is fast in bulk workloads](dioxus-performance-analysis-2026-10-25.md):
  current source paths, bridge/DOM counts, reactive granularity, post-GC host references and Glory priorities.
- [Persistent portfolio update scenarios](portfolio-scenarios-benchmark-2026-10-25.md):
  Glory versus coarse and row-local Dioxus, two scales, independent builds and full data/identity/pixel evidence.
- [Plan preparation and batch identity optimization](plan-batch-preparation-optimization-2026-10-25.md):
  shared immutable facts, ordered-slot checks, independent paired measurements, dense-update tradeoffs and full regression.
- [Plan physical parts and bridge batching](plan-bridge-batching-2026-10-26.md):
  lazy original-node handles, partial retirement and independent bridge/GC measurements.
- [Typed Plan host goals](plan-host-goals-2026-10-26.md):
  finite field/placement/removal calls, controlled-input barriers and memory-growth recovery.
- [Render pipeline architecture and implementation](render-pipeline-implementation-2026-10-26.md):
  authoritative retirement inventories, shared Entry initialization, bounded control-change summaries,
  full regression and paired measurements with explicit environmental limits.
- [Cross-mutation field submission experiment](field-submit-plan-2026-10-26.md):
  reverted after paired measurements; journal/prefix validation and experimental results remain documented.
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
- [API guide](api-guide.md): current semantic entry points, ownership and hosts.
- [Project-local CLI](../tools/cli/README.md): library-based tooling, shared
  application lockfile, bin launchers, build/preview and version diagnostics.
- [Renderer author guide](renderer-author-guide.md): semantic mutations,
  preflight, query watermarks and adapter conformance.
- [Native clipboard](native-clipboard.md): explicit desktop service, editor
  commands, bounded admission and platform acceptance boundaries.
- [Release readiness](release-readiness.md): current workspace and platform checks.

Dated architecture assessments and performance reports are historical evidence,
not current API guides. Removed source links in those reports point to pinned
Git history. The [2026-10-13 assessment](project-improvement-report-2026-10-13.md)
records the pre-rewrite baseline; the implementation ledger records the current
state and remaining platform work.
