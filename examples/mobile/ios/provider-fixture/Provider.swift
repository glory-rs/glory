import FileProvider
@objc(GloryReplicatedProvider)
final class GloryReplicatedProvider: NSObject, NSFileProviderReplicatedExtension {
    private let manager: NSFileProviderManager?
    private let queue = DispatchQueue(label: "rs.glory.hydration")
    required init(domain: NSFileProviderDomain) { manager = NSFileProviderManager(for: domain); super.init(); NSLog("Glory hydration initialized") }
    func invalidate() {}
    func item(for identifier: NSFileProviderItemIdentifier, request: NSFileProviderRequest, completionHandler: @escaping (NSFileProviderItem?, Error?) -> Void) -> Progress {
        NSLog("Glory hydration item %@", identifier.rawValue)
        do { completionHandler(try FixtureItem.get(identifier), nil) } catch { completionHandler(nil, error) }
        return Progress()
    }
    func enumerator(for containerItemIdentifier: NSFileProviderItemIdentifier, request: NSFileProviderRequest) throws -> NSFileProviderEnumerator { NSLog("Glory hydration enumerator %@", containerItemIdentifier.rawValue); return FixtureEnumerator(containerItemIdentifier) }
    func fetchContents(for itemIdentifier: NSFileProviderItemIdentifier, version requestedVersion: NSFileProviderItemVersion?, request: NSFileProviderRequest, completionHandler: @escaping (URL?, NSFileProviderItem?, Error?) -> Void) -> Progress {
        do {
            let item = try FixtureItem.content(itemIdentifier, version: requestedVersion)
            guard let manager = manager else { throw NSFileProviderError(.providerNotFound) }
            return FixtureDownload.start(body: item.body, directory: { try manager.temporaryDirectoryURL() }, queue: queue,
                record: { FixtureEvidence.record(item.filename, event: $0) },
                completion: { url, error in completionHandler(url, url == nil ? nil : item, error) })
        } catch {
            completionHandler(nil, nil, error)
            return Progress()
        }
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
