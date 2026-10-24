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
fn node(looping: bool, paused: bool) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.style = glory_render::style::Style::new().width(glory_render::style::Length::Px(80.));
    node.semantics = Some(Box::new(Semantics {
        inert: true,
        translation: Some(TranslationMotion {
            kind: if looping {
                TranslationKind::Loop {
                    from: TranslationOffset::new(-12., 3.),
                    to: TranslationOffset::new(24., -6.),
                    period_millis: 1500,
                }
            } else {
                TranslationKind::To {
                    target: TranslationOffset::new(17., -25.),
                    duration_millis: 500,
                }
            },
            paused,
        }),
        ..Default::default()
    }));
    node
}
#[test]
fn full_shape_and_every_field_patch_agree_for_translation_default_pause_clear_and_styles() {
    for base in [
        Descriptor::new(Props::View),
        node(false, false),
        node(true, false),
        node(true, true),
    ] {
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Clear),
            Patch::Semantics(Change::Set(node(false, false).semantics)),
            Patch::Semantics(Change::Set(node(true, true).semantics)),
            Patch::Style(Change::Clear),
            Patch::Style(Change::Set(Box::new(
                glory_render::style::Style::new().padding(glory_render::style::Edges::all(2.)),
            ))),
            Patch::Classes(Change::Set(vec!["current".into()])),
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
            assert!(
                matches!(&sink.0[0],Op::Patch{edits,..}if edits==&expected),
                "{:?}!={expected:?}",
                sink.0
            );
        }
    }
    let shape = shape(&node(true, true)).unwrap();
    let Kind::Element { attributes, .. } = &shape.pieces[0].kind else {
        panic!()
    };
    assert_eq!(attributes["data-gly-translation"], "loop");
    assert_eq!(attributes["data-gly-translation-duration"], "1500");
    assert!(attributes["style"].contains("transform:translate(-12px,3px);"));
    assert!(attributes.contains_key("inert"));
}
#[test]
fn invalid_translation_never_reaches_sink_and_retains_successful_prefix() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    mapper
        .apply(
            &[Mutation::Create {
                id,
                node: Box::new(node(false, false)),
            }],
            &mut sink,
        )
        .unwrap();
    sink.0.clear();
    let mut invalid = node(true, false);
    invalid.semantics.as_mut().unwrap().inert = false;
    let prefix = Mutation::Patch {
        id,
        patch: Patch::Classes(Change::Set(vec!["prefix".into()])),
    };
    assert!(
        mapper
            .apply(
                &[
                    prefix,
                    Mutation::Patch {
                        id,
                        patch: Patch::Semantics(Change::Set(invalid.semantics))
                    }
                ],
                &mut sink
            )
            .is_err()
    );
    assert_eq!(sink.0.len(), 1);
    assert_eq!(
        mapper
            .tree()
            .node(id)
            .unwrap()
            .descriptor
            .view()
            .classes
            .as_slice(),
        ["prefix"]
    );
    assert!(
        mapper
            .tree()
            .node(id)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .as_deref()
            .unwrap()
            .translation
            .is_some()
    );
}
