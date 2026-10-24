use crate::*;
use glory_render::*;
use std::rc::Rc;
#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}
fn document() -> Rc<RichDocument> {
    Rc::new(
        RichDocument::new(vec![RichBlock {
            kind: RichBlockKind::Heading(2),
            runs: vec![RichRun {
                text: "<script>&😀".into(),
                marks: RichMarks {
                    bold: true,
                    italic: true,
                    code: true,
                    link: Some("https://example.test/?x=1&y=2".into()),
                },
            }],
        }])
        .unwrap(),
    )
}
#[test]
fn every_rich_field_and_reset_matches_full_shape_oracle() {
    let base = Descriptor::new(Props::RichEditor(RichEditorProps {
        document: document(),
        revision: 3,
        ..Default::default()
    }));
    let mut patches = vec![
        Patch::RichDocument {
            document: Change::Set(Rc::new(RichDocument::default())),
            revision: 4,
        },
        Patch::RichDocument {
            document: Change::Set(document()),
            revision: 4,
        },
        Patch::RichDocument {
            document: Change::Set(document()),
            revision: 3,
        },
        Patch::RichDocument {
            document: Change::Clear,
            revision: 4,
        },
        Patch::RichDocument {
            document: Change::Keep,
            revision: 0,
        },
        Patch::Enabled(Change::Set(false)),
        Patch::Enabled(Change::Clear),
        Patch::Name(Change::Set(Some("Editor".into()))),
        Patch::Focus(Change::Set(FocusMode::TabStop)),
    ];
    for readonly in [true, false] {
        patches.push(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            readonly,
            ..Default::default()
        })))));
    }
    patches.push(Patch::Semantics(Change::Clear));
    for mut base in [base.clone(), base] {
        for readonly in [false, true] {
            base.semantics = Some(Box::new(Semantics {
                readonly,
                ..Default::default()
            }));
            for patch in &patches {
                let next = base.patched(patch.clone()).unwrap();
                let expected = difference(
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
                    .apply(
                        &[Mutation::Patch {
                            id,
                            patch: patch.clone(),
                        }],
                        &mut sink,
                    )
                    .unwrap();
                assert!(
                    matches!(&sink.0[0],Op::Patch{edits,..} if edits==&expected),
                    "{patch:?}: {:?} != {expected:?}",
                    sink.0
                );
            }
        }
    }
}
#[test]
fn rich_serializer_uses_real_bounded_tags_and_escapes_text_and_links() {
    let doc = document();
    let html = rich_document(&doc);
    assert!(
        html.starts_with("<h2><a href=\"https://example.test/?x=1&amp;y=2\"><strong><em><code>")
    );
    assert!(html.contains("&lt;script&gt;&amp;😀"));
    assert!(!html.contains("<script>"));
    let shaped = shape(&Descriptor::new(Props::RichEditor(Default::default()))).unwrap();
    assert_eq!(shaped.edit, EditKind::Rich);
    assert_eq!(shaped.children, None);
    let Kind::Element {
        tag, attributes, ..
    } = &shaped.pieces[0].kind
    else {
        panic!()
    };
    assert_eq!(*tag, "div");
    assert_eq!(attributes["role"], "textbox");
    assert_eq!(attributes["aria-multiline"], "true");
    assert_eq!(rich_document(&RichDocument::default()), "<p><br></p>");
}

#[test]
fn rich_contextual_shape_and_serialization_follow_current_fieldset_ancestry() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let fieldset = ids.allocate();
    let editor = ids.allocate();
    let mut tree = memory::Memory::new(runtime);
    let mut parent = Descriptor::new(Props::View);
    parent.semantics = Some(Box::new(Semantics {
        field_part: Some(FieldPart::Fieldset),
        disabled: true,
        ..Default::default()
    }));
    let mut descriptor = Descriptor::new(Props::RichEditor(Default::default()));
    descriptor.focus = FocusMode::TabStop;
    tree.apply(&[
        Mutation::Create {
            id: fieldset,
            node: Box::new(parent),
        },
        Mutation::Place {
            id: fieldset,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: editor,
            node: Box::new(descriptor),
        },
        Mutation::Place {
            id: editor,
            parent: Some(fieldset),
            before: None,
        },
    ])
    .unwrap();
    let attributes = |shape: Shape| {
        let Kind::Element { attributes, .. } = shape.pieces.into_iter().next().unwrap().kind else {
            panic!()
        };
        attributes
    };
    let disabled = attributes(shape_in(&tree, editor).unwrap());
    assert_eq!(disabled["contenteditable"], "false");
    assert_eq!(disabled["tabindex"], "-1");
    assert_eq!(disabled["aria-disabled"], "true");
    assert!(disabled.contains_key("data-gly-rich-ancestor-disabled"));
    let html = serialize(&tree, |id| Some(if id == fieldset { 0 } else { 1 })).unwrap();
    assert!(html.contains("data-gly-rich-ancestor-disabled=\"\""));
    tree.apply(&[Mutation::Place {
        id: editor,
        parent: None,
        before: None,
    }])
    .unwrap();
    let enabled = attributes(shape_in(&tree, editor).unwrap());
    assert_eq!(enabled["contenteditable"], "true");
    assert_eq!(enabled["tabindex"], "0");
    assert!(!enabled.contains_key("data-gly-rich-ancestor-disabled"));
    assert!(!enabled.contains_key("aria-disabled"));
}
