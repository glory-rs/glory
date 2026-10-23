use glory_html::{Edit, Mapper, Op, Sink};
use glory_html::{Kind, Part, shape};
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
fn name_and_role_deltas_preserve_identity_and_keep_clear_differ() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    mapper
        .apply(
            &[
                Mutation::Create {
                    id,
                    node: Box::new(Descriptor::new(Props::View)),
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
    let key = mapper.key(id);
    for patch in [
        Patch::Name(Change::Set(Some("Panel".into()))),
        Patch::Role(Change::Set(Some(Role::Status))),
        Patch::Role(Change::Clear),
    ] {
        mapper
            .apply(&[Mutation::Patch { id, patch }], &mut sink)
            .unwrap();
        assert_eq!(mapper.key(id), key);
    }
    assert!(mapper.html().unwrap().contains("role=\"group\""));
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Name(Change::Keep),
            }],
            &mut sink,
        )
        .unwrap();
    assert!(matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if edits.is_empty()));
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Name(Change::Clear),
            }],
            &mut sink,
        )
        .unwrap();
    let Some(Op::Patch { edits, .. }) = sink.0.last() else {
        panic!()
    };
    assert!(edits.contains(&Edit::Attribute {
        part: Part::Main,
        name: "aria-label",
        value: None
    }));
    assert!(edits.contains(&Edit::Attribute {
        part: Part::Main,
        name: "role",
        value: None
    }));
    assert!(
        !mapper.html().unwrap().contains("aria-label") && !mapper.html().unwrap().contains("role=")
    );
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Role(Change::Set(Some(Role::Group))),
            }],
            &mut sink,
        )
        .unwrap();
    assert!(mapper.html().unwrap().contains("role=\"group\""));
    assert_eq!(mapper.key(id), key);
}

#[test]
fn control_names_use_the_control_part_and_do_not_replace_content_or_value() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    for (props, control) in [
        (
            Props::Button(ButtonProps {
                label: "Button label".into(),
                ..Default::default()
            }),
            Part::Main,
        ),
        (
            Props::Input(InputProps {
                value: "raw🙂".into(),
                ..Default::default()
            }),
            Part::Main,
        ),
        (
            Props::Check(CheckProps {
                label: "Check label".into(),
                ..Default::default()
            }),
            Part::Control,
        ),
        (
            Props::Link(LinkProps {
                label: "Link label".into(),
                target: "#target".into(),
            }),
            Part::Main,
        ),
    ] {
        let id = ids.allocate();
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id,
                        node: Box::new(Descriptor::new(props)),
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
        mapper
            .apply(
                &[Mutation::Patch {
                    id,
                    patch: Patch::Name(Change::Set(Some("Override".into()))),
                }],
                &mut sink,
            )
            .unwrap();
        let Some(Op::Patch { edits, .. }) = sink.0.last() else {
            panic!()
        };
        assert_eq!(
            edits,
            &[Edit::Attribute {
                part: control,
                name: "aria-label",
                value: Some("Override".into())
            }]
        );
        mapper
            .apply(
                &[Mutation::Patch {
                    id,
                    patch: Patch::Name(Change::Clear),
                }],
                &mut sink,
            )
            .unwrap();
        let Some(Op::Patch { edits, .. }) = sink.0.last() else {
            panic!()
        };
        assert_eq!(
            edits,
            &[Edit::Attribute {
                part: control,
                name: "aria-label",
                value: None
            }]
        );
    }
    let html = mapper.html().unwrap();
    for label in ["Button label", "Check label", "Link label", "raw🙂"] {
        assert!(html.contains(label));
    }
}

#[test]
fn named_views_have_a_nameable_role_while_unnamed_views_remain_generic() {
    let mut node = Descriptor::new(Props::View);
    node.name = Some("Panel".into());
    let named = shape(&node).unwrap();
    let Kind::Element { attributes, .. } = &named
        .pieces
        .iter()
        .find(|piece| piece.part == Part::Main)
        .unwrap()
        .kind
    else {
        panic!()
    };
    assert_eq!(attributes.get("role").map(String::as_str), Some("group"));
    assert_eq!(
        attributes.get("aria-label").map(String::as_str),
        Some("Panel")
    );
    node.patch(Patch::Name(Change::Clear)).unwrap();
    let generic = shape(&node).unwrap();
    let Kind::Element { attributes, .. } = &generic.pieces[0].kind else {
        panic!()
    };
    assert!(!attributes.contains_key("role") && !attributes.contains_key("aria-label"));
}
