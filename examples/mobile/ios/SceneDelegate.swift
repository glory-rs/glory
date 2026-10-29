import Foundation
import CoreGraphics

// A single-window application may reconnect its scene without recreating the
// Rust event loop/document. Only the current native scene can drive lifecycle;
// delayed callbacks from a disconnected scene must not suspend its successor.
struct GlorySceneLifecycle<Token: Equatable> {
    private(set) var scene: Token?
    private(set) var active = false

    func owns(_ token: Token) -> Bool { scene == token }
    mutating func connect(_ token: Token) -> Bool {
        guard scene == nil || scene == token else { return false }
        scene = token
        return true
    }
    mutating func activate(_ token: Token) -> Bool {
        guard owns(token), !active else { return false }
        active = true
        return true
    }
    mutating func deactivate(_ token: Token) -> Bool {
        guard owns(token), active else { return false }
        active = false
        return true
    }
    mutating func disconnect(_ token: Token) -> Bool {
        guard owns(token) else { return false }
        scene = nil
        active = false
        return true
    }
}

// Docked keyboards reserve the covered bottom strip. Floating/split keyboards
// do not shrink the entire viewport. Keep one point for the native viewport.
func gloryKeyboardInset(bounds: CGRect, keyboard: CGRect?, safeTop: CGFloat,
                        safeBottom: CGFloat) -> CGFloat {
    guard let keyboard = keyboard,
          [bounds.minX, bounds.minY, bounds.width, bounds.height,
           keyboard.minX, keyboard.minY, keyboard.width, keyboard.height,
           safeTop, safeBottom].allSatisfy({ $0.isFinite }),
          bounds.width > 0, bounds.height > 0, keyboard.width > 0,
          keyboard.height > 0 else { return 0 }
    let covered = bounds.intersection(keyboard)
    guard !covered.isNull, covered.width >= bounds.width - 1,
          covered.maxY >= bounds.maxY - 1 else { return 0 }
    let top = max(0, safeTop), bottom = max(0, safeBottom)
    return min(max(0, covered.height - bottom), max(0, bounds.height - top - bottom - 1))
}

#if canImport(UIKit)
import UIKit

@MainActor
private enum GlorySceneHost {
    static var lifecycle = GlorySceneLifecycle<ObjectIdentifier>()
    // Wingy owns the window strongly. These do not retain disconnected scenes
    // or create another native/semantic window, including after delegate deinit.
    static weak var window: UIWindow?
    static weak var responder: UIResponder?
    static var restoreResponder = false
    #if GLORY_SCENE_PROBE
    static weak var anchorScene: UIWindowScene?
    #endif

    static func capture(_ scene: UIWindowScene, rememberResponder: Bool = false) {
        if let native = scene.windows.first(where: {
            NSStringFromClass(type(of: $0)) == "WingyUIWindow"
        }) {
            if window !== native {
                FileHandle.standardError.write(Data("Glory UIKit captured window=\(ObjectIdentifier(native))\n".utf8))
            }
            window = native
            if rememberResponder { responder = firstResponder(in: native) }
        }
    }

    private static func firstResponder(in view: UIView) -> UIResponder? {
        if view.isFirstResponder { return view }
        for child in view.subviews {
            if let responder = firstResponder(in: child) { return responder }
        }
        return nil
    }
}

// A single application scene feeds Wingy's existing lifecycle notifications.
// Wingy owns the UIWindow, semantic document and renderer. It associates that
// window with the connected UIWindowScene before making the window visible.
@objc(GlorySceneDelegate)
final class GlorySceneDelegate: NSObject, UIWindowSceneDelegate {
    private weak var windowScene: UIWindowScene?
    private var keyboardObservers: [NSObjectProtocol] = []
    #if GLORY_SCENE_PROBE
    private var sceneProbe: UIButton?
    private var keepAliveProbe: UIButton?
    private var anchorWindow: UIWindow?
    #endif

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
               options connectionOptions: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        #if GLORY_SCENE_PROBE
        if !GlorySceneHost.lifecycle.connect(ObjectIdentifier(scene)) {
            installAnchor(scene)
            return
        }
        #else
        guard GlorySceneHost.lifecycle.connect(ObjectIdentifier(scene)) else { return }
        #endif
        windowScene = scene
        if let window = GlorySceneHost.window, window.windowScene !== scene {
            window.windowScene = scene
            window.rootViewController?.additionalSafeAreaInsets.bottom = 0
            window.frame = scene.coordinateSpace.bounds
            window.makeKeyAndVisible()
            window.rootViewController?.view.setNeedsLayout()
            window.rootViewController?.view.layoutIfNeeded()
            FileHandle.standardError.write(Data("Glory UIKit scene reattached window=\(ObjectIdentifier(window))\n".utf8))
        }
        FileHandle.standardError.write(Data("Glory UIKit scene connected session=\(session.persistentIdentifier)\n".utf8))
        stopKeyboardObservers()
        for name in [UIResponder.keyboardWillChangeFrameNotification,
                     UIResponder.keyboardWillHideNotification] {
            keyboardObservers.append(NotificationCenter.default.addObserver(
                forName: name, object: nil, queue: .main) { [weak self] notification in
                    self?.keyboardChanged(notification)
                })
        }
    }

    private func keyboardChanged(_ notification: Notification) {
        guard let scene = windowScene,
              GlorySceneHost.lifecycle.owns(ObjectIdentifier(scene)),
              scene.activationState != .unattached,
              notification.userInfo?[UIResponder.keyboardIsLocalUserInfoKey] as? Bool != false,
              let window = scene.windows.first(where: { $0.isKeyWindow }),
              let controller = window.rootViewController else { return }
        var frame: CGRect?
        if notification.name != UIResponder.keyboardWillHideNotification,
           let screenFrame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect {
            frame = window.convert(screenFrame, from: scene.screen.coordinateSpace)
        }
        let bottom = gloryKeyboardInset(bounds: window.bounds, keyboard: frame,
                                       safeTop: window.safeAreaInsets.top,
                                       safeBottom: window.safeAreaInsets.bottom)
        if controller.additionalSafeAreaInsets.bottom != bottom {
            controller.additionalSafeAreaInsets.bottom = bottom
            // Wingy's layout callback publishes the same physical surface size
            // with refreshed safe-area insets; only the content viewport shrinks.
            controller.view.setNeedsLayout()
            controller.view.layoutIfNeeded()
            controller.view.setNeedsDisplay()
            FileHandle.standardError.write(Data("Glory UIKit keyboard inset \(bottom)\n".utf8))
        }
    }

    private func stopKeyboardObservers() {
        for observer in keyboardObservers { NotificationCenter.default.removeObserver(observer) }
        keyboardObservers.removeAll()
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        stopKeyboardObservers()
        #if GLORY_SCENE_PROBE
        sceneProbe?.removeFromSuperview()
        sceneProbe = nil
        keepAliveProbe?.removeFromSuperview()
        keepAliveProbe = nil
        anchorWindow = nil
        if GlorySceneHost.anchorScene === scene { GlorySceneHost.anchorScene = nil }
        #endif
        guard GlorySceneHost.lifecycle.owns(ObjectIdentifier(scene)) else { return }
        if let scene = scene as? UIWindowScene {
            GlorySceneHost.capture(scene, rememberResponder: GlorySceneHost.lifecycle.active)
        }
        suspend(scene)
        GlorySceneHost.restoreResponder = GlorySceneHost.responder != nil
        if let window = GlorySceneHost.window, window.windowScene === scene {
            window.endEditing(true)
            window.rootViewController?.additionalSafeAreaInsets.bottom = 0
            window.isHidden = true
            window.windowScene = nil
        }
        _ = GlorySceneHost.lifecycle.disconnect(ObjectIdentifier(scene))
        windowScene = nil
        FileHandle.standardError.write(Data("Glory UIKit scene disconnected\n".utf8))
    }

    deinit {
        for observer in keyboardObservers { NotificationCenter.default.removeObserver(observer) }
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        guard GlorySceneHost.lifecycle.activate(ObjectIdentifier(scene)) else { return }
        NotificationCenter.default.post(name: UIApplication.didBecomeActiveNotification,
                                        object: UIApplication.shared)
        if let scene = scene as? UIWindowScene {
            GlorySceneHost.capture(scene)
            if GlorySceneHost.restoreResponder {
                GlorySceneHost.restoreResponder = false
                // The weak native responder may already have been released.
                // Restore only after the window and CPU surface are attached.
                let restored = GlorySceneHost.responder?.becomeFirstResponder() ?? false
                FileHandle.standardError.write(Data("Glory UIKit responder restored=\(restored)\n".utf8))
            }
            GlorySceneHost.responder = nil
            #if GLORY_SCENE_PROBE
            installSceneProbe(scene)
            #endif
        }
        FileHandle.standardError.write(Data("Glory UIKit scene active\n".utf8))
        #if GLORY_PERMISSION_HOST_PROBE
        gloryPermissionsHostProbe()
        #endif
    }
    func sceneWillResignActive(_ scene: UIScene) {
        guard GlorySceneHost.lifecycle.owns(ObjectIdentifier(scene)) else { return }
        if let scene = scene as? UIWindowScene { GlorySceneHost.capture(scene, rememberResponder: true) }
        suspend(scene)
    }
    private func suspend(_ scene: UIScene) {
        guard GlorySceneHost.lifecycle.deactivate(ObjectIdentifier(scene)) else { return }
        NotificationCenter.default.post(name: UIApplication.willResignActiveNotification,
                                        object: UIApplication.shared)
        FileHandle.standardError.write(Data("Glory UIKit scene inactive\n".utf8))
    }
    func sceneWillEnterForeground(_ scene: UIScene) {
        guard GlorySceneHost.lifecycle.owns(ObjectIdentifier(scene)) else { return }
        NotificationCenter.default.post(name: UIApplication.willEnterForegroundNotification,
                                        object: UIApplication.shared)
    }
    func sceneDidEnterBackground(_ scene: UIScene) {
        guard GlorySceneHost.lifecycle.owns(ObjectIdentifier(scene)) else { return }
        NotificationCenter.default.post(name: UIApplication.didEnterBackgroundNotification,
                                        object: UIApplication.shared)
    }

    #if GLORY_SCENE_PROBE
    // Opt-in acceptance controls call UIKit itself. They do not simulate scene
    // delegate callbacks. The ordinary application contains no probe overlay.
    private func installSceneProbe(_ scene: UIWindowScene) {
        guard ProcessInfo.processInfo.environment["GLORY_IOS_SCENE_PROBE"] == "1",
              let view = GlorySceneHost.window?.rootViewController?.view else { return }
        // Resuming CPU presentation replaces its native layer. Recreate the
        // test overlays above it, including ordinary foreground resumes.
        sceneProbe?.removeFromSuperview()
        keepAliveProbe?.removeFromSuperview()
        let button = UIButton(type: .system)
        button.setTitle("Discard scene", for: .normal)
        button.accessibilityIdentifier = "glory-scene-discard"
        button.backgroundColor = .systemBackground
        button.layer.zPosition = 1
        button.translatesAutoresizingMaskIntoConstraints = false
        button.addAction(UIAction { [weak scene, weak button] _ in
            guard let scene = scene,
                  GlorySceneHost.lifecycle.owns(ObjectIdentifier(scene)) else { return }
            button?.isEnabled = false
            UIApplication.shared.requestSceneSessionDestruction(scene.session, options: nil) { error in
                button?.isEnabled = true
                FileHandle.standardError.write(Data("Glory UIKit discard failed: \(error)\n".utf8))
            }
        }, for: .touchUpInside)
        view.addSubview(button)
        NSLayoutConstraint.activate([
            button.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -24),
            button.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 96),
            button.widthAnchor.constraint(equalToConstant: 160),
            button.heightAnchor.constraint(equalToConstant: 44),
        ])
        sceneProbe = button
        let keep = UIButton(type: .system)
        keep.setTitle("Keep process alive", for: .normal)
        keep.accessibilityIdentifier = "glory-scene-keep-alive"
        keep.backgroundColor = .systemBackground
        keep.layer.zPosition = 1
        keep.translatesAutoresizingMaskIntoConstraints = false
        keep.addAction(UIAction { _ in
            UIApplication.shared.requestSceneSessionActivation(GlorySceneHost.anchorScene?.session, userActivity: nil, options: nil) { error in
                FileHandle.standardError.write(Data("Glory UIKit anchor failed: \(error)\n".utf8))
            }
        }, for: .touchUpInside)
        view.addSubview(keep)
        NSLayoutConstraint.activate([
            keep.trailingAnchor.constraint(equalTo: button.trailingAnchor),
            keep.topAnchor.constraint(equalTo: button.bottomAnchor, constant: 8),
            keep.widthAnchor.constraint(equalTo: button.widthAnchor),
            keep.heightAnchor.constraint(equalTo: button.heightAnchor),
        ])
        keepAliveProbe = keep
    }

    private func installAnchor(_ scene: UIWindowScene) {
        // Discarding the last scene may terminate the process. A test-only
        // native anchor keeps UIKit alive while the real Glory scene is
        // disconnected/recreated. It creates no Rust document or event loop.
        let window = UIWindow(windowScene: scene)
        let controller = UIViewController()
        controller.view.backgroundColor = .systemBackground
        let label = UILabel()
        label.text = "Glory scene keep-alive"
        label.translatesAutoresizingMaskIntoConstraints = false
        controller.view.addSubview(label)
        let open = UIButton(type: .system)
        open.setTitle("Open Glory scene", for: .normal)
        open.accessibilityIdentifier = "glory-scene-open"
        open.translatesAutoresizingMaskIntoConstraints = false
        open.addAction(UIAction { _ in
            UIApplication.shared.requestSceneSessionActivation(
                GlorySceneHost.window?.windowScene?.session,
                userActivity: nil, options: nil) { error in
                    FileHandle.standardError.write(Data("Glory UIKit activation failed: \(error)\n".utf8))
                }
        }, for: .touchUpInside)
        controller.view.addSubview(open)
        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: controller.view.centerXAnchor),
            label.topAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.topAnchor, constant: 80),
            open.centerXAnchor.constraint(equalTo: label.centerXAnchor),
            open.topAnchor.constraint(equalTo: label.bottomAnchor, constant: 24),
        ])
        window.rootViewController = controller
        window.makeKeyAndVisible()
        anchorWindow = window
        GlorySceneHost.anchorScene = scene
        FileHandle.standardError.write(Data("Glory UIKit anchor connected\n".utf8))
    }
    #endif
}
#endif
