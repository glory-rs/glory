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
fn capture_focus_policy_keep_set_and_clear_match_the_full_shape_oracle() {
    let mut base = Descriptor::new(Props::View);
    base.focus = FocusMode::TabStop;
    base.semantics = Some(Box::new(Semantics {
        drag_capture: Some(DragCapture::Press),
        ..Default::default()
    }));
    for patch in [
        Patch::Semantics(Change::Keep),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            drag_capture: Some(DragCapture::PressFocus),
            ..Default::default()
        })))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics::default())))),
        Patch::Semantics(Change::Clear),
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
    assert!(crate::BASE_CSS.contains("[data-gly-drag-capture]"));
}
