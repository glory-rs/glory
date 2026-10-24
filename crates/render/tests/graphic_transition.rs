use glory_render::{memory::Memory, *};
fn graphic(y: f64) -> GraphicProps {
    GraphicProps {
        commands: vec![
            PathCommand::Move(GraphicPoint::new(2., y)),
            PathCommand::Line(GraphicPoint::new(22., y)),
        ],
        transition: Some(GraphicTransition {
            duration_millis: 200,
        }),
        ..Default::default()
    }
}
#[test]
fn durations_topology_mutual_exclusion_and_finite_interpolation_are_explicit() {
    for duration_millis in [0, 1, 500] {
        GraphicTransition { duration_millis }.validate().unwrap();
    }
    for duration_millis in [501, u16::MAX] {
        assert!(GraphicTransition { duration_millis }.validate().is_err());
    }
    let from = graphic(4.);
    let to = graphic(20.);
    assert_eq!(
        GraphicTransition::sample(&from.commands, &to.commands, 0.5).unwrap(),
        graphic(12.).commands
    );
    for progress in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(GraphicTransition::sample(&from.commands, &to.commands, progress).is_err());
    }
    let mut bad = to.clone();
    bad.commands[0] = PathCommand::Close;
    assert!(!GraphicTransition::compatible(
        &from.commands,
        &bad.commands
    ));
    assert!(GraphicTransition::sample(&from.commands, &bad.commands, 0.5).is_err());
    let mut both = from;
    both.animation = Some(GraphicAnimation::Rotate {
        period_millis: 1000,
    });
    assert!(both.validate().is_err());
}
#[test]
fn raw_prepared_direct_rejection_retains_path_stats_and_prefix_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut memory = Memory::new(runtime);
    let original = Descriptor::new(Props::Graphic(graphic(4.)));
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(original.clone()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let stats = memory.stats();
    let mut bad = graphic(20.);
    bad.commands.push(PathCommand::Close);
    let patch = Patch::Graphic(Change::Set(Box::new(bad)));
    assert!(original.patched(patch.clone()).is_err());
    assert!(memory.prepare_patch(id, &patch).is_err());
    assert!(memory.apply(&[Mutation::Patch { id, patch }]).is_err());
    assert_eq!(memory.stats(), stats);
    assert_eq!(memory.node(id).unwrap().descriptor, original);
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Graphic(Change::Set(Box::new(graphic(20.)))),
        }])
        .unwrap();
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Graphic(Change::Clear),
        }])
        .unwrap();
    assert!(
        matches!(memory.node(id).unwrap().descriptor.view().props,Props::Graphic(p) if p.transition.is_none())
    );
}
