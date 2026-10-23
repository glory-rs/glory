use glory_html::{Edit, Mapper, Op, Part, Property, Sink};
use glory_render::{
    Change, CheckProps, Descriptor, Error, Mutation, NodeIds, Patch, Props, RuntimeId,
};

#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}

#[test]
fn explicit_checked_writes_restore_host_state_but_keep_and_metadata_do_not() {
    for checked in [false, true] {
        let runtime = RuntimeId::new();
        let id = NodeIds::new(runtime).allocate();
        let mut mapper = Mapper::new(runtime);
        let mut sink = Record::default();
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id,
                        node: Box::new(Descriptor::new(Props::Check(CheckProps {
                            checked,
                            ..Default::default()
                        }))),
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
            Patch::Checked(Change::Set(checked)),
            Patch::Checked(Change::Set(checked)),
        ] {
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .unwrap();
            assert!(
                matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if edits == &vec![Edit::Property {
                    part: Part::Control, name: "checked", value: Property::Bool(checked),
                }])
            );
        }
        mapper
            .apply(
                &[Mutation::Patch {
                    id,
                    patch: Patch::Checked(Change::Clear),
                }],
                &mut sink,
            )
            .unwrap();
        assert!(
            matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if edits.contains(&Edit::Property {
                part: Part::Control, name: "checked", value: Property::Bool(false),
            }))
        );
        mapper
            .apply(
                &[Mutation::Patch {
                    id,
                    patch: Patch::Checked(Change::Keep),
                }],
                &mut sink,
            )
            .unwrap();
        assert!(matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if edits.is_empty()));
        mapper
            .apply(
                &[Mutation::Patch {
                    id,
                    patch: Patch::Name(Change::Set(Some("renamed".into()))),
                }],
                &mut sink,
            )
            .unwrap();
        assert!(
            matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if !edits.iter().any(|edit| matches!(edit, Edit::Property { .. })))
        );
        assert_eq!(mapper.key(id), key);
    }
}
