use glory_html::{Kind, Part, shape};
use glory_render::*;
#[test]
fn navigation_disclosure_and_group_popup_keep_actual_roles_without_menu_claims() {
    let mut content = Descriptor::new(Props::View);
    content.role = Some(Role::Group);
    content.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("content").unwrap()),
        navigation: Some(Orientation::Vertical),
        navigation_panel: Some(NavigationPanel::Content),
        layer: Some(Layer {
            modal: false,
            focus: LayerFocus::Preserve,
            placement: LayerPlacement::AnchorStart,
            anchor: Some(SemanticId::new("trigger").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    let mut trigger = Descriptor::new(Props::Button(ButtonProps {
        label: "Products".into(),
        ..Default::default()
    }));
    trigger.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("trigger").unwrap()),
        controls: Some(SemanticId::new("content").unwrap()),
        expanded: Some(true),
        navigation_item: true,
        navigation_panel: Some(NavigationPanel::Trigger),
        ..Default::default()
    }));
    for (descriptor, role, marker) in [
        (content, Some("group"), "content"),
        (trigger, None, "trigger"),
    ] {
        let shaped = shape(&descriptor).unwrap();
        let Kind::Element {
            tag, attributes, ..
        } = &shaped
            .pieces
            .iter()
            .find(|piece| piece.part == Part::Main)
            .unwrap()
            .kind
        else {
            panic!("physical element")
        };
        assert_eq!(
            attributes
                .get("data-gly-navigation-panel")
                .map(String::as_str),
            Some(marker)
        );
        assert_eq!(attributes.get("role").map(String::as_str), role);
        assert!(!attributes.contains_key("aria-haspopup"));
        assert_eq!(*tag, if marker == "content" { "div" } else { "button" });
        if marker == "trigger" {
            assert_eq!(
                attributes.get("aria-expanded").map(String::as_str),
                Some("true")
            );
            assert_eq!(
                attributes.get("aria-controls").map(String::as_str),
                Some("content")
            );
        }
    }
}
