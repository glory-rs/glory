# Renderer Author Guide

Adapters implement `glory_render::Render` over finite semantic nodes, typed
mutations, events and queries. Start with [the trait](../crates/render/src/lib.rs),
the [reference Memory interpreter](../crates/render/src/memory.rs) and the
[semantic contract](semantic-ui-contract.md). The former Core command wire and
HTML Node abstraction have been removed.

Creation and patch preflight validate descriptor-local adapter limits before
application model or queue changes. Placement preflight reads the supplied
staged semantic Memory, never the adapter's committed host tree. Direct apply
must enforce the same limits and preserve an accepted prefix on later failure.

Nonempty successful apply calls strictly advance the applied watermark. Query
responses retain their accepted watermark even after later commits. Queries
are commit barriers; they do not certify that pixels were displayed.

Use the shared target and payload validation rules. Group has no physical box;
UTF-16 selection conversion, actual focus, native input and host layout belong
to the adapter. Foreign callbacks enqueue events; business callbacks run through
the owning UI loop.

HTML hosts share [glory-html](../crates/html/src/lib.rs). Current implementations
include [SSR](../crates/ssr/src/lib.rs), [Web](../crates/web/src/lib.rs) and
[Blitz](../crates/blitz/README.md). The optional render `conformance` feature
provides adapter probes; compilation or reference tests do not replace real
browser, window, OS input or accessibility acceptance.
