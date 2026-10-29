# Display API / XML example

Enable `glory`'s opt-in `dsl` feature and import `glory::display::prelude::*`.
This example deliberately names the dependency `g` to verify macro hygiene.

The default build uses `view!`; `--features hand` uses the equivalent static
Template plus configuration chain. Both display a counter, a conditional read,
one-time text, dynamic class, and an independently owned Counter component.
The acceptance-only exports exercise temporary rows/components and cleanup.

```powershell
./scripts/build-display-web.ps1
./scripts/build-display-web.ps1 -Hand
node examples/display-web/acceptance.cjs my-run
```

Builds, temporary files and evidence default to `D:/BuildCache/glory-dsl-20261028`.
The Node driver owns its local HTTP server and browser and closes both.
Open the built `dsl/site` with a local HTTP server to use it interactively.

`App::render` installs the root once; later writes to Cage update its fields.
For lists, `view(...).mount(app, scope)` creates an unplaced instance; place it
in an Outlet and keep its Scope. Dropping that Scope retires it. Dropping only
the Mounted handle does not dispose a live Scope.

For a normal application without acceptance glue, use
[src/bin/counter.rs](src/bin/counter.rs): `#[component]`, `App::launch`, `<Each>`,
`<Show>`, component children, `field!` and typed class toggles.

```powershell
./scripts/build-display-web.ps1 -Bin counter
```

The profile supports div/span/button/a/h1/table/tbody/tr/td, text, finite static
attributes, class/title/aria-label bindings, boolean class toggles and click.
`<Content value={...}/>` accepts a view, optional view or sequence; fragments
and component children retain real DOM ranges without a visible wrapper.
`Show` retires its branch when hidden and creates fresh state on reentry.
`Each` retains keyed row state across movement and same-key value replacement.
See the [API guide](../../docs/v2-display-keyed-content-guide-2026-10-28.md)
for ownership and error contracts. Mixed trees also work with standard
`web::launch` / `web::hydrate`, `ssr::snapshot` and the native Widget host.
Controlled editors use Widget tags within the same DSL.

The same `Widget` contract now backs XML component tags, Sapid and function
widgets; see the [unification guide](../../docs/v2-widget-dsl-unification-2026-10-29.md).
`unified-acceptance.cjs` tests the mixed HTML/Sapid fixture, including keyed
focus preservation, a self-dismissing branch and owned cleanup. Point
`GLORY_WIDGET_SITE` at the built fixture's `site` directory when using a custom
build root.

`src/mixed.rs` is shared by the actual SSR generator, browser CSR/adoption and
Sapid's `examples/native/tests/mixed_dsl.rs`. Run
`../../scripts/build-mixed-display.ps1 -Verify` from this directory to build and
exercise it. Existing `fn button(...) -> impl Widget` factories use
`<{button(...)}/>`; concrete builders also accept setters and children, e.g.
`<{sapid::card()}><sapid::Button label="Save"/></>`.

`../../scripts/build-mixed-stream.ps1 -Verify` exercises a real gated Rust HTTP
stream and final WASM adoption using `src/stream_mixed.rs`. It also checks dirty
editors/focus, keyed tables, template suspension, declaration retirement, invalid
slot payloads and pending-response cancellation. The Node driver owns and closes
its HTTP server, Chrome process and server children; evidence goes to the D-drive
`mixed-stream-site` directory.
