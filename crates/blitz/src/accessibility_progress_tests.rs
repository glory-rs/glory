use super::*;
use glory_render::{ProgressValue, Semantics};
#[test]
fn real_accesskit_progress_is_read_only_numeric_or_indeterminate_with_text_alternative() {
    let app = WindowApp::new();
    let value = app.app.scope().cage(ProgressValue {
        min: 0.,
        max: 100.,
        current: Some(25.),
        value_text: Some("Quarter uploaded".into()),
    });
    app.mount(
        view()
            .role(Role::ProgressBar)
            .name("Upload")
            .semantics(value.map(|value| {
                Some(Box::new(Semantics {
                    progress: Some(value.clone()),
                    ..Default::default()
                }))
            })),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let update = build(&app, true);
    let node = &update
        .nodes
        .iter()
        .find(|(key, _)| *key == id(&app, root))
        .unwrap()
        .1;
    assert_eq!(node.role(), AxRole::ProgressIndicator);
    assert_eq!(node.numeric_value(), Some(25.));
    assert_eq!(node.min_numeric_value(), Some(0.));
    assert_eq!(node.max_numeric_value(), Some(100.));
    assert_eq!(node.value(), Some("Quarter uploaded"));
    assert!(node.is_read_only());
    for action in [
        AxAction::Increment,
        AxAction::Decrement,
        AxAction::SetValue,
        AxAction::Click,
        AxAction::Focus,
    ] {
        assert!(!node.supports_action(action));
    }
    value.revise(|value| {
        value.current = None;
        value.value_text = Some("Waiting for server".into());
    });
    let update = build(&app, false);
    let node = &update
        .nodes
        .iter()
        .find(|(key, _)| *key == id(&app, root))
        .unwrap()
        .1;
    assert_eq!(node.numeric_value(), None);
    assert_eq!(node.value(), Some("Waiting for server"));
    assert_eq!(node.role(), AxRole::ProgressIndicator);
}
