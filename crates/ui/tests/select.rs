use glory_render::{Change, Error, Patch, Props, SelectProps};
use glory_ui::{App, Widget, select};

#[test]
fn widget_selection_allocator_uses_live_revision_and_exhaustion_never_wraps() {
    let app = App::memory();
    app.mount(select()).unwrap();
    let node = app.tree().roots()[0];
    app.scope()
        .patch(
            node,
            Patch::SelectSelection {
                keys: Change::Clear,
                revision: 50,
            },
        )
        .unwrap();
    app.scope().set_select_selection(node, vec![]).unwrap();
    let Props::Select(value) = app
        .tree()
        .node(node)
        .unwrap()
        .descriptor
        .view()
        .props
        .clone()
    else {
        panic!()
    };
    assert_eq!(value.selection.revision, 51);
    app.scope()
        .patch(
            node,
            Patch::SelectSelection {
                keys: Change::Clear,
                revision: u64::MAX,
            },
        )
        .unwrap();
    assert_eq!(
        app.scope().set_select_selection(node, vec![]),
        Err(glory_ui::Error::Render(Error::SelectRevisionExhausted))
    );
}

#[test]
fn select_builder_reactive_updates_validate_before_model_changes_and_retire() {
    let app = App::memory();
    let selected = app.scope().cage(Vec::new());
    let mounted = select()
        .configuration(SelectProps::default())
        .selected(selected)
        .mount(app.scope())
        .unwrap();
    app.flush().unwrap();
    let node = mounted.roots()[0];
    selected.revise(|keys| keys.push(glory_render::SemanticId::new("unknown").unwrap()));
    assert!(app.flush().is_err());
    let Props::Select(value) = app
        .tree()
        .node(node)
        .unwrap()
        .descriptor
        .view()
        .props
        .clone()
    else {
        panic!()
    };
    assert!(value.selection.keys.is_empty());
    assert_eq!(value.selection.revision, 1);
    let scope = mounted.scope().clone();
    drop(mounted);
    app.flush().unwrap();
    assert!(scope.set_select_selection(node, vec![]).is_err());
}

#[test]
fn initial_nonempty_binding_uses_a_new_selection_revision() {
    let app = App::memory();
    let key = glory_render::SemanticId::new("a").unwrap();
    let selected = app.scope().cage(vec![key.clone()]);
    app.mount(
        select()
            .configuration(SelectProps {
                entries: std::rc::Rc::new(vec![glory_render::SelectEntry::Option(
                    glory_render::SelectOption {
                        key,
                        label: "A".into(),
                        value: "business".into(),
                        disabled: false,
                    },
                )]),
                ..Default::default()
            })
            .selected(selected),
    )
    .unwrap();
    let tree = app.tree();
    let Props::Select(value) = tree.node(tree.roots()[0]).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(value.selection.revision, 1);
    assert_eq!(value.selection.keys.len(), 1);
}
