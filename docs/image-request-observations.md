# Image request observations

`ImageProps` stores `source` and a monotonic `revision`. `Patch::Source` now takes
`{ source: Change<String>, revision: u64 }`. A smaller revision is rejected;
an equal revision requires an equal canonical source. Keep ignores its supplied
revision. Clear uses an empty source and a new revision. Shared text
canonicalization makes HTML-equivalent NUL/CR inputs compare consistently.
Descriptor construction, prepared live patches, rollback and direct adapter
application enforce the same rule. This is a breaking Rust API change.

Glory's Image builder allocates a bound source revision from the live node,
including after explicit writes. `Scope::set_image_source` also allocates that
revision and can explicitly retry the same URL. Higher-revision same-URL writes
reassert the physical src attribute. The HTML full-shape oracle covers that
barrier and canonical/default resets. Existing classic stream format v3 already
carries Shape.revision and revision/src edits, so its wire shape is unchanged.

`Event::ImageStatus { revision, status: ImageStatus::{Loaded,Failed} }` belongs
only to Image. Future revisions fail validation; old revisions are discarded
before application callbacks. Events never create identities or revive an Owner.
SSR records valid interests without inventing any image outcome. Hydration starts
a new semantic identity/revision lifetime and observes the adopted real image.

Web coalesces pending source observations per live image. Source writes immediately
withdraw the previous controller's acceptance; the host read phase starts one
`HTMLImageElement.decode()` Promise with the captured revision. The actual image
request's Promise supplies its result, including cache hits and decode errors.
This follows the [HTML decode algorithm](https://html.spec.whatwg.org/multipage/embedded-content.html#dom-img-decode),
which delays source selection by a microtask and rejects when that request
changes. A/B/A correctness does not depend on comparing URL strings from a late
load/error listener. Disposal aborts the owned Rust waiter and drops its weak
acceptance handles; it does not claim to cancel the browser's non-abortable
decode or underlying OS/network work. Suspended results wait for Resume.

Native maps the installed semantic revision to an engine image consumer identity.
The engine pins a separate consumer generation for each live img, independently
of URL fetch sharing. Pending waiters retain that generation. Completion is
recorded after resource consumption or cache application and only once per
consumer, including after a prior result was drained. Replacement and disposal
retire stale consumers/results. A new consumer denied an identity has no engine
request; the adapter maps that captured installation to a deterministic Failed,
alongside empty-source failure. The private adapter probe and fork exhaustion
test cover both sides of this path. The engine stores only live consumer records
and at most one pending terminal result per live element, with completion order.
The adapter retains published state per current semantic revision and preserves
accepted mutation prefixes. Ordinary moves do not republish its terminal status;
pending Suspend/Resume consumers can restart without accepting stale results.

The native default installs no HTTP/filesystem/data transport. Resource completion
requires an explicit base URL/provider and its existing origin, body, request,
decode and cache budgets. The injected test provider supplies deterministic PNG
bytes and proves the protocol, not production transport or OS accessibility.

Evidence: render and UI `image_revision` tests, HTML field-delta oracle, SSR stream
image test, native `image_events`, `object_fit` raster tests and the genuine fork's
six `image_observation_tests`. The fork is pinned to
`fa10341fd05899041079b30ab66363f89ccf0404`; its 106 library tests pass in sequential
and parallel construction profiles.
