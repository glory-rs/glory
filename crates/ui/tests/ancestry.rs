use glory_render::{Change, Descriptor, Patch, Props, Semantics};
use glory_ui::App;

#[test]
fn scope_ancestry_uses_live_staged_parents_and_accepts_hidden_or_suspended_ancestors() {
    let app = App::memory();
    let root = app.scope();
    let left = root.create(Descriptor::new(Props::View)).unwrap();
    let right = root.create(Descriptor::new(Props::View)).unwrap();
    let owner = root.child_in(left).unwrap();
    let container = owner.create(Descriptor::new(Props::View)).unwrap();
    let content = root.child_in(container).unwrap();
    let child = content.create(Descriptor::new(Props::View)).unwrap();
    let nested = root.child_in(child).unwrap();
    assert!(content.is_within(container).unwrap());
    assert!(nested.is_within(container).unwrap());
    assert!(nested.is_within(left).unwrap());
    assert!(!nested.is_within(right).unwrap());
    assert!(!root.is_within(container).unwrap());
    let before = (app.stats(), app.tree().stats());
    assert!(nested.is_within(left).unwrap());
    assert_eq!((app.stats(), app.tree().stats()), before);
    root.patch(
        container,
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            hidden: true,
            ..Default::default()
        })))),
    )
    .unwrap();
    assert!(nested.is_within(container).unwrap());
    root.place(container, Some(right), None).unwrap();
    assert!(!nested.is_within(left).unwrap());
    assert!(nested.is_within(right).unwrap());
    let foreign = App::memory();
    let other = foreign
        .scope()
        .create(Descriptor::new(Props::View))
        .unwrap();
    assert!(nested.is_within(other).is_err());
    owner.dispose();
    assert!(nested.is_within(container).is_err());
    nested.dispose();
    assert!(nested.is_within(right).is_err());
    assert!(matches!(
        nested.is_within(other),
        Err(glory_ui::Error::Gone)
    ));
}

#[test]
fn suspended_keep_mounted_context_remains_a_valid_construction_ancestor() {
    use glory_ui::{Widget, view};
    let app = App::memory();
    let reference = app.scope().cage(None);
    let mounted = view().node_ref(reference).mount(app.scope()).unwrap();
    let container = reference.get().unwrap();
    let content = app.scope().child_in(container).unwrap();
    mounted.suspend().unwrap();
    assert!(content.is_within(container).unwrap());
}
