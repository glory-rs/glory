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
for ownership and error contracts. Controlled editors, SSR/hydration and native
mounting have not migrated to this new display frontend.
