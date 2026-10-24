use glory_html::{Kind, Part, shape};
use glory_render::*;
#[test]
fn menu_item_roles_refine_actual_buttons_and_checks_and_detach_successful_form_ownership() {
    for (kind, props) in [
        (MenuItemKind::Action, Props::Button(ButtonProps::default())),
        (MenuItemKind::Checkbox, Props::Check(CheckProps::default())),
        (
            MenuItemKind::Radio,
            Props::Check(CheckProps {
                mode: CheckMode::Radio,
                ..Default::default()
            }),
        ),
    ] {
        let mut descriptor = Descriptor::new(props);
        descriptor.semantics = Some(Box::new(Semantics {
            menu_item: Some(MenuItemSemantics {
                kind,
                owner: SemanticId::new("menu-owner").unwrap(),
            }),
            check_group: (kind == MenuItemKind::Radio)
                .then(|| SemanticId::new("menu-radios").unwrap()),
            ..Default::default()
        }));
        let shape = shape(&descriptor).unwrap();
        let part = if kind == MenuItemKind::Action {
            Part::Main
        } else {
            Part::Control
        };
        let Kind::Element {
            tag, attributes, ..
        } = &shape
            .pieces
            .iter()
            .find(|piece| piece.part == part)
            .unwrap()
            .kind
        else {
            panic!()
        };
        assert_eq!(
            *tag,
            if kind == MenuItemKind::Action {
                "button"
            } else {
                "input"
            }
        );
        assert_eq!(attributes["form"], "menu-owner");
        assert_eq!(
            attributes["role"],
            match kind {
                MenuItemKind::Action => "menuitem",
                MenuItemKind::Checkbox => "menuitemcheckbox",
                MenuItemKind::Radio => "menuitemradio",
            }
        );
        if kind == MenuItemKind::Radio {
            assert_eq!(attributes["type"], "radio");
            assert_eq!(attributes["name"], "menu-radios");
        }
    }
}
