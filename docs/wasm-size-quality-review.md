# Code-quality review of the WASM size candidates

Reviewed on 2026-10-26 after the [minimal-app attribution](wasm-size-attribution.md).
The acceptance criterion is an independent improvement in correctness,
maintainability or a required capability boundary. A smaller initial download
alone does not justify an architectural change.

The later [benchmark attribution and lowering cleanup](framework-benchmark-size-analysis-2026-10-26.md)
identified duplicated role/range/direction lowering within the existing HTML
boundary. It consolidates that source logic without implementing either of
the architectural changes rejected below; its measured benchmark Brotli size does not improve.

## Decisions

| Candidate | Independent code-quality evidence | Decision |
|---|---|---|
| Sever the common renderer's references to optional host capabilities | Domain implementations are already separate modules; a smaller artifact would require an additional selection/registration boundary | Do not implement a new capability-feature matrix or plugin registry |
| Merge internal collections to reduce generic instantiations | The measured bodies are compiler specializations of the standard collection implementation, not duplicated handwritten algorithms | Do not erase types, unify unrelated payloads, or replace the collections solely for code size |

## Shared renderer

`browser.rs` already delegates to `rich`, `files`, `clipboard`, `layers`,
`sheet`, `collapse`, `images`, `translation` and other private modules. This
provides source-level separation and a place for each domain's behavior. The
common renderer still coordinates their mutation and observation phases.

Those references are not sufficient evidence of an incorrect dependency:
the renderer implements one finite semantic target. `Session`'s
`Box<dyn Render>` is the actual technology boundary, shared by browser, SSR,
reference and native consumers. An initially empty Group does not imply an
application with a permanently Group-only renderer.

Removing references for size would require more than moving functions into
files. Cargo gates would add supported/rejected configuration combinations
across validation, mapping, browser behavior, SSR and hydration. A runtime
registration mechanism would add registration, ordering and teardown rules,
while runtime registration alone would not guarantee linker elimination.
Neither is justified here by a new host requirement or a demonstrated ownership
or correctness problem.

Keep the explicit coordination and existing module boundaries. Revisit a
boundary when there is a concrete independent use case, a reproducible ordering
or lifecycle defect, or actual duplicated domain logic. Function-body size
alone is not such evidence. This review does not claim the current renderer
cannot be improved; it rejects this proposed reason for restructuring it.

## Generic collections

The attribution found 548 BTree implementation bodies. Rust specializes the
same library implementation for distinct keys and payloads. There are not 548
source implementations for maintainers to keep consistent.

The current maps and sets represent different invariants:

- UI Owner records distinguish node ownership, subscriptions and queries.
- Query/target indices support targeted cancellation and cleanup without
  scanning unrelated entries.
- HTML attributes and typed properties preserve deterministic serialization
  and lowering.
- Runtime Owner/node/task membership and field-path indices have their own
  ordering, generational identities and destruction/borrow constraints.

Combining these into a common integer key and an erased or enum payload could
share generated instructions. It would also move checks from Rust's type system
into runtime code and couple unrelated lifetimes. An extra payload enum or
indirection can increase per-entry memory and branching. Replacing BTree maps
with HashMaps or vectors also changes ordering and operation costs. The size
attribution establishes none of those changes as a net design improvement.

Keep the standard typed collections. Consolidate a registry only when its
ownership and operations are actually the same, or when repeated handwritten
invariant-maintenance code has been identified. Do not introduce a custom
collection implementation merely to reduce monomorphization.

## Outcome

No production Rust, public features, runtime behavior or collection layout was
changed for these two candidates. The existing measurement tools and baselines
remain useful for detecting accidental retention, but their byte counts are
not an architectural quality score. No new size baseline is needed for this
documentation-only decision.
