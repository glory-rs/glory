#if canImport(UIKit) && GLORY_IO_PROBE
import UIKit
// Probe-only state. Original pasteboard representations remain in RAM and are
// restored only if the board still has our last owned change count.
final class GloryIOProbe {
    static let shared = GloryIOProbe()
    private let lock = NSLock()
    private let readGate = NSCondition()
    private var readsPaused = false
    private var ready: Int64 = 0, mode: Int64?
    private var original: [[String: Any]]?, ownedChange: Int?
    private var directory: URL?
    private let enabled = ProcessInfo.processInfo.environment["GLORY_IOS_IO_PROBE"] == "1"
    func command(_ service: Int64, _ action: Int64) -> Int64 {
        guard enabled else { return GloryIO.unavailable }
        if service == 2 && (action == 10 || action == 11) {
            readGate.lock(); readsPaused = action == 10
            if !readsPaused { readGate.broadcast() }; readGate.unlock(); return 0
        }
        lock.lock(); defer { lock.unlock() }
        if service == 2 {
            guard (1...4).contains(action), mode == nil else { return GloryIO.busy }
            mode = action; return 0
        }
        if action == 0 { return ready }
        if action == 1 {
            guard ready == 1 else { return ready }
            ready = 0
            DispatchQueue.main.async { self.restore() }
            return 0
        }
        return GloryIO.failed
    }
    @MainActor func initialize() {
        guard enabled else { return }
        lock.lock(); defer { lock.unlock() }
        guard original == nil, ownedChange == nil, ready == 0 else { return }
        let board = UIPasteboard.general
        original = board.items
        board.string = "glory-owned-ios-clipboard-" + UUID().uuidString
        ownedChange = board.changeCount; ready = 1
    }
    @MainActor func checkOwned(_ board: UIPasteboard) throws {
        guard enabled else { return }
        lock.lock(); defer { lock.unlock() }
        guard ready == 1, ownedChange == board.changeCount else { throw GloryIOError(GloryIO.denied) }
    }
    @MainActor func didWrite(_ board: UIPasteboard) {
        guard enabled else { return }
        lock.lock(); defer { lock.unlock() }; ownedChange = board.changeCount
    }
    @MainActor private func restore() {
        guard gloryIOCanRestoreProbe() else {
            DispatchQueue.main.asyncAfter(deadline: .now()+0.02) { self.restore() }; return
        }
        lock.lock(); defer { lock.unlock() }
        let board = UIPasteboard.general
        defer { original = nil; ownedChange = nil }
        guard let original = original, board.changeCount == ownedChange else { ready = GloryIO.denied; return }
        board.items = original; ready = 2
    }
    func takeFileMode() -> Int64? {
        guard enabled else { return nil }
        lock.lock(); defer { lock.unlock() }; let value = mode; mode = nil; return value
    }
    func waitForReadGate() {
        readGate.lock(); defer { readGate.unlock() }
        while readsPaused { readGate.wait() }
    }
    @MainActor func urls(_ mode: Int64) throws -> [URL] {
        if directory == nil {
            let dir = FileManager.default.temporaryDirectory.appendingPathComponent("glory-owned-io-" + UUID().uuidString, isDirectory: true)
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: false)
            try Data("A中😀Z".utf8).write(to: dir.appendingPathComponent("中文😀.txt"))
            let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQ2RL1HwAEJAIiMMFhVgAAAABJRU5ErkJggg==")!
            try png.write(to: dir.appendingPathComponent("pixel.png"))
            directory = dir
        }
        let dir = directory!
        if mode == 3 { return [dir.appendingPathComponent("中文😀.txt"), dir.appendingPathComponent("pixel.png"), dir.appendingPathComponent("pixel.png")] }
        if mode == 4 { return [dir.appendingPathComponent("pixel.png")] }
        return [dir.appendingPathComponent("中文😀.txt"), dir.appendingPathComponent("pixel.png")]
    }
}
func gloryIOProbeSceneActive() { MainActor.assumeIsolated { GloryIOProbe.shared.initialize() } }
#endif
