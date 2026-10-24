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
fn node(text: &str, revision: u64) -> Descriptor {
    let mut d = Descriptor::new(Props::Button(ButtonProps {
        label: "Copy".into(),
        ..Default::default()
    }));
    d.semantics = Some(Box::new(Semantics {
        clipboard: Some(ClipboardText {
            text: text.into(),
            revision,
        }),
        ..Default::default()
    }));
    d
}
#[test]
fn nonphysical_clipboard_semantics_create_patch_clear_keep_match_full_shape_zero_edits() {
    for base in [
        Descriptor::new(Props::Button(ButtonProps {
            label: "Copy".into(),
            ..Default::default()
        })),
        node("private <& 😀", 0),
        node("", 1),
    ] {
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Clear),
            Patch::Semantics(Change::Set(node("another private value", 2).semantics)),
        ] {
            let next = base.patched(patch.clone()).unwrap();
            assert_eq!(shape(&base).unwrap(), shape(&next).unwrap());
            let oracle = crate::difference(
                &shape(&base).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            assert!(oracle.is_empty());
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
            assert!(matches!(&sink.0[..],[Op::Patch{edits,..}]if edits.is_empty()));
            let html = mapper.html().unwrap();
            assert!(
                !html.contains("private")
                    && !html.contains("another")
                    && !html.contains("clipboard")
            );
        }
    }
}
#[test]
fn clipboard_revision_and_payload_reject_before_sink_and_preserve_accepted_prefix() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    mapper
        .apply(
            &[Mutation::Create {
                id,
                node: Box::new(node("original", 4)),
            }],
            &mut sink,
        )
        .unwrap();
    sink.0.clear();
    assert!(
        mapper
            .apply(
                &[
                    Mutation::Patch {
                        id,
                        patch: Patch::Classes(Change::Set(vec!["prefix".into()]))
                    },
                    Mutation::Patch {
                        id,
                        patch: Patch::Semantics(Change::Set(node("different", 4).semantics))
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
            .semantics
            .as_ref()
            .unwrap()
            .clipboard
            .as_ref()
            .unwrap()
            .text,
        "original"
    );
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Semantics(Change::Set(node("valid", 5).semantics)),
            }],
            &mut sink,
        )
        .unwrap();
    assert_eq!(mapper.receipt().applied, 3);
}
