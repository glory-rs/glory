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
fn tags_policy_and_required_defaults_use_the_prepared_field_full_shape_oracle() {
    let mut base = Descriptor::new(Props::Input(InputProps {
        value: "native 😀 editor".repeat(200),
        revision: 19,
        ..Default::default()
    }));
    base.semantics = Some(Box::new(Semantics {
        required: true,
        ..Default::default()
    }));
    let mut originals = vec![base.clone()];
    for policy in [
        TagSeparators::Enter,
        TagSeparators::EnterComma,
        TagSeparators::EnterCommaSemicolon,
    ] {
        let mut value = base.clone();
        value.semantics.as_mut().unwrap().tags_input = Some(policy);
        originals.push(value);
    }
    for base in originals {
        let mut metadata = base.semantics.clone().unwrap();
        metadata.tags_input = if metadata.tags_input.is_some() {
            None
        } else {
            Some(TagSeparators::EnterCommaSemicolon)
        };
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Set(Some(metadata))),
            Patch::Semantics(Change::Clear),
            Patch::Semantics(Change::Set(None)),
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
            let Props::Input(input) = mapper.tree().node(id).unwrap().descriptor.view().props
            else {
                panic!()
            };
            assert_eq!(input.revision, 19);
            assert_eq!(input.value, "native 😀 editor".repeat(200));
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
        let policy = base.semantics.as_ref().unwrap().tags_input;
        assert_eq!(attrs.contains_key("required"), policy.is_none());
        assert_eq!(
            attrs.get("data-gly-tags").map(String::as_str),
            policy.map(TagSeparators::as_str)
        );
        assert_eq!(attrs.get("aria-required").map(String::as_str), Some("true"));
        assert!(
            !attrs.contains_key("role")
                && !attrs.contains_key("aria-expanded")
                && !attrs.contains_key("aria-haspopup")
        );
    }
}
