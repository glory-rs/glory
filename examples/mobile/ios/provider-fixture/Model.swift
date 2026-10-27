import FileProvider
import UniformTypeIdentifiers

// Owned read-only server model. No file contents exist on disk until fetchContents.
// The consumer has no entitlement for this extension's private/shared containers.
final class FixtureItem: NSObject, NSFileProviderItem {
    static let names = ["glory-provider-text.txt", "glory-provider-image.png", "glory-provider-retry.txt", "rejected.bin"]
    let itemIdentifier: NSFileProviderItemIdentifier
    init(_ id: NSFileProviderItemIdentifier) { itemIdentifier = id }
    var parentItemIdentifier: NSFileProviderItemIdentifier { .rootContainer }
    var filename: String { itemIdentifier == .rootContainer ? "Glory Hydration" : itemIdentifier.rawValue }
    var contentType: UTType { itemIdentifier == .rootContainer ? .folder : (filename.hasSuffix(".png") ? .png : (filename.hasSuffix(".txt") ? .plainText : .data)) }
    var capabilities: NSFileProviderItemCapabilities { itemIdentifier == .rootContainer ? [.allowsContentEnumerating, .allowsReading] : [.allowsReading] }
    var documentSize: NSNumber? { itemIdentifier == .rootContainer ? nil : NSNumber(value: body.count) }
    var itemVersion: NSFileProviderItemVersion { NSFileProviderItemVersion(contentVersion: Data([1]), metadataVersion: Data([1])) }
    var body: Data {
        if filename.hasSuffix(".txt") { return Data("Glory external provider: 中文😀\n".utf8) }
        if filename.hasSuffix(".png") { return Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQ2RL1HwAEJAIiMMFhVgAAAABJRU5ErkJggg==")! }
        return Data([1, 2, 3, 4])
    }
    static func get(_ id: NSFileProviderItemIdentifier) throws -> FixtureItem {
        guard id == .rootContainer || names.contains(id.rawValue) else { throw NSFileProviderError(.noSuchItem) }
        return FixtureItem(id)
    }
}
final class FixtureEnumerator: NSObject, NSFileProviderEnumerator {
    let container: NSFileProviderItemIdentifier
    init(_ id: NSFileProviderItemIdentifier) { container = id }
    func invalidate() {}
    func enumerateItems(for observer: NSFileProviderEnumerationObserver, startingAt page: NSFileProviderPage) {
        NSLog("Glory hydration enumerate %@", container.rawValue)
        observer.didEnumerate(container == .rootContainer || container == .workingSet ? FixtureItem.names.map { FixtureItem(NSFileProviderItemIdentifier($0)) } : [])
        observer.finishEnumerating(upTo: nil)
    }
    func enumerateChanges(for observer: NSFileProviderChangeObserver, from anchor: NSFileProviderSyncAnchor) {
        observer.didUpdate(FixtureItem.names.map { FixtureItem(NSFileProviderItemIdentifier($0)) })
        observer.finishEnumeratingChanges(upTo: NSFileProviderSyncAnchor(Data([1])), moreComing: false)
    }
    func currentSyncAnchor(completionHandler: @escaping (NSFileProviderSyncAnchor?) -> Void) { completionHandler(NSFileProviderSyncAnchor(Data([1]))) }
}
