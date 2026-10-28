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

Separate `files-ui-probe` now has actual ARM64 Android 16 DocumentsUI acceptance
(2026-10-28). Native input selected only the four owned Download fixtures; no
Activity result/URI injection, private selector/provider, broad storage permission,
persisted grant or personal cloud account was used. Single TXT and TXT+PNG reads
match exact bytes; the PNG decodes and paints through the private preview path.
System Cancel preserves one/two prior grants. Selecting three documents against
max_files=2 refuses the group before opening descriptors and preserves prior reads.
Clear rejects the retired token, revokes the private cache address, releases native
and shared budgets, and balances original PFD opens/closes at 3/3.

The real picker exposed two paused-host issues, fixed in the genuine engine forks:
Winit now dispatches explicit proxy wakes while the Activity is paused; Blitz
retains the live View's document poll waker when suspending its render surface.
Redraw remains gated by surface/activity state; a paused redraw alone does not
force polling. During the system Activity, the requesting Owner actually retires
after three seconds, the outstanding picker retains admission, and a second pick
returns Busy. Returning a real late selection then releases that permit without
opening a PFD or delivering a callback. This does not keep a process alive against
Android's own freezing/termination policies.

`check-android-system-files.py` verifies ordered phase logs, the still-foreground
DocumentsUI record captured after retirement and before returning a result,
installed APK bytes, fixture hashes, absent injection classes and storage
permissions. It sends no UI input. Evidence and screenshots are under
`/Volumes/Data/Caches/glory/android-system-files/`; `system-result.json` records
APK SHA-256 `cbad8e048aa13f6422f529f4a2b0d8a610a5db40555e7e1d621dcf448a079f00`.
Real-device sandbox enforcement, commercial cloud/hydration and TalkBack remain open.

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
iOS commercial cloud/replicated provider flows, Android cloud providers and non-seekable streaming support,
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

## Actual iOS picker and external-container acceptance

`ios-files-ui-probe` now exercises the production `UIDocumentPickerViewController`
through actual Device Hub touch input. A separate `rs.glory.files.fixture` app
exposes only its owned Documents using UIFileSharingEnabled and
LSSupportsOpeningDocumentsInPlace. Apple's **Local Storage File Provider** supplies
those documents to the consumer; there is no shared app group, URL injection,
consumer-container fixture copy or cloud/account access. The application label
“Glory Provider” identifies that document fixture, not a custom extension.

On the owned iPhone 17 and iPad Air M4 / iOS 27 simulators, real Browse/Open
produces the UIKit delegate callback and container-external URLs with successful
security-scope starts. Text and PNG bytes match exactly; private PNG previews
actually paint. A nonmatching BIN is disabled by the system picker. Cancel keeps
prior selected grants usable; iPad outside-tap dismissal also settles as Cancel.
Retiring a requesting Owner while the actual sheet is visible dismisses it,
releases the picker slot after native completion and delivers no late callback.
Clear pairs every successful scope start with a stop and every open with a close,
then drains native/shared references, bytes, reads and private preview/cache state.

The dedicated probe fixes two harness omissions: File activation explicitly
calls Scope::pick_files, and clearing without a preview does not expect the
unrelated package-image cache to be empty. Temporary query child scopes are
retired after each response rather than accumulating completed response storage.
No production IO or grant imbalance was found by these flows; production runtime
crates, the integer ABI declaration and immutable engine pins are unchanged.
Test-only opcode 101 supplies body/path-free counters and is absent from normal
builds. Raw UI screenshots/logs, hashes and result checks are under
`/Volumes/Data/Caches/glory/ios-system-files/`.

This closes actual system-picker input/callback and external Local Storage
provider scope/read/close acceptance on these simulators. It does not establish
third-party provider extensions, remote hydration/error recovery, real-device
sandbox enforcement, store distribution or assistive-reader behavior.

### Independent iOS extension and delayed content acceptance (2026-10-27)

`examples/mobile/build-ios-provider-fixture.sh` now builds a separate host and
`com.apple.fileprovider-nonui` extension. The default enumerating variant appears
as **Glory Downloads** (`rs.glory.hydration.enumerating`). This is a real
`NSFileProviderExtension`, not the earlier Documents folder exposed by Local
Storage. Only its own host/extension share `group.rs.glory.hydration.enumerating`;
the Glory consumer has no provider-group entitlement and receives URLs only
through the actual system picker.

The fixed four-item model enumerates metadata without writing file bodies.
Actual system `startProvidingItem` callbacks provide the owned 36-byte Unicode
text and 70-byte PNG after three seconds. A separate retry TXT fails once with
`NSFileProviderError.serverUnreachable`, then supplies the same expected text.
Files may request contents for thumbnails while browsing, before Open; this
acceptance does not claim that hydration starts exclusively at Open. The provider
writes only its owned storage and never recursively coordinates inside the
system-coordinated callback. Fixed counters/timestamps contain no user paths or
bodies. The test server is in-process static data, not a real network server.

Actual iPhone/iPad iOS 27 simulator runs selected retry TXT + PNG in the system
picker, recovered from the provider error, read exact bytes through external
security-scoped URLs, and painted private previews. Cancel preserved both grants;
iPad outside-tap cancellation, Owner retirement, successful close/scope-stop
pairing and zero native/shared/private-preview quotas passed. No production IO
change or new FFI declaration was needed. `check-ios-system-files.py
--provider-kind enumerating` checks installed host/extension tree hashes, consumer
entitlements, authoritative provider counters and raw consumer/provider logs.
Evidence: `/Volumes/Data/Caches/glory/ios-hydration/result.json`.

`GLORY_PROVIDER_MODE=replicated` builds a separate
`NSFileProviderReplicatedExtension` variant (`rs.glory.hydration`, **Glory
Hydration**) with an explicit owned domain. It compiled, registered and enumerated
on both simulators, but their `fileproviderd` repeatedly failed dataless item
materialization with POSIX EPERM / `cannotSetMetadata`; contents could not be
selected. That path is **not accepted**. Its failure logs are retained separately.
No private entitlement or sandbox bypass is used. Replicated-provider hydration,
commercial/cloud providers, actual network/offline/cancellation recovery,
real-device sandbox/distribution and VoiceOver remain open.


### Read-only replicated diagnosis and Android FIFO refusal (2026-10-28)

The replicated result is narrower than a blanket simulator limitation. Fresh
writable-capability control domains list all four dataless items and complete
actual picker/download/scoped reads/private PNG painting. The unchanged read-only
model remains empty; resolving its known PNG through public getUserVisibleURL
returns POSIX EPERM / cannotSetMetadata before fetchContents. Explicit POSIX flag
controls did not restore the read-only listing. This isolates a metadata-policy
path; it does not identify the precise OS defect. Writable controls are diagnostics
under the data cache, not a shipped relaxation of the read-only fixture. A real
device comparison of the read-only case remains necessary before claiming its
platform acceptance. No private entitlement or permission bypass was introduced.

The replicated fixture now uses its domain manager's temporaryDirectoryURL,
validates requested contentVersion, and has exactly-once prompt Progress
cancellation, suppressed late writes and temporary-file cleanup if cancellation
races writing. `sh examples/mobile/check-ios-provider.sh` covers these paths on
the host. The fixed download implementation also passed an iPad actual-picker
control. This is not OS-driven download-cancellation or real-device proof.
Apple's [fetch contract](https://developer.apple.com/documentation/fileprovider/nsfileproviderreplicatedextension/fetchcontents(for:version:request:completionhandler:))
and [temporary directory contract](https://developer.apple.com/documentation/fileprovider/nsfileprovidermanager/temporarydirectoryurl())
define the relevant version/volume requirements.

Android files-probe now returns real FIFO PFDs from its non-exported owned
provider, separately and after a valid PNG in a mixed selection. Existing
production fstat/regular/seek guards refuse them as Bytes; descriptors close,
the partially opened regular file rolls back, and the prior PNG grant still
reads. A bounded writer-side poll(POLLERR) observes the final reader endpoint
closing, rather than inferring closure from a Rust receipt drop. Both cases
passed on Glory_Input ARM64 with two pipe opens/closes and zero observer failures;
all native/provider/shared/preview quotas drain. The checker requires these
specific new stages plus installed APK hashes. This closes non-seekable refusal
acceptance, not support for random reads over pipes or actual DocumentsUI input.
