use super::*;
#[test]
fn alert_decision_model_exports_distinct_role_name_description_and_no_implicit_live_announcement() {
    let host = WindowApp::new();
    let title = glory_render::SemanticId::new("decision-model-title").unwrap();
    let description = glory_render::SemanticId::new("decision-model-description").unwrap();
    host.mount(
        view()
            .role(Role::AlertDialog)
            .focus(glory_render::FocusMode::Programmatic)
            .semantics(Some(Box::new(glory_render::Semantics {
                labelled_by: Some(title.clone()),
                described_by: Some(description.clone()),
                ..Default::default()
            })))
            .fill((
                text().content("Remove draft?").semantics(Some(Box::new(
                    glory_render::Semantics {
                        id: Some(title),
                        ..Default::default()
                    },
                ))),
                text()
                    .content("This removes the local draft.")
                    .semantics(Some(Box::new(glory_render::Semantics {
                        id: Some(description),
                        ..Default::default()
                    }))),
                button().label("Keep draft"),
            )),
    )
    .unwrap();
    let tree = Consumer::new(build(&host, true), true);
    let all = nodes(&tree);
    let decision = all
        .iter()
        .find(|node| node.role() == AxRole::AlertDialog)
        .unwrap();
    assert_eq!(decision.label().as_deref(), Some("Remove draft?"));
    assert_eq!(
        decision.description().as_deref(),
        Some("This removes the local draft.")
    );
    assert_eq!(decision.data().live(), None);
}
