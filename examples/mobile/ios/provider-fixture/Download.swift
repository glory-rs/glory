import Foundation

// One finite download. All state and terminal delivery are serialized on queue.
// Cancelling releases the completion immediately; the cancelled delay cannot
// write a file later. No provider/client completion history is retained.
final class FixtureDownload {
    typealias Completion = (URL?, Error?) -> Void
    private let queue: DispatchQueue
    private let body: Data
    private let directory: () throws -> URL
    private let record: (String) -> Void
    private var completion: Completion?
    private var work: DispatchWorkItem?
    let progress = Progress(totalUnitCount: 100)

    private init(body: Data, directory: @escaping () throws -> URL,
                 queue: DispatchQueue, record: @escaping (String) -> Void,
                 completion: @escaping Completion) {
        self.body = body; self.directory = directory; self.queue = queue
        self.record = record; self.completion = completion
    }
    static func start(body: Data, directory: @escaping () throws -> URL,
                      delay: TimeInterval = 3, queue: DispatchQueue,
                      record: @escaping (String) -> Void,
                      completion: @escaping Completion) -> Progress {
        let download = FixtureDownload(body: body, directory: directory, queue: queue,
                                       record: record, completion: completion)
        download.progress.cancellationHandler = { [weak download] in
            guard let download = download else { return }
            queue.async { download.finish(nil, CocoaError(.userCancelled)) }
        }
        queue.async {
            download.record("started")
            if download.progress.isCancelled { download.finish(nil, CocoaError(.userCancelled)); return }
            let work = DispatchWorkItem { download.perform() }
            download.work = work
            queue.asyncAfter(deadline: .now() + delay, execute: work)
        }
        return download.progress
    }
    private func perform() {
        guard completion != nil else { return }
        var file: URL?
        do {
            if progress.isCancelled { throw CocoaError(.userCancelled) }
            // The caller supplies NSFileProviderManager's same-volume directory.
            let url = try directory().appendingPathComponent(UUID().uuidString)
            file = url
            try body.write(to: url, options: .atomic)
            if progress.isCancelled { throw CocoaError(.userCancelled) }
            finish(url, nil)
        } catch {
            if let file = file { try? FileManager.default.removeItem(at: file) }
            finish(nil, error)
        }
    }
    private func finish(_ url: URL?, _ error: Error?) {
        guard let completion = completion else { return }
        self.completion = nil
        work?.cancel(); work = nil
        progress.cancellationHandler = nil
        if error == nil { progress.completedUnitCount = 100; record("completed") }
        else {
            let failure = error as NSError?
            record(failure?.domain == NSCocoaErrorDomain && failure?.code == NSUserCancelledError ? "cancelled" : "failed")
        }
        completion(url, error)
    }
}
