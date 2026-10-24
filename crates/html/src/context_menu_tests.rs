use crate::*;
use glory_render::*;
#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}
#[test]
fn context_policy_and_point_default_resets_match_the_full_shape_oracle() {
    let mut region = Descriptor::new(Props::View);
    region.role = Some(Role::Group);
    region.focus = FocusMode::TabStop;
    region.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("context-area").unwrap()),
        context_menu: Some(ContextEditPolicy::Native),
        ..Default::default()
    }));
    let mut menu = Descriptor::new(Props::View);
    menu.role = Some(Role::Menu);
    menu.focus = FocusMode::Programmatic;
    menu.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("context-panel").unwrap()),
        navigation: Some(Orientation::Vertical),
        layer: Some(Layer {
            modal: false,
            anchor: Some(SemanticId::new("context-area").unwrap()),
            placement: LayerPlacement::ContextPoint(ContextPoint { x: 12.5, y: 71.25 }),
            ..Default::default()
        }),
        ..Default::default()
    }));
    for base in [region, menu] {
        let mut next = base.semantics.clone().unwrap();
        if let Some(layer) = next.layer.as_mut() {
            layer.placement = LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                glory_render::AnchorSide::Bottom,
                glory_render::AnchorAlign::End,
            ));
        } else {
            next.context_menu = Some(ContextEditPolicy::Override);
        }
        let mut cleared = base.semantics.clone().unwrap();
        cleared.context_menu = None;
        cleared.layer = None;
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Set(Some(next))),
            Patch::Semantics(Change::Set(Some(cleared))),
            Patch::Semantics(Change::Clear),
        ] {
            let next = base.patched(patch.clone()).unwrap();
            let expected = crate::difference(
                &shape(&base).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            let runtime = RuntimeId::new();
            let id = NodeIds::new(runtime).allocate();
            let mut mapper = Mapper::new(runtime);
            let mut sink = Record::default();
            mapper
                .apply(
                    &[
                        Mutation::Create {
                            id,
                            node: Box::new(base.clone()),
                        },
                        Mutation::Place {
                            id,
                            parent: None,
                            before: None,
                        },
                    ],
                    &mut sink,
                )
                .unwrap();
            sink.0.clear();
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..}if edits==&expected));
        }
        let shape = shape(&base).unwrap();
        let attrs = shape
            .pieces
            .iter()
            .find_map(|piece| match &piece.kind {
                Kind::Element { attributes, .. } => Some(attributes),
                _ => None,
            })
            .unwrap();
        if base.role == Some(Role::Menu) {
            assert_eq!(
                attrs.get("data-gly-context-x").map(String::as_str),
                Some("12.5")
            );
            assert_eq!(
                attrs.get("data-gly-context-y").map(String::as_str),
                Some("71.25")
            );
        } else {
            assert_eq!(
                attrs.get("data-gly-context").map(String::as_str),
                Some("native-editors")
            );
        }
        assert!(!attrs.contains_key("aria-haspopup") && !attrs.contains_key("aria-expanded"));
    }
}
