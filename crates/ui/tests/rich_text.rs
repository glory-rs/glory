use glory_render::rich_text::{Command, Document, Inline, Marks, Point, Selection};
use glory_render::{Change, Event, EventKind, Patch, Props};
use glory_ui::{App, rich_text};
fn doc(text: &str) -> Document {
    let mut d = Document::default();
    d.root.children[0].spans = vec![Inline::Text {
        text: text.into(),
        marks: Marks::default(),
    }];
    d
}
fn version(app: &App) -> u64 {
    let tree = app.tree();
    let Props::RichText(p) = &tree.node(tree.roots()[0]).unwrap().descriptor.props else {
        panic!()
    };
    p.revision
}
fn selection(offset: usize) -> Selection {
    let p = Point { block: 1, offset };
    Selection {
        anchor: p,
        focus: p,
    }
}
#[test]
fn stale_and_future_documents_cannot_reenter_business_handlers_and_invalid_patch_is_atomic() {
    let app = App::memory();
    let value = app.scope().cage(doc("initial"));
    let calls = app.scope().cage(0usize);
    app.mount(
        rich_text()
            .document(value)
            .on(EventKind::RichTextChanged, move |event| {
                if let Event::RichTextChanged { document, .. } = event {
                    value.revise(|v| *v = document);
                }
            })
            .on(EventKind::RichTextCommand, move |_| {
                calls.revise(|n| *n += 1)
            }),
    )
    .unwrap();
    let id = app.tree().roots()[0];
    app.scope()
        .patch(
            id,
            Patch::RichValue {
                document: Change::Set(Box::new(doc("explicit"))),
                revision: 10,
            },
        )
        .unwrap();
    app.flush().unwrap();
    value.revise(|d| *d = doc("A😀"));
    app.flush().unwrap();
    assert_eq!(version(&app), 11);
    let event = |revision| Event::RichTextCommand {
        document: doc("A😀"),
        revision,
        selection: selection(3),
        command: Command::Bold,
    };
    assert!(!app.dispatch(id, event(10)).unwrap());
    assert!(app.dispatch(id, event(12)).is_err());
    assert_eq!(calls.get(), 0);
    assert!(app.dispatch(id, event(11)).unwrap());
    assert_eq!(calls.get(), 1);
    assert!(
        app.dispatch(
            id,
            Event::RichTextChanged {
                document: doc("A😀"),
                revision: 11,
                selection: selection(2),
                composing: false
            }
        )
        .is_err()
    );
    assert_eq!(value.get(), doc("A😀"));
    assert!(
        app.scope()
            .patch(
                id,
                Patch::RichValue {
                    document: Change::Set(Box::new(doc("different"))),
                    revision: 11
                }
            )
            .is_err()
    );
    assert_eq!(version(&app), 11);
    assert!(
        app.dispatch(
            id,
            Event::RichTextChanged {
                document: doc("typed"),
                revision: 11,
                selection: selection(5),
                composing: false
            }
        )
        .unwrap()
    );
    assert_eq!(value.get(), doc("typed"));
    assert_eq!(version(&app), 12);
    app.clear().unwrap();
    assert!(!app.dispatch(id, event(12)).unwrap());
    assert_eq!(calls.get(), 1);
}
#[test]
fn rich_selection_and_typing_actions_reject_wrong_target_and_unsafe_links_before_queue() {
    use glory_render::Action;
    let app = App::memory();
    app.mount(rich_text().document(doc("A😀"))).unwrap();
    let id = app.tree().roots()[0];
    assert!(
        app.scope()
            .action(Action::RichSelection {
                node: id,
                selection: selection(2)
            })
            .is_err()
    );
    assert!(
        app.scope()
            .action(Action::RichMarks {
                node: id,
                marks: Box::new(Marks {
                    link: Some("javascript:alert(1)".into()),
                    ..Marks::default()
                })
            })
            .is_err()
    );
    app.scope()
        .patch(id, Patch::RichReadOnly(Change::Set(true)))
        .unwrap();
    app.flush().unwrap();
    assert!(
        app.scope()
            .action(Action::RichMarks {
                node: id,
                marks: Box::new(Marks::default())
            })
            .is_err()
    );
}
