#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::*;
use glory_ui::{App, button};
#[test]
fn native_refuses_physical_clipboard_after_canonical_revision_and_inherited_checks() {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let r = Renderer::new(runtime);
        renderer = Some(r.clone());
        r
    });
    let mut renderer = renderer.unwrap();
    let node = app.scope().cage(None);
    app.mount(
        button()
            .node_ref(node)
            .label("Copy")
            .semantics(Some(Box::new(Semantics {
                clipboard: Some(ClipboardText {
                    text: "private native source".into(),
                    revision: 0,
                }),
                ..Default::default()
            }))),
    )
    .unwrap();
    let id = node.get().unwrap();
    assert!(!renderer.capabilities().clipboard_write);
    assert!(matches!(
        renderer.action(Action::CopyText {
            node: id,
            request: 1,
            revision: 1
        }),
        Err(Error::InvalidProps(_))
    ));
    assert!(matches!(
        renderer.action(Action::CopyText {
            node: id,
            request: 1,
            revision: 0
        }),
        Err(Error::Unsupported("native text clipboard write"))
    ));
    assert_eq!(app.tree().next_clipboard_request(id), Ok(1));
    let applied = app.stats();
    assert!(app.scope().copy_text(id).is_err());
    assert_eq!(app.stats(), applied);
    app.scope()
        .patch(id, Patch::Enabled(Change::Set(false)))
        .unwrap();
    app.flush().unwrap();
    assert!(
        renderer
            .action(Action::CopyText {
                node: id,
                request: 1,
                revision: 0
            })
            .is_err()
    );
    app.clear().unwrap();
    assert!(matches!(
        renderer.action(Action::CopyText {
            node: id,
            request: 1,
            revision: 0
        }),
        Err(Error::NodeGone(_))
    ));
}
