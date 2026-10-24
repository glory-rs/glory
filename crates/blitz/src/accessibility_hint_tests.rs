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
