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
fn node(open: bool, duration_millis: u16, revision: u64) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.style =
        glory_render::style::Style::new().width(glory_render::style::Length::Percent(100.));
    node.semantics = Some(Box::new(Semantics {
        collapse: Some(CollapseConfig {
            open,
            duration_millis,
            revision,
        }),
        hidden: !open,
        inert: !open,
        ..Default::default()
    }));
    node
}
#[test]
fn finite_collapse_attributes_and_every_accepted_field_patch_match_full_shape_oracle() {
    for base in [
        Descriptor::new(Props::View),
        node(false, 240, 0),
        node(true, 240, 0),
    ] {
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Set(node(false, 0, 1).semantics)),
            Patch::Semantics(Change::Set(node(true, 500, 1).semantics)),
            Patch::Style(Change::Set(Box::new(glory_render::style::Style::new()))),
            Patch::Style(Change::Clear),
            Patch::Name(Change::Set(Some("Named motion".to_owned()))),
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
                "{:?} != {expected:?}",
                sink.0
            );
        }
    }
    let shape = shape(&node(false, 240, 7)).unwrap();
    let Kind::Element { attributes, .. } = &shape.pieces[0].kind else {
        panic!("element")
    };
    assert_eq!(
        attributes.get("data-gly-collapse").map(String::as_str),
        Some("closed")
    );
    assert_eq!(
        attributes
            .get("data-gly-collapse-duration")
            .map(String::as_str),
        Some("240ms")
    );
    assert_eq!(
        attributes
            .get("data-gly-collapse-revision")
            .map(String::as_str),
        Some("7")
    );
    assert_eq!(
        attributes.get("aria-hidden").map(String::as_str),
        Some("true")
    );
    assert!(attributes.contains_key("inert") && attributes.contains_key("data-gly-hidden"));
    assert!(
        !attributes.contains_key("hidden"),
        "UA hidden would cut off a closing transition"
    );
    assert!(BASE_CSS.contains("[data-gly-hidden]:not([data-gly-collapse])"));
    assert!(BASE_CSS.contains("attr(data-gly-collapse-duration type(<time>),0ms)"));
}
#[test]
fn incompatible_collapse_updates_reject_before_sink_or_mapper_changes() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    mapper
        .apply(
            &[
                Mutation::Create {
                    id,
                    node: Box::new(node(true, 240, 7)),
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
    for patch in [
        Patch::Semantics(Change::Clear),
        Patch::Semantics(Change::Set(node(false, 240, 7).semantics)),
        Patch::Semantics(Change::Set(node(false, 501, 8).semantics)),
    ] {
        sink.0.clear();
        let before = mapper.receipt();
        assert!(
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .is_err()
        );
        assert!(sink.0.is_empty());
        assert_eq!(mapper.receipt(), before);
    }
}
