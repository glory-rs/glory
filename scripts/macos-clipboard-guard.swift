// Own only this test's pasteboard changes. Keep original bytes in RAM and never
// print or persist clipboard contents. A concurrent external change wins.
import AppKit
import Foundation

enum Failure: Error { case invalidArguments, snapshotUnavailable, restoreFailed }
func guarded() throws -> Int32 {
    guard CommandLine.arguments.count == 2 else { throw Failure.invalidArguments }
    let board = NSPasteboard.general
    let change = board.changeCount
    let original: [[(NSPasteboard.PasteboardType, Data)]] = try (board.pasteboardItems ?? []).map { item in
        try item.types.map { type in
            guard let bytes = item.data(forType: type) else { throw Failure.snapshotUnavailable }
            return (type, bytes)
        }
    }
    // Prepare every restoration object before allowing a test write.
    let objects = try original.map { entries in
        let item = NSPasteboardItem()
        for (type, data) in entries {
            guard item.setData(data, forType: type) else { throw Failure.snapshotUnavailable }
        }
        return item
    }
    guard board.changeCount == change else { throw Failure.snapshotUnavailable }
    let token = "glory-owned-clipboard-" + UUID().uuidString
    let process = Process()
    process.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
    process.arguments = ["clipboard::macos_tests::guarded_system_clipboard_round_trip", "--exact", "--ignored", "--nocapture"]
    var environment = ProcessInfo.processInfo.environment
    environment["GLORY_CLIPBOARD_TEST_TOKEN"] = token
    process.environment = environment
    try process.run()
    process.waitUntilExit()
    // Check ownership immediately before restoration, including on test failure.
    if board.changeCount != change {
        let observed = board.changeCount
        guard board.string(forType: .string)?.hasPrefix(token) == true,
              board.changeCount == observed else {
            print("Clipboard changed externally; external contents preserved")
            return process.terminationStatus == 0 ? 1 : process.terminationStatus
        }
        board.clearContents()
        guard objects.isEmpty || board.writeObjects(objects) else { throw Failure.restoreFailed }
        let restored = board.pasteboardItems ?? []
        guard restored.count == original.count else { throw Failure.restoreFailed }
        for (item, entries) in zip(restored, original) {
            guard Set(item.types) == Set(entries.map { $0.0 }),
                  entries.allSatisfy({ item.data(forType: $0.0) == $0.1 }) else { throw Failure.restoreFailed }
        }
        print("Original pasteboard items and representations restored")
    }
    return process.terminationStatus
}
do { exit(try guarded()) }
catch { fputs("Clipboard guard failed: \(error)\n", stderr); exit(1) }
