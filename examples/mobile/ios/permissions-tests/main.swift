import Foundation

let ledger = GloryPermissionLedger()
let client = ledger.open()
let first = try ledger.begin(client, 1, 1, 1)
try ledger.claimPrompt(first)
ledger.cancel(client, 1)
precondition(ledger.stats(client) == 0x10000)
let second = try ledger.begin(client, 1, 1, 1)
do { try ledger.claimPrompt(second); preconditionFailure("cancel refunded live OS prompt") }
catch GloryPermissionFailure.busy {}
ledger.endPrompt(first)
ledger.finish(first, 0x102)
precondition(ledger.poll(client, 1) == 0)
ledger.finish(second, 0x103)
precondition(ledger.poll(client, 1) == 0x103)
precondition(ledger.poll(client, 1) == GloryPermissionFailure.closed.rawValue)

var operations: [Int64] = []
for id in 1...32 {
    let record = try ledger.begin(client, Int64(id), 6, 0)
    operations.append(try ledger.beginQuery(record))
    ledger.cancel(client, Int64(id))
}
precondition(ledger.stats(client) == 32 << 17)
let waiting = try ledger.begin(client, 33, 6, 0)
do { _ = try ledger.beginQuery(waiting); preconditionFailure("cancel refunded uncancellable query") }
catch GloryPermissionFailure.busy {}
ledger.close(client)
for id in operations { if let record = ledger.endQuery(id) { ledger.finish(record, 0x102) } }
precondition(ledger.queries.isEmpty && ledger.clients.isEmpty)
let successor = ledger.open()
let successorRecord = try ledger.begin(successor, 1, 1, 0)
ledger.finish(first, 0x102)
precondition(ledger.poll(successor, 1) == 0)
ledger.finish(successorRecord, 0x104)
precondition(ledger.poll(successor, 1) == 0x104)
ledger.close(successor)
for _ in 1...1000 { let id = ledger.open(); precondition(id > 0); ledger.close(id) }
precondition(ledger.clients.isEmpty && ledger.queries.isEmpty && ledger.prompt == nil)
print("UIKit permission ledger: cancellation, stale replies, 32 query slots and 1000 client retirements passed")
