use glory_render::{ButtonProps, Descriptor, Error, InputProps, Props, TextProps};
use glory_ui::{App, Error as UiError, Widget, view};

#[test]
fn shared_child_rules_reject_before_identity_allocation_and_leave_the_scope_usable() {
    for (parent, child, error) in [
        (
            Props::Input(InputProps::default()),
            Props::View,
            "leaf component cannot contain children",
        ),
        (
            Props::Button(ButtonProps::default()),
            Props::View,
            "button children must be inline text, images or groups",
        ),
        (
            Props::Button(ButtonProps::default()),
            Props::Input(InputProps::default()),
            "interactive controls cannot be nested",
        ),
    ] {
        let app = App::memory();
        let owner = app.scope().child().unwrap();
        let parent = owner.create(Descriptor::new(parent)).unwrap();
        let scope = owner.child_in(parent).unwrap();
        let before = (app.stats(), app.tree().stats());
        assert_eq!(
            scope.create(Descriptor::new(child)),
            Err(UiError::Render(Error::InvalidProps(error)))
        );
        assert_eq!((app.stats(), app.tree().stats()), before);
        let next = owner
            .create(Descriptor::new(Props::Text(TextProps::default())))
            .unwrap();
        assert_eq!(next.slot(), parent.slot() + 1);
        app.flush().unwrap();
        assert!(app.scope().is_active(next).unwrap());
        owner.dispose();
        app.flush().unwrap();
        assert_eq!(app.stats().nodes, 0);
    }
}

#[test]
fn a_surviving_scope_cannot_allocate_children_under_a_retired_or_reused_parent() {
    let app = App::memory();
    let parent = view().mount(app.scope()).unwrap();
    let old = parent.roots()[0];
    // Ownership is independent of logical placement. This scope survives parent.
    let child = app.scope().child_in(old).unwrap();
    drop(parent);
    let replacement = view().mount(app.scope()).unwrap();
    let new = replacement.roots()[0];
    assert_eq!(old.slot(), new.slot());
    assert_ne!(old, new);
    let before = (app.stats(), app.tree().stats());
    assert_eq!(
        child.create(Descriptor::new(Props::View)),
        Err(UiError::Render(Error::NodeGone(old)))
    );
    assert_eq!((app.stats(), app.tree().stats()), before);
    app.flush().unwrap();
    assert!(app.scope().is_active(new).unwrap());
}
