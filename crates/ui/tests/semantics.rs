use glory_render::{Change, Descriptor, Error, InputProps, Patch, Props, Role, TextProps};
use glory_ui::{App, Error as UiError, input};

#[test]
fn rejected_semantic_overrides_preserve_identity_queue_and_existing_bindings() {
    let app = App::memory();
    let value = app.scope().cage("raw🙂".to_owned());
    let name = app.scope().cage("Editor".to_owned());
    app.mount(input().value(value).name(name)).unwrap();
    let id = app.tree().roots()[0];
    let before = (app.stats(), app.tree().stats());
    assert_eq!(
        app.scope()
            .patch(id, Patch::Role(Change::Set(Some(Role::List)))),
        Err(UiError::Render(Error::InvalidProps(
            "structural roles require a view"
        )))
    );
    for props in [
        Props::Text(TextProps::default()),
        Props::Image(Default::default()),
    ] {
        let mut node = Descriptor::new(props);
        node.name = Some("silently ignored".into());
        assert!(app.scope().create(node).is_err());
    }
    assert_eq!((app.stats(), app.tree().stats()), before);
    let next = app
        .scope()
        .create(Descriptor::new(Props::Input(InputProps::default())))
        .unwrap();
    assert_eq!(next.slot(), id.slot() + 1);
    value.revise(|v| *v = "updated🙂".into());
    name.revise(|v| *v = "Updated editor".into());
    app.flush().unwrap();
    let tree = app.tree();
    let node = tree.node(id).unwrap().descriptor.view();
    assert_eq!(node.name.as_deref(), Some("Updated editor"));
    assert!(matches!(&node.props, Props::Input(input) if input.value == "updated🙂"));
}
