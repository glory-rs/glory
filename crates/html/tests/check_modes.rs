use glory_html::{Edit, Kind, Mapper, Op, Part, Property, Sink, shape};
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
fn check_modes_preserve_real_properties_and_independent_switch_role() {
    for mode in [
        CheckMode::Checkbox,
        CheckMode::Mixed,
        CheckMode::Switch,
        CheckMode::Radio,
    ] {
        for checked in [false, true] {
            let descriptor = Descriptor::new(Props::Check(CheckProps {
                mode,
                checked,
                label: "Choice".into(),
                ..Default::default()
            }));
            let shape = shape(&descriptor).unwrap();
            let Kind::Element {
                attributes,
                properties,
                tag,
            } = &shape
                .pieces
                .iter()
                .find(|piece| piece.part == Part::Control)
                .unwrap()
                .kind
            else {
                panic!()
            };
            assert_eq!(*tag, "input");
            assert_eq!(
                attributes["type"],
                if mode == CheckMode::Radio {
                    "radio"
                } else {
                    "checkbox"
                }
            );
            assert_eq!(properties["checked"], Property::Bool(checked));
            assert_eq!(
                properties["indeterminate"],
                Property::Bool(mode == CheckMode::Mixed)
            );
            assert_eq!(
                attributes.get("role").map(String::as_str),
                (mode == CheckMode::Switch).then_some("switch")
            );
            assert_eq!(
                attributes["aria-checked"],
                if mode == CheckMode::Mixed {
                    "mixed"
                } else if checked {
                    "true"
                } else {
                    "false"
                }
            );
        }
    }
}
#[test]
fn equal_mode_reasserts_indeterminate_but_keep_and_names_do_not_touch_native_state() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    mapper
        .apply(
            &[Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::Check(CheckProps {
                    mode: CheckMode::Mixed,
                    ..Default::default()
                }))),
            }],
            &mut sink,
        )
        .unwrap();
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::CheckMode(Change::Set(CheckMode::Mixed)),
            }],
            &mut sink,
        )
        .unwrap();
    assert!(
        matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if edits == &vec![Edit::Property { part: Part::Control, name: "indeterminate", value: Property::Bool(true) }])
    );
    for patch in [
        Patch::CheckMode(Change::Keep),
        Patch::Name(Change::Set(Some("Renamed".into()))),
    ] {
        mapper
            .apply(&[Mutation::Patch { id, patch }], &mut sink)
            .unwrap();
        assert!(
            matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if !edits.iter().any(|edit| matches!(edit, Edit::Property { .. })))
        );
    }
    mapper
        .apply(
            &[Mutation::Patch {
                id,
                patch: Patch::CheckMode(Change::Clear),
            }],
            &mut sink,
        )
        .unwrap();
    assert!(
        matches!(sink.0.last(), Some(Op::Patch { edits, .. }) if edits.contains(&Edit::Property { part: Part::Control, name: "indeterminate", value: Property::Bool(false) }))
    );
}
