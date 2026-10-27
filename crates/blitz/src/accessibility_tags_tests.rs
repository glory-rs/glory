use super::*;
use crate::document::shell::focus_tests::settle;
use glory_render::{Semantics, TagSeparators};

#[test]
fn native_tags_accesskit_keeps_textbox_value_actions_and_retires_without_popup_roles() {
    let mut host = WindowApp::new();
    let scope = host.app.scope();
    let value = scope.cage("中文 draft".to_owned());
    let reference = scope.cage(None);
    host.mount(
        input()
            .name("Free tags")
            .value(value)
            .node_ref(reference)
            .semantics(Some(Box::new(Semantics {
                tags_input: Some(TagSeparators::default()),
                ..Default::default()
            })))
            .on(EventKind::Tags, |_| {})
            .on(EventKind::ValueChanged, move |event| {
                if let Event::ValueChanged { value: next, .. } = event {
                    value.revise(|v| *v = next);
                }
            }),
    )
    .unwrap();
    let input = reference.get().unwrap();
    let update = build(&host, true);
    let target = id(&host, input);
    let mut consumer = Consumer::new(update, true);
    let exported = nodes(&consumer);
    let node = exported
        .iter()
        .find(|n| n.label().as_deref() == Some("Free tags"))
        .unwrap();
    assert_eq!(node.role(), AxRole::TextInput);
    assert_eq!(node.value().as_deref(), Some("中文 draft"));
    assert!(exported.iter().all(|n| !matches!(
        n.role(),
        AxRole::ComboBox | AxRole::EditableComboBox | AxRole::ListBox
    )));
    drop(exported);
    assert!(
        action(
            &host,
            &request(
                target,
                AxAction::SetValue,
                Some(ActionData::Value("AT draft,still text".into()))
            )
        )
        .unwrap()
    );
    settle(&mut host);
    assert_eq!(value.get(), "AT draft,still text");
    consumer.update_and_process_changes(build(&host, false), &mut Changes);
    assert!(
        nodes(&consumer)
            .iter()
            .any(|n| n.label().as_deref() == Some("Free tags")
                && n.value().as_deref() == Some("AT draft,still text"))
    );
    host.app.clear().unwrap();
    build(&host, false);
    assert!(
        !action(
            &host,
            &request(
                target,
                AxAction::SetValue,
                Some(ActionData::Value("late".into()))
            )
        )
        .unwrap()
    );
}
