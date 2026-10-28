import UIKit
import FileProvider

final class ProviderDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: "Fixture", sessionRole: session.role)
        configuration.delegateClass = ProviderScene.self
        return configuration
    }
}
final class ProviderScene: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let controller = UIViewController(); controller.view.backgroundColor = .systemBackground
        let label = UILabel(); label.numberOfLines = 0; label.textAlignment = .center
        label.text = "Registering owned replicated provider…"
        label.translatesAutoresizingMaskIntoConstraints = false; controller.view.addSubview(label)
        NSLayoutConstraint.activate([label.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor, constant: 24), label.trailingAnchor.constraint(equalTo: controller.view.trailingAnchor, constant: -24), label.centerYAnchor.constraint(equalTo: controller.view.centerYAnchor)])
        let window = UIWindow(windowScene: scene); window.rootViewController = controller; window.makeKeyAndVisible(); self.window = window
        #if GLORY_REPLICATED
        let domain = NSFileProviderDomain(identifier: NSFileProviderDomainIdentifier("glory-owned-hydration-v1"), displayName: "Glory Hydration")
        NSFileProviderManager.add(domain) { error in
            DispatchQueue.main.async { label.text = error.map { "Registration failed: \($0)" } ?? "Glory Hydration registered\nUse Files → Browse → Glory Hydration\nContents are provided after a three-second delay." }
            NSLog("Glory hydration registration: %@", error.map(String.init(describing:)) ?? "ready")
            if error == nil, let manager = NSFileProviderManager(for: domain) {
                manager.signalEnumerator(for: .rootContainer) { NSLog("Glory hydration signal: %@", String(describing: $0)) }
                manager.signalEnumerator(for: .workingSet) { _ in }
            }
        }
        #else
        label.text = "Glory Downloads enumerating extension ready\nUse Files → Browse → Glory Downloads\nContents are provided after a three-second delay."
        #endif
    }
}
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(ProviderDelegate.self))
