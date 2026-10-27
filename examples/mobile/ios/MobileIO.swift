import Foundation

// Application-local protocol v1. Every ABI argument is an integer. Bodies use
// sequential little-endian eight-byte words, bounded before either allocation.
enum GloryIO {
    static let unavailable: Int64 = -1, busy: Int64 = -2, failed: Int64 = -3
    static let count: Int64 = -4, type: Int64 = -5, bytes: Int64 = -6
    static let metadata: Int64 = -7, denied: Int64 = -8
    static let maxBody = 65_536, maxMetadata = 32_768
}
struct GloryFileOptions {
    let multiple: Bool, count: Int, fileBytes: UInt64, totalBytes: UInt64
    let filters: [String]
    init(_ body: [UInt8]) throws {
        var input = GloryPacket(body)
        guard try input.integer(4) == 1 else { throw GloryIOError(GloryIO.failed) }
        let multiple = try input.integer(4), count = try input.integer(4)
        let fileBytes = try input.integer(8), totalBytes = try input.integer(8)
        let filterCount = try input.integer(4)
        guard multiple <= 1, (1...32).contains(count), multiple == 1 || count == 1,
              fileBytes <= 1 << 30, totalBytes <= 1 << 30, filterCount <= 32 else { throw GloryIOError(GloryIO.failed) }
        var filters: [String] = [], filterBytes = 0
        for _ in 0..<filterCount {
            let length = try input.integer(2); filterBytes += Int(length)
            guard filterBytes <= 2048 else { throw GloryIOError(GloryIO.metadata) }
            let text = try input.text(Int(length))
            guard GloryFileOptions.validFilter(text) else { throw GloryIOError(GloryIO.type) }
            filters.append(text)
        }
        guard input.remaining == 0 else { throw GloryIOError(GloryIO.failed) }
        self.multiple = multiple == 1; self.count = Int(count)
        self.fileBytes = fileBytes; self.totalBytes = totalBytes; self.filters = filters
    }
    static func token(_ text: String) -> Bool {
        !text.isEmpty && text.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || Array("!#$&-^_.+".utf8).contains($0) }
    }
    static func validFilter(_ text: String) -> Bool {
        if text.hasPrefix(".") { return text.utf8.count <= 32 && text.dropFirst().utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || [46,45,95].contains($0) } && text.utf8.count > 1 }
        let parts = text.split(separator: "/", omittingEmptySubsequences: false)
        return text.utf8.count <= 127 && parts.count == 2 && token(String(parts[0])) && (parts[1] == "*" || token(String(parts[1])))
    }
    func matches(_ name: String, _ mime: String) -> Bool {
        filters.isEmpty || filters.contains { filter in
            if filter.hasPrefix(".") { return String(decoding: name.utf8.map { (65...90).contains($0) ? $0+32 : $0 }, as: UTF8.self).hasSuffix(filter) }
            if filter.hasSuffix("/*") { return mime.hasPrefix(filter.dropLast()) }
            return mime == filter
        }
    }
}
struct GloryIOError: Error { let code: Int64; init(_ code: Int64) { self.code = code } }
struct GloryPacket {
    let bytes: [UInt8]; var cursor = 0
    init(_ bytes: [UInt8]) { self.bytes = bytes }
    var remaining: Int { bytes.count - cursor }
    mutating func integer(_ count: Int) throws -> UInt64 {
        guard count <= remaining else { throw GloryIOError(GloryIO.metadata) }
        var result: UInt64 = 0
        for _ in 0..<count { result = result << 8 | UInt64(bytes[cursor]); cursor += 1 }
        return result
    }
    mutating func text(_ count: Int) throws -> String {
        guard count <= remaining, let text = String(bytes: bytes[cursor..<cursor+count], encoding: .utf8) else { throw GloryIOError(GloryIO.metadata) }
        cursor += count; return text
    }
}
func gloryAppend(_ value: UInt64, _ count: Int, to bytes: inout [UInt8]) {
    for i in (0..<count).reversed() { bytes.append(UInt8(truncatingIfNeeded: value >> (i*8))) }
}
final class GloryIOClient {
    let service: Int64
    var watermark: Int64 = 0, records: [Int64: GloryIORecord] = [:]
    var files: [Int64: GloryIOSource] = [:]
    init(_ service: Int64) { self.service = service }
}
final class GloryIORecord {
    weak var client: GloryIOClient?
    let id: Int64, kind: Int64, length: Int
    var input: [UInt8] = [], code: Int64 = 0, output: [UInt8] = []
    var running = false, committed = false
    var files: [Int64: GloryIOSource] = [:]
    init(_ client: GloryIOClient, _ id: Int64, _ kind: Int64, _ length: Int) {
        self.client = client; self.id = id; self.kind = kind; self.length = length
        input.reserveCapacity(length)
    }
}
// All ledger access is locked. Native UI and file work execute outside this
// lock; callbacks update receipts only, never Rust/Owner/application state.
final class GloryIOLedger {
    private let lock = NSRecursiveLock()
    private var clients: [Int64: GloryIOClient] = [:], nextClient: Int64 = 1, nextFile: Int64 = 1
    private var clipboard: GloryIORecord?, picker: GloryIORecord?
    private var reads: [ObjectIdentifier: GloryIORecord] = [:]
    private var handles = 0, metadata = 0, declared: UInt64 = 0
    var start: ((GloryIORecord, Int64) -> Void)?
    var cancelPicker: ((GloryIORecord) -> Void)?
    func locked<T>(_ body: () throws -> T) rethrows -> T { lock.lock(); defer { lock.unlock() }; return try body() }
    func clipboardIdle() -> Bool { locked { clipboard == nil } }
    func live(_ record: GloryIORecord) -> Bool { record.client?.records[record.id] === record }
    private func releaseAdmission(_ record: GloryIORecord) {
        if clipboard === record { clipboard = nil }
        if picker === record { picker = nil }
        reads.removeValue(forKey: ObjectIdentifier(record))
    }
    func finish(_ record: GloryIORecord, _ code: Int64, _ output: [UInt8] = [], _ files: [Int64: GloryIOSource] = [:]) {
        locked {
            record.running = false
            if live(record) {
                record.code = code; record.output = output; record.files = files
                if clipboard === record { clipboard = nil }
                if picker === record { picker = nil }
                // Completed reads remain charged until their receipt drains.
            } else { releaseAdmission(record) }
        }
    }
    func call(_ service: Int64, _ clientID: Int64, _ op: Int64, _ id: Int64, _ a: Int64, _ b: Int64) -> Int64 {
        var action: (() -> Void)?
        let result: Int64 = locked {
            guard service == 1 || service == 2 else { return GloryIO.unavailable }
            if op == 12 { return 1 }
            if op == 0 {
                guard clients.values.filter({ $0.service == service }).count < 32 else { return GloryIO.busy }
                guard nextClient < Int64.max else { return GloryIO.failed }
                let key = nextClient; nextClient += 1; clients[key] = GloryIOClient(service); return key
            }
            guard let client = clients[clientID], client.service == service else { return GloryIO.unavailable }
            if op == 1 {
                let old = clients.removeValue(forKey: clientID)!
                let records = Array(old.records.values); old.records.removeAll(); old.files.removeAll()
                for record in records { if !record.running { releaseAdmission(record) } }
                if let record = picker, records.contains(where: { $0 === record }) { action = { self.cancelPicker?(record) } }
                return 0
            }
            if op == 11 {
                switch a {
                case 0: return Int64(client.records.count)
                case 1: return service == 1 ? (clipboard == nil ? 0 : 1) : (picker == nil ? 0 : 1)
                case 2: return Int64(reads.count)
                case 3: return Int64(handles)
                case 4: return Int64(metadata)
                case 5: return Int64(declared)
                case 6: return Int64(clients.values.filter { $0.service == service }.count)
                default: return GloryIO.failed
                }
            }
            if op == 10, service == 2 { client.files.removeValue(forKey: id); return 0 }
            if op == 2 {
                guard id > client.watermark, id > 0 else { return GloryIO.failed }
                client.watermark = id
                guard a == 0 || a == 1, b >= 0, b <= (service == 1 ? 65_536 : 2176) else { return GloryIO.bytes }
                guard client.records.count < (service == 1 ? 1 : 5) else { return GloryIO.busy }
                if service == 1 { guard clipboard == nil else { return GloryIO.busy } }
                else if a == 0 { guard picker == nil else { return GloryIO.busy } }
                else { guard reads.count < 4 else { return GloryIO.busy } }
                let record = GloryIORecord(client, id, a, Int(b)); client.records[id] = record
                if service == 1 { clipboard = record }
                else if a == 0 { picker = record }
                else { reads[ObjectIdentifier(record)] = record }
                return 0
            }
            guard let record = client.records[id] else { return GloryIO.unavailable }
            switch op {
            case 3:
                guard !record.committed, a >= 0, a == record.input.count, record.input.count < record.length else { return GloryIO.failed }
                let count = min(8, record.length-record.input.count), bits = UInt64(bitPattern: b)
                for i in 0..<count { record.input.append(UInt8(truncatingIfNeeded: bits >> (i*8))) }
                return 0
            case 4:
                guard !record.committed, record.input.count == record.length else { return GloryIO.failed }
                record.committed = true; record.running = true
                action = { self.start?(record, service) }; return 0
            case 5: return record.code
            case 6: return record.code == 2 ? Int64(record.output.count) : GloryIO.failed
            case 7:
                guard record.code == 2, a >= 0, a < record.output.count, a % 8 == 0 else { return 0 }
                var bits: UInt64 = 0
                for i in 0..<min(8, record.output.count-Int(a)) { bits |= UInt64(record.output[Int(a)+i]) << (i*8) }
                return Int64(bitPattern: bits)
            case 8:
                client.records.removeValue(forKey: id)
                record.files.removeAll(); record.output.removeAll(); record.input.removeAll()
                if !record.running { releaseAdmission(record) }
                if picker === record { action = { self.cancelPicker?(record) } }
                return 0
            case 9:
                guard service == 2, record.kind == 0, record.code == 2 else { return GloryIO.failed }
                client.files.merge(record.files) { _, new in new }; record.files.removeAll()
                client.records.removeValue(forKey: id); releaseAdmission(record); return 0
            default: return GloryIO.failed
            }
        }
        action?(); return result
    }
    func source(_ record: GloryIORecord, _ id: Int64) -> GloryIOSource? { locked { live(record) ? record.client?.files[id] : nil } }
    func snapshot(_ record: GloryIORecord) -> [UInt8]? { locked { live(record) ? record.input : nil } }
    func reserve(_ record: GloryIORecord, _ count: Int, _ options: GloryFileOptions) throws -> GloryIOLease {
        try locked {
            guard live(record) else { throw GloryIOError(GloryIO.unavailable) }
            guard handles + count <= 160, metadata + 32_768 <= 160*1024, declared + options.totalBytes <= 2 << 30 else { throw GloryIOError(GloryIO.busy) }
            handles += count; metadata += 32_768; declared += options.totalBytes
            let charge = GloryIOCharge(32_768, options.totalBytes)
            return GloryIOLease(release: {
                self.locked { self.handles -= count; self.metadata -= charge.metadata; self.declared -= charge.bytes }
            }, settle: { metadata, bytes in
                self.locked {
                    guard metadata >= 0, metadata <= charge.metadata, bytes <= charge.bytes else { return false }
                    self.metadata -= charge.metadata-metadata; self.declared -= charge.bytes-bytes
                    charge.metadata = metadata; charge.bytes = bytes; return true
                }
            })
        }
    }
    func publish(_ record: GloryIORecord, _ sources: [GloryIOSource]) throws {
        try locked {
            guard live(record), nextFile <= Int64.max-Int64(sources.count) else { throw GloryIOError(GloryIO.unavailable) }
            var body: [UInt8] = [], files: [Int64: GloryIOSource] = [:]
            gloryAppend(UInt64(sources.count), 4, to: &body)
            for source in sources {
                let id = nextFile; nextFile += 1; files[id] = source
                gloryAppend(UInt64(id), 8, to: &body); gloryAppend(source.size, 8, to: &body)
                gloryAppend(UInt64(source.name.utf8.count), 2, to: &body); gloryAppend(UInt64(source.mime.utf8.count), 2, to: &body)
                body += source.name.utf8; body += source.mime.utf8
            }
            finish(record, 2, body, files)
        }
    }
}
private final class GloryIOCharge {
    var metadata: Int, bytes: UInt64
    init(_ metadata: Int, _ bytes: UInt64) { self.metadata = metadata; self.bytes = bytes }
}
final class GloryIOLease {
    private let release: () -> Void
    private let resize: (Int, UInt64) -> Bool
    init(release: @escaping () -> Void, settle: @escaping (Int, UInt64) -> Bool) { self.release = release; resize = settle }
    func settle(_ metadata: Int, _ bytes: UInt64) throws {
        guard resize(metadata, bytes) else { throw GloryIOError(GloryIO.failed) }
    }
    deinit { release() }
}

func gloryCanonicalClipboard(_ input: [UInt8], _ limit: Int) throws -> [UInt8] {
    guard input.count <= limit, String(bytes: input, encoding: .utf8) != nil else { throw GloryIOError(GloryIO.bytes) }
    var output: [UInt8] = []; output.reserveCapacity(input.count)
    var i = 0
    while i < input.count {
        let byte = input[i]
        let part: [UInt8]
        if byte == 0 { part = [239,191,189] }
        else if byte == 13 { part = [10]; if i+1 < input.count && input[i+1] == 10 { i += 1 } }
        else { part = [byte] }
        guard output.count + part.count <= limit else { throw GloryIOError(GloryIO.bytes) }
        output += part; i += 1
    }
    return output
}
