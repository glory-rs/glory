import UIKit

// Independent test application: its Documents are exported by Apple's actual
// Local Storage File Provider. No shared app group or consumer-container copy.
final class DocumentsDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: "Documents", sessionRole: session.role)
        configuration.delegateClass = DocumentsScene.self
        return configuration
    }
}
final class DocumentsScene: NSObject, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let controller = UIViewController(); controller.view.backgroundColor = .systemBackground
        let label = UILabel(); label.numberOfLines = 0; label.textAlignment = .center
        do {
            let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            try Data("Glory external provider: 中文😀\n".utf8).write(to: docs.appendingPathComponent("glory-provider-text.txt"))
            try Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQ2RL1HwAEJAIiMMFhVgAAAABJRU5ErkJggg==")!.write(to: docs.appendingPathComponent("glory-provider-image.png"))
            try Data([1,2,3,4]).write(to: docs.appendingPathComponent("rejected.bin"))
            label.text = "Glory Provider\nOwned sample documents ready\nUse Files → On My iPhone/iPad → Glory Provider"
        } catch { label.text = "Fixture creation failed" }
        label.translatesAutoresizingMaskIntoConstraints = false; controller.view.addSubview(label)
        NSLayoutConstraint.activate([label.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor, constant: 24), label.trailingAnchor.constraint(equalTo: controller.view.trailingAnchor, constant: -24), label.centerYAnchor.constraint(equalTo: controller.view.centerYAnchor)])
        let window = UIWindow(windowScene: scene); window.rootViewController = controller; window.makeKeyAndVisible(); self.window = window
    }
}
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(DocumentsDelegate.self))
