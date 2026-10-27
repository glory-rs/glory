import Foundation
// Fixed item/event counters, atomic file replacement, no content, user paths or history.
// Only the provider process writes. The host and external verifier read snapshots.
enum FixtureEvidence {
    static var root: URL {
        #if GLORY_ENUMERATING
        let group = "group.rs.glory.hydration.enumerating"
        #else
        let group = "group.rs.glory.hydration"
        #endif
        return FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)!
    }
    static var url: URL { root.appendingPathComponent("fetch-evidence.json") }
    static func count(_ name: String, event: String) -> Int {
        let counts = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: [String: Int]] ?? [:]
        return counts[name]?[event] ?? 0
    }
    static func record(_ name: String, event: String) {
        var counts = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: [String: Int]] ?? [:]
        var entry = counts[name] ?? [:]
        entry[event] = min((entry[event] ?? 0) + 1, 1_000_000)
        entry[event + "AtMilliseconds"] = Int(Date().timeIntervalSince1970 * 1000)
        counts[name] = entry
        if let data = try? JSONSerialization.data(withJSONObject: counts, options: [.sortedKeys]) { try? data.write(to: url, options: .atomic) }
        NSLog("Glory hydration %@ %@", name, event)
    }
}
