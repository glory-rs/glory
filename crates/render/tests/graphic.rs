use glory_render::memory::Memory;
use glory_render::*;
fn drawing() -> GraphicProps {
    GraphicProps {
        commands: vec![
            PathCommand::Move(GraphicPoint::new(0.0, 0.0)),
            PathCommand::Line(GraphicPoint::new(24.0, 24.0)),
        ],
        stroke: Some(GraphicStroke {
            color: style::Color(1, 2, 3, 255),
            width: 2.0,
            cap: StrokeCap::Round,
        }),
        alternative: Some("Icon".into()),
        ..Default::default()
    }
}
#[test]
fn raw_and_prepared_graphic_validation_have_identical_rollback() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let initial = Descriptor::new(Props::Graphic(drawing()));
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(initial.clone()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let stats = memory.stats();
    for bad in [
        GraphicProps {
            width: f64::NAN,
            ..drawing()
        },
        GraphicProps {
            animation: Some(GraphicAnimation::Rotate { period_millis: 0 }),
            ..drawing()
        },
        GraphicProps {
            commands: vec![PathCommand::Close],
            ..drawing()
        },
        GraphicProps {
            commands: vec![PathCommand::Move(GraphicPoint::new(0.0, 0.0)); 257],
            ..drawing()
        },
    ] {
        let patch = Patch::Graphic(Change::Set(Box::new(bad)));
        let mut raw = initial.clone();
        assert!(raw.patch(patch.clone()).is_err());
        assert_eq!(raw, initial);
        assert!(memory.prepare_patch(id, &patch).is_err());
        assert_eq!(memory.node(id).unwrap().descriptor, initial);
        assert_eq!(memory.stats(), stats);
    }
    let next = GraphicProps {
        width: 48.0,
        ..drawing()
    };
    {
        let prepared = memory
            .prepare_patch(id, &Patch::Graphic(Change::Set(Box::new(next.clone()))))
            .unwrap();
        assert!(matches!(prepared.descriptor().props,Props::Graphic(props) if props.width==48.0));
    }
    assert_eq!(memory.node(id).unwrap().descriptor, initial);
    assert_eq!(memory.stats(), stats);
    memory
        .prepare_patch(id, &Patch::Graphic(Change::Set(Box::new(next))))
        .unwrap()
        .commit();
    assert!(
        matches!(memory.node(id).unwrap().descriptor.view().props,Props::Graphic(props) if props.width==48.0)
    );
    memory
        .prepare_patch(id, &Patch::Graphic(Change::Clear))
        .unwrap()
        .commit();
    assert!(
        matches!(memory.node(id).unwrap().descriptor.view().props,Props::Graphic(props) if props==&GraphicProps::default())
    );
}
#[test]
fn graphics_are_leaves_with_intrinsic_image_semantics_and_explicit_decoration() {
    let mut graphic = Descriptor::new(Props::Graphic(drawing()));
    graphic.validate().unwrap();
    assert!(!graphic.props.accepts_children());
    graphic.role = Some(Role::Group);
    assert!(graphic.validate().is_err());
    graphic.role = None;
    graphic.patch(Patch::Graphic(Change::Clear)).unwrap();
    graphic.name = Some("Name".into());
    assert!(graphic.validate().is_err());
    graphic.name = None;
    graphic.focus = FocusMode::TabStop;
    assert!(graphic.validate().is_err());
    graphic.focus = FocusMode::Auto;
    assert!(
        Action::Focus(NodeIds::new(RuntimeId::new()).allocate())
            .validate_target(&graphic)
            .is_err()
    );
    graphic.semantics = Some(Box::new(Semantics {
        labelled_by: Some(SemanticId::new("label").unwrap()),
        ..Default::default()
    }));
    assert!(graphic.validate().is_err());
}
