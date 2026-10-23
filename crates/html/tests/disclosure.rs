use glory_html::*;
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
fn disclosure_html_uses_real_identity_and_retirement_clears_surviving_button() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let target = ids.allocate();
    let button = ids.allocate();
    let mut mapper = Mapper::new_in(runtime, Namespace::Server);
    let mut sink = Record::default();
    for (id, props) in [
        (target, Props::View),
        (
            button,
            Props::Button(ButtonProps {
                disclosure: Some(Disclosure {
                    controls: target,
                    expanded: false,
                }),
                ..Default::default()
            }),
        ),
    ] {
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
    }
    let identity = element_id(target, Namespace::Server);
    let html = mapper.html().unwrap();
    assert!(html.contains(&format!("id=\"{identity}\"")));
    assert!(html.contains(&format!("aria-controls=\"{identity}\"")));
    assert!(html.contains("aria-expanded=\"false\""));
    mapper.canonicalize().unwrap();
    assert_eq!(html, mapper.html().unwrap());
    let count = sink.0.len();
    mapper
        .apply(
            &[Mutation::Patch {
                id: button,
                patch: Patch::Disclosure(Change::Keep),
            }],
            &mut sink,
        )
        .unwrap();
    assert!(matches!(&sink.0[count],Op::Patch{edits,..}if edits.is_empty()));
    mapper
        .apply(
            &[Mutation::Patch {
                id: button,
                patch: Patch::Disclosure(Change::Set(Some(Disclosure {
                    controls: target,
                    expanded: true,
                }))),
            }],
            &mut sink,
        )
        .unwrap();
    assert!(mapper.html().unwrap().contains("aria-expanded=\"true\""));
    mapper
        .apply(&[Mutation::Dispose { id: target }], &mut sink)
        .unwrap();
    assert!(!mapper.html().unwrap().contains("aria-controls"));
    assert!(!mapper.html().unwrap().contains("aria-expanded"));
    assert!(matches!(&sink.0[sink.0.len()-2],Op::Patch{id,edits}if *id==button&&edits.len()==2));
}
