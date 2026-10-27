import Foundation
import Darwin
import UniformTypeIdentifiers

// A failed close has unknown native settlement. Keep that handle, scope and
// quota charged instead of refunding it. At most the admitted 160 handles can
// occupy this fault state; this is unsettled work, not completion history.
private final class GloryIOCloseFailures {
    static let shared = GloryIOCloseFailures()
    private let lock = NSLock()
    private var pending: [(FileHandle, URL, Bool, GloryIOLease)] = []
    func retain(_ file: FileHandle, _ url: URL, _ scoped: Bool, _ lease: GloryIOLease) {
        lock.lock(); defer { lock.unlock() }; pending.append((file, url, scoped, lease))
    }
}
private func gloryCloseSource(_ file: FileHandle, _ url: URL, _ scoped: Bool, _ lease: GloryIOLease) {
    do {
        try file.close()
        if scoped { url.stopAccessingSecurityScopedResource() }
        withExtendedLifetime(lease) {}
    } catch { GloryIOCloseFailures.shared.retain(file, url, scoped, lease) }
}

// A selected URL is opened once under coordination. Its scope and admission
// survive receipt cancellation until the last actual read and close settle.
final class GloryIOSource {
    let name: String, mime: String, size: UInt64
    private let url: URL, handle: FileHandle, scoped: Bool, lease: GloryIOLease
    private let lock = NSLock()
    init(_ url: URL, _ options: GloryFileOptions, _ lease: GloryIOLease) throws {
        guard url.isFileURL else { throw GloryIOError(GloryIO.type) }
        let scoped = url.startAccessingSecurityScopedResource()
        let home = URL(fileURLWithPath: NSHomeDirectory()).resolvingSymlinksInPath().path + "/"
        guard scoped || url.resolvingSymlinksInPath().path.hasPrefix(home) else { throw GloryIOError(GloryIO.denied) }
        var opened: FileHandle?
        do {
            var error: NSError?, failure: Error?, details: (String, String, UInt64)?
            NSFileCoordinator().coordinate(readingItemAt: url, options: [], error: &error) { selected in
                do {
                    let file = try FileHandle(forReadingFrom: selected); opened = file
                    var info = stat()
                    guard fstat(file.fileDescriptor, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_size >= 0 else { throw GloryIOError(GloryIO.type) }
                    let name = selected.lastPathComponent
                    let type = try selected.resourceValues(forKeys: [.contentTypeKey]).contentType ?? UTType(filenameExtension: selected.pathExtension)
                    let mime = type?.preferredMIMEType?.lowercased() ?? ""
                    guard mime.isEmpty || (GloryFileOptions.validFilter(mime) && !mime.contains("*")) else { throw GloryIOError(GloryIO.metadata) }
                    let size = UInt64(info.st_size)
                    guard name.utf8.count <= 1024, !name.isEmpty, !name.contains("\r"), !name.contains("\0"), mime.utf8.count <= 127 else { throw GloryIOError(GloryIO.metadata) }
                    guard size <= options.fileBytes else { throw GloryIOError(GloryIO.bytes) }
                    guard options.matches(name, mime) else { throw GloryIOError(GloryIO.type) }
                    details = (name, mime, size)
                } catch { failure = error }
            }
            if let error = failure ?? error { throw error }
            guard let file = opened, let details = details else { throw GloryIOError(GloryIO.failed) }
            self.handle = file; self.url = url; self.scoped = scoped; self.lease = lease
            self.name = details.0; self.mime = details.1; self.size = details.2
        } catch {
            if let file = opened { gloryCloseSource(file, url, scoped, lease) }
            else if scoped { url.stopAccessingSecurityScopedResource() }
            throw error
        }
    }
    func read(_ offset: UInt64, _ length: Int) throws -> [UInt8] {
        guard length >= 0, length <= 65_536, offset <= size, UInt64(length) <= size-offset else { throw GloryIOError(GloryIO.bytes) }
        lock.lock(); defer { lock.unlock() }
        var output: [UInt8]?, failure: Error?, error: NSError?
        NSFileCoordinator().coordinate(readingItemAt: url, options: [], error: &error) { _ in
            do {
                var info = stat()
                guard fstat(handle.fileDescriptor, &info) == 0, info.st_size >= 0, UInt64(info.st_size) == size else { throw GloryIOError(GloryIO.bytes) }
                try handle.seek(toOffset: offset)
                var bytes: [UInt8] = []; bytes.reserveCapacity(length)
                while bytes.count < length {
                    guard let chunk = try handle.read(upToCount: length-bytes.count), !chunk.isEmpty else { throw GloryIOError(GloryIO.bytes) }
                    bytes += chunk
                }
                output = bytes
            } catch { failure = error }
        }
        if let error = failure ?? error { throw error }
        guard let output = output else { throw GloryIOError(GloryIO.failed) }
        return output
    }
    deinit {
        let file = handle, url = url, scoped = scoped, admission = lease
        DispatchQueue.global(qos: .utility).async {
            gloryCloseSource(file, url, scoped, admission)
        }
    }
}
func gloryIOFailure(_ error: Error) -> Int64 {
    if let error = error as? GloryIOError { return error.code }
    let error = error as NSError
    if error.domain == NSCocoaErrorDomain && [NSFileReadNoPermissionError, NSFileWriteNoPermissionError].contains(error.code) { return GloryIO.denied }
    return GloryIO.failed
}
func gloryOpenSelection(_ ledger: GloryIOLedger, _ record: GloryIORecord, _ urls: [URL], _ options: GloryFileOptions) {
    do {
        guard !urls.isEmpty, urls.count <= options.count else { throw GloryIOError(GloryIO.count) }
        let lease = try ledger.reserve(record, urls.count, options)
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                var sources: [GloryIOSource] = [], metadata = 0, bytes: UInt64 = 0
                for url in urls {
                    let source = try GloryIOSource(url, options, lease)
                    metadata += source.name.utf8.count + source.mime.utf8.count
                    bytes += source.size
                    guard metadata <= 32_768 else { throw GloryIOError(GloryIO.metadata) }
                    guard bytes <= options.totalBytes else { throw GloryIOError(GloryIO.bytes) }
                    sources.append(source)
                }
                try lease.settle(metadata, bytes)
                try ledger.publish(record, sources)
            } catch { ledger.finish(record, gloryIOFailure(error)) }
        }
    } catch { ledger.finish(record, gloryIOFailure(error)) }
}
func gloryStartRead(_ ledger: GloryIOLedger, _ record: GloryIORecord, _ input: [UInt8]) {
    do {
        var packet = GloryPacket(input)
        let id = try packet.integer(8), offset = try packet.integer(8), length = try packet.integer(4)
        guard packet.remaining == 0, id > 0, id <= Int64.max, offset <= 1 << 30, length <= 65_536 else { throw GloryIOError(GloryIO.bytes) }
        guard let source = ledger.source(record, Int64(id)) else { throw GloryIOError(GloryIO.unavailable) }
        DispatchQueue.global(qos: .userInitiated).async {
            #if GLORY_IO_PROBE
            GloryIOProbe.shared.waitForReadGate()
            #endif
            do { ledger.finish(record, 2, try source.read(offset, Int(length))) }
            catch { ledger.finish(record, gloryIOFailure(error)) }
        }
    } catch { ledger.finish(record, gloryIOFailure(error)) }
}
