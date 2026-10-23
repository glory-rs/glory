use glory_html::{HtmlTextOptions, Kind, Part, TextTag};
use glory_render::*;
fn document(part: DocumentPart) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        document_part: Some(part),
        ..Default::default()
    }));
    d
}
fn tag(d: &Descriptor) -> &'static str {
    let shape = glory_html::shape(d).unwrap();
    let Kind::Element { tag, .. } = shape
        .pieces
        .iter()
        .find(|p| p.part == Part::Main)
        .unwrap()
        .kind
    else {
        panic!("expected main")
    };
    tag
}
#[test]
fn document_and_inline_kinds_keep_their_intrinsic_tags_and_cannot_be_replaced() {
    for (part, expected) in [
        (DocumentPart::Blockquote, "blockquote"),
        (DocumentPart::OrderedList, "ol"),
        (DocumentPart::UnorderedList, "ul"),
        (DocumentPart::ListItem, "li"),
        (DocumentPart::DescriptionList, "dl"),
        (DocumentPart::Term, "dt"),
        (DocumentPart::Definition, "dd"),
    ] {
        let mut d = document(part);
        d.name = Some("Named content".into());
        assert_eq!(tag(&d), expected);
        let shape = glory_html::shape(&d).unwrap();
        let Kind::Element { attributes, .. } = &shape.pieces[0].kind else {
            unreachable!()
        };
        assert!(!attributes.contains_key("role"));
        assert!(glory_html::preflight(&d, Some(&Patch::Semantics(Change::Clear))).is_err());
    }
    for (kind, expected) in [
        (TextKind::Code, "code"),
        (TextKind::Kbd, "kbd"),
        (TextKind::Mark, "mark"),
    ] {
        let mut d = Descriptor::new(Props::Text(TextProps {
            kind,
            content: "<&>".into(),
            ..Default::default()
        }));
        assert_eq!(tag(&d), expected);
        assert!(glory_html::preflight(&d, Some(&Patch::TextKind(Change::Clear))).is_err());
        d.options
            .insert(HtmlTextOptions::default().tag(TextTag::Span));
        assert!(glory_html::shape(&d).is_err());
    }
}
