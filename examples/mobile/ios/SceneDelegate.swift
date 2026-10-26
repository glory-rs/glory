import UIKit

// A single application scene feeds Winit's existing lifecycle notifications.
// Winit owns the UIWindow, semantic document and renderer. It associates that
// window with the connected UIWindowScene before making the window visible.
@objc(GlorySceneDelegate)
final class GlorySceneDelegate: NSObject, UIWindowSceneDelegate {
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
