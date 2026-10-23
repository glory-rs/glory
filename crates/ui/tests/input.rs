use glory_render::{Change, Error, Event, EventKind, Patch, Props};
use glory_ui::{App, input};

fn state(app: &App) -> (String, u64) {
    let tree = app.tree();
    let Props::Input(input) = &tree.node(tree.roots()[0]).unwrap().descriptor.props else {
        panic!("input")
    };
    (input.value.clone(), input.revision)
}

#[test]
fn bound_writes_follow_explicit_versions_and_future_events_cannot_replace_business_state() {
    let app = App::memory();
    let value = app.scope().cage(String::from("initial"));
    let commits = app.scope().cage(0usize);
    app.mount(
        input()
            .value(value)
            .on(EventKind::ValueChanged, move |event| {
                if let Event::ValueChanged { value: next, .. } = event {
                    value.revise(|v| *v = next);
                }
            })
            .on(EventKind::Commit, move |event| {
                if let Event::Commit { value: next, .. } = event {
                    commits.revise(|n| *n += 1);
                    value.revise(|v| *v = next);
                }
            }),
    )
    .unwrap();
    let id = app.tree().roots()[0];
    app.scope()
        .patch(
            id,
            Patch::Value {
                value: Change::Set("manual".into()),
                revision: 100,
            },
        )
        .unwrap();
    app.flush().unwrap();
    value.revise(|v| *v = "auto".into());
    app.flush().unwrap();
    assert_eq!(state(&app), ("auto".into(), 101));
    let event = |revision, text: &str| Event::ValueChanged {
        value: text.into(),
        composing: false,
        revision,
    };
    assert_eq!(
        app.dispatch(id, event(102, "future")),
        Err(glory_ui::Error::Render(Error::InputRevision {
            current: 101,
            incoming: 102
        }))
    );
    assert!(!app.dispatch(id, event(100, "old")).unwrap());
    assert_eq!(value.get(), "auto");
    assert!(app.dispatch(id, event(101, "typed🙂")).unwrap());
    assert_eq!(state(&app), ("typed🙂".into(), 102));
    assert!(
        !app.dispatch(
            id,
            Event::Commit {
                value: "old".into(),
                revision: 101
            }
        )
        .unwrap()
    );
    assert_eq!(
        app.dispatch(
            id,
            Event::Commit {
                value: "future".into(),
                revision: 103
            }
        ),
        Err(glory_ui::Error::Render(Error::InputRevision {
            current: 102,
            incoming: 103
        }))
    );
    assert_eq!(commits.get(), 0);
    assert_eq!(
        app.scope().patch(
            id,
            Patch::Value {
                value: Change::Set("conflict".into()),
                revision: 102
            }
        ),
        Err(glory_ui::Error::Render(Error::InputRevision {
            current: 102,
            incoming: 102
        }))
    );
    assert_eq!(state(&app), ("typed🙂".into(), 102));
    assert!(
        app.dispatch(
            id,
            Event::Commit {
                value: "submitted".into(),
                revision: 102
            }
        )
        .unwrap()
    );
    assert_eq!(state(&app), ("submitted".into(), 103));
    assert_eq!(commits.get(), 1);
}

#[test]
fn canonical_model_does_not_rewrite_raw_business_values_and_equivalent_echoes_advance_only_versions()
 {
    let app = App::memory();
    let raw = "\r\ninitial\r\0🙂";
    let value = app.scope().cage(raw.to_owned());
    app.mount(input().value(value).multiline(true)).unwrap();
    assert_eq!(value.get(), raw);
    assert_eq!(state(&app), ("\ninitial\n�🙂".into(), 1));
    value.revise(|v| *v = "\ninitial\n�🙂".into());
    app.flush().unwrap();
    assert_eq!(state(&app), ("\ninitial\n�🙂".into(), 2));
}

#[test]
fn exhausted_auto_versions_report_an_error_without_wrapping_and_remount_restores_progress() {
    let app = App::memory();
    let value = app.scope().cage(String::from("initial"));
    app.mount(input().value(value)).unwrap();
    let old = app.tree().roots()[0];
    app.scope()
        .patch(
            old,
            Patch::Value {
                value: Change::Set("last".into()),
                revision: u64::MAX,
            },
        )
        .unwrap();
    app.flush().unwrap();
    value.revise(|v| *v = "next".into());
    assert_eq!(
        app.flush(),
        Err(glory_ui::Error::Render(Error::InputRevisionExhausted))
    );
    assert_eq!(state(&app), ("last".into(), u64::MAX));
    app.clear().unwrap();
    app.mount(input().value(value)).unwrap();
    assert_eq!(state(&app), ("next".into(), 1));
    assert_ne!(app.tree().roots()[0], old);
}
