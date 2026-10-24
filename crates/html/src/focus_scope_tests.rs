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
fn every_focus_scope_field_and_reset_matches_full_shape_difference() {
    let mut base = Descriptor::new(Props::View);
    base.focus = FocusMode::Programmatic;
    base.semantics = Some(Box::new(Semantics {
        focus_scope: Some(FocusScopeConfig::default()),
        ..Default::default()
    }));
    let all_false = FocusScopeConfig {
        active: false,
        loop_tab: false,
        contain_focus: false,
        auto_focus: false,
        restore_focus: false,
        initial_focus: Some(SemanticId::new("initial").unwrap()),
        return_focus: Some(SemanticId::new("return").unwrap()),
    };
    for config in [FocusScopeConfig::default(), all_false] {
        let mut before = base.clone();
        before.semantics.as_mut().unwrap().focus_scope = Some(config.clone());
        let mut alternatives = vec![Some(config.clone()), None];
        for field in 0..7 {
            let mut next = config.clone();
            match field {
                0 => next.active = !next.active,
                1 => next.loop_tab = !next.loop_tab,
                2 => next.contain_focus = !next.contain_focus,
                3 => next.auto_focus = !next.auto_focus,
                4 => next.restore_focus = !next.restore_focus,
                5 => next.initial_focus = Some(SemanticId::new("new-initial").unwrap()),
                _ => next.return_focus = Some(SemanticId::new("new-return").unwrap()),
            }
            alternatives.push(Some(next));
        }
        let mut patches = vec![
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Clear),
        ];
        patches.extend(alternatives.into_iter().map(|focus_scope| {
            Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                focus_scope,
                ..Default::default()
            }))))
        }));
        for patch in patches {
            let after = before.patched(patch.clone()).unwrap();
            let expected = crate::difference(
                &shape(&before).unwrap(),
                &shape(&after).unwrap(),
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
                            node: Box::new(before.clone()),
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
            assert_eq!(
                mapper.tree().focus_scopes(),
                usize::from(
                    after
                        .semantics
                        .as_ref()
                        .is_some_and(|s| s.focus_scope.is_some())
                )
            );
        }
    }
}
