use glory_render::{Descriptor, Props};
use glory_ui::App;

#[test]
fn explicit_place_preserves_original_owner_and_validates_before_queueing() {
    let app = App::memory();
    let root = app.scope();
    let left = root.create(Descriptor::new(Props::View)).unwrap();
    let right = root.create(Descriptor::new(Props::View)).unwrap();
    let owner = root.child_in(left).unwrap();
    let item = owner.create(Descriptor::new(Props::View)).unwrap();
    app.flush().unwrap();
    root.place(item, Some(right), None).unwrap();
    app.flush().unwrap();
    assert_eq!(app.tree().node(item).unwrap().parent, Some(right));
    assert!(app.tree().node(left).unwrap().children.is_empty());
    let before = (app.stats(), app.tree().stats());
    assert!(root.place(right, Some(item), None).is_err());
    assert_eq!((app.stats(), app.tree().stats()), before);
    let foreign = App::memory();
    let other = foreign
        .scope()
        .create(Descriptor::new(Props::View))
        .unwrap();
    assert!(root.place(item, Some(other), None).is_err());
    assert_eq!((app.stats(), app.tree().stats()), before);
    owner.dispose();
    app.flush().unwrap();
    assert!(app.tree().node(item).is_err());
    assert!(app.tree().node(right).unwrap().children.is_empty());
    assert!(owner.place(right, None, None).is_err());
    assert!(root.place(item, Some(left), None).is_err());
}
