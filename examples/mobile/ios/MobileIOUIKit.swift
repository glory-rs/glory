#if canImport(UIKit)
import UIKit
import UniformTypeIdentifiers

private let gloryApplicationIO: GloryIOLedger = {
    let ledger = GloryIOLedger()
    ledger.start = { [weak ledger] record, service in
        guard let ledger = ledger else { return }
        guard let input = ledger.snapshot(record) else { ledger.finish(record, GloryIO.unavailable); return }
        if service == 2 && record.kind == 1 { gloryStartRead(ledger, record, input) }
        else { DispatchQueue.main.async { GloryIOUI.shared.start(ledger, record, service, input) } }
    }
    ledger.cancelPicker = { record in DispatchQueue.main.async { GloryIOUI.shared.cancel(record) } }
    return ledger
}()
#if GLORY_IO_PROBE
@MainActor func gloryIOCanRestoreProbe() -> Bool {
    GloryIOUI.shared.window() != nil && gloryApplicationIO.clipboardIdle()
}
#endif
// One proposed application-only integer ABI. It accepts every integer, retains
// no caller pointers and never requires callers to own a native thread/object.
@_cdecl("glory_mobile_io")
func gloryMobileIO(_ service: Int64, _ client: Int64, _ op: Int64, _ request: Int64, _ a: Int64, _ b: Int64) -> Int64 {
    guard service == 1 || service == 2 else { return GloryIO.unavailable }
    #if !GLORY_CLIPBOARD_HOST
    if service == 1 { return GloryIO.unavailable }
    #endif
    #if !GLORY_FILES_HOST
    if service == 2 { return GloryIO.unavailable }
    #endif
    #if GLORY_IO_PROBE
    if op == 100 { return GloryIOProbe.shared.command(service, a) }
    #endif
    return gloryApplicationIO.call(service, client, op, request, a, b)
}
@MainActor
private final class GloryIOUI {
    static let shared = GloryIOUI()
    var picker: GloryIOPicker?
    func window() -> UIWindow? {
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes where scene.activationState == .foregroundActive {
            if let window = scene.windows.first(where: { !$0.isHidden && NSStringFromClass(type(of: $0)) == "WinitUIWindow" }) { return window }
        }
        return nil
    }
    func start(_ ledger: GloryIOLedger, _ record: GloryIORecord, _ service: Int64, _ input: [UInt8]) {
        guard ledger.snapshot(record) != nil, let window = window() else { ledger.finish(record, GloryIO.unavailable); return }
        if service == 1 {
            #if GLORY_CLIPBOARD_HOST
            do {
                let board = UIPasteboard.general
                #if GLORY_IO_PROBE
                try GloryIOProbe.shared.checkOwned(board)
                #endif
                if record.kind == 0 {
                    let bytes = try gloryCanonicalClipboard(input, 65_536)
                    board.string = String(decoding: bytes, as: UTF8.self)
                    #if GLORY_IO_PROBE
                    GloryIOProbe.shared.didWrite(board)
                    #endif
                    ledger.finish(record, 1)
                } else {
                    var packet = GloryPacket(input); let limit = try packet.integer(4)
                    guard packet.remaining == 0, (1...65_536).contains(limit) else { throw GloryIOError(GloryIO.bytes) }
                    let hasText = board.hasStrings
                    let value = hasText ? board.string : ""
                    guard let value = value else { throw GloryIOError(GloryIO.denied) }
                    // OS NSString allocation is outside the transfer budget.
                    let raw = Array(value.utf8.prefix(Int(limit)+1))
                    ledger.finish(record, 2, try gloryCanonicalClipboard(raw, Int(limit)))
                }
            } catch { ledger.finish(record, gloryIOFailure(error)) }
            #else
            ledger.finish(record, GloryIO.unavailable)
            #endif
            return
        }
        #if GLORY_FILES_HOST
        do {
            let options = try GloryFileOptions(input)
            #if GLORY_IO_PROBE
            if let mode = GloryIOProbe.shared.takeFileMode() {
                if mode == 2 { ledger.finish(record, 1); return }
                let urls = try GloryIOProbe.shared.urls(mode)
                gloryOpenSelection(ledger, record, urls, options); return
            }
            #endif
            guard let presenter = window.rootViewController, presenter.presentedViewController == nil else { throw GloryIOError(GloryIO.busy) }
            var types: [UTType] = []
            for filter in options.filters {
                let type: UTType?
                if filter.hasPrefix(".") { type = UTType(filenameExtension: String(filter.dropFirst())) }
                else if filter.hasSuffix("/*") {
                    switch filter { case "image/*": type = .image; case "audio/*": type = .audio; case "video/*": type = .movie; case "text/*": type = .text; default: type = .data }
                } else { type = UTType(mimeType: filter) }
                // Unknown mappings broaden the picker only. Selected metadata
                // is always validated against the original finite filters.
                types.append(type ?? .data)
            }
            let controller = UIDocumentPickerViewController(forOpeningContentTypes: types.isEmpty ? [.data] : types, asCopy: false)
            controller.allowsMultipleSelection = options.multiple
            let delegate = GloryIOPicker(ledger, record, options, window.windowScene, self)
            controller.delegate = delegate; delegate.controller = controller; picker = delegate
            presenter.present(controller, animated: true) { controller.presentationController?.delegate = delegate }
        } catch { ledger.finish(record, gloryIOFailure(error)) }
        #else
        ledger.finish(record, GloryIO.unavailable)
        #endif
    }
    func cancel(_ record: GloryIORecord) { if picker?.record === record { picker?.cancel() } }
}
@MainActor
private final class GloryIOPicker: NSObject, UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate {
    let ledger: GloryIOLedger, record: GloryIORecord, options: GloryFileOptions
    weak var scene: UIWindowScene?, controller: UIDocumentPickerViewController?, host: GloryIOUI?
    private var ended = false
    init(_ ledger: GloryIOLedger, _ record: GloryIORecord, _ options: GloryFileOptions, _ scene: UIWindowScene?, _ host: GloryIOUI) {
        self.ledger = ledger; self.record = record; self.options = options; self.scene = scene; self.host = host
    }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard !ended else { return }; ended = true
        controller.dismiss(animated: true) {
            self.host?.picker = nil
            gloryOpenSelection(self.ledger, self.record, urls, self.options)
        }
    }
    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { cancel() }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        guard !ended else { return }; ended = true; host?.picker = nil; ledger.finish(record, 1)
    }
    func cancel() {
        guard !ended else { return }; ended = true
        guard let controller = controller else { host?.picker = nil; ledger.finish(record, 1); return }
        controller.dismiss(animated: true) { self.host?.picker = nil; self.ledger.finish(self.record, 1) }
    }
}
func gloryIOSceneDisconnected(_ scene: UIScene) {
    MainActor.assumeIsolated {
        if GloryIOUI.shared.picker?.scene === scene { GloryIOUI.shared.picker?.cancel() }
    }
}
#endif
