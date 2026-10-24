use super::*;
use glory_render::*;

#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}
fn descriptor(placement: LayerPlacement) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Dialog);
    d.focus = FocusMode::Programmatic;
    d.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("anchor-oracle-layer").unwrap()),
        layer: Some(Layer {
            modal: false,
            placement,
            anchor: matches!(placement, LayerPlacement::Anchor(_))
                .then(|| SemanticId::new("anchor-oracle-trigger").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    d
}
#[test]
fn every_anchor_field_and_reset_matches_the_full_shape_differential_oracle() {
    let mut placements = vec![LayerPlacement::Center];
    let viewport = ViewportPlacement::default();
    placements.extend(
        [
            viewport,
            ViewportPlacement {
                x: 173.5,
                ..viewport
            },
            ViewportPlacement {
                y: -23.25,
                ..viewport
            },
            ViewportPlacement {
                viewport_padding: 43.,
                ..viewport
            },
            ViewportPlacement {
                constrain: false,
                ..viewport
            },
        ]
        .map(LayerPlacement::Viewport),
    );
    for side in [
        AnchorSide::Top,
        AnchorSide::Bottom,
        AnchorSide::Left,
        AnchorSide::Right,
    ] {
        for align in [AnchorAlign::Start, AnchorAlign::Center, AnchorAlign::End] {
            let p = AnchorPlacement::new(side, align);
            placements.push(LayerPlacement::Anchor(p));
            placements.push(LayerPlacement::Anchor(AnchorPlacement { gap: 17., ..p }));
            placements.push(LayerPlacement::Anchor(AnchorPlacement {
                viewport_padding: 23.,
                ..p
            }));
            placements.push(LayerPlacement::Anchor(AnchorPlacement { flip: false, ..p }));
            placements.push(LayerPlacement::Anchor(AnchorPlacement {
                shift: false,
                ..p
            }));
        }
    }
    for base in placements.iter().copied().map(descriptor) {
        for next in placements.iter().copied().map(descriptor) {
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
            let expected = difference(
                &shape(&base).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            mapper
                .apply(
                    &[Mutation::Patch {
                        id,
                        patch: Patch::Semantics(Change::Set(next.semantics.clone())),
                    }],
                    &mut sink,
                )
                .unwrap();
            let Some(Op::Patch { edits, .. }) = sink.0.last() else {
                panic!()
            };
            assert_eq!(edits, &expected);
            assert_eq!(
                shape(&mapper.tree().node(id).unwrap().descriptor).unwrap(),
                shape(&next).unwrap()
            );
        }
    }
}
