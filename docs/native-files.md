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
threads. Clone one `Files` service to share admission across documents. Separate
service instances have separate budgets; custom drivers must themselves retain
physical permits for uncancellable work after their receiver is dropped.

## Selection and reading

One service admits one current picker and four reads, including pending results.
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
[`6a9ca05b246ecec6a7844e18dc6d264eb8f076cf`](https://github.com/glory-rs/blitz/commit/6a9ca05b246ecec6a7844e18dc6d264eb8f076cf).
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
Android/iOS platform pickers, file drag/drop and actual assistive-technology
interaction remain open. `FileCapabilities::drop` is false, and declarations
requiring drop are explicitly rejected. Injected-driver, CPU and compile evidence
do not establish those flows or complete the full file-service/platform gap.
