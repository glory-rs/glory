import FileProvider

@objc(GloryEnumeratingProvider)
final class GloryEnumeratingProvider: NSFileProviderExtension {
    private let queue = DispatchQueue(label: "rs.glory.hydration.enumerating")
    private var storage: URL { NSFileProviderManager.default.documentStorageURL }
    override func item(for identifier: NSFileProviderItemIdentifier) throws -> NSFileProviderItem { try FixtureItem.get(identifier) }
    override func urlForItem(withPersistentIdentifier identifier: NSFileProviderItemIdentifier) -> URL? {
        guard (try? FixtureItem.get(identifier)) != nil else { return nil }
        return identifier == .rootContainer ? storage : storage.appendingPathComponent(identifier.rawValue)
    }
    override func persistentIdentifierForItem(at url: URL) -> NSFileProviderItemIdentifier? {
        if url.standardizedFileURL == storage.standardizedFileURL { return .rootContainer }
        guard url.deletingLastPathComponent().standardizedFileURL == storage.standardizedFileURL,
              FixtureItem.names.contains(url.lastPathComponent) else { return nil }
        return NSFileProviderItemIdentifier(url.lastPathComponent)
    }
    override func enumerator(for containerItemIdentifier: NSFileProviderItemIdentifier) throws -> NSFileProviderEnumerator { FixtureEnumerator(containerItemIdentifier) }
    override func providePlaceholder(at url: URL, completionHandler: @escaping (Error?) -> Void) {
        do {
            guard let id = persistentIdentifierForItem(at: url) else { throw NSFileProviderError(.noSuchItem) }
            try FileManager.default.createDirectory(at: storage, withIntermediateDirectories: true)
            try NSFileProviderManager.writePlaceholder(at: NSFileProviderManager.placeholderURL(for: url), withMetadata: FixtureItem.get(id))
            completionHandler(nil)
        } catch { completionHandler(error) }
    }
    override func startProvidingItem(at url: URL, completionHandler: @escaping (Error?) -> Void) {
        guard let id = persistentIdentifierForItem(at: url), let item = try? FixtureItem.get(id), id != .rootContainer else { completionHandler(NSFileProviderError(.noSuchItem)); return }
        queue.async {
            if FileManager.default.fileExists(atPath: url.path) { completionHandler(nil); return }
            let failOnce = item.filename == "glory-provider-retry.txt" && FixtureEvidence.count(item.filename, event: "failed") == 0
            FixtureEvidence.record(item.filename, event: "started")
            self.queue.asyncAfter(deadline: .now() + 3) {
                do {
                    if failOnce { throw NSFileProviderError(.serverUnreachable) }
                    try FileManager.default.createDirectory(at: self.storage, withIntermediateDirectories: true)
                    // The system already coordinates this callback. Never coordinate recursively here.
                    try item.body.write(to: url, options: .atomic)
                    FixtureEvidence.record(item.filename, event: "completed")
                    completionHandler(nil)
                } catch {
                    FixtureEvidence.record(item.filename, event: "failed")
                    completionHandler(error)
                }
            }
        }
    }
    override func stopProvidingItem(at url: URL) {
        // Keep the bounded four-file cache; later acceptance uses fresh owned installs.
    }
    override func itemChanged(at url: URL) {}
}
