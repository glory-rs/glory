# glory-blitz

Native document interpreter for Glory's semantic render port. It
applies the shared `glory-html` mapper's typed edits directly to Blitz nodes;
updates do not serialize and reparse the document. Optional shell adds a desktop
window host with CPU presentation; the headless default creates no window/GPU.

The shell also exposes `Shell::android(AndroidApp)` and `Shell::from_event_loop`;
its existing `Shell::new` supports the UIKit entry path. The
[mobile example](../../examples/mobile/README.md) records Android emulator
packaging/interaction checks and Apple-host iOS simulator/pinyin IME evidence. Optional `package`
provides the same bounded offline Loader on desktop and mobile. Optional
`android-assets` adds explicit APK AssetManager reads. Optional `network` shares
HTTP(S) on desktop, Android and iOS; file mounts and AccessKit remain desktop
integrations.

```rust
use glory::{App, prelude::*};

fn main() -> glory::ui::Result {
    let app = App::new(glory::blitz::Renderer::new);
    app.mount(text().content("Hello"))?;
    let turn = app.settle(App::DEFAULT_SETTLE_PASSES)?;
    assert!(!turn.pending); // This finite text fixture has no feedback loop.
    Ok(())
}
```

Enable the facade's native-only `blitz` feature. Run the working example with
`cargo run -p glory --example semantic_blitz --features blitz`. Default Glory,
HTML and SSR applications do not depend on this engine. All build artifacts
must use the D: paths described in the repository's AGENTS.md.

Current boundary:

The pinned local engine supports Password through a shared extended-grapheme
display projection: one bullet per grapheme, with display geometry for paint,
pointer/keyboard navigation, selection, caret and scrolling. Values and UTF-16
query offsets stay in the original text. Copy/cut are blocked and paste works.
Switching Purpose preserves the control, selection and active composition.
Native Input placeholders use an independent bounded, lazily shaped hint layout,
including multiline and empty password Inputs. Dynamic hint updates preserve
editable values, selection, composition and control identity. Hints are neutral
gray, clipped to the content box, and never enter IME/clipboard/pointer geometry.
`:placeholder-shown` follows live editor state; AccessKit exports a separate
placeholder property, preserving names and protected values. See
[the contract](../../docs/input-placeholder.md) for the finite styling policy.
IME preedit is masked and underlined; cursor-area/purpose requests reach the
host, which coalesces unchanged updates. OS IME/assistive-technology acceptance
remains separate from this editor protocol and CPU/window evidence.

| Facility | Current implementation |
|---|---|
| Style and layout | Blitz CSS cascade, Taffy boxes and Parley text; system fonts; explicit viewport/DPI |
| Incremental nodes | Create/patch/place, comment-delimited Groups, offline suspension and full subtree disposal |
| Forms | Controlled value/checked/disabled, including deferred attachment and multiline initialization; moves preserve identity |
| Geometry | Bounds for boxed nodes, Scroll queries/actions, coalesced Resize/Scroll observations; unresolved layout runs once before reads |
| Unsupported physical queries | Group has no box; standalone headless selection is Unsupported. WindowApp returns UTF-16 editor selections |
| Focus readback | WindowApp returns actual control document focus at the query barrier; standalone focus is Unsupported. Focus requests alone do not imply success |
| Native input/focus | Standalone Renderer has no driver. WindowApp enables focus/input, queues typed editing/checkbox/commit/composition events, Tab traversal and logical list navigation |
| Resource loads | Explicit Config base URL/provider, bounded cache/pending/decode/font budgets, failure/cancel/close and host wake. Default installs no HTTP/file loader |
| Text clipboard | Optional `clipboard` (facade `blitz-clipboard`), explicit Config service; semantic CopyText and shell copy/cut/paste. Desktop worker/macOS round trip and Android safe-JNI/ARM64 foreground system round trip verified; iOS driver and mobile menus remain open. See [native clipboard](../../docs/native-clipboard.md) |
| File services | Optional `files` (facade `blitz-files`), explicit Config service; desktop picker/drop and Android safe-JNI picker/PFD reads, bounded chunks and private PNG/JPEG previews. Android owned-source SDK chain verified; actual system dialog/drag input, iOS driver, sandbox security scopes and readers remain open. See [native files](../../docs/native-files.md) |
| Drawing | Optional `paint`: complete AnyRender frames; optional `raster`: Vello CPU premultiplied sRGB RGBA8 |
| Window/platform | Optional desktop shell: winit + Vello CPU/softbuffer; default creates no window |
| Platform accessibility | Optional `accesskit` (facade `blitz-accesskit`): semantic roles/names/values, collection metadata and Focus/Click/SetValue; virtual-row ScrollIntoView uses the shared list controller, then native ancestors/viewport. Windows UIA and macOS AX clients tested. Linux AT-SPI clients and screen-reader announcements remain pending |

`Renderer` is a cloneable handle to one document. `set_css` replaces a single
author sheet; `set_viewport` uses physical dimensions plus DPI. `stats` records
live engine nodes, semantic ranges, observation interests, layout calls and
controlled-property writes. `inspect` provides read-only engine inspection;
engine identities stay outside core/UI state. Drive `App::pump` or bounded
`App::settle` explicitly in this headless configuration. Listening for activation does not create a native
input driver; WindowApp provides the input/event-loop bridge separately.

The pinned engine derives from upstream `0.3.0-beta.4`. Engine fixes live in
[glory-rs/blitz](https://github.com/glory-rs/blitz); fractional flex/gap fixes live
in [glory-rs/taffy](https://github.com/glory-rs/taffy). Glory uses immutable Git
revisions with upstream package names and consistent shared type identities.
See [fork maintenance and exact revisions](../../docs/engine-forks.md).
Core/UI types contain no engine objects. Linux
system-font discovery uses Fontique's `fontconfig-dlopen` feature: the target
runtime needs Fontconfig and fonts, while cross-compilation does not probe the
Windows host's pkg-config. Actual document/pixel tests also run on Ubuntu/WSL
with system fonts; this does not prove display-server/window acceptance.
The selected upstream separates its document, paint and shell
layers; see the [Blitz architecture](https://github.com/DioxusLabs/blitz#architecture).

The independent `blitz` v4 inline style profile rejects unimplemented sticky
offsets and automatic anchoring; explicit anchor disabling remains supported.
Maximum sizes use `Limit::{None,Px,Percent}` rather than invalid `auto` values;
arbitrary stylesheet coverage follows the engine. It is not a Portable CSS
validator or a promise of pixel equality with browsers. The pinned-version
adapter compensates beta.4's own-scroll subtraction for a scroller's Bounds;
the regression checks that its border box stays fixed while content moves.
Full transforms/writing modes and cross-engine pixel behavior need separate
acceptance.

`cargo run -p glory-blitz --example window --features accesskit` enables the
desktop platform bridge without changing application builders. Core/render/UI
contain no AccessKit objects. The adapter creates its platform connection before
showing the window, queues foreign-thread requests onto the existing event loop,
and removes the connection before closing its window. Only an active client
causes tree export; unchanged visual revisions skip work, and changed snapshots
emit only changed AccessKit nodes. Deactivation frees the snapshot while keeping
the monotonic ID allocator, so late requests cannot hit newly recycled rows.

The tree follows semantic nodes and flattens logical Group ranges, not Blitz's
physical wrappers. Password inputs expose their protected role with no raw value.
List indices stay logical: AccessKit receives zero-based item positions and a
known total on the nearest list container. Export visits live nodes and their
geometry, never the indexed business source. Tests retain one offscreen focused
row across far jumps without changing its accessible identity or reading extra
keys. Removing that row retires its accessible action target.

Run the ignored `real_accesskit_windows_ui_automation` test with `--features
accesskit -- --ignored --nocapture --test-threads=1` on Windows for a separate
.NET UIA client driving two owned windows. This establishes native Invoke,
Toggle, Focus, SetValue, password protection, bounded list hierarchy and cleanup.
It does not establish spoken announcements or a complete text-selection/TextPattern
implementation. Unix uses the AccessKit AT-SPI adapter; a running native window
or successful build alone does not prove an AT-SPI client consumed the tree.

`App::set_profile` also connects the optional shared phase collector to this
document. Layout measures Glory's combined style/box/text resolve call; Paint
measures scene recording, and Draw covers changed offscreen Raster work,
including nested layout/paint and buffer setup. Unchanged Raster pixels skip
all three phases. Resource consumption is measured separately from network
waits. These inclusive totals overlap; they are not exclusive CPU slices.
The 13 nested engine aggregates below also cover the shell's Dom::resolve
calls. Direct engine work outside resolve and desktop paint/present need their
own instrumentation; zero samples there do not imply zero platform cost.

Run `cargo run -p glory-blitz --features raster --example profile --release`
for fixed/measured, million/100-million-row, 160×120 CPU-image runs. It separates
document setup, first content/font use, warm-up and 100 logical seeks, repeats
each case three times, and records actual draw counts, node bounds and backbuffer size.
The same binary alternates the previous pump/draw pipeline and a bounded
observe/flush-before-draw pipeline, three runs each. Measured rows retain the
necessary layout passes while avoiding intermediate paints. No OS window or
compositor is involved; phase maxima are not frame percentiles or FPS.

`App::settle(limit)` accepts 1..=32 passes (default 8) and returns
`Settle { passes, handled, pending }`. Each pass observes inputs and flushes
their changes. If `pending`, schedule a later host turn; do not loop until
stable without a budget. The shell does this automatically before painting,
independently of its 64-task-poll budget. A custom host can draw its latest
state on exhaustion and yield. No async task polling occurs inside settle.
Queued query replies, geometry changes and completed resource messages can
progress locally; incomplete network I/O and animation time cannot. Web keeps
its asynchronous RAF/ResizeObserver scheduling and uses a single local pass.
Settling is not presentation acknowledgement and cannot exclude a resource
arriving after it returns. Raster::draw still does not flush staged App changes.

Measured lists also observe intrinsic image-size changes. Shared UI logic keeps
one adaptive navigation reveal intent beyond a placeholder's first measurement,
until native movement, an external seek/data change or focus elsewhere cancels
it. Controlled on_scroll echoes acknowledge the reported logical value without
re-seeking. Scroll events retain the last ScrollAck (requested/accepted offsets)
separately from current offsets; quantization must not hide real 1px movement.
The adapter retains one acknowledgement per live scrolling node, not its history.

PNG regression coverage has no fixed image height: above-window growth preserves
the same anchor's -7px inset; a last row growing 24→96px after End stays within
the 180px viewport at y=84. Oversized targets align at their start. Injected PNG
tests cover late/stale results, shrink, width changes, measurement eviction and
40 far seeks in a 100-million-row source. Another headless WindowApp test uses
gated real loopback HTTP for anchor/End growth. Browser acceptance independently
uses PNG decode/ResizeObserver with intercepted responses. These are document
and browser-layout results; the real-window gate remains the editing/lifecycle
fixture, and no network FPS, OS IME or assistive-technology result is implied.

## Resource boundary

`Renderer::with_config(runtime, Config)` and `WindowApp::with_config(Config)`
install an explicit base URL, `ResourceLimits` and optional provider. The default
base is `glory://app/`, with no network/filesystem access. Transport implementers
use the adapter's `assets::{NetProvider, NetHandler, Request, Bytes, Url}` exports;
they need no workspace-wide Cargo patch or dependency on core/UI internals.

The current engine limits are 64 decoded-image reuse entries / 16MiB payload,
128 pending image URLs, 4096 waiters per URL, 4096px per image dimension and 16MiB
decoded RGBA per image. External font registrations allow 64 successful loads /
32MiB per document; invalid font data consumes no successful-load quota. These
limits are configurable. Reuse-cache bytes exclude live node ownership, engine
metadata/system fonts and backend conversions; entry limits bound cache metadata.
SVG is not enabled and its opaque tree is not charged as a guessed RGBA size.

All external resource handlers now share 256 request leases (fetching, decoding
and completed), a 32MiB completion charge and an 8MiB encoded-input limit.
`ResourceStats::{requests, completions, completion_bytes, decode_waiters}` exposes
these independently of cache/pending-image counts. `ResourceLimits::{requests,
completion_bytes, encoded_bytes}` configures them. Limits cannot shrink below
live leases; increasing capacity wakes waiters. Request IDs are document-local
and never wrap. Request-slot exhaustion refuses admission before fetch or a
critical-CSS registration; refusal cannot enqueue another unbounded error stream.

Providers should await `handler.prepare(&resolved_url, &bytes)` before calling
`bytes`. Loader includes that wait in its existing deadline/cancellation path.
It waits before parsing/decoding and without holding CSS locks. Direct synchronous
providers still enforce the limits, but complete with an error if capacity is
unavailable instead of waiting. Each queued response holds its lease until the
host consumes/discards it or the receiver closes. Cancelling a queued response
marks it stale and keeps its charge until the actual payload drops; refunding on
cancel alone would let repeated scrolling exceed the budget. Resource payloads
drop before their lease wakes another producer. Close retires admission, wakes
waiters, drops the receiver/results and closes the provider.

Raster charges RGBA payload; fonts charge bytes plus alias; HTML charges retained
UTF-8 bytes; CSS/SVG charge source bytes, with a count limit on their opaque
structures. Diagnostic URLs/errors retain at most 4096/512 UTF-8 bytes and cannot
retain a larger String capacity. CSS/SVG AST overhead, decoder scratch, queued
request metadata, live nodes/fonts/cache and paint surfaces have separate
ownership; this is not a total-process RSS budget. Glory does not enable SVG/WOFF.

`LoadConfig::mounts` explicitly grants up to 32 read-only directory roots using
named `asset://` hosts. `app` remains reserved for the immutable embedded package:

```rust,ignore
let loader = Loader::new(LoadConfig {
    mounts: std::collections::BTreeMap::from([
        ("media".into(), std::path::PathBuf::from("D:/MyApp/assets")),
    ]),
    ..Default::default()
})?;
// Config base_url: asset://media/; image source: pictures/photo.png
```

Roots must be absolute and exist at construction. A cap-std directory capability
owns resolution, including symlink containment; no canonicalize/reopen race or
ambient file:// access. UTF-8 path segments decode once, rejecting encoded
separators, drive/ADS syntax, controls and portable Windows device-name aliases.
Only regular files are read; directories, sockets and FIFOs are refused. Query
and fragment remain URL identity but do not alter the file path. A hardlink
within the granted tree is itself a granted file; mutable content is not a
transactional snapshot. Use the embedded immutable package when that is needed.

File reads share transport/body/deadline and decoded completion limits. Before
spawn_blocking they acquire one of `workers` file permits; the actual blocking
closure keeps it until return, even after async cancellation. Cancellation is
checked between 64KiB chunks; a stuck OS filesystem call cannot be forcibly
interrupted. `file_active` / `file_peak_active` report actual operations, including
cancelled ones that have not returned, separately from queued/active transport
and stopped. Close releases idle root handles and wakes permit waiters; in-flight
reads retain only their own root clones until return. No UI state crosses threads.
On Windows the selected root handle prevents rename until close; on Linux an
opened root remains the original directory after its pathname is replaced.

Windows junction and Linux symlink escape tests, root replacement/close, nested
CSS and relative PNG loads, and fixed/measured 100-million-row file-image lists
have direct acceptance tests. Linux additionally retargets symlinks concurrently
and rejects FIFO/socket entries. These are document/filesystem tests, not an OS
window, mobile asset, RSS or FPS result. Run the asset window example with no
argument for its embedded theme, or grant a directory containing theme.css:

```powershell
cargo run -p glory-blitz --features assets,shell --example assets -- D:/MyApp/assets
```

The engine cancels image loads without a live consumer; suspended background
layers retry on Resume. Stale request IDs cannot fill a new waiter, even for the
same URL or recycled node slot. Failures complete critical CSS requests too.
The provider's cancel/close hooks must stop queued work/revoke publication.
PNG/JPEG/WebP codecs are enabled; image dimensions are checked before decode/RGBA
expansion. The CPU renderer has a separate upstream conversion-cache policy.

Resource handlers use a stable host-waker relay: a request started before a
window exists can wake its later host. WindowApp registers it each turn; custom
hosts call `observe_resources`, then `App::pump` or `poll_resources`. Completions
invalidate visual state without a business Cage write. Raster draw ingests them
before its unchanged-pixel shortcut, and frame metadata follows ingestion/layout.
Staged UI mutations still require App::flush/pump. No-op polls do not redraw.

`tests/resources.rs` injects delayed resource bytes through real engine handlers,
including a worker-thread completion, CSS imports, fonts, background images,
actual PNG geometry/pixels and a 100-million-row fixed/measured list with unique
image URLs. It bounds reuse/pending/waiter tables, cancels abandoned rows and
checks suspend/resume, stale completion, invalid input and close cleanup.

The optional native `package` feature adds `assets::{Loader, LoadConfig,
LoadLimits, LoadStats}` (facade: `blitz-package`). A Loader belongs to one document;
put it in `Config::provider`. `asset://app/` serves the explicitly supplied,
immutable `LoadConfig::package`; keys are encoded paths such as `/theme.css`.
Choose that base URL for package-relative stylesheets/images, with bytes supplied
by `include_bytes!` or the application's own packaging. Desktop, Android and iOS
use the same queue, deadlines, decode admission and cancellation. Package-only
builds reject HTTP origins and file mounts at construction, and do not compile
the HTTP/TLS/filesystem transport dependencies. No Android AssetManager or UIKit
bundle discovery is installed. The mobile example embeds CSS and PNG bytes.

Native `network` (facade: `blitz-network`) includes `package` and adds explicit
HTTP(S) origins on Windows/Linux/macOS/Android/iOS. Desktop `assets` (facade:
`blitz-assets`) includes `network` and adds mounted-file transport. Run the packaged CSS/window/list example with
`cargo run --locked -p glory-blitz --example assets --features assets,shell`.

Android `android-assets` (facade: `blitz-android-assets`) includes `package` and
exposes `AndroidAssets` on Android. Bind this invocation's application-scoped
`AndroidApp::asset_manager()` with an explicit APK prefix:

```rust,ignore
let assets = AndroidAssets::new(app.asset_manager(), "glory")?;
let config = LoadConfig { android: Some(assets), ..Default::default() };
// asset://android/theme.css reads assets/glory/theme.css from the APK.
```

An empty prefix explicitly grants all APK assets. Nonempty prefixes have bounded,
validated components. URL components decode UTF-8 once and refuse separators,
controls and empty/dot components; URL normalization cannot escape the separately
joined prefix. Unicode names are supported. The in-memory `asset://app/` map is
independent and provides no fallback. No ambient filesystem, permission lookup,
global AndroidApp or new unsafe bridge is introduced. HTTP requires `network`;
file mounts remain unsupported on Android. The safe NDK streaming API opens/closes each Asset on one
bounded blocking worker, checks declared length before allocation and enforces
actual encoded bytes, cancellation and deadline between chunks. Cancelling an
async waiter does not return admission until its actual blocking operation exits;
`file_active`/`file_peak_active` include APK reads. OS calls are not interruptible.
Responses still await `NetHandler::prepare` before the normal decode pipeline.

The mobile `android-assets-probe` fixture checks compressed CSS/Unicode paths,
PNG, missing/refused URLs, body limits, decode admission and close cancellation
through the real ARM64 emulator AssetManager, then mounts Counter with no embedded
entries. Its read-only capture script verifies the installed APK hash, success
log and actual CSS/PNG pixels. These are emulator asset checks, not networking,
permission, interaction or real-device acceptance; see the mobile README.

### Mobile HTTP(S)

The same bounded HTTP worker validates every original/redirect origin, denies
HTTPS downgrade, enforces queue/body/deadline budgets and awaits decode admission.
An empty origin list creates no HTTP client and grants no networking. File/APK
and HTTP are separate capabilities. Network-only builds do not acquire cap-std.
Requests use native reqwest/Rustls sockets, with no environment proxy, automatic
redirect/retry or decompression. Explicit HTTP origins grant cleartext access;
HTTPS chain and hostname checks use the platform certificate verifier.

iOS uses Security.framework without a separate initialization token. Android
requires `LoadConfig::android_tls: Some(AndroidTls::new(env, application)?)` before
constructing a Loader with origins. Pass an application-owned JNI Application,
not an Activity; null/wrong contexts are rejected. Include the matching
`org.rustls:rustls-platform-verifier` AAR selected by Cargo's
rustls-platform-verifier-android version. The constructor checks that its class
is available and initializes the verifier via safe JNI APIs. The capability
contains no Activity or UI state; the verifier library retains the process's
Application. The mobile example obtains the already-initialized JVM/class loader
through safe APIs and adds INTERNET only for builds using this component.

The simulator `network-probe` checks trusted public HTTPS, refusal of a local
self-signed certificate, redirects, origin/body policy and actual socket closure
on cancel/close/deadline. Its Counter fetches HTTP CSS/PNG with no embedded fallback.
These are native mobile transport checks. Runtime permission prompts, real-device
networks and signed distribution remain separate work.

HTTP(S) requires explicit exact origins in `LoadConfig::origins`. An origin
includes scheme/host/effective port and has path `/`. Every redirect is checked;
HTTP cannot redirect into a package, and HTTPS cannot downgrade to HTTP.
Only GET with empty body/headers is accepted. The HTTP/1 client has no automatic
redirect/retry, cookie store, environment proxy, referer or decompression;
non-identity Content-Encoding is rejected. Rustls uses an explicit Ring provider
and the platform certificate/hostname verifier, without changing a process-global
crypto default. TLS success on every platform is not implied by compilation.

Defaults are 4 active workers, 64 queued jobs, 8MiB encoded payload per body,
15 seconds from admission through queue/all redirects, 4 redirect hops and
4096 URL bytes. The package has independent limits of 256 entries/32MiB (paths
plus payload). Configuration has explicit hard guards. A dedicated local Tokio
runtime performs I/O and engine parsing; completions wake the current host relay.
No user business callback or UI state crosses onto it. Cancel/close drops pending
HTTP futures/sockets; close revokes queued work and shuts down the runtime without
joining it on the UI thread. Finished task entries are reaped, and idle loaders
wait for notification instead of polling a timer.

DNS resolution is shared once per configured host (maximum 64 origins), keeping
at most 16 addresses or a failure for the document lifetime. This prevents repeated
request cancellation from queuing system lookups. OS getaddrinfo itself cannot
be aborted; it can outlive close without retaining App/Runtime/window state.
Create a new Loader to refresh a DNS result. Origin authorization is not a
separate private-address/DNS trust policy.

`tests/loader.rs` uses real loopback TCP/HTTP, including peer-observed socket
closure on cancel/close, bounded queue overflow, deadlines/AbortSignal,
denied redirects, fixed/unknown-length body limits, negative TLS certificate
verification, redirected CSS/import/font/background/image resolution, and huge
fixed/measured lists with unique row URLs. `tests/resources.rs` separately checks
linked stylesheet redirect bases. Each image completion is matched by request
ID and cached under its original URL; CSS relative resources use its final URL.

Tests now pause host consumption with real HTTP PNGs, verify producers wait at a
256-byte completion budget, then cancel/consume/resume/close. Another scenario
submits 2000 imports plus 400 image failures and holds only four request/results;
critical-CSS rejection, lease/waker reentry and actual payload-before-wake drop
order have separate regressions. The huge-list test also bounds request leases
and completion charges, and requires both to return to zero after clearing.
`tests/package.rs` separately checks package-only configuration rejection,
budgets, document ownership, bounded decode admission, cancel/close
and real imported CSS/PNG layout and CPU pixels. The Android example checks that
same embedded CSS/PNG path through its actual shell and Activity recreation.
The canonical script also checks the package-only production dependency graph.
These budgets do not claim total process memory. Android AssetManager and mobile
HTTP(S) transport have the separate simulator evidence above. Deployment/CDN behavior,
broader OS input/accessibility and real-device acceptance remain pending. Mounted
files remain desktop-only and native transports are excluded from WASM;
macOS has actual window/AX evidence and iOS has simulator CSS/PNG, keyboard/pinyin,
Home/resume and rotation evidence; signed devices and production Apple
distribution remain separate.

`tests/document.rs` runs real CSS/box/text layout on Windows and Ubuntu/WSL, including a
100-million-row indexed source with fixed and measured variable heights, far
seeks, the last visible row, a native-engine 1px scroll, and complete disposal.
These are document/layout and bounded-count results. `tests/paint.rs` additionally
checks real CPU pixels, frame reuse, disposal, DPI, alpha and painted fixed/
variable-height lists. None of these are frame-rate, OS window, IME, screen-reader,
Linux display-server or Android-device acceptance.

With `paint`, `Renderer::paint` resets the selected scene and records a complete
frame after resolving layout. It does not flush an App's staged changes; call
App::flush/pump first. FrameInfo distinguishes the applied mutation watermark
from the visual revision and includes the engine document identity: two
consumers of the same Runtime must not reuse each other's pixels. A recorded
frame is not proof that pixels were displayed.

With `raster`, `Raster::draw` keeps one backend and one backbuffer. Unchanged
visual state returns rendered=false without layout/encoding/rasterization;
logical no-op commits can still advance FrameInfo.applied. A document/size
change replaces the backend/buffer, and zero-sized surfaces or clear release
them. The default backbuffer budget is 64MiB, configurable with with_limit;
dimensions beyond u16::MAX are rejected before the pinned CPU backend can
truncate them. Budget/dimension errors preserve the last good frame. This
does not limit arbitrary allocations inside Blitz/Vello or promise OOM recovery.

Pixels are premultiplied sRGB RGBA8. The native example converts to straight
alpha for PNG and generates initial/end screenshots from the same Rust builders:

```powershell
cargo run -p glory-blitz --example raster --features raster -- D:/BuildCache/glory-semantic-ui/blitz-paint.png
```

The CPU path redraws the full backbuffer for changed frames. It has no dirty
rectangle or per-node paint cache yet; finite virtual rows bound document/paint
commands, while raster work also depends on physical viewport area. External
resource or animation hosts must call invalidate when their engine state changes.

## Desktop window host

Enable `glory-blitz/shell` or facade `blitz-shell` explicitly. The initial shell
uses CPU/softbuffer, independent of the offscreen Raster API. Its Rust entry is
`shell::{WindowApp, Shell, WindowAttributes}`. WindowApp owns an App, Renderer and
ready-only local executor; move it into Shell::add_window, then call Shell::run.
The executable example contains a counter and a 100-million-row windowed list:

```powershell
cargo run -p glory-blitz --features shell --example window
./scripts/check-semantic-ui.ps1 -Window
```

Native callbacks enqueue semantic activation/focus/editing; business code runs after engine
borrows end. Geometry/scroll observations and commits run before frame paint.
Local task polls are limited to 64 per document turn. Task/dirty wakes coalesce
per host turn, and a repeated document poll is deferred to the next event-loop
turn. Unchanged polling does not advance the visual revision or request redraw.
The executor is shared with SSR's pull host; SSR keeps its own backpressure rules.

The native editing module owns revision acknowledgements, deferred program writes
during composition and UTF-8 IME boundaries. Equal value echoes preserve native
caret/preedit. Newer program writes supersede obsolete input, including a Commit
after empty Preedit; cancellation restores the replaced selection. Native paste
and IME normalize CR/LF like text controls (single-line removes line breaks;
multiline uses LF), preserving the caret on paste cleanup. The engine's
complex-scripts feature includes segmentation data needed for Chinese/Japanese.
Checkbox changes use typed bool events; Enter commits single-line text, while
multiline Enter remains native. Button/checkbox Space activates on release and
cancels when focus changes. WindowApp's enabled capability describes this
protocol implementation, not successful OS IME or assistive-technology acceptance.

`CheckMode::Mixed` has a live indeterminate property independent of checkedness.
The CPU renderer draws a dash; program checked updates preserve mixed, while
click/Space/AccessKit activation clears it and emits one boolean change. Mode
changes and Clear preserve control identity. The AX tree reads the current native
mixed state, including after activation. These
engine/adapter checks do not establish screen-reader speech or device acceptance.

Radio controls select in one direction: Click/Space/AccessKit emits one true
change on a newly selected target, and silently withdraws checked peers with the
same nonempty name, tree and form owner. Empty names are independent. Program
checked properties preserve defaults and can reassert an accepted value after a
rejected native proposal. Name/type/form changes and reparenting follow current
membership; no retired-member/group history is retained. Group discovery traverses
only the live physical host tree, never a business or virtual data source.

Without an explicit Navigate listener, native radio arrows wrap through editable
members, Home/End select endpoints and Auto focus exposes one eligible Tab stop.
Disabled/hidden/inert/modal-excluded targets cannot receive that selection.
Explicit navigation/tab policies remain with the application's controller.
`radio-probe` in the mobile example has actual simulator initial-rendering evidence;
framework-injected keys/clicks and AccessKit tree/action tests remain separate from physical
input and screen-reader speech. Typeahead and native menus are separate gaps.

Surface suspension preserves App/document; zero dimensions release CPU/surface
buffers and later resize restores them. Closing a window drops App and cancels
its tasks before the executor. CPU dimensions are checked before allocation
(u16 limit, default 64MiB RGBA budget); this is not a limit on all engine/window
allocations. `CpuRenderer::frames` counts draws that returned after softbuffer
presentation, not a compositor/vsync completion notification. The initial host
is opaque and redraws full changed frames.

`Shell::request_size` handles both winit resize paths: a returned Some(size) is
applied immediately because a native event may never follow; None waits for the
event. Geometry queries use the same monotonic animation clock as shell paint;
headless hosts can drive `Renderer::advance_time` explicitly. Static clock
changes do not dirty frames. Once attached, let the shell own that clock.

Desktop shells target Windows/Linux/macOS. Mobile shells use explicit AndroidApp
and app-owned UIKit scene entry paths, documented in the mobile example.
Android chooses one of `android-native-activity` (existing path) or
`android-game-activity` (GameTextInput IME). Do not enable both on Android.
Program input selection accepts scalar-safe UTF-16 CJK/emoji ranges in both
directions; actual shaped-cluster interiors are refused before selection changes.
`tests/macos_window.rs` is a separate AppKit main-thread process; opt in with
`GLORY_MACOS_WINDOW=1`. It checks presented frames, native resizing and Owner
cleanup. `Shell::window_stats` reads live surface/frame counters without invalidation;
`take_event_loop` lets an application-owned handler delegate to Shell.
The pinned UIKit UITextInput bridge synchronizes only a bounded current-editor
context and keeps password context empty. Real pinyin composition is accepted;
arbitrary UIKit selection gestures/rectangles, VoiceOver and signed devices
remain separate. See the implementation ledger for actual tested displays.
The explicitly ignored `real_windows_present_scroll_resume_and_release_owned_tasks`
test requires a real display, and exercises two independently owned windows,
queued synthetic pointer activation, fixed/measured lists, 20 far seeks,
controlled text echoes and a composing editor retained offscreen, logical row
Home/End/Tab and UTF-16 selection,
surface suspension/resume, zero-size recovery, native resize, worker-thread
wakeup and close cleanup. It does not claim physical mouse/keyboard, OS IME,
screen-reader, Android lifecycle, performance timing or FPS acceptance.
An additional headless WindowApp test injects native wheel events into fixed and
measured huge lists: a 1px delta stays 1px, 200 queued deltas are observed at the
final offset with fewer than 100 extra key reads and no handler-side row builds.

`App::set_profile` or `Render::set_profile` optionally attaches the local Glory
collector and a document-owned engine collector. The latter measures 13 fixed
aggregates for style, construction, box layout and actual inline text work;
results merge after resolve, including parallel construction. None allocates no
engine collector and performs no profiling clock reads. Attach/reset/detach do
not dirty the document, and clean frames reuse layout and pixels. Samples are
inclusive; text worker totals can overlap and are not display frame times.
TextShape covers inline `builder.build_into`, not every editor/font operation.
Run the `profile` raster example with `--quick` for a short workload and/or
`--no-profile` for the same workload without instrumentation. The root
`scripts/check-semantic-profile.py` saves alternating on/off comparisons and
verifies equal mounted budgets. It measures offscreen CPU work, not present/FPS.
The example verifies actual 24/28/32px row boxes; an inline Text height alone
does not establish those sizes. Its default measurement budget retains the
mounted window; `--measure-limit N` enables a larger history of measurements.
`--scroll` exercises 17px programmatic physical scrolls from the source midpoint;
default seeks can keep the physical scroll position at the segment center.
Use identical benchmark source and independent targets for before/after builds,
then `scripts/compare-semantic-profile.py` verifies final image sequences and
mounted budgets outside timing. Necessary measurement/anchor feedback remains.

### Owned permission services

Optional `permissions` (facade `blitz-permissions`) exposes a local `Permissions`
service with explicit `PermissionDriver` injection. It supports Camera, Microphone,
PhotosRead, PhotosAdd, LocationWhenInUse and Notifications, with typed status,
rationale and errors. A query never asks for authorization. Constructing a future
has no native effect; its first poll admits it. Request from an application user
action after explaining its purpose, following the [Android permission workflow](https://developer.android.com/training/permissions/requesting).

The defaults are eight pending futures, a 120-second deadline and a 20ms pending
poll interval. Only live pending futures register timers. Admission is reserved
before a foreign driver call, so synchronous reentry cannot exceed the budget.
Closing during a native poll revokes a reply before delivery. IDs never wrap.
Use an existing `Owner::spawn`; Owner retirement drops the future and calls native
cancel without retaining the Owner. Close the service on its host's cleanup;
clones share that closed state and cannot reopen it.

Cancellation withdraws delivery, not an OS dialog. Native hosts retain their
actual prompt/query permits until the operation ends. `PermissionStats::waiting`
is the local future count; `NativeStats` separately reports live reply records,
active prompts and uncancellable queries. No history of completed IDs is retained.

`AndroidPermissions::new(env, host)` accepts an explicit app-owned Java v1 host.
The example host holds its Activity weakly, runs bounded batches on Java main,
allows one native prompt, and closes on Activity destruction. Its bounded records
and request codes reject stale/unrelated results; callbacks never invoke Rust
business code. SDK-controlled ARM64 emulator queries cover Denied, Granted,
selected-photo Limited and approximate-location Limited, with zero remaining jobs.
Android's Denied also covers an initial unrequested permission. The rationale flag
is the OS value; false does not establish a permanent denial.

PhotosAdd on Android 29+ means adding app-owned media through scoped MediaStore,
which needs no general storage grant. It grants no mounted-file or arbitrary
filesystem access. PhotosRead and Location Limited keep their partial semantics.
Permission state does not start capture, recording, photo reads or location updates.

The iOS app-owned Swift host implements the same scalar service, usage-description
checks and native authorization APIs. A weak-client ledger bounds uncancellable
queries and retains prompt admission after cancellation; it has stale-result and
1,000-client-retirement tests. SDK-controlled simulator queries cover authorization,
refusal and reset. The Rust application connection is still pending approval for
one application-only integer FFI declaration; runtime crates contain no new unsafe.
The Swift host alone does not establish end-to-end Rust iOS permissions. Actual
dialog choices, restricted/provisional/ephemeral cases and real devices remain
separate acceptance, as do other platform-specific permission categories.


Native combobox refinements (`Select`, `Editable`, `Autocomplete`, `Multiple`)
connect shared Choice commands, scalar Typeahead, mouse/pen pointer observations,
listbox/tree top layers and local option reveal. Matching and controlled selection
remain component-owned. Listbox options keep editor focus; Tree popups manage
and restore it. AccessKit exports current active descendants and multiple-state
metadata, preserving password protection. See [native choices](../../docs/native-choice.md).
Intrinsic `Props::Select` is still rejected, including the hidden form proxies
used by some higher-level choice widgets; this connection does not close that
separate implementation or establish OS reader/gesture acceptance.
