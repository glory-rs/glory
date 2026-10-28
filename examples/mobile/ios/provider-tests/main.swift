import FileProvider

func require(_ condition: @autoclosure () -> Bool, _ message: String) { precondition(condition(), message) }
let root = URL(fileURLWithPath: ProcessInfo.processInfo.environment["GLORY_BUILD_ROOT"]!).appendingPathComponent("provider-unit-\(UUID().uuidString)")
try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: root) }
let queue = DispatchQueue(label: "rs.glory.provider.tests")
let text = NSFileProviderItemIdentifier("glory-provider-text.txt")
let current = try FixtureItem.content(text, version: nil)
let sameContent = NSFileProviderItemVersion(contentVersion: Data([1]), metadataVersion: Data([2]))
let sameBody = try FixtureItem.content(text, version: sameContent).body
require(sameBody == current.body, "metadata version is not content identity")
for (id, version) in [(text, NSFileProviderItemVersion(contentVersion: Data([2]), metadataVersion: Data([1]))), (.rootContainer, nil), (NSFileProviderItemIdentifier("unknown"), nil)] as [(NSFileProviderItemIdentifier, NSFileProviderItemVersion?)] {
    do { _ = try FixtureItem.content(id, version: version); fatalError("invalid content/version accepted") }
    catch {
        let error = error as NSError
        require(error.domain == NSCocoaErrorDomain || error.domain == NSFileProviderErrorDomain, "invalid protocol error domain")
        if id == text { require(error.domain == NSCocoaErrorDomain && error.code == NSFileReadUnknownError, "version refusal must not delete the current item") }
    }
}

var events = [String](), received: URL?, receivedError: Error?, calls = 0
let done = DispatchSemaphore(value: 0)
let success = FixtureDownload.start(body: current.body, directory: { root }, delay: 0.01, queue: queue, record: { events.append($0) }) { url, error in
    received = url; receivedError = error; calls += 1; done.signal()
}
require(done.wait(timeout: .now() + 1) == .success, "download did not complete")
require(calls == 1 && receivedError == nil && received?.deletingLastPathComponent().standardizedFileURL.path == root.standardizedFileURL.path, "wrong temporary volume/directory or terminal")
let delivered = try Data(contentsOf: received!)
require(delivered == current.body && success.completedUnitCount == 100, "wrong delivered bytes/progress")
require(events == ["started", "completed"], "success events")
success.cancel(); queue.sync {}
require(calls == 1, "late cancel called completion twice")
try FileManager.default.removeItem(at: received!)

let failed = DispatchSemaphore(value: 0)
_ = FixtureDownload.start(body: current.body, directory: { throw CocoaError(.fileReadNoPermission) }, delay: 0, queue: queue, record: { _ in }) { url, error in
    require(url == nil && (error as NSError?)?.code == NSFileReadNoPermissionError, "directory failure turned into success")
    failed.signal()
}
require(failed.wait(timeout: .now() + 1) == .success, "directory error missing")

let cancelled = DispatchSemaphore(value: 0)
var directories = 0, cancelledCalls = 0
let pending = FixtureDownload.start(body: current.body, directory: { directories += 1; return root }, delay: 0.1, queue: queue, record: { _ in }) { url, error in
    require(url == nil && (error as NSError?)?.code == NSUserCancelledError, "cancellation result")
    cancelledCalls += 1; cancelled.signal()
}
pending.cancel()
require(cancelled.wait(timeout: .now() + 0.05) == .success, "cancellation waited for download delay")
Thread.sleep(forTimeInterval: 0.15); queue.sync {}
require(directories == 0 && cancelledCalls == 1, "cancelled delay performed late work")

let entered = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0), raced = DispatchSemaphore(value: 0)
var racedCalls = 0
let racing = FixtureDownload.start(body: current.body, directory: {
    entered.signal(); require(release.wait(timeout: .now() + 1) == .success, "test gate"); return root
}, delay: 0, queue: queue, record: { _ in }) { url, error in
    require(url == nil && (error as NSError?)?.code == NSUserCancelledError, "cancel during write published content")
    racedCalls += 1; raced.signal()
}
require(entered.wait(timeout: .now() + 1) == .success, "download never entered directory")
racing.cancel(); release.signal()
require(raced.wait(timeout: .now() + 1) == .success, "racing cancellation not settled")
queue.sync {}
let remaining = try FileManager.default.contentsOfDirectory(atPath: root.path)
require(racedCalls == 1 && remaining.isEmpty, "racing cancel left temporary file or duplicate completion")
print("Provider download: version refusal, same-volume placement, exact bytes, directory failure, prompt cancellation, late-work suppression and write/cancel cleanup passed")
