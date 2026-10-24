use glory_render::*;
#[test]
fn separate_context_metadata_only_belongs_to_links_and_rolls_back_other_nodes() {
    for props in [
        Props::View,
        Props::Group,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
        Props::Check(CheckProps::default()),
    ] {
        let mut node = Descriptor::new(props);
        let before = node.clone();
        assert!(
            node.patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                link_open: Some(LinkOpen::NewContext),
                ..Default::default()
            })))))
            .is_err()
        );
        assert_eq!(node, before);
    }
    let mut link = Descriptor::new(Props::Link(LinkProps {
        target: "/guide".into(),
        label: "Guide".into(),
    }));
    for open in [LinkOpen::Current, LinkOpen::NewContext] {
        link.patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            link_open: Some(open),
            ..Default::default()
        })))))
        .unwrap();
    }
    assert!(
        matches!(&link.props,Props::Link(value) if value.target=="/guide"&&value.label=="Guide")
    );
}
