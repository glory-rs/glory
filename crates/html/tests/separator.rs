use glory_html::{Kind, Part};
use glory_render::*;
#[test]
fn separator_mapping_retains_passive_role_and_exact_orientation() {
    for axis in [Orientation::Horizontal, Orientation::Vertical] {
        let mut d = Descriptor::new(Props::View);
        d.role = Some(Role::Separator);
        d.semantics = Some(Box::new(Semantics {
            separator_axis: Some(axis),
            ..Default::default()
        }));
        let shape = glory_html::shape(&d).unwrap();
        let Kind::Element {
            tag, attributes, ..
        } = &shape
            .pieces
            .iter()
            .find(|piece| piece.part == Part::Main)
            .unwrap()
            .kind
        else {
            panic!("element")
        };
        assert_eq!(*tag, "div");
        assert_eq!(attributes["role"], "separator");
        assert_eq!(attributes["aria-orientation"], axis.as_str());
        assert!(!attributes.contains_key("tabindex"));
    }
}
