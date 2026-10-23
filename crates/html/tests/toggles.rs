use glory_html::{Kind, Part, shape};
use glory_render::*;
#[test]
fn pressed_preserves_button_tag_label_and_default_false_distinguishes_plain_buttons() {
    for pressed in [None, Some(false), Some(true)] {
        let mut descriptor = Descriptor::new(Props::Button(ButtonProps {
            label: "Bold".into(),
            ..Default::default()
        }));
        descriptor.semantics = Some(Box::new(Semantics {
            pressed,
            ..Default::default()
        }));
        let mapped = shape(&descriptor).unwrap();
        let Kind::Element {
            tag, attributes, ..
        } = &mapped
            .pieces
            .iter()
            .find(|piece| piece.part == Part::Main)
            .unwrap()
            .kind
        else {
            panic!()
        };
        assert_eq!(*tag, "button");
        assert_eq!(
            attributes.get("aria-pressed"),
            pressed.map(|p| p.to_string()).as_ref()
        );
        assert_eq!(
            attributes.contains_key("data-gly-pressed"),
            pressed == Some(true)
        );
        assert!(!attributes.contains_key("aria-selected"));
    }
}
