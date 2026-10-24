use super::*;
use glory_render::{FocusMode, Layer, SemanticId, Semantics};

#[test]
fn native_modal_ax_exports_real_dialog_focus_only_current_scope_and_rejects_late_background_actions()
 {
    let app = WindowApp::new();
    let open = app.app.scope().cage(false);
    let calls = app.app.scope().cage(0usize);
    let outside = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    let dialog = app.app.scope().cage(None);
    app.mount(
        view().name("Application").fill((
            button()
                .label("Outside action")
                .node_ref(outside)
                .on(glory_render::EventKind::Activate, move |_| {
                    calls.revise(|v| *v += 1)
                }),
            view()
                .name("Decision")
                .role(Role::Dialog)
                .focus(FocusMode::Programmatic)
                .node_ref(dialog)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(160.)),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(SemanticId::new("ax-modal").unwrap()),
                        hidden: !*open,
                        layer: Some(Layer::default()),
                        ..Default::default()
                    }))
                }))
                .fill(input().name("Decision editor").node_ref(editor)),
        )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let previous = id(&app, outside.get().unwrap());
    open.revise(|v| *v = true);
    app.app.settle(12).unwrap();
    // The live modality check also rejects an old AT id before its next tree
    // update retires that mapping. No application callback is invoked.
    assert!(!action(&app, &request(previous, AxAction::Click, None)).unwrap());
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let all = nodes(&tree);
    let modal = all
        .iter()
        .find(|node| node.role() == AxRole::Dialog)
        .unwrap();
    assert_eq!(modal.label().as_deref(), Some("Decision"));
    assert!(modal.data().is_modal());
    assert!(
        all.iter()
            .all(|node| node.label().as_deref() != Some("Outside action"))
    );
    assert_eq!(
        tree.state().focus().unwrap().id(),
        tree.state()
            .node_by_tree_local_id(id(&app, editor.get().unwrap()), TreeId::ROOT)
            .unwrap()
            .id()
    );
    assert_eq!(calls.get(), 0);
    drop(all);
    open.revise(|v| *v = false);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .all(|node| node.role() != AxRole::Dialog)
    );
    assert_ne!(
        id(&app, outside.get().unwrap()),
        previous,
        "reactivated AX mappings are never recycled"
    );
    assert!(!action(&app, &request(previous, AxAction::Click, None)).unwrap());
}
