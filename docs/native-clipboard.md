# Explicit native text clipboard

Enable `glory/blitz-clipboard` (or `glory-blitz/clipboard`) and supply a
`Clipboard` in `Config`. The feature does not imply a window host. The default
configuration opens no clipboard and advertises no clipboard-write capability.

```rust,ignore
use glory::blitz::{Config, clipboard::Clipboard, shell::WindowApp};
let window = WindowApp::with_config(Config {
    clipboard: Some(Clipboard::desktop()?),
    ..Default::default()
});
```

`Clipboard::desktop()` is available on Windows, Linux and macOS. It starts one
worker, lazily opens Arboard 3.6.1 on the first polled request, and retains the
native clipboard on that worker. Retention also preserves Linux clipboard
ownership. Wayland uses the data-control implementation; actual availability
depends on the compositor. Image clipboard support is not enabled.

Applications can instead construct `Clipboard::new(Arc<dyn ClipboardDriver>)`.
Drivers return Send futures containing owned results, never UI state or business
callbacks. Constructors must return promptly. Native callbacks wake a future;
the document delivers accepted semantic events on its UI loop. Foreign future
construction, polling and destruction run outside the document borrow.

## Admission and cancellation

`CopyText` uses the existing node/source revision and non-wrapping request
watermark. Identity, availability, current modal scope and source validity are
checked before admission. One current native operation per document admits no
growing queue. A busy semantic request receives `Failed`. Source replacement or
node retirement withdraws acceptance without resetting its request watermark.
An invalid consumer before the first poll never starts its native write.

The default deadline is 30 seconds. `with_timeout` accepts nonzero durations up
to 120 seconds. Timeout reports failure once; cancellation or timeout of an
already-started operation retains document admission until the driver actually
settles. Each shared desktop driver additionally owns one physical worker permit
across its documents. Dropping its result future cannot refund that permit.
Closing the service does not join or interrupt an OS call on the UI thread.

The native result is `Written`, `Denied`, `Unavailable` or `Failed`, not an
optimistic activation result. Injected drivers exercise every outcome; Arboard
errors are mapped conservatively and do not establish an OS permission-denial
classification. Default unsupported behavior remains covered separately.

## Editor commands

With `shell`, configured text controls use this same service for Copy/Cut/Paste,
platform shortcuts, named clipboard keys and standard Apple commands. One
current AppKit command marker consumes the paired physical key exactly once,
including after a tag-policy callback removes that policy. Other input and
focus retirement clear the marker; it contains no text.

Copy/cut never export password text. Readonly controls allow copy and refuse
cut/paste. Composition retains its input. Cut deletes the selection only after a
successful write and a still-current receipt. Paste validates current semantic
and physical focus, node/control identity, availability, modal/focus scope,
input revision, selection, purpose and tag policy before editing. Editing,
selection actions or leaving and returning focus invalidate an old receipt.
Presentation-only relayout preserves valid receipts; Parley's layout generation
is not used as an edit identity.

Clipboard bodies are bounded to 65,536 UTF-8 bytes before UI delivery and after
normalization. Tags use their existing 16 KiB paste/4 KiB draft limits and atomic
UTF-16 `TagsCommand::Paste` proposal, including body-free budget rejection.
Ordinary paste uses the real editor and preserves InputEdit's before-selection;
its resulting-size limit is checked before mutation. No text history is retained.
An OS/provider may allocate a String before returning it: these payload limits
do not bound that OS allocation or total process memory.

## Verification and remaining platforms

`tests/clipboard_driver.rs` covers lazy writes, exact outcomes, source/target
retirement, deadlines without early permit refund, busy sources, default privacy
and foreign-code borrow boundaries. `clipboard_editor_tests.rs` covers actual
native editors, CJK/emoji selections, cut failure, stale/ABA receipts, passwords,
readonly/IME, Tags, InputEdit budgets, presentation changes and document cleanup.
Private desktop-ticket tests verify lazy admission, cancellation and closure
without touching the user's clipboard.

Run `GLORY_BUILD_ROOT=/Volumes/Data/Caches/glory/apple sh
scripts/check-macos-clipboard.sh` on macOS for the opt-in system check. It builds
the test first, snapshots all general-pasteboard items/representations in RAM,
then runs actual worker write/read, a bounded-read failure and semantic CopyText.
It checks the current owned token/change count before restoration and verifies
restored bytes without logging or persisting clipboard contents. An observed
external clipboard change is preserved and makes acceptance fail. The successful
run is `/Volumes/Data/Caches/glory/clipboard-macos-system.log`.

Sapid's tracked `examples/native` clipboard feature tests actual CopyButton,
CodeHighlight, both JsonViewer modes and TagsInput in four themes, including CPU
pixels and feedback. Evidence is `/Volumes/Data/Caches/glory/clipboard-sapid-tests.log`.
Windows/Linux OS round trips, mobile clipboard menus,
actual keyboard/selection gestures and assistive-reader acceptance remain open.
Injected drivers and CPU tests do not establish those platform results.

## Android application host

Android adds `AndroidClipboard::new(env, host)`, implementing the same Send
`ClipboardDriver`. Supply an explicit protocol-v1 Java host, then put
`Clipboard::new(Arc::new(driver))` in Config. JNI uses checked references and
bounded byte arrays; no raw JNI pointers, native callback exports or new unsafe
are required. The mobile example owns `GloryClipboard`/`ClipboardLedger`.

The Java main-thread coordinator holds Activity weakly and closes on destruction.
There are at most 32 current clients and one queued/running OS operation across
them. Each client holds at most one request/result; numeric IDs never repeat.
Cancellation removes a receipt but keeps actual admission until the queued or
running job settles. Posted work and OS calls run outside the ledger lock.
There is no clipboard listener, completed-ID table or saved clipboard history.

`ClipboardManager` is accessed only with a live foreground window. Reads use the
first item's text; URI coercion, HTML conversion, files and image decoding are
not requested. Java bounds UTF-16 work, raw UTF-8 and canonical UTF-8 before a
body crosses JNI, normalizing CRLF/CR, NUL and unpaired surrogates. Rust checks
byte-array length before copying and validates UTF-8 again. OS-created ClipData
and CharSequence allocation precede these transfer limits. Security exceptions
map to Denied; no text/background access maps to Unavailable.

JNI only submits/polls bounded records. An 8 ms timer exists while that one
future is pending; idle/retired documents do not poll. `stats` reports current
receipt count and actual coordinator admission separately. The application
clones the existing driver/service to share its request-ID sequence; constructing
a second driver around the same Host is refused by its one-time claim.

Enable the mobile example's `clipboard` feature for the normal Counter plus
explicit Android service, or `clipboard-probe` for the isolated acceptance view.
The ordinary build creates neither clipboard coordinator nor probe guard, and
adds no permission. `check-android-clipboard.sh` runs the real ledger's JVM
budget/cancellation/lifecycle tests. `check-android-clipboard-sdk.py` installs the
probe only on the owned Glory_Input AVD and verifies the installed APK hash.

Actual Android 36 ARM64 emulator runs (final claim/lifecycle build PID 8619)
passed foreground Chinese/emoji/CRLF/NUL write/read, read/write budget refusal,
semantic CopyText and restoration of the prior ClipData from RAM. The guard
passes the complete original items/types back to Android and checks restored
item/type counts; it does not claim a byte snapshot of URI targets or OS data.
No clipboard body is logged/persisted, and no screenshot captures a possible
system preview of restored original data. Evidence is
`/Volumes/Data/Caches/glory/android-clipboard/final/sdk-result.json`.
These are real OS service calls and SDK launches, separately from actual menu,
keyboard, selection gestures, TalkBack and real-device acceptance.

## iOS application host

The mobile example's `ios_io::clipboard()` constructs an explicit `IosClipboard`
for `Clipboard::new` in Config. Enable its `clipboard` feature; construction
opens only a bounded native client, without reading the pasteboard. The adapter
implements the existing Send/Sync driver trait and shares non-wrapping IDs across
clones. Futures are lazy; only waiting futures own an 8 ms polling timer.

The application-owned Swift host accesses `UIPasteboard.general` on main with an
active Winit window. It never installs a listener or coerces URIs. Explicit reads
inspect `hasStrings` and read one string snapshot, bound its returned UTF-8, then
canonicalize CRLF/CR and NUL within the same 65,536-byte limit. OS NSString/board
allocation is outside that transfer limit. Writes bound raw and canonical bytes
before transfer. There is one physical operation, at most 32 clients and one
current receipt per client. Cancellation/close removes delivery without refunding
queued/running work. Current-body receipt storage has no completion history.
Apple documents the distinction between content-presence checks and content
reads in [UIPasteboard.hasStrings](https://developer.apple.com/documentation/uikit/uipasteboard/hasstrings).

A single separately user-authorized application-only `safe fn` declaration in an
`unsafe extern "C"` block connects both iOS services. Six i64 arguments and one
i64 result carry validated commands and bounded eight-byte packet words; no
pointer, native object or callback crosses the ABI. Runtime crates gain no unsafe.
Length is checked before copying reply words; native records remain immutable
until consumption and Rust rechecks concurrent close before delivery.

`ios-io-probe` is a separate, explicitly enabled acceptance bundle. Only with
`GLORY_IOS_IO_PROBE=1` does its guard retain original pasteboard representations
in RAM, write a unique owned value and permit automated service tests. Restoration
requires the last owned change count; an observed external change is preserved and fails
the test. Owner cleanup requests restoration too, after physical work settles
and an app window is active. It logs/persists no original clipboard body and takes
no clipboard screenshots. The public API does not expose all pasteboard policy
metadata, such as an original expiration option; the guard proves restoration of
readable item representations, not every OS/provider property. Abrupt process
death cannot guarantee restoration.

Actual iPhone 17 and iPad Air M4 / iOS 27 simulator runs pass Chinese/emoji,
CRLF/NUL normalization, read/write budgets, semantic CopyText through Config and
conditional restoration. Native records/admission finish at zero. Evidence is
`/Volumes/Data/Caches/glory/ios-io/result.json`. Cross-app paste alerts, actual
copy/cut/paste menus, keyboard/selection gestures, VoiceOver and real devices
remain separate acceptance; owned same-app values do not establish those flows.

The integer ABI and reproducible host checks are specified in
[ios-mobile-io.md](ios-mobile-io.md).
