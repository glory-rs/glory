use glory_render::*;

#[test]
fn current_is_finite_location_metadata_with_atomic_target_validation_and_reset() {
    for value in [Current::Page, Current::Step, Current::Location] {
        for props in [
            Props::Link(LinkProps::default()),
            Props::Button(ButtonProps::default()),
            Props::Text(TextProps::default()),
        ] {
            let mut node = Descriptor::new(props);
            node.patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                current: Some(value),
                ..Default::default()
            })))))
            .unwrap();
            assert_eq!(node.semantics.as_ref().unwrap().current, Some(value));
            assert!(node.semantics.as_ref().unwrap().selected.is_none());
            node.patch(Patch::Semantics(Change::Clear)).unwrap();
            assert!(node.semantics.is_none());
        }
        for props in [
            Props::View,
            Props::Group,
            Props::Input(InputProps::default()),
            Props::Check(CheckProps::default()),
            Props::Image(ImageProps::default()),
        ] {
            let mut node = Descriptor::new(props);
            let before = node.clone();
            assert!(
                node.patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    current: Some(value),
                    ..Default::default()
                })))))
                .is_err()
            );
            assert_eq!(node, before);
        }
    }
}
