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
Windows/Linux OS round trips, Android/iOS system drivers, mobile clipboard menus,
actual keyboard/selection gestures and assistive-reader acceptance remain open.
Injected drivers and CPU tests do not establish those platform results.
