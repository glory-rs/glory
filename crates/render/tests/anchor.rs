use glory_render::{memory::Memory, *};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn twelve_physical_placements_preserve_alignment_without_collisions() {
    let anchor = rect(100., 100., 40., 20.);
    let panel = rect(0., 0., 30., 10.);
    let viewport = rect(0., 0., 400., 300.);
    for side in [
        AnchorSide::Top,
        AnchorSide::Bottom,
        AnchorSide::Left,
        AnchorSide::Right,
    ] {
        for (align, cross) in [
            (AnchorAlign::Start, 100.),
            (AnchorAlign::Center, 105.),
            (AnchorAlign::End, 110.),
        ] {
            let position = AnchorPlacement::new(side, align)
                .resolve(anchor, panel, viewport)
                .unwrap();
            assert_eq!(position.side, side);
            let expected = match side {
                AnchorSide::Top => (cross, 84.),
                AnchorSide::Bottom => (cross, 126.),
                AnchorSide::Left => (64., cross),
                AnchorSide::Right => (146., cross),
            };
            assert_eq!((position.x, position.y), expected);
        }
    }
}

#[test]
fn collision_policies_flip_only_to_less_overflow_and_shift_independently() {
    let viewport = rect(0., 0., 200., 200.);
    let panel = rect(0., 0., 60., 40.);
    for (side, anchor, opposite) in [
        (AnchorSide::Top, rect(90., 0., 10., 10.), AnchorSide::Bottom),
        (
            AnchorSide::Bottom,
            rect(90., 190., 10., 10.),
            AnchorSide::Top,
        ),
        (AnchorSide::Left, rect(0., 90., 10., 10.), AnchorSide::Right),
        (
            AnchorSide::Right,
            rect(190., 90., 10., 10.),
            AnchorSide::Left,
        ),
    ] {
        let mut placement = AnchorPlacement::new(side, AnchorAlign::Start);
        assert_eq!(
            placement.resolve(anchor, panel, viewport).unwrap().side,
            opposite
        );
        placement.flip = false;
        let shifted = placement.resolve(anchor, panel, viewport).unwrap();
        assert_eq!(shifted.side, side);
        assert!((8.0..=132.0).contains(&shifted.x) && (8.0..=152.0).contains(&shifted.y));
        placement.shift = false;
        let unshifted = placement.resolve(anchor, panel, viewport).unwrap();
        assert!(unshifted.x < 8. || unshifted.y < 8. || unshifted.x > 132. || unshifted.y > 152.);
    }
    let huge = AnchorPlacement::default()
        .resolve(rect(50., 50., 10., 10.), rect(0., 0., 500., 500.), viewport)
        .unwrap();
    assert_eq!((huge.x, huge.y), (8., 8.));
    assert!(
        AnchorPlacement::default()
            .resolve(rect(f64::MAX, 0., f64::MAX, 1.), panel, viewport)
            .is_err()
    );
}

fn layer() -> Descriptor {
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.role = Some(Role::Dialog);
    descriptor.focus = FocusMode::Programmatic;
    descriptor.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("layer").unwrap()),
        layer: Some(Layer {
            modal: false,
            placement: LayerPlacement::Anchor(AnchorPlacement::default()),
            anchor: Some(SemanticId::new("anchor").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    descriptor
}

#[test]
fn every_numeric_policy_rejects_nonfinite_negative_and_excess_before_live_commit() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut memory = Memory::new(runtime);
    let descriptor = layer();
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(descriptor.clone()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let stats = memory.stats();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., 4097.] {
        for padding in [false, true] {
            let mut invalid = descriptor.clone();
            let LayerPlacement::Anchor(ref mut policy) = invalid
                .semantics
                .as_mut()
                .unwrap()
                .layer
                .as_mut()
                .unwrap()
                .placement
            else {
                panic!()
            };
            if padding {
                policy.viewport_padding = value;
            } else {
                policy.gap = value;
            }
            assert!(invalid.validate().is_err());
            assert!(
                memory
                    .prepare_patch(id, &Patch::Semantics(Change::Set(invalid.semantics)))
                    .is_err()
            );
            assert_eq!(memory.node(id).unwrap().descriptor, descriptor);
            assert_eq!(memory.stats(), stats);
        }
    }
}

#[test]
fn named_anchor_loss_and_late_binding_use_current_ancestry_not_slot_or_history() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let anchor = ids.allocate();
    let panel = ids.allocate();
    let group = ids.allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id: panel,
                node: Box::new(layer()),
            },
            Mutation::Place {
                id: panel,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let loss = Event::Dismiss(DismissReason::AnchorLost);
    assert!(!memory.layer_anchor_live(panel));
    assert!(memory.validate_event(panel, &loss).unwrap());
    let mut button = Descriptor::new(Props::Button(Default::default()));
    button.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("anchor").unwrap()),
        ..Default::default()
    }));
    memory
        .apply(&[
            Mutation::Create {
                id: anchor,
                node: Box::new(button),
            },
            Mutation::Place {
                id: anchor,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert!(memory.layer_anchor_live(panel));
    assert!(!memory.validate_event(panel, &loss).unwrap());
    memory.apply(&[Mutation::Suspend { id: anchor }]).unwrap();
    assert!(!memory.layer_anchor_live(panel));
    memory
        .apply(&[Mutation::Resume {
            id: anchor,
            parent: None,
            before: None,
        }])
        .unwrap();
    assert!(memory.layer_anchor_live(panel));
    let mut hidden = Descriptor::new(Props::View);
    hidden.semantics = Some(Box::new(Semantics {
        hidden: true,
        ..Default::default()
    }));
    memory
        .apply(&[
            Mutation::Create {
                id: group,
                node: Box::new(hidden),
            },
            Mutation::Place {
                id: group,
                parent: None,
                before: None,
            },
            Mutation::Place {
                id: anchor,
                parent: Some(group),
                before: None,
            },
        ])
        .unwrap();
    assert!(!memory.layer_anchor_live(panel));
    memory
        .apply(&[
            Mutation::Place {
                id: anchor,
                parent: None,
                before: None,
            },
            Mutation::Dispose { id: group },
        ])
        .unwrap();
    assert!(memory.layer_anchor_live(panel));
    memory.apply(&[Mutation::Dispose { id: anchor }]).unwrap();
    assert!(!memory.layer_anchor_live(panel));
    let replacement = ids.allocate();
    memory
        .apply(&[
            Mutation::Create {
                id: replacement,
                node: Box::new(Descriptor::new(Props::Button(Default::default()))),
            },
            Mutation::Place {
                id: replacement,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert!(!memory.layer_anchor_live(panel));
}
