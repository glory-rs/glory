# Native file services

`glory-blitz/files` (facade `glory/blitz-files`) adds an explicit `Config::files`
service. Default builds install no picker, filesystem transport or preview
provider. The shared `FileToken`, `FileRead` and `FilePreview` contracts remain
independent of native handles and browser objects.

```rust
use glory::blitz::{Config, files::Files, shell::WindowApp};
let host = WindowApp::with_config(Config {
    files: Some(Files::desktop().expect("native file worker")),
    ..Config::default()
});
```

The desktop driver uses pinned Rfd 0.17.2 native dialogs on macOS/Windows and
XDG desktop portals on Linux. An application can instead implement the safe
`FilePicker`/`FileReader` traits. Constructors must return promptly; work begins
only when polled, completions wake the host, and no UI/runtime state crosses
threads. `Files::desktop()` also connects its native drop opener. Custom services
must opt into `with_drop_driver(Arc<dyn FileDrop>)`; an injected chooser does not
implicitly grant ambient file opening. Clone one `Files` service to share admission across documents. Separate
service instances have separate budgets; custom drivers must themselves retain
physical permits for uncancellable work after their receiver is dropped.

## Selection and reading

One service admits one current picker, one native drop transfer/opener and four
reads, including pending results.
The default deadline is 120 seconds, configurable from a nonzero duration through
600 seconds. Cancellation/timeout withdraws delivery; it does not dismiss an OS
dialog or interrupt a blocking read. The desktop driver separately retains its
actual picker/read permits until the dialog or OS call finishes, even after all
receivers or the semantic document disappear. Workers have eight queued and eight
active task slots, drain admitted work on shutdown and keep no completion history.

Selected references share limits of 128 files, 128 KiB of display metadata and
1 GiB of declared file sizes. The latter is not a memory allocation measurement.
Per-control count, metadata, filter and per-file/total byte limits also apply.
Replacement is transactional: cancellation, invalid metadata or capacity refusal
preserves the previous selection. Node/configuration retirement, clear and remove
revoke tokens and release corresponding references. File and batch identities do
not wrap or reuse an earlier grant.

Only explicitly chosen regular files are opened. A display name is never a path
and is never reopened. Reads seek the original opened file, use exact ranges of
at most 65,536 bytes, and return no partial body on EOF. Replacing the original
path does not change that handle. This is a handle grant, not an atomic content
snapshot: another process may still modify or truncate the same file. MIME is
extension-derived metadata, not a trusted content classification. Known filter
extensions are combined as one OS hint; unknown MIME/wildcard hints do not hide
otherwise allowed files. Every returned selection still undergoes the declared
filter checks.

Foreign picker/reader constructors, futures and retired handle destructors run
outside the document borrow. Results retain the request's accepted watermark and
are checked against the current document, node and selected token. Shared UI
also rechecks a successful file response at delivery: an earlier event in the
same drained tick may already have cleared the grant. Scope cancellation retires
callbacks; it does not claim to interrupt physical work.

The native file button uses intrinsic focus, pointer and Enter/Space activation.
Sapid retains selection policy and attachment state in its normal widget Owners.
No host business controller or source scan is added.

## Native file drop

An interactive desktop WindowApp advertises `FileCapabilities::drop` only when
an explicit opener is configured. Headless/mobile hosts still reject declarations
requiring drop. A node must also opt into `FileConfig::drop`; Sapid Dropzone does
this on its existing real File button.

Shell forwards Winit `DragEntered`, `DragPosition`, `DragDropped`, `DragLeft`,
`DataTransferReceived` and availability events. Custom Winit embedders can forward
all their owned window events once through `WindowApp::handle_native_file_event`;
this queues input, leaving business callbacks to ordinary observation. Hover
queries only advertised URI-list metadata and advertises Copy only. It does not
fetch URIs or open files. Native physical coordinates are divided by the current
DPI/zoom before real layout hit testing; modal/focus scopes and current enabled,
visible, connected ancestry restrict targets.

Only a confirmed native drop starts a fetch. One current transfer ID and its
returned request serial match incoming data; foreign, duplicate and stale receipts
are ignored before reading the source. URI conversion runs on the native event
thread outside the document borrow. Platform `TypedData::try_as_uris` allocation
precedes Glory's transfer budget: at most the declared count (globally 32), 4096
UTF-8 bytes per URI and 128 KiB aggregate are retained before any paths are copied
or opened. Only absolute local file URIs are accepted; remote hosts, network/text
coercion, credentials, queries, fragments and raw/decoded controls are rejected.
A native local-file URI accepted by the application is an explicit path grant,
not an OS sandbox or proof that its producer came from a filesystem browser.

The drop opener returns the same stable readers as the chooser, using bounded
worker admission and a physical permit owned by the actual blocking closure.
Selection metadata/filter/size budgets and transactional replacement are shared.
Clear/remove/revision change, hiding, focus loss, surface/DPI changes and Owner
retirement revoke late acceptance. Author CSS visibility is resolved before raw
reads and after foreign work; presentation cannot revive a retired grant. A
successful chooser result retires an older drop; a successful drop retires the
pending chooser. Rejected drops preserve both the prior selection and any pending
chooser. No callback or completed-transfer history is retained.

`WouldBlock`/`Deadlock` waits for another matching native receipt, rather than
polling the OS. The normal file-service deadline reports once. Cancellation or
expiry withdraws delivery while an uncancellable raw transfer/opener retains its
slot until receipt/settlement or document teardown. Already-started blocking
opener permits remain owned by the driver through the actual OS operation.

## Android picker and provider reads

The same `files` feature exports `AndroidFiles` on Android. Create it with safe
JNI from the application-owned protocol-v1 Java Host in
[GloryFiles.java](../examples/mobile/android-game/src/main/java/rs/glory/counter/GloryFiles.java), wrap it in `Arc`, and install
`Files::new(driver.clone())` in the document's explicit Config. Clone one driver
to share its request sequence. Each Host can be claimed only once. JNI request
submission serializes ID allocation and native arrival, including concurrent
readers; there is no completed-ID set. Native replies are bounded binary metadata
or exact byte arrays, checked before Rust copies them. A completed failed or
cancelled Future releases its reply record even if its caller retains the Future.
Only a currently pending Future has an 8 ms polling timer.

The app launches `ACTION_OPEN_DOCUMENT` from a resumed, focused Activity and
accepts only the matching result with an explicit read grant and bounded content
URI list. MIME/extension hints do not replace returned metadata/filter checks.
No display name becomes a path, no storage permission is added, and no persistent
URI grant or bookmark is saved. ContentResolver query/open calls run on the
bounded worker pool. The current implementation accepts finite, seekable regular
file descriptors only; unknown sizes, pipes and non-seekable cloud streams are
refused. Reads use the originally opened PFD/FileChannel, not the URI or display
name again. This is a live handle grant, not an immutable content snapshot.
Provider/OS Cursor, String and ClipData allocations precede the bounded transfer;
the metadata/URI limits do not bound those foreign allocations.

One process-lifetime native admission ledger spans Activity coordinators. It
holds at most one actual picker/opening operation, four reads (including ready,
undrained replies), 32 weak clients, 160 charged handles, 160 KiB display metadata
and 2 GiB declared sizes. These independent native limits allow the shared Rust
service's 128-reference/1-GiB limit and a transactional replacement. Five bounded
workers also close retired sources; idle workers expire. Closing an Activity or
dropping a reply revokes delivery, while an already-running open/read/close keeps
its physical charge until the call settles. Retired sources close after their
last actual read returns. Neither operation cancellation nor these counters
claim total RSS or interruption of foreign provider calls.

Native callbacks update current records only. A monotonically increasing Android
request-code watermark is saved through instance recreation, with no URI or
authorization state; exhaustion fails rather than wrapping. A destroyed session's
late callback cannot issue reads or publish to a successor. The Activity is weak,
and foreign launch/query/read/close work runs outside ledger/document locks.

`files-probe` is a separate APK feature. It uses an application-owned selector
Activity and a non-exported fixture ContentProvider, both omitted from ordinary
APK manifests and DEX classes. The ARM64 SDK probe verifies actual activity-result
delivery, cancellation/count/type/grant/filter/size refusals, PFD reads, semantic
multi-selection/read, private PNG decode, token/cache revocation and the provider's
actual descriptor-close callbacks. It uses no injected UI input. This evidence
does not establish system DocumentsUI gestures, real devices or reader speech.
Commands are in the [mobile example](../examples/mobile/README.md).

## Image previews

Enable `FileConfig::preview` explicitly. PNG/JPEG requests first read at most
64 KiB and use the shared header parser: at most 8 MiB encoded, 16,777,216 pixels
and 16,384 pixels per dimension, with optional stricter control limits. Unknown
formats or inadmissible headers return `FilePreview(None)`. Encoded-limit and
read failures return an error. Further encoded data is read in bounded chunks;
revocation prevents starting another chunk after the current read settles.

Preview bodies, including in-progress construction and cancelled decoder leases,
share a 32 MiB encoded-byte budget per service. A private resource address binds
all fields of the selected token:
`file-preview://document-<id>/<runtime>/<slot>/<generation>/<revision>/<serial>`.
The shared validator accepts only the exact native address or a browser blob URL.
The native provider matches its current registry exactly and never delegates an
unknown private address to another transport. It has four current decode jobs,
awaits `NetHandler::prepare`, and cancels waiting preparation on resource/selection
retirement or document close. Its preparation deadline is 15 seconds.

Removing a selected grant releases its encoded body and calls the genuine Blitz
fork's `BaseDocument::revoke_image_url`: this evicts only that URL's reusable
image and rejects pending/late delivery, without scanning the document or keeping
revoked URL history. Future consumers must pass through the provider again.
Already delivered pixels may remain in live elements; revocation is not secure
erasure. Engine decoded-image, live-element and paint-backend allocations retain
their own limits. These budgets do not measure total decoder/process RSS.

The engine is pinned to
[`ee125981e0e5a217e758be5c470df23cdd9e2fc3`](https://github.com/glory-rs/blitz/commit/ee125981e0e5a217e758be5c470df23cdd9e2fc3).
The fork-owned regression proves precise cache eviction, pending failure and late
result rejection while unrelated cache entries remain usable. Winit/Taffy pins
are unchanged. No new unsafe code, vendor tree or managed Cargo source edits.

## Verification and remaining work

Evidence lives under `/Volumes/Data/Caches/glory/` in `native-files-*.log`,
`native-files-metadata.json`, and the consolidated `native-files/result.json`.
Tracked tests cover real opened-file replacement/truncation, non-abortable read
admission, document isolation, deadlines, foreign future/destructor reentry,
transactional budgets, preview header limits, actual PNG decode, revocation and
shared same-tick query delivery. Actual Sapid FileInput/FileAttachment and
FileButton consumers run across all four themes, including reading, explicit
preview, removal, cancellation/refusal and cleanup.

Reproduce with the approved environment from `scripts/apple-env.sh`:

```sh
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo clippy --locked --workspace --all-features --all-targets --target wasm32-unknown-unknown -- -D warnings
python scripts/check-engine-forks.py --test
cargo test --locked --manifest-path ../sapid/examples/native/Cargo.toml --features files --test file_driver
```

Actual desktop dialog selection/cancellation has not been accepted through OS
input. macOS sandbox security-scoped URLs/bookmarks, Windows/Linux dialog runtime,
iOS platform pickers, Android system DocumentsUI input and non-seekable providers,
actual OS drag-source/destination interaction and
assistive-technology acceptance remain open. Native drop protocol/engine tests
use application-owned native event sources and do not prove a system drag gesture. Injected-driver, CPU and compile evidence
do not establish those flows or complete the full file-service/platform gap.

Android's separate owned-source SDK evidence and APK hashes are recorded under
`/Volumes/Data/Caches/glory/android-files/`; JVM lifecycle/admission tests are run
by `examples/mobile/check-android-files.sh` on every GameActivity build.


The drop follow-up is recorded in `native-file-drop-*.log` and
`native-file-drop/result.json`. Thirteen focused tests cover nonzero, offset
1x/2x hit targets, ID/receipt admission, URI budgets, close/disable/config/Owner
retirement, actual CSS visibility, modal/focus scopes, decoder/read reuse,
reentrant native callbacks and future/destructor ownership, cross-document
admission, deadlines and chooser/drop races. The actual Sapid Dropzone test uses
Winit's real event-loop interface with an owned event source and the production
desktop opener: selected files are renamed and their original paths replaced,
yet chunk reads and PNG previews still decode the original opened data. All four
themes cover rejection preserving selection, Remove and cleanup. Actual OS
pointer/drag and reader acceptance remain separate from this evidence.

## iOS application host

Enable the mobile example's `files` feature and construct `ios_io::files()` for
an explicit `Files::new` in Config. The safe Rust application driver implements
the existing picker/reader traits. Construction opens a client only; the first
poll presents `UIDocumentPickerViewController(forOpeningContentTypes:asCopy:)`
with `asCopy: false` on the active application window. The example owns a single
Winit window; independent multiple-window/scene routing is not implied. Ordinary
mobile drop remains unsupported.

UTType mapping supplies picker filters; unknown mappings can broaden its UI,
so the original finite extension/MIME, count, size and metadata rules are checked
again after selection and before Rust adoption. Files open once under
`NSFileCoordinator`, keep the original regular `FileHandle`, and read exact
bounded chunks under coordination and a per-source lock. Each read rechecks the
original descriptor's declared size. Display names never become reopen paths;
no bookmark or URI grant is persisted. Selected external URLs must successfully
start security-scoped access; already accessible own-container URLs are allowed.
Scope stop follows actual close after the last in-flight read. This follows
[Apple's document picker coordination guidance](https://developer.apple.com/documentation/uikit/uidocumentpickerviewcontroller).

One process-lifetime ledger spans native clients. Maxima are 32 clients, one
picker, four admitted reads including undrained replies, 160 reserved handles,
160 KiB reserved metadata and 2 GiB reserved declared bytes. Opening selections
reserve their configured total-byte budget and 32 KiB metadata. Once all original
handles open, metadata and declared-byte charges settle to actual sizes; completed
small groups do not consume repeated worst-case reservations. Group charges release
when the last actual handle close settles. Close failures retain that
handle/scope/quota as unsettled work. These are admission/transfer bounds, not
total process RSS or OS-provider allocation limits. Uploads, replies and IDs are
bounded; no completed-ID history is retained.

Cancellation removes receipts, not native operations. Picker cancellation
requests dismissal of only the owned controller and retains admission through
dismissal/open completion; scene disconnect does the same. Cancelled reads keep
source/admission until actual settlement, and dropping readers schedules close
outside ledger/document locks. Native callbacks only update weak-client records;
Rust adopts opaque handles after validating the entire metadata packet. Rust
terminal futures release receipts even if callers retain completed futures.
The one authorized application-only integer ABI is shared with clipboard; it
adds no unsafe to runtime crates, pointer transfer or arbitrary path capability.

The opt-in `ios-io-probe` supplies app-owned post-selection URLs and a worker read
gate, excluded from ordinary bundles. Actual iPhone/iPad simulator runs verify
multi-selection, Unicode metadata, coordinated reads, count/type refusal and
cancellation, four held worker jobs/fifth refusal, dropped receipts, Owner
retirement without late delivery, and last-read/close quota release. The ordinary
semantic File widget also performs read, private PNG decode, Clear, token refusal
and preview/cache eviction. Native and shared grants finish at zero. Evidence,
installed executable hashes and body-free logs are in
`/Volumes/Data/Caches/glory/ios-io/result.json`.

These owned URLs are inside the application container. They do not establish
an actual system picker gesture/callback, an external File Provider's security
scope grant or iCloud hydration. Those flows, UIKit cancellation/scene behavior
under real OS input, provider mutation/failure cases, real devices and VoiceOver
remain platform acceptance work. Device SDK linking is recorded separately.

The integer ABI and reproducible host checks are specified in
[ios-mobile-io.md](ios-mobile-io.md).
