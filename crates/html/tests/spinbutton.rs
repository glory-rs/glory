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
fn spin_semantic_deltas_keep_the_input_and_omit_unknown_coordinates() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    let mut descriptor = Descriptor::new(Props::Input(InputProps {
        value: "1.00".into(),
        ..Default::default()
    }));
    descriptor.semantics = Some(Box::new(Semantics {
        spinbutton: Some(SpinValue {
            value: Some(1.),
            value_text: Some("1.00 exact".into()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    mapper
        .apply(
            &[
                Mutation::Create {
                    id,
                    node: Box::new(descriptor),
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
    assert!(html.contains("role=\"spinbutton\""));
    assert!(html.contains("aria-valuenow=\"1\""));
    assert!(html.contains("aria-valuetext=\"1.00 exact\""));
    assert!(!html.contains("aria-valuemin"));
    assert!(html.contains("value=\"1.00\""));
    let empty = Semantics {
        spinbutton: Some(SpinValue {
            min: Some(0.),
            max: Some(10.),
            value: None,
            ..Default::default()
        }),
        ..Default::default()
    };
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Semantics(Change::Set(Some(Box::new(empty)))),
            }],
            &mut sink,
        )
        .unwrap();
    assert_eq!(mapper.key(id), key);
    let html = mapper.html().unwrap();
    assert!(!html.contains("aria-valuenow"));
    assert!(!html.contains("aria-valuetext"));
    assert!(html.contains("aria-valuemin=\"0\""));
    assert!(html.contains("value=\"1.00\""));
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::Semantics(Change::Clear),
            }],
            &mut sink,
        )
        .unwrap();
    let html = mapper.html().unwrap();
    assert!(!html.contains("spinbutton"));
    assert!(!html.contains("aria-value"));
    assert_eq!(mapper.key(id), key);
    assert!(html.contains("value=\"1.00\""));
}
