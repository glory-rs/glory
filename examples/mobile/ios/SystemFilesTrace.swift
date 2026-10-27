#if GLORY_SYSTEM_FILES_PROBE
import Foundation
// Counters only: no document paths, clipboard data or model callbacks.
final class GlorySystemFilesTrace {
    static let shared = GlorySystemFilesTrace()
    private let lock = NSLock()
    private var values = [Int64](repeating: 0, count: 10)
    func record(_ index: Int) { lock.lock(); values[index] += 1; lock.unlock() }
    func value(_ index: Int64) -> Int64 {
        lock.lock(); defer { lock.unlock() }
        guard index >= 0, index < values.count else { return GloryIO.failed }
        return values[Int(index)]
    }
    func opened(_ scoped: Bool, _ external: Bool) {
        lock.lock(); values[5] += 1
        if scoped && external { values[7] += 1 }
        lock.unlock()
        FileHandle.standardError.write(Data("Glory system files open: external=\(external) scoped=\(scoped)\n".utf8))
    }
}
#endif
