// Glory iOS host entry: hands control to the Rust staticlib, which builds
// the tao event loop + wry WKWebView and never returns.
import UIKit

@_silgen_name("start_app")
func start_app()

start_app()
