use glory_html::{Mapper, Op, Sink};
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
fn hints_override_only_requested_members_and_clear_restores_purpose_without_rewriting_the_value() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    let mut node = Descriptor::new(Props::Input(InputProps {
        purpose: Purpose::Number,
        value: "public-test".into(),
        ..Default::default()
    }));
    node.semantics = Some(Box::new(Semantics {
        input_assist: Some(InputAssist {
            autofill: Some(InputAutofill::OneTimeCode),
            mode: Some(InputMode::Numeric),
        }),
        ..Default::default()
    }));
    mapper
        .apply(
            &[
                Mutation::Create {
                    id,
                    node: Box::new(node),
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
    let html = mapper.html().unwrap();
    assert!(html.contains("autocomplete=\"one-time-code\""));
    assert!(html.contains("inputmode=\"numeric\""));
    sink.0.clear();
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Purpose(Change::Set(Purpose::Password)),
            }],
            &mut sink,
        )
        .unwrap();
    let html = mapper.html().unwrap();
    assert!(html.contains("type=\"password\""));
    assert!(html.contains("inputmode=\"numeric\""));
    assert!(!sink.0.iter().any(|op| matches!(op, Op::Patch { edits, .. } if edits.iter().any(|e| matches!(e, glory_html::Edit::Property { name: "value", .. })))));
    mapper
        .apply(
            &[
                Mutation::Patch {
                    id,
                    patch: Patch::Purpose(Change::Set(Purpose::Number)),
                },
                Mutation::Patch {
                    id,
                    patch: Patch::Semantics(Change::Clear),
                },
            ],
            &mut sink,
        )
        .unwrap();
    let html = mapper.html().unwrap();
    assert!(!html.contains("autocomplete="));
    assert!(html.contains("inputmode=\"decimal\""));
    assert!(html.contains("value=\"public-test\""));
    assert_eq!(mapper.key(id), key);
}
