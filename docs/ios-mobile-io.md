# Application-owned iOS IO bridge

The mobile example owns the Swift implementation and safe Rust drivers in
`examples/mobile/{ios,src/ios_io}`. Runtime crates retain `unsafe_code = deny`.
The separately user-authorized exception is one application-local declaration:

```rust
unsafe extern "C" {
    safe fn glory_mobile_io(
        service: i64, client: i64, operation: i64, request: i64, a: i64, b: i64,
    ) -> i64;
}
```

The actual block has a local lint allowance; no unsafe call block, pointer,
native object or callback crosses this ABI. `service` is 1 (clipboard) or 2
(files). Invalid integers are rejected before conversion/indexing; invalid body
word lookups return zero. No input is ever interpreted as a pointer. UI calls
are queued on main, IO runs on workers, and all native ledger mutations are
locked. Neither native completion nor FFI calls enter Rust UI/business code.

| Operation | Meaning | `request` | `a`, `b` |
| --- | --- | --- | --- |
| 0 | Open client; positive ID or error | unused | unused |
| 1 | Close client and withdraw all its receipts/grants | unused | unused |
| 2 | Admit an upload | new monotonic request ID | kind, exact byte length |
| 3 | Append next upload word | request ID | byte cursor, word bits |
| 4 | Commit complete upload/start native operation | request ID | unused |
| 5 | Poll receipt | request ID | unused |
| 6 | Read ready body's exact bounded length | request ID | unused |
| 7 | Read an immutable reply word | request ID | byte cursor, unused |
| 8 | Withdraw/drain exact receipt | request ID | unused |
| 9 | Adopt all validated selected file handles | picker request ID | unused |
| 10 | Release selected handle | file ID | unused |
| 11 | Read one diagnostic counter | unused | counter index, unused |
| 12 | Protocol version (1) | unused | unused |

Clipboard kind 0 writes canonical UTF-8; kind 1 uploads a four-byte big-endian
read limit. Files kind 0 uploads picker configuration; kind 1 uploads file ID
(u64), offset (u64), length (u32), all big-endian. Configuration is version
(u32=1), multiple (u32=0/1), count (u32), per-file and total limits (u64 each),
filter count (u32), then each filter's u16 byte length and UTF-8 bytes.

Transport words contain up to eight bytes in least-significant-byte-first order;
negative i64 values are valid word bit patterns. Upload cursors must equal the
current byte count. Reply cursors are eight-byte aligned. Rust checks length
before allocation/copy and rechecks concurrent close after transfer. Poll codes
are 0 pending, 1 written/cancelled, 2 body ready; all other positive values fail.

Metadata replies are count (u32), then file ID (u64), size (u64), name and MIME
byte lengths (u16 each), and exact UTF-8 strings. Rust validates the complete
packet, original filters/limits and unique IDs before adoption. Dropping an
unadopted receipt closes its candidate sources. A lazy read retains its selected
source capability and releases that reference at terminal delivery, even if the
caller retains the completed future.

Errors: -1 unavailable, -2 busy, -3 failed, -4 count, -5 type, -6 bytes,
-7 metadata, -8 denied. Each driver maps these to its existing finite error type.
Clipboard rejects invalid limits in Rust before submission. Stats counter 0 is
client receipt count; 1 is global physical clipboard/picker admission, 2 global
read admission, 3 reserved handles, 4 metadata, 5 declared bytes, 6 client count
for that service. Unsupported counters fail. Counter reads are individual
snapshots, not a transactional multi-counter view.

One process ledger limits clipboard to one physical operation and one current
receipt/client, with 32 clients and 64 KiB raw/canonical bodies. Files allow one
picker and four admitted reads including undrained replies, 32 clients, 160
reserved handles, 160 KiB metadata and 2 GiB declared sizes. Opening selections
reserve worst-case configuration costs; completed opens settle to actual metadata
and size. Group costs remain charged through final actual close. Failed closes
retain their handle/scope/quota as unsettled work. Futures reserve before foreign
calls, serialize new request submission and use timers only while waiting.

Probe operation 100 exists only with `ios-io-probe` and requires
`GLORY_IOS_IO_PROBE=1`. It controls only the owned selector fixture/read gate or
clipboard preservation/restoration. Normal builds cannot use these hooks.
Original readable pasteboard representations stay in RAM and restoration is
conditional on the owned change count. An abrupt process exit cannot guarantee
restoration. No original clipboard body or user document is logged/persisted.

Host unit checks, after sourcing `scripts/apple-env.sh` with an approved cache:

```sh
cargo test --locked --manifest-path examples/mobile/Cargo.toml --lib \
  --no-default-features --features clipboard,files,permissions
xcrun swiftc examples/mobile/ios/MobileIO.swift \
  examples/mobile/ios/MobileIOFiles.swift examples/mobile/ios/io-tests/main.swift \
  -warnings-as-errors -module-cache-path "$GLORY_BUILD_ROOT/SwiftIOTests" \
  -o "$GLORY_BUILD_ROOT/ios-io-tests"
"$GLORY_BUILD_ROOT/ios-io-tests"
```

See [native files](native-files.md), [native clipboard](native-clipboard.md) and
the [mobile host](../examples/mobile/README.md) for SDK acceptance and boundaries.

The separate `ios-files-ui-probe` compiles test-only operation 101 for ten
body/path-free counters: presented pickers, selection callbacks, user Cancel,
scope starts/stops, opens/closes, scoped external opens, denied unscoped external
URLs and completed native cancellation dismissals. It uses the same authorized
function declaration and never accepts an arbitrary path or selected-URL packet.
It is mutually exclusive with `ios-io-probe`. See the actual-system acceptance
section in [native-files.md](native-files.md).

The separate provider fixture now also tests actual enumerating-extension
`startProvidingItem` callbacks, delayed hydration and one server-unreachable
failure followed by retry. Its App Group is shared only by the independent host
and extension; the Glory consumer has no such entitlement. No new ABI operation,
FFI declaration or production IO code was needed. See [native files](native-files.md)
for the accepted enumerating path and separately unaccepted replicated path.
