import Foundation

func config(_ count: UInt64 = 2, _ filters: [String] = [".txt", ".png"]) -> [UInt8] {
    var body: [UInt8] = []
    for value: UInt64 in [1, count == 1 ? 0 : 1, count] { gloryAppend(value, 4, to: &body) }
    gloryAppend(16*1024*1024, 8, to: &body); gloryAppend(32*1024*1024, 8, to: &body)
    gloryAppend(UInt64(filters.count), 4, to: &body)
    for filter in filters { gloryAppend(UInt64(filter.utf8.count), 2, to: &body); body += filter.utf8 }
    return body
}
let good = config()
let options = try GloryFileOptions(good)
precondition(options.matches("HELLO.TXT", "text/plain") && !options.matches("photo.jpg", "image/jpeg"))
for length in 0..<good.count {
    do { _ = try GloryFileOptions(Array(good.prefix(length))); preconditionFailure("truncated packet accepted") } catch {}
}
precondition(GloryFileOptions.validFilter(".tar.gz") && GloryFileOptions.validFilter("image/*") && !GloryFileOptions.validFilter("image/**"))
let ledger = GloryIOLedger()
func call(_ service: Int64, _ client: Int64, _ op: Int64, _ id: Int64 = 0, _ a: Int64 = 0, _ b: Int64 = 0) -> Int64 { ledger.call(service, client, op, id, a, b) }
for invalid in [Int64.min, -1, 0, Int64.max] {
    precondition(call(1, invalid, 5, 1) == GloryIO.unavailable)
    precondition(call(2, invalid, 8, 1) == GloryIO.unavailable)
}
let client = call(1, 0, 0), second = call(1, 0, 0)
var held: GloryIORecord?
var work: [GloryIORecord] = []
ledger.start = { record, _ in held = record; work.append(record) }
precondition(call(1, client, 2, 1, 0, 3) == 0)
precondition(call(1, client, 3, 1, 1, 0) == GloryIO.failed)
precondition(call(1, client, 3, 1, 0, 0x434241) == 0)
precondition(call(1, client, 4, 1) == 0)
precondition(call(1, client, 4, 1) == GloryIO.failed)
precondition(call(1, client, 8, 1) == 0)
precondition(call(1, client, 11, 0, 0) == 0 && call(1, client, 11, 0, 1) == 1)
precondition(call(1, second, 2, 1, 0, 0) == GloryIO.busy)
ledger.finish(held!, 1); held = nil
precondition(call(1, client, 11, 0, 1) == 0)
precondition(call(1, second, 2, 2, 1, 0) == 0)
precondition(call(1, second, 4, 2) == 0)
ledger.finish(held!, 2, [0x00,0xff,0x7f,0x80,0x01,0x02,0x03,0xfe,0x55])
precondition(call(1, second, 6, 2) == 9)
precondition(UInt64(bitPattern: call(1, second, 7, 2, 0)) == 0xfe030201807fff00)
precondition(call(1, second, 7, 2, 8) == 0x55)
precondition(call(1, second, 8, 2) == 0)
precondition(call(1, second, 2, 2, 0, 0) == GloryIO.failed)
_ = call(1, client, 1); _ = call(1, second, 1)
let fileClient = call(2, 0, 0)
for id: Int64 in 1...4 { precondition(call(2, fileClient, 2, id, 1, 0) == 0); precondition(call(2, fileClient, 4, id) == 0) }
precondition(call(2, fileClient, 2, 5, 1, 0) == GloryIO.busy)
precondition(call(2, fileClient, 11, 0, 2) == 4)
let read = held!
_ = call(2, fileClient, 8, 4)
precondition(call(2, fileClient, 11, 0, 2) == 4)
ledger.finish(read, 2)
precondition(call(2, fileClient, 11, 0, 2) == 3)
_ = call(2, fileClient, 1)
for record in work { ledger.finish(record, 2) }
work.removeAll(); held = nil
let observer = call(1, 0, 0)
precondition(call(1, observer, 11, 0, 2) == 0)
_ = call(1, observer, 1)
let slots = GloryIOLedger()
var clients: [Int64] = []
for _ in 0..<32 { let client = slots.call(1, 0, 0, 0, 0, 0); precondition(client > 0); clients.append(client) }
precondition(slots.call(1, 0, 0, 0, 0, 0) == GloryIO.busy)
for client in clients { _ = slots.call(1, client, 1, 0, 0, 0) }
for _ in 0..<1000 { let client = slots.call(2, 0, 0, 0, 0, 0); precondition(client > 0); _ = slots.call(2, client, 1, 0, 0, 0) }
print("Mobile IO ledger: finite packets, integer boundaries, staged words, native cancellation admission, four reads, 32 clients and 1000 retirements passed")

let canonical = try gloryCanonicalClipboard(Array("中😀\r\n\0".utf8), 64)
precondition(String(decoding: canonical, as: UTF8.self) == "中😀\n�")
for bytes in [[UInt8](repeating: 0, count: 32), [UInt8](repeating: 120, count: 33), [0xff]] {
    do { _ = try gloryCanonicalClipboard(bytes, 32); preconditionFailure("clipboard bound/UTF-8 missing") } catch {}
}
print("Clipboard canonical UTF-8: CRLF, NUL expansion, raw/canonical budgets and malformed input passed")

let quota = GloryIOLedger()
var quotaRecord: GloryIORecord?
quota.start = { record, _ in quotaRecord = record }
let quotaClient = quota.call(2, 0, 0, 0, 0, 0)
let observerClient = quota.call(2, 0, 0, 0, 0, 0)
var leases: [GloryIOLease] = []
for id: Int64 in 1...5 {
    precondition(quota.call(2, quotaClient, 2, id, 0, 0) == 0)
    precondition(quota.call(2, quotaClient, 4, id, 0, 0) == 0)
    leases.append(try quota.reserve(quotaRecord!, 32, options))
    quota.finish(quotaRecord!, 1)
    _ = quota.call(2, quotaClient, 8, id, 0, 0)
}
precondition(quota.call(2, observerClient, 11, 0, 3, 0) == 160)
precondition(quota.call(2, observerClient, 11, 0, 4, 0) == 160*1024)
precondition(quota.call(2, quotaClient, 2, 6, 0, 0) == 0)
precondition(quota.call(2, quotaClient, 4, 6, 0, 0) == 0)
do { _ = try quota.reserve(quotaRecord!, 1, options); preconditionFailure("quota exceeded") } catch {}
quota.finish(quotaRecord!, 1)
_ = quota.call(2, quotaClient, 1, 0, 0, 0)
precondition(quota.call(2, observerClient, 11, 0, 3, 0) == 160)
leases.removeAll()
precondition(quota.call(2, observerClient, 11, 0, 3, 0) == 0)
precondition(quota.call(2, observerClient, 11, 0, 4, 0) == 0)
precondition(quota.call(2, observerClient, 11, 0, 5, 0) == 0)
_ = quota.call(2, observerClient, 1, 0, 0, 0)
print("Native file quota: 160 reserved handles, 160 KiB metadata, closed-client admission retained until final lease release passed")

let small = GloryIOLedger()
var smallRecord: GloryIORecord?
small.start = { record, _ in smallRecord = record }
let smallClient = small.call(2, 0, 0, 0, 0, 0)
var smallLeases: [GloryIOLease] = []
for id: Int64 in 1...6 {
    precondition(small.call(2, smallClient, 2, id, 0, 0) == 0)
    precondition(small.call(2, smallClient, 4, id, 0, 0) == 0)
    let lease = try small.reserve(smallRecord!, 1, options)
    try lease.settle(16, 9)
    smallLeases.append(lease)
    small.finish(smallRecord!, 1)
    _ = small.call(2, smallClient, 8, id, 0, 0)
}
precondition(small.call(2, smallClient, 11, 0, 3, 0) == 6)
precondition(small.call(2, smallClient, 11, 0, 4, 0) == 96)
precondition(small.call(2, smallClient, 11, 0, 5, 0) == 54)
smallLeases.removeAll()
precondition(small.call(2, smallClient, 11, 0, 3, 0) == 0)
precondition(small.call(2, smallClient, 11, 0, 4, 0) == 0)
precondition(small.call(2, smallClient, 11, 0, 5, 0) == 0)
_ = small.call(2, smallClient, 1, 0, 0, 0)
print("Settled native selections: six small groups use actual metadata/declared size, not six worst-case reservations")
