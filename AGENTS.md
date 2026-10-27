# AGENTS.md

## Active architecture

The user authorized the breaking semantic rewrite in
`docs/semantic-ui-rendering-architecture-report-2026-10-21.md`. Follow that
report and `docs/semantic-ui-implementation.md`; do not preserve the former
HTML API/feature/wire format through compatibility aliases.

The active workspace currently contains style, render, core, resource, i18n, ui,
html, web, ssr, blitz, glory and salvo-adapter. The opt-in i18n package was authorized after
the semantic rewrite; see `crates/i18n/README.md` for its boundaries.
Truck, the old core runtime, routing, CLI/macros/serverfn/hot-reload and the old
desktop/native/liveview/tui adapters and examples have been removed. Only the
12 active runtime packages and migrated examples/benchmarks are supported. The
new `tools/cli` is a separate library-only host tooling workspace, explicitly
authorized after the cleanup. It must not depend on runtime crates or enter
the facade/default/HTML/SSR/WASM runtime graphs. Applications own a tiny private
workspace executable and shared Cargo.lock; bin launchers set approved per-host caches and the
native tool target before Cargo. The canonical check script also verifies CLI
metadata, fmt, tests and Clippy independently of the runtime workspace. Historical
design and measurement reports may refer to removed sources through Git history;
they do not advertise current APIs or platform acceptance. Do not reintroduce
retired feature aliases or count compilation as proof of platform acceptance.

- `glory-style`: explicit style declarations, no runtime/engine dependencies.
  Optional css pins cssparser 0.38.0 and provides immutable finite Sheet rules;
  no parser in the default graph. Reject unknown rules/declarations/values and
  check all original declarations against the target profile before installing.
  Retain selector/declaration order and specificity, global budgets and the
  128-term per-selector cap. CSS parsing/cascade must not enter reactive patches.
  Maximum sizes use Limit::{None,Px,Percent}, not Length::Auto. Option::None
  leaves the stylesheet in charge; Some(Limit::None) explicitly removes its cap.
  Keep min > max valid (the engine applies the minimum). HTML/RECORD profiles
  are v4 after FieldSizing; the native Blitz profile is v5 and rejects FieldSizing
  until intrinsic editor sizing exists. Fixed-row Textarea uses ordinary dimensions.
  Family distinguishes named faces from generics; named faces serialize as CSS
  strings (escape quotes, backslashes, controls and '<', including in Sheet).
  WhiteSpace controls collapsing/wrapping, not word breaking. Keep border width,
  style and color independent; radius is uniform px, opacity is group opacity.
  Never use a border/font shorthand that resets absent declarations. Support
  masks are u64; every property/value needs independent rejection coverage.
- `glory-render`: finite semantic nodes, typed patches/events/queries, identity
  allocation and headless reference interpreter. No HTML or GPU types.
  Render::preflight checks descriptor-local adapter limits before UI model/queue
  changes; no layout, host-tree lookup, I/O or application callbacks there. The
  supplied descriptor includes staged changes. Direct apply enforces the same
  limits and retains successful-prefix query watermarks on later rejection.
  Render::preflight_place receives the App's borrowed staged Memory and a
  Subtree::New descriptor or Existing identity, plus the proposed parent. Shared
  child/identity/placement rules run first; creation checks precede allocation,
  and Place/Resume checks precede model/queue changes. Read only the supplied
  semantic tree/configuration, never the adapter's committed/host tree. Normal
  moves walk ancestors only; restricted button/interactive contexts may inspect
  the live moved subtree. No full model clone or business-source scan.
  Canonical live nodes patch through Memory::prepare_patch: the guard swaps
  only the touched field, validates affected payloads/cross-field/content rules,
  and rolls back on drop/unwind without stats/watermark changes. commit accepts
  exactly one mutation. Raw Descriptor::patch/patches keep full construction
  validation. Do not expose mutable live descriptors that bypass this invariant.
  Explicit Plan declarations share immutable metadata across instances/Memories;
  live Props and metadata overrides belong independently to each Memory.
  Read through DescriptorView; do not materialize a full Descriptor on patch or
  preflight paths. Only the touched metadata field detaches. Rejection, dropped
  guards and unwind must restore both the value and prior sharing/override state.
  Ordinary Create remains owned; do not infer a static declaration from business
  values or retain visited instances in a declaration cache.
- `glory-core`: per-Runtime graph, direct dependencies, Owner/Binding/Instance,
  keyed Resource/ResourceSeed, tasks and host execution services. No DOM aliases
  or global scheduler mode; resource wire serialization remains an outer concern.
- `glory-resource`: opt-in per-request named registry and versioned JSON seeds.
  Depends on core/serde; no HTML/HTTP/browser. Facade feature `resources` enables
  it and forwards to already-selected web/ssr adapters. Core/default facade/HTML
  mapper must not acquire a serde dependency through this optional integration.
- `glory-i18n`: opt-in Fluent resources and app-local static/Cage/Bond language
  selection. Catalogs are immutable; reactive messages belong to explicit Owners.
  No routing, HTML, host discovery, global locale or implicit message cache.
  Keep Fluent out of core and default facade/HTML/SSR graphs.
- `glory-ui`: Rust builders and control flow, including windowed list logic.
  Measurement batches coalesce by key at the first notification's FIFO position,
  replacing its final index/height. Apply in that order before assigning eviction
  ages; rebuild sparse heights in retained-age order, never HashMap order. Reuse
  only bounded pending-batch storage, with no visited-key history or source scan.
- `glory-html`: pure semantic-to-HTML mapping, typed DOM edits and serialization;
  no runtime, browser, HTTP or GPU dependencies. Browser and SSR share it.
  Optional css lowers Sheet selectors with zero-specificity main-node guards;
  Check focus maps to its internal control, enabled/disabled/checked to semantic
  main-node state attributes. Keep full shape, field lowering and classic stream
  HTML format v4 in agreement. State attributes exist independently of css flags
  so SSR and CSR builds do not acquire different mapping shapes.
  Patch lowering reads the prepared field/inverse, not two complete Shapes;
  full shaping remains for create/snapshot/hydration. Keep the cfg(test) full
  shape difference as a differential oracle when extending any Patch variant.
  All finite variants, default resets, canonicalization, property barriers and
  controlled inputs must agree. Lowering is pure while the guard is borrowed;
  host sink work follows commit and retains the existing prefix/fault contract.
- `glory-web`: wasm32 DOM interpreter and RAF/async host; no business reentry
  from native listeners. Read coalesced viewport inputs before frame writes.
- `glory-ssr`: synchronous HTML/document output and versioned Snapshot fragments.
  Optional resources exports named terminal data before App cleanup; web/hydrate
  adopts DOM and explicitly restores those seeds. Native-only stream feature
  provides a pull-driven Shell/Patch/End request body and local Host.
  Its private Renderer independently implements both preflight hooks, shared
  target checks and successful-prefix query watermarks; synchronous Renderer
  changes do not automatically reach this path. Keep both under conformance.
- `glory-web/stream`: shared classic JS asset (STREAM_SCRIPT) applies finite
  HTML edits before WASM, then performs End rekey/resource handoff. Serve it
  under the app's CSP. Bootstrap::head_script installs pre-edit capture;
  Frame::to_html emits complete inert data + parser-blocking external script.
- `glory`: facade; outer hosts join by actual technology (web/ssr/blitz/etc.).
- `glory-salvo`: native HTTP integration over semantic SSR. Ordinary responses
  use glory-ssr; optional stream owns each !Send request on an admitted local
  worker, transferring only owned HTML/errors to Salvo. Share bounded admission,
  request one frame per body demand and cancel pending I/O on body drop. Never
  eagerly collect the stream or move UI state across threads. Serve the matching
  STREAM_SCRIPT under the app's CSP. Axum/Actix legacy adapters were removed.
- `glory-blitz`: native-only document interpreter over html's typed edits.
  Pinned dom/traits/paint beta.4; system fonts and real CSS/box/text layout.
  STYLE_PROFILE is native-specific (blitz v4), not Profile::HTML: typed Sticky
  and AnchorAuto are rejected until actual sticky offsets/automatic anchoring
  exist. Explicit AnchorNone remains valid for manual virtual-list anchors.
  Keep App checks, descriptor-local preflight and direct apply in agreement;
  reject per operation to preserve an accepted prefix/query watermark. Raw
  author CSS retains engine-specific coverage and does not widen this promise.
  Optional paint records AnyRender frames; raster uses Vello CPU without a GPU.
  Optional desktop shell provides winit/CPU-softbuffer windows, ready-only Host
  and queued activation/focus/navigation/editing. WindowApp supports controlled
  text/check events, UTF-16 selection and native IME protocol; the standalone
  Renderer has no input driver.
  Mixed checkboxes use an independent live indeterminate property and native dash
  painting. Program checkedness preserves it; pointer/Space/AT activation clears
  it while toggling checked once. Mode Clear resets it without replacing the
  control. AccessKit reads the actual native mixed/boolean state.
  Radio uses a live checked property with name/tree/form association. Selecting
  a new member withdraws its peers without false change events; selecting the
  current member emits no value change. Equal true property reassertion repairs
  a rejected proposal. No group/history registry or business-source scan.
  Intrinsic arrows/Home/End and one Auto Tab stop use current host-tree order,
  skipping unavailable/readonly selection targets. Explicit Navigate handlers
  and explicit tab stops remain component-owned. Detached trees are independent;
  reattachment and name/type/form changes re-evaluate current association.
  Combobox/option refinements route shared Choice and scalar Typeahead through
  the current focused control. Components own matching/selection; no host query,
  visited-option registry or competing controller. Listbox layers Preserve focus;
  option pointerdown/click must not focus the intrinsic Button, including after
  the popup closes. Tree layers Manage focus and restore the current trigger.
  RevealOption scrolls only its nearest matching live listbox, never the viewport.
  Mouse/pen Pointer uses one dispatch's previous hover identity; App derives
  logical PointerWithin boundaries. Touch never becomes hover/typeahead input.
  AppKit empty Multiple Backspace cancels the command's no-op deletion and keeps
  one pending current NodeId/command kind until the following physical key supplies modifiers.
  Clear it on other input/focus retirement; do not remove a token after deleting
  the final text character or repeat password deletion on the raw Mac key.
  AccessKit resolves active descendants from current semantic ownership and live
  exported nodes. Preserve PasswordInput's protected role and omit its value.
  Intrinsic Select uses bounded immutable declarations and private physical rows,
  independent of combobox refinements. Single popup and multiple/rows listboxes
  have live selected-key properties, revision barriers, disabled/group behavior,
  keyboard/typeahead, pointer and engine-owned touch pan. Keep one current popup;
  preserve option identities across patches/moves, retire its lease on suspension,
  disposal, disabling and focus loss. Popup input must not dismiss its enclosing
  modal. Escape falls through after the popup closes; Tab uses normal traversal.
  Shared Select rejects readonly. Do not clone declarations/labels/business values
  on selection patches or retain a visited-option registry. AccessKit exports
  current groups/options, selected state and validated actions, never form values.
  Actual Sapid Select/Combobox/MultiSelect consumers now pass; retain prior failed
  evidence. OS input/reader acceptance stays separate. See docs/native-choice.md.
  Menu/Menubar/ContextMenu/Palette reuse shared commands and current ancestry;
  components retain matching, controlled state and submenu policy. ContextPoint
  uses finite viewport CSS pixels and current modal/focus scopes. Border-box
  placement/hit tests undo only the queried node's own scroll. ExitMenu permits
  its own controlled-close hidden range, rejects disconnected/suspended ancestry,
  and bounds current Tab traversal while excluding all Menu trees and the whole
  menubar/context boundary. No visited-menu registry or host-side controller.
  Sapid requests explicit reveal after visible bindings; Focus still does not
  scroll. Explicit programmatic scrolling cancels old touch inertia in the fork.
  Protocol/CPU/AX models and SDK initial display do not prove OS input/reader
  acceptance or platform system-menu services. See docs/native-menus.md.
  Intrinsic-height CollapseConfig now has native presentation and revisioned
  completion. Keep the current phase per live declaration only; no per-frame
  Cage, Owner capture or revision history. Stylo interpolates temporary numeric
  min/max-height constraints, while native layout measures actual intrinsic
  targets. Retarget content/viewport changes over the remaining interval; release
  constraints at completion. Close is logically hidden/inert immediately, with
  retained clipped pixels until its matching terminal event. Initial/zero/Reduce,
  unavailable ancestors, CSS hiding, suspension and zero viewport settle once;
  Resume cannot replay completion and Dispose retires transitions. A settled
  closed extent is zero even when a hidden ancestor left a stale physical box.
  The fork retires display-none descendant animations after style and drops
  obsolete boxed-container rosters/anonymous boxes on display:contents. Keep
  current-pin tests/provenance and actual Sapid policy consumers in agreement.
  CPU/component/target checks do not prove OS input, readers or mobile devices.
  See docs/native-collapse.md.
  TagsInput retains an ordinary Input and shared revisioned Tags commands; Sapid
  owns accepted values, tokenization, duplicates and atomic proposals. Preserve
  caret/native input, composition and terminal candidates. Uncomposed software
  punctuation/Return may use the shared commit/form path. One current AppKit
  (NodeId, command kind) pairs empty deletion/paste with the physical key; other
  input/focus retirement clears it. Never retain clipboard bodies or tag history.
  Read one provider clipboard snapshot, bound its returned UTF-8 before copying
  or editing, and retain UTF-16 selection on body-free budget rejection. Provider
  OS allocation is outside that payload budget. Ordinary paste uses the same
  editor; delimited paste remains an atomic component proposal. Stock platform
  clipboard services and real mobile input/readers stay separate from injected
  provider/CPU/AX evidence. See docs/native-tags.md.
  Optional clipboard (facade blitz-clipboard) requires an explicit Config service.
  Semantic writes and shell editor copy/cut/paste share one current operation;
  cancellation/deadline cannot refund an unsettled native permit. Foreign future
  construction/poll/drop run outside document borrows. Revalidate current focus,
  input revision/selection and policy before asynchronous editing; presentation
  layout generations are not edit identities. Never export password copy/cut.
  Desktop uses one lazy Arboard worker/permit; default graphs gain no clipboard.
  Payload limits apply after OS allocation. macOS system round trips preserve
  all pasteboard representations in RAM; mobile drivers and other OS acceptance
  remain separate. See docs/native-clipboard.md.
  Android's explicit AndroidClipboard uses safe JNI to the app-owned main-thread
  host. Keep one physical operation and at most 32 weak clients per coordinator,
  one current result per client, non-reused IDs and no completion history. Clone
  a driver/service to share its ID sequence; do not wrap one Host twice. Bound
  returned byte arrays before Rust copying, and raw/canonical UTF-8 in Java;
  OS ClipData allocation stays outside that transfer budget. Never coerce URIs,
  install listeners or keep an Activity strong. Pending-only timer polling and
  native permits survive receipt cancellation; ordinary APKs initialize nothing.
  The owned SDK probe preserves original ClipData only in RAM. Its service-call
  evidence does not establish native menus, keyboard, TalkBack or real devices.
  Optional files (facade blitz-files) requires explicit Config::files. Keep
  one picker/four reads and shared live reference/metadata/declared-byte budgets;
  native driver permits survive cancelled receipts until actual OS completion.
  Open selected regular files once; display names never become paths. Read exact
  bounded chunks, retaining the request watermark and current-token validation.
  Poll/construct/drop foreign futures and retired readers outside document borrows.
  PNG/JPEG previews use shared header limits before bounded encoded reads. Charge
  pending/registered/cancelled decoder bodies to one 32 MiB service budget; await
  NetHandler::prepare and revoke current private addresses without fallback.
  Clear/remove/revision retirement also evicts that URL's engine reuse cache and
  pending delivery. Already delivered pixels are separate from revocable grants.
  Shared UI rechecks file rights after earlier events in an already drained tick.
  Interactive desktop drop requires an explicit opener and FileConfig opt-in;
  headless/mobile still reject it. Match the current Winit transfer/receipt pair,
  fetch only after Drop, and accept Copy/local absolute file URIs only. Bound URI
  count/bytes before path copies/opens; platform raw allocation stays separate.
  One shared drop slot survives cancelled receipts until native settlement. Resolve
  actual CSS visibility before reads/publication; retain modal/focus scope checks.
  A successful drop/chooser supersedes the other current result; rejected drops
  preserve the chooser. Poll/drop opener futures and native URI callbacks outside
  document borrows. Custom hosts forward native file/availability events once;
  mobile and actual OS drag/dialog/reader acceptance remain separate. See docs/native-files.md.
  Android files uses an explicit protocol-v1 safe-JNI Host and ACTION_OPEN_DOCUMENT;
  retain original finite seekable regular PFDs, never reopen names or persist URI
  grants. One process-lifetime native admission ledger spans Activity sessions:
  one picker, four reads including undrained replies, 32 weak clients and bounded
  charged handles/metadata/declared sizes. Serialize numeric request allocation
  with JNI submission across concurrent readers. Terminal Futures release reply
  records even if callers retain them. Cancellation/Activity close revokes delivery
  without refunding actual opens/reads/closes; sources close after final real reads.
  Keep foreign provider work outside ledger/document locks and callbacks out of
  Rust UI state. Save only
  the non-wrapping native request-code watermark, no URI/authorization history.
  files-probe's owned selector/provider and their DEX classes belong only to the
  probe APK. SDK result/PFD/private-decode evidence does not prove DocumentsUI
  input, non-seekable provider, real-device, TalkBack or iOS file acceptance.
  iOS's application-owned ios_io drivers use the separately authorized single
  integer ABI. UIKit/pasteboard work runs on main, coordinated regular-handle
  opens/reads/closes on workers, all outside ledger/document locks. Clients and
  request watermarks are bounded; callbacks retain records with weak clients.
  Cancellation revokes delivery and preserves actual native admission. Selected
  URLs retain their security scope through the last actual read and close; never
  reopen a display name or persist a bookmark. Opening groups reserve configured
  declared bytes and 32 KiB metadata; after opening they settle to actual metadata
  and sizes. Group charges remain until every original handle has actually closed.
  Global maxima: 160 handles/160 KiB/2 GiB, one picker
  and four reads including undrained receipts. A failed native close stays charged.
  Clipboard raw/canonical UTF-8 is at most 64 KiB; one physical operation and one
  receipt/client, at most 32 clients. Ordinary construction reads no clipboard or
  file and presents no UI. ios-io-probe and its owned selector/read gate/clipboard
  guard are absent from normal builds. iPhone/iPad simulator owned-source service
  and semantic evidence does not prove system picker input, external File Provider
  security-scope grants, cross-app paste alerts, real devices or readers.
  Input placeholder is a bounded, independent presentation layout, lazily shaped
  only while empty/non-composing and released when hidden/cleared. Hint-only
  changes preserve editor identity, raw generation, selection, IME and value
  property barriers. Use its layout for hint paint only, clipped to the content
  box; all editing/geometry/context stay on the real editor/password projection.
  Keep :placeholder-shown snapshots local and preserve selector invalidation.
  AccessKit placeholder metadata never supplies a label or password value.
  Password uses the pinned local engine's grapheme-to-bullet display projection.
  Raw editor bytes remain for values/IME/UTF-16 selection; use display_editor for
  layout, pointer navigation, selection paint, caret and scrolling. Never paint
  the raw editor layout for a password. Copy/cut are blocked; paste is supported.
  Purpose changes preserve identity and active composition. IME purpose/area
  requests do not replace actual OS IME or assistive-technology acceptance.
  Config exposes an explicit base URL/provider and bounded resource limits;
  the default installs no HTTP/filesystem transport.
  Optional package adds a document-owned immutable-byte Loader on native targets;
  desktop assets includes it and adds HTTP(S)/mounted-file transport with explicit
  origins/roots and job/body/deadline budgets. Shared resource leases
  also bound requests/completed results and charged payloads. Optional accesskit
  (facade blitz-accesskit) adds a semantic tree/action bridge; Windows UIA has
  actual client acceptance. Optional android-assets (facade blitz-android-assets)
  binds an explicit application AssetManager/prefix under asset://android/;
  bounded APK streaming and ARM64 emulator CSS/PNG/Unicode evidence exist.
  Optional network (facade blitz-network) supplies the same bounded native HTTP(S)
  worker on desktop/Android/iOS. Actual mobile simulator HTTP/TLS/socket cancellation
  evidence exists; Android requires explicit Application-based AndroidTls setup.
  Runtime permissions, real-device networks, Unix AT-SPI clients and
  screen-reader/text-selection acceptance remain pending. Facade
  `blitz` is optional and must not enter default/HTML/SSR dependency graphs.
  Optional permissions (facade blitz-permissions) has explicit drivers and
  Owner-cancellable local futures. Android safe JNI and the app's Swift host
  have SDK-controlled authorization-query evidence, separately from actual
  permission-dialog acceptance. The Rust iOS application connection uses the
  separately authorized six-function integer ABI in examples/mobile only.
  Linux Fontique uses fontconfig-dlopen; cross-checks do not prove runtime font
  availability or display-server acceptance.
  Optional shell also has explicit AndroidApp and UIKit entry paths; see
  examples/mobile. Android emulator evidence stays separate from real-device
  network/permission, accessibility and store acceptance. GameActivity has
  ARM64 emulator system-pinyin preedit/commit/cancel evidence; NativeActivity's
  baseline keyboard evidence does not imply the same CJK support.
  Android embedded CSS/PNG has actual emulator pixel/lifecycle evidence. iOS now has
  simulator/device linking and actual iOS 27 simulator pixel, touch, software-key,
  Home/restore and rotation evidence. The app-owned Swift scene bridge forwards
  actual UIKit lifecycle events; the Winit fork attaches windows to UIWindowScene.
  The app-owned Swift host also reattaches that same single window after scene
  disconnect/reconnect; an isolated iPad simulator fixture has real UIKit session
  destruction/reactivation evidence. Multiple independent Glory scenes and
  process-death persistence are separate capabilities.
  UIKit now has a narrowly authorized UITextInput bridge, bounded surrounding
  context and real pinyin preedit/candidate/commit/cancel/restore evidence.
  Android GameActivity is an explicit opt-in alternative to NativeActivity;
  its safe GameTextInput bridge provides bounded current-editor context and
  validated UTF-16/UTF-8 composition/deletion translation. Never enable both
  Activity features for an Android build. NativeActivity retains its prior path.
  Arbitrary native selection gestures, VoiceOver and signed device installation
  remain open. Passwords export empty context; never overwrite live marked text
  with acknowledgements or native purpose changes.
  macOS has actual CPU window/resize/cleanup and external AX client evidence;
  Developer ID/notarization and VoiceOver speech remain separate acceptance.

## Design rules

- Pure handwritten Rust builders. No RSX/UI DSL or template VM.
- Keep Cage/Bond/Lotus as Glory's public reactive model; do not substitute an
  unrelated third-party Signal API.
- Copy reactive handles are local (!Send/!Sync), generational, and Owner-owned.
  `get()` now returns an owned clone. For large data use `with(...)` to borrow
  just the indexed data needed; never clone or scan a full list on scroll.
- Reads track only the immediate Bond/Binding/Effect. Bare component-build
  reads are not subscriptions. Derived equality is opt-in; do not require all
  values to implement PartialEq.
- Cage::field(name, get, get_mut) aliases a fixed inline field in the original
  model. Pure accessors must select the same subobject without reactive reads;
  heap/Deref projection through this method and moving layouts are unsupported. Same parent/name
  paths share a channel, with exactly one alias per constructing Owner. Repeated
  selection reuses it without invalidating notification targets. Validate type,
  layout, access and Owner first; discard unused accessor captures outside graph
  borrows. Construct projection objects lazily only for a new canonical path. Retire
  empty paths on alias/child disposal; root disposal invalidates all aliases.
  Field writes notify root/ancestors and target aliases/descendants, not siblings;
  root writes conservatively notify every live registered field. Keep equality
  optional. Do not register every item in a large source or retain visited rows.
  Field projection captures, like user values, drop outside graph borrows.
- Vec::item(index) and map::entry(key) on Cages create logical collection paths.
  Vec paths follow positions; map paths follow complete keys through rehash,
  removal and reinsertion. Missing reads/writes return MissingField; reads still
  subscribe, writes must not execute callbacks or publish. Existing field children
  recover after reinsert; initial inline-field registration needs a present parent.
  Keys/values need not be Clone. Hash/Eq/Ord must be pure; user hashing/comparison
  and key destruction must run outside graph borrows. Registry Hash/Eq and key
  destruction also run outside registry borrows. Keep only live keys/weak buckets,
  remove empty paths/buckets, compare complete keys on hash collisions. Never scan
  the source or all siblings to register a dynamic key. Root writes conservatively
  notify live paths; item/entry writes notify only their ancestors/descendants.
  Do not allow new path registration while the root is mutably borrowed: writes
  snapshot affected aliases before calling business code. Deref remains pending.
- Explicit Cage collection edits do not construct item/entry aliases. Map
  insert/remove select a live full-key path; Vec push/pop/insert/remove/truncate/
  clear select only affected registered positions. Include the collection's
  aliases and ancestors, plus selected descendants; preserve unrelated paths.
  Missing remove, empty pop/clear and non-shrinking truncate publish nothing.
  Invalid indices return OutOfBounds; unrepresentable Vec length returns Capacity.
  These try_ methods do not turn allocator/user panics into AccessError. Keep
  version checks before editing, callbacks/destruction outside graph/registry
  borrows, and reuse the root's single target buffer. Arbitrary revise remains
  conservative. Vec insertion/removal still moves business elements in O(n).
- Field writes keep at most one reusable target/buffer pair per root, containing
  generational identities and the current numeric selection only (no business
  keys or values). Hit targets still precheck current versions and
  publish through current subscriber/frontier state. Misses rebuild in reused
  buffers, not fresh per-field snapshots. Validate access and versions before
  taking/changing a buffer. The guard owns it during business code; an empty
  root slot is its return permit. Every alias/schema change or root disposal
  removes that slot, so the guard must never resurrect a removed cache. Same-root
  mutable borrowing prevents slot recreation while the guard runs. Other roots
  remain independent. Free buffers on membership changes; no value, address,
  subscriber roster or written/visited-key history in this cache. Cache slots
  are boxed to keep BTreeMap node payloads small. RuntimeStats::field_target_*
  count valid preparations and live slots/identities, not RSS or completed writes.
- Keep collection-selection metadata out of ordinary field write arguments and
  guards. One optional numeric selector box belongs to the cached root slot,
  is reused across collection edits and is freed with schema/Owner invalidation.
  It is not a per-key cache. Compare both sparse edits and Vec front/bulk edits;
  do not split a coarse compound revise into multiple publishes to bias timing.
- Writes enqueue work. Runtime flush/batch and host event/frame boundaries
  determine when work executes. Derived/prop computations must remain pure;
  effects may write and schedule another pass. Nested batch must remain atomic
  with respect to observations.
- Runtime::enter(instance, callback) validates runtime/generation/Owner before
  user code and restores nested construction context on return/unwind. It is a
  synchronous, non-owning entry, not a flush or an async context carrier.
  UI events use the live listener registration's InstanceId, including listeners
  on other scopes' nodes. Keep callback identity/tree/liveness checks; do not
  retain a UI Scope merely to establish execution context. Actual observer
  callbacks run under their node's Owner, independent of the flush caller.
  Dependency validation/value destruction can retire the observer; skip its
  captured callback in that case. Pure Binding/Derived rules remain unchanged.
  Source fields are scope-owned; different keys explicitly constructed inside a
  long-lived Owner stay live until it retires. Use short Owners or collection
  edit APIs for transient work. Arbitrary map closures remain distinct nodes;
  no implicit memoization or per-callback-run garbage collection is promised.
  Each live node records whether its pending invalidation reached subscribers.
  Repeated writes still advance source versions/source_writes, but skip an
  already-forwarded frontier. Evaluation start rearms the node/old dependencies;
  dependency registration/completion and unwind rearm refreshed paths. Never
  replace this with a status-only Check/Dirty shortcut: partial reads, late
  subscribers, computing effects and retries must receive future updates.
  RuntimeStats::marked counts actual forwarded consumer visits, not attempted
  source writes. This flag is node-owned/recycled, not a visited-row cache.
- Owner disposal removes edges, queued work, instances and tasks. Never drop
  arbitrary values or execute cancellation/cleanup callbacks while holding a
  mutable graph borrow. Never wrap exhausted generations.
  The scheduler cancels queued membership immediately; numeric retired keys may
  remain in FIFO storage until bounded compaction. Keep physical entries at most
  2 * live queued + 64, and clear lengths when live queued reaches zero. Preserve
  Structure/Binding/Effect priority and each phase's live FIFO order. Only actual
  observers can enter the queues. Skip queue lookup for sources/derived values,
  and skip compaction checks with no retired entries. Compact after cancellation
  and live dequeue, before callbacks/reentry. Generation-checked arena lookup
  removes retired keys without visiting values or invoking callbacks. Neither
  IDs nor capacity are business-data caches; capacity follows peak work, not
  visits. RuntimeStats queued is live membership, queue_entries includes bounded
  retired keys, queue_capacity is allocated FIFO capacity; compaction counters
  report passes/inspected keys, not memory bytes or completed frames.
  on_cleanup callbacks run untracked under their already-retired registering
  Owner (children before parents, reverse registration order). Implicit path/map
  construction must fail instead of escaping into the disposer/source Owner;
  explicit entry into another live Owner remains allowed. Task cancellation runs
  untracked under its task Owner, live on explicit cancel or retired on teardown.
  Preserve pure-mode restrictions. A panic unwinds; there is no retry guarantee.
- App::request belongs to the application root. Scope::request belongs to its
  requesting Scope, independently of the query target. Both flush a mandatory
  barrier, revalidate the requester after it and execute replies untracked under
  that registered Owner. Retirement removes waiters/captures without calling
  application code. Target removal delivers NodeGone only to live requesters.
  Maintain both target/Owner token indices; never scan all queries or visited
  rows on retirement. Remove empty buckets on every terminal path. Cancellation
  queues contain tokens only, not captured application callbacks. Late/duplicate
  responses cannot resurrect a retired request or a reused identity. No owning
  Scope is retained by the registry. UiStats exposes live query/Owner/target counts.
- AccessKit types stay in blitz's optional desktop adapter. Export only the live
  semantic tree (Group is transparent), never a virtual-list source. Cache only
  live nodes, emit changed nodes, and allocate non-recycled document-local IDs.
  Retire mappings on removal/deactivation; never reset the ID counter. Platform
  messages carry both WindowId and RuntimeId to reject stale/reused windows.
  Foreign callbacks only enqueue; business handlers run on the UI loop. Attach
  before showing the window and drop adapters before window handles. Password
  values never enter the AX snapshot. AccessKit positions are zero-based and its
  known size belongs to the nearest list container, unlike per-item HTML ARIA.
  AT ScrollIntoView is advertised only on mounted virtual Row wrappers with a
  live Root/Navigate controller. Queue Navigation::Reveal without moving focus;
  let ui::list resolve its logical key/index, then reveal native ancestors and
  the document viewport after the logical commit. Deduplicate pending live rows,
  revalidate before physical work, and schedule observations after native scroll.
  Reveal requires scroll capability, not focus; a root/retired origin must not
  fall back to the first row or inherit an earlier batched navigation cursor.
  Current AT actions also include Focus/Click/SetValue; do not advertise TextSelection
  until a complete text-run/offset bridge exists. Windows UIA evidence does not
  replace actual screen-reader announcements or Unix AT-SPI client acceptance.
  UIA's one-based position uses i32; omit unrepresentable zero-based positions
  (including i32::MAX) rather than overflow or clamp the logical item index.
- Host::spawn queues a local future without synchronously polling it; wake
  does not synchronously reenter the runtime. No block_on fallback.
- Native core::executor::Executor is shared by stream and desktop hosts.
  observe begins a host turn and acknowledges its notification; ready/dirty
  wakes coalesce until the next observe. Poll only ready tasks, bound each host
  turn, cancel/drop outside borrows, and never reopen a closed executor.
- Owner::resource tracks only its key source and explicit refresh commands.
  Loader reads do not subscribe the driver. Key changes cancel obsolete tasks;
  refresh/cancel revoke publication immediately. Drop user futures/results with
  no graph/value/controller borrow. Terminal seeds suppress only a matching
  initial request; they are decoded values, not an automatic wire snapshot.
- Create a fresh Resources context per request/app; clones share only that
  context. Names/schema versions are explicit, keys are business values. Only
  live named terminal results export, in deterministic name order. Owner cleanup
  removes registration; no cache of every visited virtual row. Serialize user
  values with no graph/value/registry borrow. Resource JSON scripts use raw-text
  escaping, live outside the hydrated root, and are read non-destructively so
  failed attempts can restore a fresh context from the original payload.
- hydrate_resources_with removes the data block only after successful adoption,
  returns WebApp + Resources, and preserves it on failure. Low-level
  read_resources stays non-destructive for callers choosing their own policy.
- Bootstrap and DOM adoption share Symbol.for('glory.preload.composition').
  Capture before the first stream controller and after End until WASM owns the
  root; adopt initializes Editing and semantic composition events from it.
  Native InputEvent.isComposing also recovers a missed compositionstart.
- Group has no layout box or implicit accessibility role. Move/Place,
  Suspend/Resume and Dispose have distinct semantics. Queries are commit
  barriers and do not imply pixels were displayed.
  Structural Role values, including Role::Group, belong only to View; other
  builders retain intrinsic roles. Descriptor::effective_role gives a named
  generic View its default accessible Group, while unnamed Views stay generic.
  Plain inline/paragraph Text uses content, not name; Heading accepts a name.
  Decorative Image (alternative=None) rejects name and explicit focus; Focus
  actions reject it before queuing. Name/role metadata updates must not rewrite
  values, reset selection/preedit, or replace controls. Name Clear restores
  native content/alt labeling; Keep emits no physical metadata changes.
- Query/Action/EventKind::validate_target define shared semantic target rules.
  Group has no physical queries/actions or Resize/Scroll/Anchor/FocusChanged
  origin, but accepts logical Activate/FocusWithin/Navigate interests. Text
  edit events belong to Input, checked events to Check; Selection requires
  Input and still depends on the actual host/control. Validate identity first,
  then arguments/target, then capabilities, before queuing actions or replacing
  handlers. Valid SSR interests record semantics without advertising input.
  Unsupported queries on live identities still complete asynchronously with
  the accepted watermark. See docs/semantic-ui-contract.md for exact scope.
- Optional glory-render/conformance provides assertion-based adapter probes;
  SSR/Blitz enable it only as a dev dependency. Keep it out of default production
  graphs. The eight-target matrix establishes explicit CSS boxes; it does not
  prove empty-inline geometry, all props, OS input or accessibility acceptance.
- Event::validate_payload checks finite/local geometry, ScrollAck, navigation
  origin agreement and single-line edit values. Memory::validate_event adds
  live boxed origin/ancestry checks; Anchor is a proper descendant and
  FocusWithin may originate at the target or a descendant. Ignore retired or
  inactive embedded origins (except terminal focus loss), never reinterpret a
  reused slot. Group cannot be a physical origin. Ordinary focus delivery uses
  one ancestry walk; recheck derived FocusWithin only after model stats change
  during earlier callbacks. No data-source/visited-row scans or origin cache.
- Event::is_terminal_loss is shared by App/reference queue cleanup; suspension
  preserves cleanup on live identities, disposal never does. App::pump must
  process the whole drained tick despite typed routing errors, complete later
  query responses and flush valid writes. Return a flush error first, otherwise
  the first routing error. Do not orphan query waiters by returning on the first
  malformed event. Old input revisions still follow the existing discard rule.
- Stored Input values use LF/U+FFFD (CRLF/CR and NUL canonicalization), without
  silently rewriting a business Cage. Program single-line CR/LF remains an
  error; native host sanitization may remove it before emitting canonical text.
  normalize_text borrows already canonical values. Email/Url use text/inputmode
  hints so browsers do not trim whitespace or remove the selection API.
- Program revisions never regress; equal revisions require canonical equal
  values, Keep ignores its revision. Builder bindings allocate from the live
  node, including after explicit writes; exhaustion returns a typed error.
  ValueChanged and Commit { value, revision } both drop old and reject future
  versions before callbacks. Composition cleanup remains version-independent.
  Hydration starts a new NodeId/version lifetime, not the server counter.
- Web reads normalize noncanonical host text while preserving UTF-16 selection.
  Hydration validation only computes observations; do not repair live DOM until
  the entire subtree validates. Canonical-equivalent echoes acknowledge versions
  without resetting caret/preedit. Native IME byte offsets use the same helper.
- HTML text tag overrides must respect ancestor content: an Inline text mapped
  to p cannot enter a button, including through Group Place/Resume. Validate
  before changing mapper state or emitting that operation. Only button-context
  moves scan their subtree; never scan the underlying list data source.
- Successful nonempty Render::apply calls must strictly advance Receipt::applied;
  empty calls preserve it. App faults on unchanged/regressed receipts. Requests
  capture the accepted watermark; delayed responses must match that value, even
  after later commits. App validates QueryValue kind, finite geometry and ordered
  selections before callbacks. Selection offsets are UTF-16 code units; native
  adapters convert at the boundary. Duplicate/retired tokens never call back twice.
- Program text selections validate scalar boundaries against the current value,
  including CJK/emoji and both directions. Half-surrogate endpoints are rejected
  before queue changes. Native hosts additionally refuse shaped-cluster interiors
  without rounding or changing the prior selection; this is not native drag/handle
  selection support.
- Blitz Groups use offline parks and comment ranges, never a layout wrapper.
  Dispose reclaims separately suspended descendants and engine identities.
  Controlled props pending first attachment must be replayed through descendants;
  ordinary moves must not rewrite values. A private textarea text seed covers
  Blitz's lazy editor initialization. beta.4 Bounds compensates only the queried
  node's own scroll; do not add back ancestor/viewport offsets.
- Paint FrameInfo records process-local document identity, RuntimeId, visual
  revision, applied watermark and physical size/effective scale. A RuntimeId
  alone cannot distinguish two consumers of one app. No metadata means display
  completion. Generic paint resets its scene; Raster reuses one RGBA8 buffer,
  skips unchanged pixels and resets caches on document/size changes. Enforce the
  u16 CPU dimension guard and explicit byte budget before allocation. Zero-sized
  surfaces and clear release backbuffer/backend caches. New visual mutations,
  host resource invalidations and future focus/animation changes must advance
  the visual revision even if a geometry query has already resolved layout.
- Native resource completions invalidate the document before pixel-cache checks;
  paint metadata is captured after ingestion/layout. Provider callbacks use a
  stable, coalesced host-waker relay even for requests made before a shell exists.
  They never run business callbacks. Close retires the relay; late completions
  must not decode into or wake a closed document.
- Never vendor third-party code in this repository. Modify upstream code in
  genuine forks under the glory-rs GitHub organization, retaining upstream
  package names, repository structure and licenses. Pin full Git commit SHAs;
  do not use moving branch dependencies or application-level global patches.
- The six engine/layout packages use glory-rs/blitz and glory-rs/taffy Git
  revisions recorded in docs/engine-forks.json. They stay outside the twelve-package
  workspace. Run python scripts/check-engine-forks.py --test to verify Cargo's
  resolved sources/lock and run fork-owned tests in separate clean Git checkouts
  under the approved per-host build root. Do not edit Cargo's managed Git checkouts. Update the
  dependency rev, lockfiles and provenance record together after a fork change.
  The additional twelve dpi/Winit platform packages share the immutable
  glory-rs/winit revision in that record; Blitz's fork pins the same revision.
  Its Android Activity lifetime tests run from the clean fork with --test.
  Existing upstream unsafe is unchanged; local resource additions use safe Rust.
  Do not reformat unrelated third-party files with the workspace line length.
  Initial free space already includes gaps;
  fractional grow/shrink must scale that remainder without subtracting gaps a
  second time. Keep the finite flex matrix opt-in via render/conformance.
  Keep raw and display
  UTF-8 indices separate; ICU extended-grapheme boundaries map mask cells to raw
  offsets. Refresh projection after with_text_input edits and immediate type
  changes. Only the focused editor needs post-layout scroll/IME-area updates;
  do not scan every row/input. The shell coalesces unchanged IME purpose/area.
- Native image reuse cache entries/bytes, pending URL/waiter tables, image decode
  sizes and external font registration have separate limits. Live elements and
  backend conversion caches are separate ownership, not counted as reuse-cache
  bytes. Cancel abandoned image consumers, reject stale request IDs, and retry
  cancelled background layers on Resume. Failed fonts must not consume the
  successful registration budget. Providers must honor cancellation/close and
  bound encoded bodies, jobs, queues, deadlines and URL/redirect policy; the
  injected test provider is not a production transport.
- Native `glory-blitz/package` (facade blitz-package) owns one Loader per document.
  Native network (facade blitz-network) includes package and adds HTTP(S) on
  Windows/Linux/macOS/Android/iOS. Desktop assets includes network and file mounts.
  Package-only builds reject nonempty origins/mounts and exclude desktop transport
  dependencies. Do not bypass NetHandler::prepare when serving immutable bytes;
  queue, deadline, cancellation and decode/result budgets still apply on mobile.
  Never share one across independent WindowApps or reopen it after close. Empty
  origins denies HTTP; no implicit file/data URL or environment proxy access.
  Validate every redirect and preserve original request ownership separately
  from final CSS base URLs. Transport jobs, encoded payloads, decoded caches and
  already-published engine messages are distinct budgets; never infer total RSS
  from any one of them. DNS is
  single-flight once per allowed host; OS lookup is not abortable. Reap completed
  JoinSet entries and never poll an idle loader timer. Keep user handlers and
  destructors outside state locks; cancellation revokes future acceptance, not a
  handler that has already begun. No UI state crosses onto the transport runtime.
- HTTP transport is shared across native hosts; mounted files remain desktop-only.
  On Android, nonempty origins require an explicit AndroidTls capability made
  through safe JNI with an Application context, never an Activity/raw pointer.
  Its matching verifier AAR follows Cargo metadata and committed Gradle hashes;
  ordinary non-network APKs must not gain INTERNET or the verifier component.
  No global AndroidApp or crypto provider installation. iOS uses Security.framework.
  Explicit HTTP origins grant cleartext; HTTPS still verifies chains/hostnames.
  Keep protocol/HTTP tests runnable under network alone, without cap-std/assets.
- Permissions are explicit host capabilities, not render nodes or ambient grants.
  Creating a future must not query/request the OS before its first poll. Reserve
  local admission before foreign driver calls; revalidate close/deadline after
  polling. Owner retirement drops the future and revokes delivery. Never claim
  that dropping a future dismisses an OS dialog or refunds an uncancellable query.
  NativeStats distinguishes records/prompts/queries from local waiting futures.
  Driver callbacks only update bounded native records, never Rust UI state.
  Android hosts hold Activities weakly, coalesce their main messages, bound each
  turn and close on Activity destruction; old callbacks cannot target a successor.
  Swift records hold clients weakly and retain actual prompt/query admission until
  native completion. No completed-ID history or persisted authorization cache.
  Usage strings and sensitive manifest permissions belong only to opted-in app
  builds. SDK grant/revoke/reset checks are not actual dialog/device acceptance.
- Optional android-assets includes package and binds an explicit application
  AssetManager plus validated APK prefix under asset://android/. Never discover
  a global AndroidApp, use raw JNI pointers or fall back to the embedded package.
  Decode path components once; reject controls, separators and empty/dot parts.
  Prefix joining is independent of URL normalization. Use safe streaming NDK
  reads on the shared bounded blocking pool, not whole-asset mapping. Declared
  and actual body limits, deadline/cancellation checks and prepare admission all
  apply. file_active/file_peak_active include APK reads; closing async jobs does
  not interrupt an OS call or refund its blocking permit early. The generated
  Unicode asset and native acceptance code belong only to android-assets-probe;
  ordinary APKs must exclude them. This feature grants no network/file mounts.
- Engine ResourceHandler admission uses document-local, non-wrapping request
  leases. Requests, charged completion bytes and encoded input have independent
  limits. Providers await NetHandler::prepare before decode; synchronous providers
  must explicitly fail when capacity is unavailable. No await/blocking under a
  CSS/resource mutex. A response retains its lease through actual consumption
  or receiver drop; cancellation marks queued data stale without prematurely
  refunding it. Declare the response lease after its payload so Rust drops the
  payload before waking budget waiters. Errors use registered slots; no unbounded
  rejection channel. Request admission failure must not register critical CSS.
  Wake/drop user wakers outside pool locks; capacity changes wake pending decodes.
  CSS/SVG charges are source bytes, not measured AST allocations or total RSS.
- LoadConfig::mounts grants at most 32 absolute directory roots under named
  asset:// hosts; app remains the embedded package. Never enable ambient file://
  reads or canonicalize a string and then reopen through that ambient path.
  cap-std owns root-relative resolution; decode UTF-8 path segments once and
  reject separator/drive/device-name aliases. Only regular files are read.
  Per-file encoded budgets and transport deadlines still apply. Acquire a file
  permit before spawn_blocking and move it into the actual closure: cancelling
  its JoinHandle must not refund a still-running OS operation. Check cancellation
  between bounded chunks. No OS-call interruption claim. file_active exposes
  lingering reads independently of Loader::stopped. Close removes idle root
  handles, leaving only individual in-flight clones until the OS read returns.
  Mount content/hardlinks are explicit grants, not an OS sandbox or atomic file
  snapshot. Windows root handles may prevent rename until Loader close.
- Desktop shell owns one native loop and independent WindowApps. Native event
  callbacks only queue semantic inputs. Observe/commit the virtual window before
  painting; do not invalidate on every mutable document borrow or paint will
  feed an endless redraw loop. Task polls are budgeted; defer repeated document
  polls to the next loop turn. Surface suspension/zero size preserve App and are
  distinct from semantic subtree Suspend. Close drops App/tasks before executor;
  renderer/surface release precedes window release. Desktop shell is target-gated
  and must not make Android builds pull its window dependencies.
- App::settle accepts 1..=32 local observe/flush passes and returns passes,
  handled and pending. Render::needs_observe is a read-only opt-in for work that
  can progress now; do not use pending I/O, animations or future RAF as its flag.
  WindowApp uses 8 passes before painting, separately from its 64 task polls.
  Exhaustion yields and schedules another turn, not an unbounded loop or error.
  Blitz includes dirty layout, unobserved revisions, queued inputs/completions,
  new Resize/Scroll interests and no-op ScrollAck in this flag. Focus cleanup
  can dirty style after resolution and requires another pass. Stable geometry
  does not require an extra redraw. A ready turn is not an atomic display or
  promise that an asynchronous completion cannot arrive immediately afterward.
- When a Blitz stylesheet changes, upgrade only pending state-only snapshots
  before Stylo stylesheet invalidation reads their classes/ids. Preserve already
  captured attributes and the original element state; no full source-list scan.
  This safe fork fix is recorded alongside the existing resource patch.
- Shell drives the document's monotonic advance_time clock; geometry resolves
  must not reset animation time to zero. Static time changes do not dirty frames.
  Shell::request_size applies Some(size) immediately (resize events are optional
  on that path); None waits for SurfaceResized. Validate native surface size,
  safe-area-adjusted viewport and actual present, not just renderer activation.
- Windowed list work/memory must depend on the visible window plus overscan.
  Stable keys preserve retained rows; at most one focused row may be pinned
  outside the window. Other leaving rows release owners and tasks immediately.
  Blur/key deletion releases the pin; no visited-row keep-alive cache. Include
  the pinned source key's dependencies even when visible keys stay equal, and
  track its new indexed key after relocation. ListStats separates range/pinned.
  Algorithm tests do not substitute for actual DOM/frame/platform tests.
- FocusChanged is a direct origin event. App broadcasts derived FocusWithin
  observations along logical ancestors (including Groups), independent of a
  control's own handler. Skip observers removed by earlier callbacks. Native
  focus transitions are ordering barriers; do not coalesce A/B/A out of order.
  Live suspended scopes still receive terminal FocusChanged(false) and
  Composition(false) to release pins/editing; disposed identities receive none.
- Blitz controlled value/checked attribute edits must not bypass their property
  path. Equal text echoes acknowledge revisions without resetting the editor.
  Composition defers differing program values; terminal input carries its old
  revision, so a newer program write wins. Empty Preedit ends/cancels immediately;
  a differing program write supersedes the trailing Commit, including repeated
  empty Preedit. Preserve selected replacement on cancellation. Normalize input
  line endings at the native boundary and retain caret offsets on paste cleanup.
  Native IME offsets are UTF-8 bytes; validate boundaries before driver calls.
  Enable complex-scripts segmentation for the selected engine. Synthetic IME
  protocol tests do not prove an OS IME or an Android input connection works.
- Navigate carries its origin NodeId and routes to the nearest logical handler.
  Web maps unmodified arrows/Home/End/Page keys only on virtual root/row shells,
  never on descendant controls (including unowned DOM children). Tab remains
  native. Collection focus/scroll actions run after the target rows commit.
- FocusMode and ItemPosition are semantic metadata, not HTML options. List roots
  are tab stops, rows accept programmatic focus; logical index/total update in
  the existing row coordinator, without extra per-row reactive nodes. A count
  change must update item totals even when the window keys compare equal.
  Generic Role::ListItem alone must not invent a tab stop.
- A measured-row navigation keeps one reveal intent after its first measurement:
  placeholders can resize again when images/fonts arrive. Controlled position
  echoes acknowledge the last reported logical offset; they must not re-seek
  or cancel that intent. Report only changed logical offsets. The intent may
  correct its target after Resize, but
  user scrolling, data/program-position changes or a later focus transition to
  another row/outside cancel that intent. Deleting focused data returns focus
  to the collection when the host supports it.
- Scroll observations carry the last ScrollAck (requested and actually accepted
  coordinates) separately from current offsets. Keep it on later observations:
  native movement can race the first acknowledgement. Web reads back the setter's
  accepted offset at the action barrier; Blitz does the same. Per-live-node
  metadata is bounded and disposed with the node. List preserves quantization
  bias within a destination and resets it on a new navigation/external seek;
  never widen a tolerance to swallow a genuine 1px input. Runtime Event metadata
  is separate from the HTML Snapshot format.
- Measured lists use sparse height deltas and a bounded metadata cache;
  unknown/evicted rows use estimates. A stable-key locate callback must be
  indexed, never an implicit full-data scan. Compute the anchor/window before
  reconciling rows in the same controller to preserve identity during prepend.
- List Cache keeps committed order positions on live row entries only. Capture
  old positions while validating next-window keys; monotonic retained order
  needs no LIS. Actual inversions still use LIS for minimal moves. Write the
  new order during the existing mount/update loop, not another per-row hash
  lookup. Duplicate/pin validation precedes row effects; disposed keys retain
  no order metadata. Never rebuild a cloned old-key map on ordinary scrolling.
- List::row_limit defaults to 4096 window rows including overscan, plus at most
  one focused pin. Check the actual initial position before key/factory access;
  dynamic RowLimit/Allocation errors release rows and tasks and permit recovery.
  Never silently truncate an oversized viewport. ListStats inspection reports
  successful windows; on failure use actual nodes/tasks to verify cleanup.
  The measurement cache limit and row limit are independent. Neither limits
  arbitrary allocations inside user row factories or ordinary nonvirtual Each.
- Resize separates fractional border-box size from client viewport dimensions.
  Browser row measurements prefer observer borderBoxSize (untransformed).
  Scroll consumers additionally observe content-box changes for scrollbars.
  Clip the physical list surface without introducing an inner scroll container.
- Snapshot adoption validates the entire initial subtree before attaching
  runtime metadata. Never replay initial value setters or Place commands over
  existing DOM. Version/structure mismatches preserve it and diagnose the key.
- HTML snapshot format is now v2: visible final-tree preorder keys, hidden nodes
  get trailing keys. Final ordering must not depend on async completion order.
  Stream shell uses gly:stream markers and cannot be hydrated before End applies
  its rekey map/resources and converts the markers to the final HTML version.
- Stream polling supplies backpressure: return shell before polling loaders,
  poll only ready local tasks while demanded, and yield each committed patch
  before polling another producer. Never accumulate an eager HTML channel.
  Bound task/ready sets and per-frame bytes; cancel/Drop disposes App before
  closing its executor. Only ids/wakers cross threads, never UI state.
- Include the current BASE_CSS with SSR snapshots. VirtualPart previews flow
  before activation; browser consumers set data-gly-live only after initial
  geometry settles. Anchor events restore an owned row and a pixel inset.
  Skip physical list actions when the host lacks scroll or the subtree is
  suspended; do not wait for acknowledgement of an unapplied action.
- Dynamic factories must enter the component's Owner. App::mount_with,
  Widget::build, each/when/list factories and task polling do so. A Cage::map
  made outside construction belongs to the source Owner; do not accidentally
  put per-row maps of global sources into that global lifetime.
- Structural controllers run before prop bindings, which remain pure. Source
  reads while constructing children must be untracked to avoid subscribing an
  entire list to every row. Do not allocate reactive nodes in pure computations.
- UI Scope wraps the logical core Scope and adds semantic parent identities;
  neither may contain physical DOM/GPU objects. Renderer actions require
  declared capabilities and run after mutations, before query barriers.
  Focus is a best-effort intent: ordinary Unsupported focus refusals do not
  fault App or discard later intents. Other backend failures remain fatal.
  Query::Focus/QueryValue::Focus(bool) captures actual control document focus
  at acceptance, not FocusWithin, OS activation or display completion. Preserve
  its query barrier; never infer success from focus() returning or emit fake
  FocusChanged(true). Web reads the control's actual Document/ShadowRoot root.
- Keep physical anchors, HTML options and engine layout inside their proper
  outer layers. No old web-csr/web-ssr/backend-command/single-app aliases.
- Render::style_profile defaults to NONE; Memory uses RECORD and HTML hosts use
  HTML. An App snapshots its immutable profile once. Preflight create/StyleSet
  before model/queue changes; invalid identities win over patch style errors.
  Adding Style fields must update the exhaustive profile field list and tests;
  finite values/units include implicit px declarations. Non-style patches do
  not rescan styles. Css dialect metadata is not a Portable stylesheet parser
  or a guarantee that engines produce identical pixels.
- Typed HTML options live in html; the facade's html feature connects them to
  UI builders. web and ssr are composable features, unlike the retired Node
  aliases. Never use textContent/innerHTML to patch a label with user children.
- Event subscriptions belong to their registering Scope, even when the target
  belongs to another Scope. Resize observers follow live event interests.
- Virtual lists disable native scroll anchoring; preserve actual fractional
  scrollTop deltas without scaling them by the logical total. Pixel reads may
  have native zoom/DPR quantization; distinguish it from framework drift.
- No new unsafe code in Glory; workspace denies it.
  User-authorized exception (2026-10-26, Apple follow-up): the genuine
  glory-rs/winit UIKit fork may add the minimum Objective-C UITextInput
  protocol bridge declarations/FFI required for native IME. Keep safety comments,
  main-thread ownership and native/model tests. This does not authorize unsafe
  in Glory runtime crates or unrelated fork/platform code.
  User-authorized exception (2026-10-26): only the Android application entry
  in examples/mobile may use #[unsafe(no_mangle)] with a local lint allowance
  to export android_main. No unsafe blocks or runtime-crate exceptions.
  User-authorized exception (2026-10-27): only examples/mobile/src/ios_permissions.rs
  may declare the six glory_permissions_{open,begin,poll,cancel,close,stats}
  functions in one locally allowed unsafe extern "C" block, each a safe fn using
  only i64 arguments/results (or unit). Swift validates every integer and owns
  native objects/main-thread dispatch. No pointers, unsafe call blocks, broader
  application exceptions or runtime-crate unsafe are authorized.
  User-authorized exception (2026-10-27, iOS files/clipboard): only
  examples/mobile/src/ios_io/mod.rs may add one locally allowed unsafe extern "C"
  block declaring safe fn glory_mobile_io(i64,i64,i64,i64,i64,i64)->i64. Swift
  validates all integers and owns native objects, scopes, threads and admission.
  No pointers, callbacks, unsafe call blocks, other declarations or runtime-crate
  unsafe are authorized by this exception.
- Optional `render::profile::Profile` is local and fixed at 31 aggregate phase
  samples, not an event/row/frame trace. App::set_profile shares it with runtime
  and supported adapters; None performs no profiling clock reads. Clock callbacks
  must be lightweight/monotonic and never reenter UI/graph work. Native uses
  Instant, WebApp::enable_profile uses that document's Performance clock and
  rejects a missing clock. Do not use std Instant at runtime on wasm32.
- Profile times are inclusive and overlap; do not sum them or label a phase max
  as p95/frame time. Reconcile units are successful RowHost key reads (often
  cached), not database/source accesses; record source reads separately.
  Validate units are prior dependency counts; Commit units are mutations.
  Blitz Layout includes Dom::resolve; its 13 nested engine phases distinguish
  style/damage/construction/box layout/transforms/paint tree/cleanup/hover/child
  documents and actual Parley shaping/breaking/alignment. TextShape covers only
  inline builder.build_into (UTF-8 byte units), not editor/password/marker work
  or all text preparation. Parallel worker totals may exceed wall time. Import
  fixed aggregates on the UI thread through an epoch-bound Recording, without
  sending the local Glory collector to workers or reading its clock. Paint
  records a scene, Draw includes changed offscreen CPU work. Engine operations
  outside Dom::resolve, browser internals and presentation waits remain separate.
  Hover additionally times the immediate post-ScrollTo hit-test refresh. A clean
  instant programmatic scroll changes pixels/geometry, not boxes/text; promote
  it to full invalidation when hover changes. Preserve the accepted scroll ack,
  visual revision, query geometry, resource and animation invalidations.
  Metrics/reset/attachment must never invalidate UI or cause redraw. Reset
  retires live spans with a non-wrapping epoch; no history cache is introduced.
  Native list benchmarks must verify actual mounted row heights. Height on an
  inline Text does not establish variable sizes. Use identical corrected source
  for before/after binaries, compare final image sequences outside the timer,
  and compare physical/logical positions and mounted box sequences across fresh
  processes, repeats and profiling modes. Exercise cache eviction before timing
  64/1024 budgets. Use independent targets with fresh local libraries; apply any
  shared correctness fix to both sides and state the baseline explicitly,
  and retain slower cases. The earlier v36 profile fixture used inline heights;
  its adaptive-mode data is historical, not proof of 24/28/32px variable rows.

## Verification and build storage

Use `./scripts/check-semantic-ui.ps1` (optional `-Bench`, `-Web`, `-Window`). It checks metadata,
fmt, active-workspace all-feature tests and clippy. `-Web` also checks wasm32
and builds the runnable fixture. `./scripts/build-semantic-web.ps1 -Serve`
serves it locally; Playwright functions are examples/semantic-web/{acceptance,
adaptive-acceptance,hydrate-acceptance,resource-acceptance,
resource-snapshot-acceptance}.js.
The sixth script, stream-acceptance.js, uses the live native fixture's /page
and /reverse endpoints with CSP, typed drafts, fallback moves and hydration.
Browser builds alone do not run that acceptance function. Current runtime tests are explicitly
listed in core/Cargo.toml; task_host is included by tests/runtime.rs. The old
HTML test modules were removed; current UI/SSR/Web suites cover semantic
behavior, not the retired wire.
For native transport acceptance, run the independent example binary
`cargo run --manifest-path examples/semantic-web/Cargo.toml --bin stream_fixture`
then `python scripts/check-semantic-stream.py` (localhost:8789). This fixture
streams NDJSON frames, observes socket disconnect and checks a paused TCP reader
on /pressure. The same binary serves progressive HTML at /page, /reverse and
the bootstrap/WASM assets from D:/BuildCache/glory-semantic-ui/web-demo.
It is an acceptance fixture, not a production HTTP framework integration.
The seventh browser script, focus-acceptance.js, checks fixed/measured virtual
row pinning, real focus/selection, composition protocol, indexed prepend,
offscreen deletion, owned task limits and release. Actual platform assistive
technology, OS IME and device acceptance remain separate requirements.
The eighth, navigation-acceptance.js, checks real keys, batched navigation,
segmented Home/End, native editor caret and controls/Tab behavior, measured
visibility, deletion fallback, logical ARIA metadata and the Chromium AX tree.
AX role/name/focus evidence does not prove a screen reader announces position
and total; keep actual platform assistive-technology acceptance separate.
The ninth, budget-acceptance.js, expects typed RowLimit diagnostics, checks
actual DOM/task cleanup, maximum queried source index and subsequent recovery,
and verifies typed background-color preserves a stylesheet background-image.
The tenth, intrinsic-acceptance.js, uses real PNG decode/ResizeObserver with
intercepted responses: placeholder growth/shrink, late End target growth,
controlled position echoes, scroll/seek cancellation, bounded far seeks and
cleanup. It proves browser layout behavior, not real network cancellation.
Native resources/loader tests separately cover injected PNGs and gated real HTTP.
The eleventh, profile-acceptance.js, verifies browser Performance-based phase
counts, disable/reset/cleanup and huge-list source reads. It deliberately leaves
browser-internal Layout/Paint/Draw at zero instead of inventing measurements.
The twelfth, target-acceptance.js, verifies eight semantic targets, 32 asynchronous
queries, boxless Group rejection, invalid interests, continued editing and cleanup.
The matching native/reference/SSR tests use optional glory-render/conformance.
The thirteenth, event-acceptance.js, rejects malformed geometry/values and
unrelated/foreign origins, exercises slot reuse, then checks query/edit recovery
and cleanup. A separate fake-host test proves errors do not discard the rest of
an already drained input batch; direct browser dispatch is not that same test.
The fourteenth, input-acceptance.js, checks seven HTML purposes, normalization,
selection, explicit/bound versions, stale/future edits and commits, composition
echoes, and native-generated SSR adoption. Failed adoption preserves dirty DOM;
successful adoption preserves identity/selection. This is not OS IME evidence.
The fifteenth, focus-result-acceptance.js, checks actual Focus replies, ordered
snapshots, hidden/inert/transparent and programmatic targets, Check controls,
preventScroll, ShadowRoot ownership, disabled/disconnected recovery and cleanup.
The sixteenth, semantics-acceptance.js, uses one Rust form in CSR and real SSR
adoption. It records Chromium AX role/name snapshots, Name/Role Keep/Clear,
native-label fallbacks, actual Tab policy, preserved pre-edit identity/selection
and zero value rewrites, fifteen rejected metadata cases and later recovery.
These are browser accessibility-tree and native engine/editor evidence; actual
platform assistive-technology bridges/announcements remain separate.
The seventeenth, size-acceptance.js, uses the same Rust form for CSR/SSR adoption:
Limit None/Px/Percent, min-over-max precedence, stylesheet fallback, Keep with no
style writes, invalid finite-domain refusal, editor identity/dirty value/selection
preservation, recovery and cleanup. Percent cases have a definite containing box;
they do not establish every intrinsic/indefinite sizing combination.
Release baselines: cargo bench -p glory-ui --bench semantic_frames; native
offscreen CPU: cargo run -p glory-blitz --example profile --features raster --release.
Both separate setup/cold/warm work and expose aggregate inclusive phases; their
logical update count need not equal the number of actual draws.

On Windows, all build artifacts, Cargo home and temporary compiler files must be on D:.
For the macOS support work requested on 2026-10-26, use the local data volume
`/Volumes/Data/Caches/glory` for artifacts, Cargo home and compiler temporary
files. Resolve paths before use and reject symlink escapes. Apple SDK tools may
read the existing Xcode installation; select it per process with DEVELOPER_DIR.
Do not require an unavailable Windows SMB share on a native Apple host.
Use D:/BuildCache/glory-semantic-ui and verify target_directory with
`cargo metadata --no-deps --format-version 1` before new build configurations.
Never introduce a C: target/cache/temp directory. Docker storage must likewise
be on D: or remote before any Docker build.
For WSL use scripts/check-semantic-linux.sh with an existing GLORY_BUILD_ROOT
under /mnt/d/BuildCache; it verifies either the D: mount or a loop device's
D:-backed image, then checks cargo metadata. GLORY_ARTIFACT_DIR can point to a
Windows-readable D: directory; --paint exports actual CPU PNGs. This task uses
the verified D:/BuildCache/glory-semantic-ui/linux-build-cache.ext4 image for
Cargo's Unix metadata operations, not the Ubuntu distribution's root disk.
Use --window to run separate X11/Wayland window acceptance on a live display.
Window gates run both shell and accesskit variants. The Windows -Window gate
also runs real_accesskit_windows_ui_automation: a separate hidden-console .NET
client drives two owned test windows and emits a unique D: JSON report. Do not
count its internal tree tests or Unix adapter initialization as reader speech or
AT-SPI client acceptance. Keep the initial-tree requirement separate from a
geometry invalidation so a move/resize does not force a full AX tree resend.
The task's optional X11 runtime libraries live in the D:-backed runtime/ directory
and are passed via LD_LIBRARY_PATH; no system package install is implied.

Use cargo fmt --all for active Rust sources. Run tests appropriate to changed
behavior. Platform cargo check proves compilation only; record real runtime,
IME, lifecycle, layout, accessibility and device evidence separately.

## Commit and PR conventions

The twenty-fourth browser suite, query-acceptance.js, shares query_form between
CSR and real SSR adoption. In each mode accept 102 requests, deliver only the two
live replies and release all captures; cancel 100 when their requesting branch
retires while the external input remains live. Preserve that input's identity,
value and selection, bounded reactive arenas and empty query indices. Native
tests separately exercise delayed/duplicate replies, target cancellation races,
barrier-driven requester retirement and huge-list row teardown. These checks do
not measure allocator/RSS, engine frame time, OS IME or screen-reader speech.

The eighteenth browser suite, flex-acceptance.js, shares render/conformance's
46 definite-size flex cases with native tests. It checks both CSR and SSR
adoption, then 36 in-place style changes and 10 text/overflow cases. Require
two actual child boxes and finite errors; an empty scope is not layout evidence.
Custom Widgets that create a child Scope directly must retain it for the mounted
lifetime. The default runtime does not allocate this optional test matrix.

- Preserve unrelated working-tree edits.
- Imperative scoped commits, e.g. `feat(core): add owned reactive bindings`.
- No `[codex]` PR title prefix unless explicitly requested.
- Use body files/heredocs for multiline GitHub text; attach any created PR.
- Report remaining report/goal requirements honestly; do not mark the full
  objective complete based only on the new leaf crates or a green unit suite.
