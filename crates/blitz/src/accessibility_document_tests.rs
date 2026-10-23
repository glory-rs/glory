use super::*;
use glory_render::{DocumentPart, TextKind};

#[test]
fn intrinsic_document_and_text_roles_reach_native_accesskit_without_group_overrides() {
    let app = WindowApp::new();
    app.mount(
        view().fill((
            view()
                .document_part(DocumentPart::Blockquote)
                .fill(text().content("Quote")),
            view().document_part(DocumentPart::OrderedList).fill(
                view()
                    .document_part(DocumentPart::ListItem)
                    .fill(text().content("Item")),
            ),
            view().document_part(DocumentPart::DescriptionList).fill((
                view()
                    .document_part(DocumentPart::Term)
                    .fill(text().content("Term")),
                view()
                    .document_part(DocumentPart::Definition)
                    .fill(text().content("Value")),
            )),
            text().kind(TextKind::Code).content("code"),
            text().kind(TextKind::Kbd).content("Control"),
            text().kind(TextKind::Mark).content("marked"),
        )),
    )
    .unwrap();
    let update = build(&app, true);
    let tree = Consumer::new(update, true);
    let roles = nodes(&tree)
        .iter()
        .map(|node| node.role())
        .collect::<Vec<_>>();
    for role in [
        AxRole::Blockquote,
        AxRole::List,
        AxRole::ListItem,
        AxRole::DescriptionList,
        AxRole::Term,
        AxRole::Definition,
        AxRole::Code,
        AxRole::Mark,
    ] {
        assert!(roles.contains(&role), "missing {role:?}");
    }
    assert!(
        nodes(&tree)
            .iter()
            .any(|node| node.role() == AxRole::Label && node.value().as_deref() == Some("Control"))
    );
}
