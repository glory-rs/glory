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

#[test]
fn reactive_initial_fields_validate_together_and_preserve_first_value_revision() {
    let app = App::memory();
    let content = app.scope().cage("first\nsecond".to_string());
    app.mount(glory_ui::input().value(content).multiline(true))
        .unwrap();
    let node = app.tree().roots()[0];
    assert!(
        matches!(&app.tree().node(node).unwrap().descriptor.view().props,Props::Input(value) if value.multiline && value.value=="first\nsecond"&&value.revision==1)
    );
    content.revise(|text| *text = "updated\nvalue".into());
    app.flush().unwrap();
    assert!(
        matches!(&app.tree().node(node).unwrap().descriptor.view().props,Props::Input(value) if value.value=="updated\nvalue"&&value.revision==2)
    );
}
#[test]
fn initial_sampling_is_untracked_by_the_surrounding_structure_and_registers_its_own_binding() {
    let app = App::memory();
    let show = app.scope().cage(true);
    let content = app.scope().cage("first".to_string());
    let builds = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = builds.clone();
    app.mount(glory_ui::when(show, move || {
        count.set(count.get() + 1);
        glory_ui::text().content(content)
    }))
    .unwrap();
    let root = app.tree().roots()[0];
    let node = app.tree().node(root).unwrap().children[0];
    content.revise(|text| *text = "second".into());
    app.flush().unwrap();
    assert_eq!(builds.get(), 1);
    assert!(
        matches!(&app.tree().node(node).unwrap().descriptor.view().props,Props::Text(value) if value.content=="second")
    );
}
#[test]
fn retired_reactive_initial_values_fail_before_identity_allocation() {
    let app = App::memory();
    let owner = app.scope().owner().child();
    let content = owner.cage("retired".to_string());
    owner.dispose();
    let before = (app.stats(), app.tree().stats());
    assert!(app.mount(glory_ui::text().content(content)).is_err());
    assert_eq!((app.stats(), app.tree().stats()), before);
}
