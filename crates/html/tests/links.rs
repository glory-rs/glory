use glory_html::{Kind, Part, shape};
use glory_render::*;
#[test]
fn separate_context_has_bounded_native_anchor_target_and_opener_protection() {
    for open in [None, Some(LinkOpen::Current), Some(LinkOpen::NewContext)] {
        let mut link = Descriptor::new(Props::Link(LinkProps {
            target: "/guide?x=1&y=2".into(),
            label: "Guide".into(),
        }));
        link.semantics = Some(Box::new(Semantics {
            link_open: open,
            ..Default::default()
        }));
        let mapped = shape(&link).unwrap();
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
        assert_eq!(*tag, "a");
        assert_eq!(attributes["href"], "/guide?x=1&y=2");
        assert_eq!(
            attributes.get("target").map(String::as_str),
            (open == Some(LinkOpen::NewContext)).then_some("_blank")
        );
        assert_eq!(
            attributes.get("rel").map(String::as_str),
            (open == Some(LinkOpen::NewContext)).then_some("noopener noreferrer")
        );
    }
}
