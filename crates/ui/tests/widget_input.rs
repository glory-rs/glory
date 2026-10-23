use glory_render::{Change, Error as RenderError, Patch, Props};
use glory_ui::{App, Error, Widget, input};

#[test]
fn widget_value_writes_allocate_from_live_revisions_and_validate_before_mutation() {
    let app = App::memory();
    app.mount(input().value("initial")).unwrap();
    let node = app.tree().roots()[0];
    app.scope()
        .patch(
            node,
            Patch::Value {
                value: Change::Set("explicit".into()),
                revision: 50,
            },
        )
        .unwrap();
    app.scope().set_input_value(node, "next").unwrap();
    let Props::Input(value) = app.tree().node(node).unwrap().descriptor.props.clone() else {
        panic!()
    };
    assert_eq!((&*value.value, value.revision), ("next", 51));
    assert!(app.scope().set_input_value(node, "bad\nline").is_err());
    let Props::Input(value) = app.tree().node(node).unwrap().descriptor.props.clone() else {
        panic!()
    };
    assert_eq!((&*value.value, value.revision), ("next", 51));
    app.scope()
        .patch(
            node,
            Patch::Value {
                value: Change::Set("last".into()),
                revision: u64::MAX,
            },
        )
        .unwrap();
    assert_eq!(
        app.scope().set_input_value(node, "exhausted"),
        Err(Error::Render(RenderError::InputRevisionExhausted))
    );
}

#[test]
fn widget_errors_surface_at_flush_and_retirement_ignores_late_reports() {
    let app = App::memory();
    let mounted = input().mount(app.scope()).unwrap();
    app.flush().unwrap();
    let scope = mounted.scope().clone();
    scope.report_error(RenderError::Unsupported("custom widget"));
    assert_eq!(
        app.flush(),
        Err(Error::Render(RenderError::Unsupported("custom widget")))
    );
    let node = mounted.roots()[0];
    drop(mounted);
    assert!(scope.set_input_value(node, "late").is_err());
    scope.report_error(RenderError::Unsupported("late"));
    app.flush().unwrap();
}
