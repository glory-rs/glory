import Foundation
import CoreGraphics

let portrait = CGRect(x: 0, y: 0, width: 402, height: 874)
func inset(_ keyboard: CGRect?, bounds: CGRect = portrait,
           top: CGFloat = 62, bottom: CGFloat = 34) -> CGFloat {
    gloryKeyboardInset(bounds: bounds, keyboard: keyboard, safeTop: top, safeBottom: bottom)
}
precondition(inset(nil) == 0)
precondition(inset(CGRect(x: 0, y: 574, width: 402, height: 300)) == 266)
precondition(inset(CGRect(x: 0, y: 874, width: 402, height: 300)) == 0)
precondition(inset(CGRect(x: 0, y: -300, width: 402, height: 300)) == 0)
precondition(inset(CGRect(x: 40, y: 574, width: 280, height: 250)) == 0)
precondition(inset(CGRect(x: 40, y: 624, width: 280, height: 250)) == 0)
precondition(inset(CGRect(x: 0, y: 850, width: 402, height: 24)) == 0)
precondition(inset(CGRect(x: 0, y: -100, width: 402, height: 1100)) == 777)
precondition(inset(CGRect(x: 0, y: 0, width: 0, height: 500)) == 0)
precondition(inset(CGRect(x: 0, y: 0, width: CGFloat.infinity, height: 500)) == 0)
precondition(inset(CGRect(x: 0, y: 0, width: 402, height: 300), bounds: .zero) == 0)
precondition(inset(CGRect(x: 0, y: 180, width: 874, height: 222),
                   bounds: CGRect(x: 0, y: 0, width: 874, height: 402), top: 0, bottom: 21) == 201)
print("UIKit keyboard geometry: 12 cases passed")

var lifecycle = GlorySceneLifecycle<Int>()
precondition(!lifecycle.activate(1))
precondition(lifecycle.connect(1))
precondition(lifecycle.connect(1))
precondition(!lifecycle.connect(2))
precondition(lifecycle.activate(1))
precondition(!lifecycle.activate(1))
precondition(!lifecycle.deactivate(2))
precondition(!lifecycle.disconnect(2))
precondition(lifecycle.active && lifecycle.owns(1))
precondition(lifecycle.deactivate(1))
precondition(!lifecycle.deactivate(1))
precondition(lifecycle.disconnect(1))
precondition(!lifecycle.disconnect(1))
precondition(lifecycle.connect(2))
precondition(lifecycle.activate(2))
precondition(!lifecycle.deactivate(1))
precondition(!lifecycle.disconnect(1))
precondition(lifecycle.active && lifecycle.owns(2))
precondition(lifecycle.disconnect(2))
precondition(!lifecycle.active && lifecycle.scene == nil)
for token in 3...1002 {
    precondition(lifecycle.connect(token))
    precondition(lifecycle.activate(token))
    precondition(lifecycle.disconnect(token))
    precondition(!lifecycle.active && lifecycle.scene == nil)
}
print("UIKit scene lifecycle: duplicate/stale callbacks and 1000 reconnects passed")
