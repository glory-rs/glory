import FileProvider
@objc(GloryReplicatedProvider)
final class GloryReplicatedProvider: NSObject, NSFileProviderReplicatedExtension {
    private let queue = DispatchQueue(label: "rs.glory.hydration")
    required init(domain: NSFileProviderDomain) { super.init(); NSLog("Glory hydration initialized") }
    func invalidate() {}
    func item(for identifier: NSFileProviderItemIdentifier, request: NSFileProviderRequest, completionHandler: @escaping (NSFileProviderItem?, Error?) -> Void) -> Progress {
        NSLog("Glory hydration item %@", identifier.rawValue)
        do { completionHandler(try FixtureItem.get(identifier), nil) } catch { completionHandler(nil, error) }
        return Progress()
    }
    func enumerator(for containerItemIdentifier: NSFileProviderItemIdentifier, request: NSFileProviderRequest) throws -> NSFileProviderEnumerator { NSLog("Glory hydration enumerator %@", containerItemIdentifier.rawValue); return FixtureEnumerator(containerItemIdentifier) }
    func fetchContents(for itemIdentifier: NSFileProviderItemIdentifier, version requestedVersion: NSFileProviderItemVersion?, request: NSFileProviderRequest, completionHandler: @escaping (URL?, NSFileProviderItem?, Error?) -> Void) -> Progress {
        let progress = Progress(totalUnitCount: 100)
        queue.async {
            do {
                let item = try FixtureItem.get(itemIdentifier)
                FixtureEvidence.record(item.filename, event: "started")
                // Explicit delay models a remote content download; metadata enumeration never writes bodies.
                self.queue.asyncAfter(deadline: .now() + 3) {
                    do {
                        if progress.isCancelled { throw CocoaError(.userCancelled) }
                        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
                        try item.body.write(to: url)
                        FixtureEvidence.record(item.filename, event: "completed")
                        progress.completedUnitCount = 100
                        completionHandler(url, item, nil)
                        // The system takes ownership of the returned temporary file.
                    } catch {
                        FixtureEvidence.record(item.filename, event: progress.isCancelled ? "cancelled" : "failed")
                        completionHandler(nil, nil, error)
                    }
                }
            } catch { completionHandler(nil, nil, error) }
        }
        return progress
    }
    func createItem(basedOn itemTemplate: NSFileProviderItem, fields: NSFileProviderItemFields, contents url: URL?, options: NSFileProviderCreateItemOptions, request: NSFileProviderRequest, completionHandler: @escaping (NSFileProviderItem?, NSFileProviderItemFields, Bool, Error?) -> Void) -> Progress {
        completionHandler(nil, [], false, CocoaError(.fileWriteNoPermission)); return Progress()
    }
    func modifyItem(_ item: NSFileProviderItem, baseVersion version: NSFileProviderItemVersion, changedFields: NSFileProviderItemFields, contents newContents: URL?, options: NSFileProviderModifyItemOptions, request: NSFileProviderRequest, completionHandler: @escaping (NSFileProviderItem?, NSFileProviderItemFields, Bool, Error?) -> Void) -> Progress {
        completionHandler(nil, [], false, CocoaError(.fileWriteNoPermission)); return Progress()
    }
    func deleteItem(identifier: NSFileProviderItemIdentifier, baseVersion version: NSFileProviderItemVersion, options: NSFileProviderDeleteItemOptions, request: NSFileProviderRequest, completionHandler: @escaping (Error?) -> Void) -> Progress {
        completionHandler(CocoaError(.fileWriteNoPermission)); return Progress()
    }
}
