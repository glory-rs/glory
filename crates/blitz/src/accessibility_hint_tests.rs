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
