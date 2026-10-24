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
fn direction_language_keep_set_clear_match_full_shape_difference_without_control_replacement() {
    let mut base = Descriptor::new(Props::View);
    base.semantics = Some(Box::new(Semantics {
        environment: Some(Environment {
            direction: Direction::Rtl,
            language: LanguageTag::new("ar").unwrap(),
        }),
        ..Default::default()
    }));
    for patch in [
        Patch::Semantics(Change::Keep),
        Patch::Semantics(Change::Clear),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics::default())))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            environment: Some(Environment {
                direction: Direction::Ltr,
                language: LanguageTag::new("zh-Hans-CN").unwrap(),
            }),
            ..Default::default()
        })))),
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
        assert!(matches!(&sink.0[0], Op::Patch { edits, .. } if edits == &expected));
    }
}
