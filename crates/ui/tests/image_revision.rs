use glory_render::{Change, Event, EventKind, ImageStatus, Patch, Props};
use glory_ui::{App, image};
use std::{cell::Cell, rc::Rc};
#[test]
fn source_binding_follows_explicit_writes_and_aba_events_keep_their_original_revision() {
    let app = App::memory();
    let source = app.scope().cage("a.png".to_string());
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    app.mount(
        image()
            .source(source)
            .alt("Photo")
            .on(EventKind::ImageStatus, move |_| seen.set(seen.get() + 1)),
    )
    .unwrap();
    let id = app.tree().roots()[0];
    app.scope()
        .patch(
            id,
            Patch::Source {
                source: Change::Set("manual.png".into()),
                revision: 30,
            },
        )
        .unwrap();
    source.revise(|value| *value = "b.png".into());
    app.flush().unwrap();
    source.revise(|value| *value = "a.png".into());
    app.flush().unwrap();
    let tree = app.tree();
    let Props::Image(value) = tree.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(value.revision, 32);
    drop(tree);
    assert!(
        !app.dispatch(
            id,
            Event::ImageStatus {
                revision: 1,
                status: ImageStatus::Failed
            }
        )
        .unwrap()
    );
    assert_eq!(count.get(), 0);
    assert!(
        app.dispatch(
            id,
            Event::ImageStatus {
                revision: 33,
                status: ImageStatus::Loaded
            }
        )
        .is_err()
    );
    assert_eq!(count.get(), 0);
    assert!(
        app.dispatch(
            id,
            Event::ImageStatus {
                revision: 32,
                status: ImageStatus::Loaded
            }
        )
        .unwrap()
    );
    assert_eq!(count.get(), 1);
    app.scope().set_image_source(id, "a.png").unwrap();
    app.flush().unwrap();
    assert!(
        !app.dispatch(
            id,
            Event::ImageStatus {
                revision: 32,
                status: ImageStatus::Loaded
            }
        )
        .unwrap()
    );
    assert_eq!(source.get(), "a.png");
}
