# Glory Docs

Start here when evaluating or extending Glory:

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
