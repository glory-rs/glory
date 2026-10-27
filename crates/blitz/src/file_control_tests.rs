use super::focus_tests::{focus, key};
use super::tests::pointer;
use super::*;
use crate::files::{FilePicker, Files, Reply, SelectedFile};
use glory_render::FileConfig;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Waker;
#[derive(Default)]
struct Picker(AtomicUsize);
impl FilePicker for Picker {
    fn pick(&self, _: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Box::pin(std::future::ready(Ok(None)))
    }
}
#[test]
fn file_controls_use_intrinsic_keyboard_pointer_and_focus_paths() {
    let picker = Arc::new(Picker::default());
    let mut host = WindowApp::with_config(Config {
        files: Some(Files::new(picker.clone())),
        ..Default::default()
    });
    let scope = host.app.scope();
    let node = scope.cage(None);
    let enabled = scope.cage(true);
    let weak = scope.downgrade();
    host.mount(
        glory_ui::file()
            .label("Choose 中文")
            .node_ref(node)
            .enabled(enabled)
            .on(EventKind::Activate, move |_| {
                weak.upgrade()
                    .unwrap()
                    .pick_files(node.get().unwrap())
                    .unwrap();
            }),
    )
    .unwrap();
    let id = node.get().unwrap();
    focus(&mut host, id);
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(picker.0.load(Ordering::Relaxed), 1);
    key(&mut host, Key::Character(" ".into()), Modifiers::empty());
    assert_eq!(
        picker.0.load(Ordering::Relaxed),
        1,
        "Space activates only on release"
    );
    host.handle_ui_event(UiEvent::KeyUp(BlitzKeyEvent {
        key: Key::Character(" ".into()),
        code: keyboard_types::Code::Space,
        modifiers: Modifiers::empty(),
        location: keyboard_types::Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: blitz_traits::events::KeyState::Released,
        text: None,
    }));
    host.observe(&Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(picker.0.load(Ordering::Relaxed), 2);
    let physical = host.renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let rect = host
        .renderer
        .inspect(|doc| doc.get_client_bounding_rect(physical).unwrap());
    let (x, y) = (
        (rect.x + rect.width / 2.) as f32,
        (rect.y + rect.height / 2.) as f32,
    );
    host.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    host.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    host.observe(&Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(picker.0.load(Ordering::Relaxed), 3);
    enabled.revise(|v| *v = false);
    host.app.flush().unwrap();
    host.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    host.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    host.observe(&Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(picker.0.load(Ordering::Relaxed), 3);
    assert_eq!(
        host.renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        physical
    );
}
