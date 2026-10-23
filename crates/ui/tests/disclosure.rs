use glory_render::{Change, Disclosure, Patch, Props};
use glory_ui::*;
#[test]
fn actual_button_binding_updates_disclosure_and_drops_retired_target() {
    let app = App::memory();
    let scope = app.scope();
    let target = scope.cage(None);
    let button_id = scope.cage(None);
    let expanded = scope.cage(false);
    let disclosure = scope.bond(move || {
        target.get().map(|controls| Disclosure {
            expanded: expanded.get(),
            controls,
        })
    });
    app.mount(
        group().fill((
            view().node_ref(target),
            button()
                .label("Menu")
                .node_ref(button_id)
                .disclosure(disclosure),
        )),
    )
    .unwrap();
    app.flush().unwrap();
    let id = button_id.get().unwrap();
    assert!(
        !app.tree()
            .node(id)
            .unwrap()
            .descriptor
            .disclosure()
            .unwrap()
            .expanded
    );
    expanded.revise(|value| *value = true);
    app.flush().unwrap();
    assert!(
        app.tree()
            .node(id)
            .unwrap()
            .descriptor
            .disclosure()
            .unwrap()
            .expanded
    );
    scope.patch(id, Patch::Disclosure(Change::Clear)).unwrap();
    app.flush().unwrap();
    assert!(matches!(
        app.tree().node(id).unwrap().descriptor.props,
        Props::Button(_)
    ));
    assert!(
        app.tree()
            .node(id)
            .unwrap()
            .descriptor
            .disclosure()
            .is_none()
    );
    app.clear().unwrap();
    assert!(target.get().is_none());
    assert!(button_id.get().is_none());
}
#[test]
fn disclosure_cross_runtime_create_and_patch_fail_before_ui_queue_or_identity_changes() {
    let app = App::memory();
    let scope = app.scope();
    let other = App::memory();
    let foreign = other
        .scope()
        .create(glory_render::Descriptor::new(Props::View))
        .unwrap();
    let descriptor = glory_render::Descriptor::new(Props::Button(glory_render::ButtonProps {
        disclosure: Some(Disclosure {
            expanded: true,
            controls: foreign,
        }),
        ..Default::default()
    }));
    let before = app.stats();
    assert!(scope.create(descriptor).is_err());
    assert_eq!(app.stats().node_slots, before.node_slots);
    assert_eq!(app.stats().pending, before.pending);
    let button = scope
        .create(glory_render::Descriptor::new(Props::Button(
            Default::default(),
        )))
        .unwrap();
    app.flush().unwrap();
    let before = app.stats();
    assert!(
        scope
            .patch(
                button,
                Patch::Disclosure(Change::Set(Some(Disclosure {
                    expanded: true,
                    controls: foreign
                })))
            )
            .is_err()
    );
    assert_eq!(app.stats().pending, before.pending);
    assert!(
        app.tree()
            .node(button)
            .unwrap()
            .descriptor
            .disclosure()
            .is_none()
    );
}
