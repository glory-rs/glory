import Foundation

enum GloryPermissionFailure: Int64, Error {
    case unsupported = -1, busy = -2, closed = -3, missing = -4
    case unavailable = -5, backend = -6, exhausted = -7
}
final class GloryPermissionClient {
    var records: [Int64: GloryPermissionRecord] = [:]
}
final class GloryPermissionRecord {
    weak var client: GloryPermissionClient?
    let id: Int64, kind: Int64, action: Int64
    var reply: Int64 = 0
    init(_ client: GloryPermissionClient, _ id: Int64, _ kind: Int64, _ action: Int64) {
        self.client = client; self.id = id; self.kind = kind; self.action = action
    }
}
// The UIKit host owns this live ledger on main. Native operations retain only
// records with weak clients, never Rust futures/Owners or a retired client.
final class GloryPermissionLedger {
    private(set) var clients: [Int64: GloryPermissionClient] = [:]
    private(set) var prompt: GloryPermissionRecord?
    private(set) var queries: [Int64: GloryPermissionRecord] = [:]
    private var nextClient: Int64 = 1, nextQuery: Int64 = 1
    func open() -> Int64 {
        guard clients.count < 32 else { return GloryPermissionFailure.busy.rawValue }
        guard nextClient < Int64.max else { return GloryPermissionFailure.exhausted.rawValue }
        let id = nextClient; nextClient += 1; clients[id] = GloryPermissionClient(); return id
    }
    func begin(_ client: Int64, _ id: Int64, _ kind: Int64, _ action: Int64) throws -> GloryPermissionRecord {
        guard let client = clients[client] else { throw GloryPermissionFailure.closed }
        guard id > 0, (1...6).contains(kind), (0...1).contains(action), client.records[id] == nil else { throw GloryPermissionFailure.backend }
        guard client.records.count < 32 else { throw GloryPermissionFailure.busy }
        let record = GloryPermissionRecord(client, id, kind, action); client.records[id] = record; return record
    }
    func live(_ record: GloryPermissionRecord) -> Bool { record.client?.records[record.id] === record }
    func finish(_ record: GloryPermissionRecord, _ code: Int64) { if live(record) { record.reply = code } }
    func poll(_ client: Int64, _ id: Int64) -> Int64 {
        guard let client = clients[client], let record = client.records[id] else { return GloryPermissionFailure.closed.rawValue }
        if record.reply != 0 { client.records.removeValue(forKey: id) }
        return record.reply
    }
    func cancel(_ client: Int64, _ id: Int64) { clients[client]?.records.removeValue(forKey: id) }
    func close(_ client: Int64) { clients.removeValue(forKey: client) }
    func claimPrompt(_ record: GloryPermissionRecord) throws {
        guard live(record) else { throw GloryPermissionFailure.closed }
        guard prompt == nil else { throw GloryPermissionFailure.busy }
        prompt = record
    }
    func endPrompt(_ record: GloryPermissionRecord) { if prompt === record { prompt = nil } }
    func beginQuery(_ record: GloryPermissionRecord) throws -> Int64 {
        guard live(record) else { throw GloryPermissionFailure.closed }
        guard queries.count < 32 else { throw GloryPermissionFailure.busy }
        guard nextQuery < Int64.max else { throw GloryPermissionFailure.exhausted }
        let id = nextQuery; nextQuery += 1; queries[id] = record; return id
    }
    func endQuery(_ id: Int64) -> GloryPermissionRecord? { queries.removeValue(forKey: id) }
    func stats(_ client: Int64) -> Int64 {
        guard let client = clients[client] else { return GloryPermissionFailure.closed.rawValue }
        return Int64(client.records.count) | (prompt == nil ? 0 : 0x10000) | (Int64(queries.count) << 17)
    }
}

#if canImport(UIKit) && GLORY_PERMISSION_HOST
import UIKit
import AVFoundation
import Photos
import CoreLocation
import UserNotifications

@MainActor
private final class GloryPermissionsHost: NSObject, CLLocationManagerDelegate {
    static let shared = GloryPermissionsHost()
    let ledger = GloryPermissionLedger()
    private var location: CLLocationManager?
    private func ready(_ status: Int64) -> Int64 { 0x100 | status }
    private func photo(_ value: PHAuthorizationStatus) -> Int64 {
        switch value {
        case .notDetermined: return ready(1)
        case .authorized: return ready(2)
        case .denied: return ready(3)
        case .restricted: return ready(4)
        case .limited: return ready(5)
        @unknown default: return GloryPermissionFailure.backend.rawValue
        }
    }
    private func media(_ kind: Int64) -> Int64 {
        switch AVCaptureDevice.authorizationStatus(for: kind == 1 ? .video : .audio) {
        case .notDetermined: return ready(1)
        case .authorized: return ready(2)
        case .denied: return ready(3)
        case .restricted: return ready(4)
        @unknown default: return GloryPermissionFailure.backend.rawValue
        }
    }
    private func locationStatus(_ manager: CLLocationManager) -> Int64 {
        switch manager.authorizationStatus {
        case .notDetermined: return ready(1)
        case .authorizedAlways, .authorizedWhenInUse: return ready(manager.accuracyAuthorization == .fullAccuracy ? 2 : 5)
        case .denied: return ready(3)
        case .restricted: return ready(4)
        @unknown default: return GloryPermissionFailure.backend.rawValue
        }
    }
    private func notification(_ value: UNAuthorizationStatus) -> Int64 {
        switch value {
        case .notDetermined: return ready(1)
        case .authorized: return ready(2)
        case .denied: return ready(3)
        case .provisional: return ready(6)
        case .ephemeral: return ready(7)
        @unknown default: return GloryPermissionFailure.backend.rawValue
        }
    }
    private func activeWindow() -> Bool {
        UIApplication.shared.connectedScenes.contains { scene in
            guard scene.activationState == .foregroundActive, let windowScene = scene as? UIWindowScene else { return false }
            return windowScene.windows.contains { !$0.isHidden && NSStringFromClass(type(of: $0)) == "WinitUIWindow" }
        }
    }
    private func declared(_ kind: Int64) -> Bool {
        let keys = [1: "NSCameraUsageDescription", 2: "NSMicrophoneUsageDescription", 3: "NSPhotoLibraryUsageDescription", 4: "NSPhotoLibraryAddUsageDescription", 5: "NSLocationWhenInUseUsageDescription"]
        guard let key = keys[Int(kind)] else { return true }
        return !(Bundle.main.object(forInfoDictionaryKey: key) as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
    func begin(_ client: Int64, _ id: Int64, _ kind: Int64, _ action: Int64) -> Int64 {
        do {
            let record = try ledger.begin(client, id, kind, action)
            if action == 1 && !declared(kind) { ledger.finish(record, GloryPermissionFailure.missing.rawValue); return 0 }
            if kind == 6 { notificationQuery(record, request: action == 1); return 0 }
            let state: Int64
            switch kind {
            case 1, 2: state = media(kind)
            case 3: state = photo(PHPhotoLibrary.authorizationStatus(for: .readWrite))
            case 4: state = photo(PHPhotoLibrary.authorizationStatus(for: .addOnly))
            case 5: state = locationStatus(CLLocationManager())
            default: state = GloryPermissionFailure.unsupported.rawValue
            }
            if action == 0 || state != ready(1) { ledger.finish(record, state); return 0 }
            request(record)
            return 0
        } catch let error as GloryPermissionFailure { return error.rawValue }
        catch { return GloryPermissionFailure.backend.rawValue }
    }
    private func notificationQuery(_ record: GloryPermissionRecord, request shouldRequest: Bool) {
        do {
            let query = try ledger.beginQuery(record)
            UNUserNotificationCenter.current().getNotificationSettings { settings in
                let status = settings.authorizationStatus
                DispatchQueue.main.async {
                    guard let record = self.ledger.endQuery(query) else { return }
                    let state = self.notification(status)
                    if shouldRequest && state == self.ready(1) { self.request(record) }
                    else { self.ledger.finish(record, state) }
                }
            }
        } catch let error as GloryPermissionFailure { ledger.finish(record, error.rawValue) }
        catch { ledger.finish(record, GloryPermissionFailure.backend.rawValue) }
    }
    private func request(_ record: GloryPermissionRecord) {
        do {
            guard ledger.live(record) else { return }
            guard declared(record.kind) else { throw GloryPermissionFailure.missing }
            if ledger.prompt != nil { throw GloryPermissionFailure.busy }
            guard activeWindow() else { throw GloryPermissionFailure.unavailable }
            try ledger.claimPrompt(record)
            switch record.kind {
            case 1, 2:
                AVCaptureDevice.requestAccess(for: record.kind == 1 ? .video : .audio) { _ in
                    DispatchQueue.main.async {
                        self.ledger.endPrompt(record); self.ledger.finish(record, self.media(record.kind))
                    }
                }
            case 3, 4:
                PHPhotoLibrary.requestAuthorization(for: record.kind == 3 ? .readWrite : .addOnly) { status in
                    DispatchQueue.main.async {
                        self.ledger.endPrompt(record); self.ledger.finish(record, self.photo(status))
                    }
                }
            case 5:
                let manager = CLLocationManager(); location = manager; manager.delegate = self
                manager.requestWhenInUseAuthorization()
            case 6:
                UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { _, error in
                    DispatchQueue.main.async {
                        self.ledger.endPrompt(record)
                        if error != nil { self.ledger.finish(record, GloryPermissionFailure.backend.rawValue) }
                        else { self.notificationQuery(record, request: false) }
                    }
                }
            default: ledger.endPrompt(record); ledger.finish(record, GloryPermissionFailure.unsupported.rawValue)
            }
        } catch let error as GloryPermissionFailure { ledger.finish(record, error.rawValue) }
        catch { ledger.finish(record, GloryPermissionFailure.backend.rawValue) }
    }
    func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        guard manager === location, let record = ledger.prompt, record.kind == 5 else { return }
        let state = locationStatus(manager)
        guard state != ready(1) else { return }
        manager.delegate = nil; location = nil
        ledger.endPrompt(record); ledger.finish(record, state)
    }
}

// Integer-only C ABI exported by the application-owned Swift host. Dispatching
// here makes every integer input safe even if an outer host calls off main.
private func gloryPermissionsOnMain<T>(_ body: @escaping @MainActor () -> T) -> T {
    if Thread.isMainThread { return MainActor.assumeIsolated { body() } }
    return DispatchQueue.main.sync { MainActor.assumeIsolated { body() } }
}
@_cdecl("glory_permissions_open")
func gloryPermissionsOpen() -> Int64 { gloryPermissionsOnMain { GloryPermissionsHost.shared.ledger.open() } }
@_cdecl("glory_permissions_begin")
func gloryPermissionsBegin(_ client: Int64, _ request: Int64, _ kind: Int64, _ action: Int64) -> Int64 {
    gloryPermissionsOnMain { GloryPermissionsHost.shared.begin(client, request, kind, action) }
}
@_cdecl("glory_permissions_poll")
func gloryPermissionsPoll(_ client: Int64, _ request: Int64) -> Int64 {
    gloryPermissionsOnMain { GloryPermissionsHost.shared.ledger.poll(client, request) }
}
@_cdecl("glory_permissions_cancel")
func gloryPermissionsCancel(_ client: Int64, _ request: Int64) { gloryPermissionsOnMain { GloryPermissionsHost.shared.ledger.cancel(client, request) } }
@_cdecl("glory_permissions_close")
func gloryPermissionsClose(_ client: Int64) { gloryPermissionsOnMain { GloryPermissionsHost.shared.ledger.close(client) } }
@_cdecl("glory_permissions_stats")
func gloryPermissionsStats(_ client: Int64) -> Int64 { gloryPermissionsOnMain { GloryPermissionsHost.shared.ledger.stats(client) } }

#if GLORY_PERMISSION_HOST_PROBE
@MainActor
func gloryPermissionsHostProbe() {
    let host = GloryPermissionsHost.shared
    let client = host.ledger.open()
    guard client > 0 else { return }
    for kind in 1...6 { _ = host.begin(client, Int64(kind), Int64(kind), 0) }
    func observe(_ remaining: Int, _ pending: [Int64]) {
        var waiting: [Int64] = []
        for id in pending {
            let result = host.ledger.poll(client, id)
            if result == 0 { waiting.append(id) }
            else { FileHandle.standardError.write(Data("Glory Swift permissions query kind=\(id) code=\(result)\n".utf8)) }
        }
        if waiting.isEmpty {
            FileHandle.standardError.write(Data("Glory Swift permissions queries complete stats=\(host.ledger.stats(client))\n".utf8))
            host.ledger.close(client)
        } else if remaining > 0 {
            DispatchQueue.main.asyncAfter(deadline: .now()+0.02) { observe(remaining-1, waiting) }
        } else {
            host.ledger.close(client)
            FileHandle.standardError.write(Data("Glory Swift permissions query deadline\n".utf8))
        }
    }
    observe(100, [1, 2, 3, 4, 5, 6])
}
#endif
#endif
