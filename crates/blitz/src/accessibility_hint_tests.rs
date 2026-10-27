use super::*;
use glory_render::{SemanticId, Semantics};

#[test]
fn passive_tooltip_model_exports_real_role_description_and_no_focus_action() {
    let host = WindowApp::new();
    let tip = SemanticId::new("native-tooltip-model").unwrap();
    host.mount(
        view().fill((
            button()
                .label("Save draft")
                .semantics(Some(Box::new(Semantics {
                    described_by: Some(tip.clone()),
                    ..Default::default()
                }))),
            view()
                .role(Role::Tooltip)
                .semantics(Some(Box::new(Semantics {
                    id: Some(tip),
                    ..Default::default()
                })))
                .fill(text().content("Drafts remain private.")),
        )),
    )
    .unwrap();
    let tree = Consumer::new(build(&host, true), true);
    let all = nodes(&tree);
    let tooltip = all
        .iter()
        .find(|node| node.role() == AxRole::Tooltip)
        .unwrap();
    assert!(!tooltip.data().supports_action(AxAction::Focus));
    let button = all
        .iter()
        .find(|node| node.role() == AxRole::Button && node.label().as_deref() == Some("Save draft"))
        .unwrap();
    assert_eq!(
        button.description().as_deref(),
        Some("Drafts remain private.")
    );
    assert_eq!(button.data().has_popup(), None);
}

#[test]
fn input_hints_are_separate_from_labels_values_and_password_protection_and_clear_incrementally() {
    use glory_render::{Change, Patch, Purpose};
    for purpose in [Purpose::Text, Purpose::Password] {
        let host = WindowApp::new();
        host.mount(
            input()
                .purpose(purpose)
                .name("Real label")
                .value("Secret中")
                .semantics(Some(Box::new(Semantics {
                    placeholder: Some("Helpful hint".into()),
                    ..Default::default()
                }))),
        )
        .unwrap();
        let logical = host.app.tree().roots()[0];
        let update = build(&host, true);
        let target = id(&host, logical);
        let node = &update.nodes.iter().find(|(id, _)| *id == target).unwrap().1;
        assert_eq!(node.label(), Some("Real label"));
        assert_eq!(node.placeholder(), Some("Helpful hint"));
        assert_eq!(
            node.value(),
            if purpose == Purpose::Password {
                None
            } else {
                Some("Secret中")
            }
        );
        host.app
            .scope()
            .patch(logical, Patch::Semantics(Change::Clear))
            .unwrap();
        let update = build(&host, false);
        let node = &update.nodes.iter().find(|(id, _)| *id == target).unwrap().1;
        assert_eq!(node.placeholder(), None);
        assert_eq!(node.label(), Some("Real label"));
    }
}

#[test]
fn choices_export_valid_active_options_multiselectable_state_and_retire_relations_without_moving_editor_focus()
 {
    use crate::document::shell::choice_tests::fixture;
    use glory_render::{Change, ComboboxKind, Patch};
    for kind in [
        ComboboxKind::Select,
        ComboboxKind::Editable,
        ComboboxKind::Autocomplete,
        ComboboxKind::Multiple,
    ] {
        let f = fixture(kind);
        f.open.revise(|v| *v = true);
        f.host.app.flush().unwrap();
        let update = build(&f.host, true);
        let trigger = id(&f.host, f.trigger);
        let panel = id(&f.host, f.panel);
        let option = id(&f.host, f.options[0]);
        let node = &update
            .nodes
            .iter()
            .find(|(id, _)| *id == trigger)
            .unwrap()
            .1;
        assert_eq!(
            node.role(),
            if kind == ComboboxKind::Select {
                AxRole::ComboBox
            } else {
                AxRole::EditableComboBox
            }
        );
        assert_eq!(node.active_descendant(), Some(option));
        assert_eq!(node.controls(), [panel]);
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == panel)
                .unwrap()
                .1
                .is_multiselectable(),
            kind == ComboboxKind::Multiple
        );
        // Semantic names may remain, but disabled or physically hidden options cannot be active.
        f.current.revise(|v| *v = 1);
        let update = build(&f.host, false);
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == trigger)
                .unwrap()
                .1
                .active_descendant(),
            None
        );
        f.current.revise(|v| *v = 2);
        let update = build(&f.host, false);
        let last = id(&f.host, f.options[2]);
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == trigger)
                .unwrap()
                .1
                .active_descendant(),
            Some(last)
        );
        f.host
            .app
            .scope()
            .patch(
                f.options[2],
                Patch::Style(Change::Set(Box::new(
                    glory_render::style::Style::new().display(glory_render::style::Display::None),
                ))),
            )
            .unwrap();
        let update = build(&f.host, false);
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == trigger)
                .unwrap()
                .1
                .active_descendant(),
            None
        );
        f.open.revise(|v| *v = false);
        let update = build(&f.host, false);
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == trigger)
                .unwrap()
                .1
                .active_descendant(),
            None
        );
        assert!(!action(&f.host, &request(last, AxAction::Click, None)).unwrap());
        f.host.app.clear().unwrap();
        build(&f.host, false);
        assert!(!f.host.accessibility.borrow().reverse.contains_key(&last));
    }
}

#[test]
fn password_combobox_refinement_keeps_protected_role_and_never_caches_raw_secret() {
    use crate::document::shell::choice_tests::fixture;
    use glory_render::{Change, ComboboxKind, Patch, Purpose};
    let f = fixture(ComboboxKind::Editable);
    f.draft.revise(|v| *v = "private choice secret".into());
    f.host
        .app
        .scope()
        .patch(f.trigger, Patch::Purpose(Change::Set(Purpose::Password)))
        .unwrap();
    f.open.revise(|v| *v = true);
    let update = build(&f.host, true);
    let trigger = id(&f.host, f.trigger);
    let node = &update
        .nodes
        .iter()
        .find(|(id, _)| *id == trigger)
        .unwrap()
        .1;
    assert_eq!(node.role(), AxRole::PasswordInput);
    assert_eq!(node.value(), None);
    assert!(node.active_descendant().is_some());
    assert!(f.host.accessibility.borrow().previous.values().all(|n| {
        n.value()
            .is_none_or(|s| !s.contains("private choice secret"))
    }));
}
