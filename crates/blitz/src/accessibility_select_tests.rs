use super::*;
use crate::document::shell::native_select_tests::{fixture, name};

#[test]
fn native_select_accesskit_groups_selected_labels_actions_and_late_option_retirement() {
    let f = fixture(false);
    let update = build(&f.host, true);
    let main = id(&f.host, f.id);
    let n = &update.nodes.iter().find(|(i, _)| *i == main).unwrap().1;
    assert_eq!(n.role(), AxRole::ComboBox);
    assert_eq!(n.is_expanded(), Some(false));
    assert_eq!(n.label(), Some("Choose a value"));
    assert_eq!(n.placeholder(), Some("请选择"));
    assert!(action(&f.host, &request(main, AxAction::Expand, None)).unwrap());
    let update = build(&f.host, false);
    let mut consumer = Consumer::new(build(&f.host, true), true);
    let all = nodes(&consumer);
    let beta = f.host.accessibility.borrow().ids[&Key::SelectOption(f.id, 2)];
    let blocked = f.host.accessibility.borrow().ids[&Key::SelectOption(f.id, 4)];
    assert!(
        all.iter()
            .any(|n| n.role() == AxRole::Group && n.label().as_deref() == Some("Group"))
    );
    let cursor = f
        .host
        .renderer
        .0
        .borrow()
        .dom
        .range(f.id)
        .unwrap()
        .select
        .as_ref()
        .unwrap()
        .cursor;
    assert!(action(&f.host, &request(beta, AxAction::ScrollIntoView, None)).unwrap());
    assert_eq!(
        f.host
            .renderer
            .0
            .borrow()
            .dom
            .range(f.id)
            .unwrap()
            .select
            .as_ref()
            .unwrap()
            .cursor,
        cursor
    );
    assert!(!action(&f.host, &request(blocked, AxAction::Click, None)).unwrap());
    assert!(action(&f.host, &request(beta, AxAction::Click, None)).unwrap());
    drop(all);
    consumer.update_and_process_changes(build(&f.host, false), &mut Changes);
    assert_eq!(f.selected.get(), vec![name("beta")]);
    let current = nodes(&consumer)
        .into_iter()
        .find(|n| n.label().as_deref() == Some("Choose a value"))
        .unwrap();
    assert_eq!(current.value().as_deref(), Some("Beta"));
    assert_eq!(current.data().is_expanded(), Some(false));
    assert!(!action(&f.host, &request(beta, AxAction::Click, None)).unwrap());
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.value().is_none_or(|s| !s.contains("private-")))
    );
    f.host.app.clear().unwrap();
    build(&f.host, false);
    assert!(!action(&f.host, &request(main, AxAction::Expand, None)).unwrap());
}
#[test]
fn native_select_accesskit_inline_multiple_exposes_selected_option_state() {
    let f = fixture(true);
    f.selected
        .revise(|v| *v = vec![name("alpha"), name("group-disabled")]);
    let tree = Consumer::new(build(&f.host, true), true);
    let all = nodes(&tree);
    let list = all
        .iter()
        .find(|n| n.label().as_deref() == Some("Choose a value"))
        .unwrap();
    assert_eq!(list.role(), AxRole::ListBox);
    assert!(list.data().is_multiselectable());
    let alpha = all
        .iter()
        .find(|n| n.role() == AxRole::ListBoxOption && n.label().as_deref() == Some("Alpha"))
        .unwrap();
    assert_eq!(alpha.data().is_selected(), Some(true));
    let alpha_id = f.host.accessibility.borrow().ids[&Key::SelectOption(f.id, 0)];
    assert!(action(&f.host, &request(alpha_id, AxAction::Click, None)).unwrap());
    build(&f.host, false);
    assert_eq!(f.selected.get(), vec![name("group-disabled")]);
}
