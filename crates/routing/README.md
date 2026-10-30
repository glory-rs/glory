# Glory routing

`glory-routing` supplies shared routing for Web, native applications and custom
renderers. Matching and history contain no DOM, HTTP, Apace or OS types. The
optional `widgets` feature uses the existing Widget/Scope contract.

Enable `glory/routing` for the facade and choose `web`, `ssr` or `apace-shell`
separately. Routing forwards host integration only to already-selected adapters.

## Definitions and views

Declare paths once and bind local Widget factories separately. Routes is
immutable and Send + Sync; RouteViews and Router are local and Owner-managed.

```rust
use glory_routing::{RouteDef, RouteKey, Params, Routes};
const SHELL: RouteKey<()> = RouteKey::new("shell");
const USER: RouteKey<Params> = RouteKey::new("user");
let routes = Routes::builder()
    .root(RouteDef::layout(SHELL, "")
        .child(RouteDef::new(USER, "users/:id")))
    .finish()?;
let mut params = Params::default();
params.insert("id", 42);
assert_eq!(routes.target(USER, params)?.path(), "/users/42");
# Ok::<(), glory_routing::Error>(())
```

Implement RouteParams for application structs, or use the supplied `()` and
Params. Patterns support static segments, `:id`, final `:id?`, final `*rest`,
index routes and pathless layouts. Declaration paths are relative to their
parent. Static matches win; equivalent patterns are rejected. An invalid typed
parameter reports BadParams instead of silently falling into a catch-all.
Internal navigation cannot leave mount base. ctx.resolve resolves child/parent,
query and fragment references against the node's consumed path. is_active_path
supports exact or descendant matching without confusing adjacent path prefixes.

`RouteViews::new(routes.clone()).route(KEY, factory)?` binds factories and
`.view(router.handle())?` produces a Widget. Layouts mount `ctx.outlet()` once.
See the [shared example](../../examples/routing) for complete code.

Factories run once per mounted branch. Parameter changes retain the page and
parents. Read ctx.read_params inside bindings/Resources, or create an owned
ctx.params(owner) projection. key_by opts a branch into reconstruction when its
business key changes. Retired contexts return Gone and hold weak view references.
RouteViews and Router must share the same Routes allocation.

## Navigation and ownership

Create Router::new under the mounted application's Scope, with an explicit
History provider. Keep weak RouterHandles in callbacks. Push, replace, back,
forward, revalidate and same-URL push_entry return navigation results. The
parent Owner retains control until cleanup; temporary Router variables may drop.

MemoryHistory bounds its entries and knows exact traversal boundaries. Browser
traversal returns Accepted; popstate confirms the actual address. Repeated URLs
have distinct EntryKeys; replace retains its key. Preparation/provider errors
do not publish a new snapshot. Renderer errors after history commits follow
the App fault contract instead of attempting DOM/history rollback.

Guards return Allow, Redirect, Block or Denied. Async guards use push_async,
replace_async, observe_async or prepare_async on an explicit local executor.
Superseding navigation disposes the pending guard Owner and wakes its waiter.
Sync APIs report AsyncRequired when async guards are installed. Redirects are
bounded and loop checked. Reentrant sync navigation returns Busy; applications
can queue another intent for their next host turn.
Navigation in a pure computation returns WriteDuringCompute before invoking
guards, cancelling pending work or changing history. Use an event/effect or the
host's observation boundary to navigate.

## Web

glory::web::routing::launch_with owns BrowserHistory listeners with its WebApp.
hydrate_with validates a Bootstrap and adopts the server's initial route before
processing a newer browser address. Use the same page factories on both sides.

routing::link(router, location, on_result) composes the portable Link and keeps
a real href. Raw anchors opt in with data-glory-route. Modifier clicks,
downloads, external links and links outside mount base keep browser behavior.
Third-party direct pushState/replaceState calls need BrowserLease::notify_location.

Window scrolling is bounded, keyed by EntryKey and restored after the UI turn.
CSR handles the initial URL fragment after mounting; hydration preserves scroll
and dirty editors. Anchor lookup retries for at most eight RAFs and is bound to
the committed navigation revision; later input/navigation cancels restoration.
Arbitrary nested scroll containers and virtual-list business-anchor registration
remain future work.

One top-level provider owns a Window. Plain-object history state retains other
fields; primitive/array state is rejected rather than overwritten. Custom state
formats can use an application History implementation. Dropping the lease removes
listeners and restores scrollRestoration. Already-observed browser traversal is
not cancellable: blocked targets display an error/fallback state at that address;
the provider never guesses an opposite-direction history jump.

## Native and other renderers

native::NativeNavigation works with any renderer. Deliver Back/Forward/Open at
an application observation boundary. Boundary leaves root dismissal/exit to the
application. deep_link maps an OS-delivered URL through an explicit scheme/host
without registering protocols or opening I/O.

For interactive return, call begin_back, then cancel_back or finish_back.
Preparation changes neither UI nor history; another navigation invalidates it.
With async guards, prepare via RouterHandle::prepare_async before starting the
visual gesture.

Apace WindowApp::attach_router binds a Router from the window's own Runtime.
BrowserBack/BrowserForward keys enqueue navigation when an entry is available;
queue_navigation accepts other platform inputs. Observe dispatches them, and
take_navigation_result retrieves the last bounded result. The shared example
has a native desktop/iOS entry point.

Android/iOS scheme registration, OS lifecycle delivery and a UIKit interactive
gesture bridge remain platform work; device acceptance is not claimed. Changing
renderer does not change Routes, Router or MemoryHistory. A replacement renderer
uses the existing Widget lifetime/mounting contract and supplies its own optional
scrolling, focus and visual transition services.

## SSR and HTTP

RequestLocation::parse(target)?.resolve(&routes) produces a transport-neutral
status, redirect and location. resolve_with accepts explicit sync request guards.
glory-salvo/routing supplies render_response_with with real 200/400/404/3xx,
denial and HEAD behavior. Redirect/denial never invokes page factories.
RequestHistory is immutable; every request owns its local UI state.

Bootstrap includes the application codec version, declaration identity, location
and branch IDs. Its bounded ASCII payload contains no '<', credentials, Scope
IDs or arbitrary Rust data. A mismatch returns HydrationMismatch and preserves
SSR DOM. HTTP supplies no fragment; publish the browser hash after adoption.
The schema identity is a consistency check, not an authentication MAC.

Route Widgets can use the existing SSR/resource/stream hosts. Dedicated stream
bootstrap/End integration, async request guard helpers, Routable derive, parallel
named outlets, page caches, prefetch, code splitting, enhanced GET forms and head
management are not included. The [architecture report](../../docs/routing-architecture-2026-10-30.md)
records this broader roadmap; this README describes the implemented API.
