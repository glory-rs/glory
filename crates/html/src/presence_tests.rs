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
        presence: Some(Box::new(PresenceConfig {
            open,
            duration_millis,
            revision,
            ..Default::default()
        })),
        hidden: !open,
        inert: !open,
        ..Default::default()
    }));
    node
}
#[test]
fn finite_presence_attributes_and_every_accepted_field_patch_match_full_shape_oracle() {
    for base in [
        Descriptor::new(Props::View),
        node(false, 240, 0),
        node(true, 240, 0),
    ] {
        for patch in [
            Patch::Semantics(Change::Set({
                let mut d = node(true, 400, 1);
                d.semantics
                    .as_mut()
                    .unwrap()
                    .presence
                    .as_mut()
                    .unwrap()
                    .effect = PresenceEffect::Custom(PresenceStyle {
                    opacity: 300,
                    scale_y: 500,
                    rotate: -12,
                    translate_y: 20,
                    ..Default::default()
                });
                d.semantics
            })),
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
        attributes.get("data-gly-presence").map(String::as_str),
        Some("closed")
    );
    assert_eq!(
        attributes
            .get("data-gly-presence-duration")
            .map(String::as_str),
        Some("240ms")
    );
    assert_eq!(
        attributes
            .get("data-gly-presence-revision")
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
    assert!(
        BASE_CSS.contains("[data-gly-hidden]:not([data-gly-collapse]):not([data-gly-presence])")
    );
    assert!(BASE_CSS.contains("attr(data-gly-presence-duration type(<time>),0ms)"));
}
