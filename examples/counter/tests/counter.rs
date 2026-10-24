use counter::Counter;
use glory::prelude::*;
use glory::render::{NodeId, Props};

fn find(app: &App, predicate: impl Fn(&Props) -> bool) -> NodeId {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).unwrap();
        if predicate(node.descriptor.view().props) {
            return id;
        }
        pending.extend(node.children.iter().copied());
    }
    panic!("counter node missing")
}

fn click(app: &App, label: &str) {
    let id = find(
        app,
        |props| matches!(props, Props::Button(p) if p.label == label),
    );
    assert!(app.dispatch(id, Event::Activate).unwrap());
}

fn edit(app: &App, value: &str) {
    let (id, revision) = input_state(app);
    assert!(
        app.dispatch(
            id,
            Event::ValueChanged {
                value: value.into(),
                composing: false,
                revision,
            }
        )
        .unwrap()
    );
}

fn input_state(app: &App) -> (NodeId, u64) {
    let id = find(app, |props| matches!(props, Props::Input(_)));
    let revision = match app.tree().node(id).unwrap().descriptor.view().props {
        Props::Input(input) => input.revision,
        _ => unreachable!(),
    };
    (id, revision)
}

fn assert_value(app: &App, value: i32) {
    find(
        app,
        |props| matches!(props, Props::Text(p) if p.content == format!("Value: {value}")),
    );
    find(
        app,
        |props| matches!(props, Props::Input(p) if p.value == value.to_string()),
    );
}

#[test]
fn shared_component_handles_buttons_signed_edits_and_invalid_values() {
    let app = App::memory();
    app.mount(Counter).unwrap();
    assert_value(&app, 0);
    click(&app, "+1");
    assert_value(&app, 1);
    click(&app, "-1");
    click(&app, "-1");
    assert_value(&app, -1);
    edit(&app, "42");
    assert_value(&app, 42);
    edit(&app, "-8");
    assert_value(&app, -8);
    click(&app, "Clear");
    assert_value(&app, 0);
    for value in ["", "invalid", "2147483648"] {
        edit(&app, value);
        find(
            &app,
            |props| matches!(props, Props::Text(p) if p.content == "Value: 0"),
        );
        find(
            &app,
            |props| matches!(props, Props::Input(p) if p.value == value),
        );
        click(&app, "Clear");
        assert_value(&app, 0);
    }
    edit(&app, "2147483647");
    click(&app, "+1");
    assert_value(&app, i32::MAX);
    edit(&app, "-2147483648");
    click(&app, "-1");
    assert_value(&app, i32::MIN);
}

#[test]
fn mounts_own_independent_state_and_remount_starts_fresh() {
    let first = App::memory();
    let second = App::memory();
    first.mount(Counter).unwrap();
    second.mount(Counter).unwrap();
    click(&first, "+1");
    assert_value(&first, 1);
    assert_value(&second, 0);
    first.clear().unwrap();
    assert_eq!(first.stats().nodes, 0);
    assert_eq!(first.runtime_stats().nodes, 0);
    first.mount(Counter).unwrap();
    assert_value(&first, 0);
}

#[test]
fn same_tick_edit_and_commit_or_clear_acknowledge_the_final_value() {
    let app = App::memory();
    app.mount(Counter).unwrap();
    for clear in [false, true] {
        let (id, revision) = input_state(&app);
        app.scope()
            .batch(|| {
                edit(&app, "2147483648");
                if clear {
                    click(&app, "Clear");
                } else {
                    assert!(
                        app.dispatch(
                            id,
                            Event::Commit {
                                value: "2147483648".into(),
                                revision,
                            }
                        )
                        .unwrap()
                    );
                }
            })
            .unwrap();
        app.flush().unwrap();
        assert_value(&app, 0);
        // An unchanged numeric result must still send a controlled input write.
        assert_eq!(input_state(&app), (id, revision + 1));
        assert!(
            !app.dispatch(
                id,
                Event::ValueChanged {
                    value: "99".into(),
                    composing: false,
                    revision,
                }
            )
            .unwrap()
        );
        assert_value(&app, 0);
    }
    edit(&app, "0007");
    let (id, revision) = input_state(&app);
    app.dispatch(
        id,
        Event::Commit {
            value: "0007".into(),
            revision,
        },
    )
    .unwrap();
    assert_value(&app, 7);
}
