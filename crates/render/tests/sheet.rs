use glory_render::{memory::Memory, *};
fn config() -> SheetConfig {
    SheetConfig {
        fractions: vec![0.25, 0.5, 1.0],
        current: 1,
        max_extent: 640.0,
        settle_millis: 180,
        dismissible: true,
        revision: 4,
    }
}
fn node() -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::Dialog);
    node.focus = FocusMode::Programmatic;
    node.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("sheet").unwrap()),
        layer: Some(Layer {
            placement: LayerPlacement::Bottom,
            sheet: Some(config()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    node
}
#[test]
fn finite_configuration_geometry_and_dismissal_are_bounded() {
    let base = config();
    assert_eq!(base.extent(900.0, 1).unwrap(), 320.0);
    assert_eq!(base.extent(12.0, 2).unwrap(), 12.0);
    assert_eq!(base.extent(0.0, 0).unwrap(), 0.0);
    assert_eq!(base.nearest(900.0, 10.0).unwrap(), SheetCommand::Dismiss);
    assert_eq!(base.nearest(900.0, 500.0).unwrap(), SheetCommand::Snap(2));
    assert!(base.extent(f64::NAN, 0).is_err());
    assert!(base.nearest(300.0, f64::INFINITY).is_err());
    let mut rejected = vec![];
    for fractions in [
        vec![],
        vec![0.5; 9],
        vec![0.0],
        vec![1.1],
        vec![f64::NAN],
        vec![0.5, 0.5],
        vec![0.8, 0.3],
    ] {
        let mut c = base.clone();
        c.fractions = fractions;
        rejected.push(c);
    }
    for value in [f64::NAN, f64::INFINITY, 127.9, 4096.1] {
        let mut c = base.clone();
        c.max_extent = value;
        rejected.push(c);
    }
    let mut c = base.clone();
    c.current = 3;
    rejected.push(c);
    let mut c = base.clone();
    c.settle_millis = 501;
    rejected.push(c);
    for config in rejected {
        assert!(config.validate().is_err(), "{config:?}");
    }
    let mut c = base;
    c.current = 0;
    c.dismissible = false;
    c.max_extent = 128.0;
    c.settle_millis = 0;
    assert_eq!(c.nearest(500.0, -20.0).unwrap(), SheetCommand::Snap(0));
    for edge in [
        LayerPlacement::Left,
        LayerPlacement::Right,
        LayerPlacement::Top,
        LayerPlacement::Bottom,
    ] {
        let mut n = node();
        n.semantics
            .as_mut()
            .unwrap()
            .layer
            .as_mut()
            .unwrap()
            .placement = edge;
        n.validate().unwrap();
    }
    let mut n = node();
    n.semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .placement = LayerPlacement::Center;
    assert!(n.validate().is_err());
}
#[test]
fn revisions_are_enforced_by_raw_live_and_direct_paths_and_events_discard_old() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(node()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let patch = |revision, current| {
        let mut next = node().semantics;
        let config = next
            .as_mut()
            .unwrap()
            .layer
            .as_mut()
            .unwrap()
            .sheet
            .as_mut()
            .unwrap();
        config.revision = revision;
        config.current = current;
        Patch::Semantics(Change::Set(next))
    };
    for bad in [patch(3, 1), patch(4, 2), Patch::Semantics(Change::Clear)] {
        let before = memory.node(id).unwrap().descriptor.view().to_owned();
        let stats = memory.stats();
        assert!(before.patched(bad.clone()).is_err());
        assert!(memory.prepare_patch(id, &bad).is_err());
        assert!(memory.apply(&[Mutation::Patch { id, patch: bad }]).is_err());
        assert_eq!(memory.node(id).unwrap().descriptor, before);
        assert_eq!(memory.stats(), stats);
    }
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: patch(5, 2),
        }])
        .unwrap();
    assert!(
        !memory
            .validate_event(
                id,
                &Event::Sheet {
                    from: id,
                    revision: 4,
                    command: SheetCommand::Snap(999)
                }
            )
            .unwrap()
    );
    assert!(matches!(
        memory.validate_event(
            id,
            &Event::Sheet {
                from: id,
                revision: 6,
                command: SheetCommand::Snap(1)
            }
        ),
        Err(Error::SheetRevision {
            current: 5,
            incoming: 6
        })
    ));
    assert!(
        memory
            .validate_event(
                id,
                &Event::Sheet {
                    from: id,
                    revision: 5,
                    command: SheetCommand::Snap(3)
                }
            )
            .is_err()
    );
    assert!(
        memory
            .validate_event(
                id,
                &Event::Sheet {
                    from: id,
                    revision: 5,
                    command: SheetCommand::Snap(1)
                }
            )
            .unwrap()
    );
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        memory.validate_event(
            id,
            &Event::Sheet {
                from: foreign,
                revision: 5,
                command: SheetCommand::Snap(1)
            }
        ),
        Err(Error::ForeignRuntime)
    );
    let other = ids.allocate();
    memory
        .apply(&[Mutation::Create {
            id: other,
            node: Box::new(Descriptor::new(Props::View)),
        }])
        .unwrap();
    assert!(
        memory
            .validate_event(
                id,
                &Event::Sheet {
                    from: other,
                    revision: 5,
                    command: SheetCommand::Snap(1)
                }
            )
            .is_err()
    );
    assert!(
        memory
            .validate_event(
                other,
                &Event::Sheet {
                    from: other,
                    revision: 5,
                    command: SheetCommand::Snap(1)
                }
            )
            .is_err()
    );
}
