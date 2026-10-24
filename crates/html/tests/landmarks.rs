use glory_html::{Kind, Part};
use glory_render::*;
#[test]
fn shell_landmarks_use_native_tags_and_toolbar_keeps_its_orientation() {
    for (role, tag, name) in [
        (Role::Banner, "header", "banner"),
        (Role::ContentInfo, "footer", "contentinfo"),
        (Role::Toolbar, "div", "toolbar"),
    ] {
        let mut d = Descriptor::new(Props::View);
        d.role = Some(role);
        d.name = Some("Application".into());
        if role == Role::Toolbar {
            d.semantics = Some(Box::new(Semantics {
                navigation: Some(Orientation::Vertical),
                ..Default::default()
            }));
        }
        let shape = glory_html::shape(&d).unwrap();
        let Kind::Element {
            tag: actual,
            attributes,
            ..
        } = &shape
            .pieces
            .iter()
            .find(|p| p.part == Part::Main)
            .unwrap()
            .kind
        else {
            panic!("element")
        };
        assert_eq!(*actual, tag);
        assert_eq!(attributes["role"], name);
        if role == Role::Toolbar {
            assert_eq!(attributes["aria-orientation"], "vertical");
        }
    }
}
