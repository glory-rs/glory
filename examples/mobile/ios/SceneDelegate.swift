import Foundation
import CoreGraphics

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

// A single application scene feeds Winit's existing lifecycle notifications.
// Winit owns the UIWindow, semantic document and renderer. It associates that
// window with the connected UIWindowScene before making the window visible.
@objc(GlorySceneDelegate)
final class GlorySceneDelegate: NSObject, UIWindowSceneDelegate {
    private weak var windowScene: UIWindowScene?
    private var keyboardObservers: [NSObjectProtocol] = []

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
               options connectionOptions: UIScene.ConnectionOptions) {
        windowScene = scene as? UIWindowScene
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
        guard let scene = windowScene, scene.activationState != .unattached,
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
            // Winit's layout callback publishes the same physical surface size
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
        windowScene = nil
    }

    deinit {
        for observer in keyboardObservers { NotificationCenter.default.removeObserver(observer) }
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        NotificationCenter.default.post(name: UIApplication.didBecomeActiveNotification,
                                        object: UIApplication.shared)
        FileHandle.standardError.write(Data("Glory UIKit scene active\n".utf8))
    }
    func sceneWillResignActive(_ scene: UIScene) {
        NotificationCenter.default.post(name: UIApplication.willResignActiveNotification,
                                        object: UIApplication.shared)
        FileHandle.standardError.write(Data("Glory UIKit scene inactive\n".utf8))
    }
    func sceneWillEnterForeground(_ scene: UIScene) {
        NotificationCenter.default.post(name: UIApplication.willEnterForegroundNotification,
                                        object: UIApplication.shared)
    }
    func sceneDidEnterBackground(_ scene: UIScene) {
        NotificationCenter.default.post(name: UIApplication.didEnterBackgroundNotification,
                                        object: UIApplication.shared)
    }
}
#endif
