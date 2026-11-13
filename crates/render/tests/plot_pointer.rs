use glory_render::{memory::Memory, *};

fn point(phase: PlotPointerPhase) -> PlotPointer {
    PlotPointer {
        pointer_id: 7,
        device: PlotPointerDevice::Pen,
        phase,
        x: 12.375,
        y: -0.125,
        viewport: Rect {
            width: 480.25,
            height: 300.5,
            ..Rect::default()
        },
    }
}

fn add(memory: &mut Memory, ids: &mut NodeIds, parent: Option<NodeId>) -> NodeId {
    let id = ids.allocate();
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id,
                parent,
                before: None,
            },
        ])
        .unwrap();
    id
}

#[test]
fn fractional_local_coordinates_and_all_devices_preserve_exact_payload() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let id = add(&mut memory, &mut ids, None);
    assert!(memory.supports_plot_pointer());
    for device in [
        PlotPointerDevice::Mouse,
        PlotPointerDevice::Pen,
        PlotPointerDevice::Touch,
    ] {
        for phase in [
            PlotPointerPhase::Move,
            PlotPointerPhase::Down,
            PlotPointerPhase::Up,
            PlotPointerPhase::Leave,
            PlotPointerPhase::Cancel,
        ] {
            let event = Event::PlotPointer {
                from: id,
                pointer: PlotPointer {
                    device,
                    ..point(phase)
                },
            };
            memory.push_event(id, event.clone()).unwrap();
            let mut drained = vec![];
            memory.drain(&mut drained);
            assert_eq!(drained, [Input::Event { node: id, event }]);
        }
    }
}

#[test]
fn malformed_geometry_is_rejected_without_queued_prefix_or_state_change() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let id = add(&mut memory, &mut ids, None);
    let valid = point(PlotPointerPhase::Move);
    let mut invalid = vec![];
    for scalar in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        MAX_PLOT_POINTER_COORDINATE + 1.,
    ] {
        invalid.push(PlotPointer { x: scalar, ..valid });
        invalid.push(PlotPointer { y: scalar, ..valid });
        invalid.push(PlotPointer {
            viewport: Rect {
                width: scalar,
                ..valid.viewport
            },
            ..valid
        });
    }
    for viewport in [
        Rect {
            x: 1.,
            ..valid.viewport
        },
        Rect {
            y: -1.,
            ..valid.viewport
        },
        Rect {
            width: -1.,
            ..valid.viewport
        },
        Rect {
            height: 0.,
            ..valid.viewport
        },
    ] {
        invalid.push(PlotPointer { viewport, ..valid });
    }
    for pointer in invalid {
        let before = memory.stats();
        assert!(matches!(
            memory.push_event(id, Event::PlotPointer { from: id, pointer }),
            Err(Error::InvalidEvent(_))
        ));
        let mut drained = vec![];
        memory.drain(&mut drained);
        assert!(drained.is_empty());
        assert_eq!(memory.stats(), before);
    }
    PlotPointer {
        phase: PlotPointerPhase::Cancel,
        viewport: Rect::default(),
        ..valid
    }
    .validate()
    .unwrap();
}

#[test]
fn exact_viewport_origin_rejects_child_other_and_foreign_identities() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = add(&mut memory, &mut ids, None);
    let child = add(&mut memory, &mut ids, Some(root));
    let other = add(&mut memory, &mut ids, None);
    for from in [child, other] {
        assert!(matches!(
            memory.validate_event(
                root,
                &Event::PlotPointer {
                    from,
                    pointer: point(PlotPointerPhase::Move)
                }
            ),
            Err(Error::InvalidEvent(_))
        ));
    }
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        memory.validate_event(
            root,
            &Event::PlotPointer {
                from: foreign,
                pointer: point(PlotPointerPhase::Move)
            }
        ),
        Err(Error::ForeignRuntime)
    );
    for props in [
        Props::Group,
        Props::Button(Default::default()),
        Props::Text(Default::default()),
    ] {
        assert!(
            EventKind::PlotPointer
                .validate_target(&Descriptor::new(props))
                .is_err()
        );
    }
}

#[test]
fn inherited_unavailability_rechecks_queue_and_keeps_terminal_cleanup_only() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = add(&mut memory, &mut ids, None);
    let child = add(&mut memory, &mut ids, Some(root));
    memory
        .push_event(
            child,
            Event::PlotPointer {
                from: child,
                pointer: point(PlotPointerPhase::Move),
            },
        )
        .unwrap();
    memory
        .apply(&[Mutation::Patch {
            id: root,
            patch: Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                inert: true,
                ..Default::default()
            })))),
        }])
        .unwrap();
    let mut drained = vec![];
    memory.drain(&mut drained);
    assert!(drained.is_empty());
    for phase in [
        PlotPointerPhase::Move,
        PlotPointerPhase::Down,
        PlotPointerPhase::Up,
    ] {
        assert_eq!(
            memory.validate_event(
                child,
                &Event::PlotPointer {
                    from: child,
                    pointer: point(phase)
                }
            ),
            Ok(false)
        );
    }
    for phase in [PlotPointerPhase::Leave, PlotPointerPhase::Cancel] {
        assert_eq!(
            memory.validate_event(
                child,
                &Event::PlotPointer {
                    from: child,
                    pointer: point(phase)
                }
            ),
            Ok(true)
        );
    }
    memory.apply(&[Mutation::Suspend { id: root }]).unwrap();
    assert_eq!(
        memory.validate_event(
            child,
            &Event::PlotPointer {
                from: child,
                pointer: point(PlotPointerPhase::Move)
            }
        ),
        Ok(false)
    );
    assert_eq!(
        memory.validate_event(
            child,
            &Event::PlotPointer {
                from: child,
                pointer: point(PlotPointerPhase::Cancel)
            }
        ),
        Ok(true)
    );
    memory.apply(&[Mutation::Dispose { id: child }]).unwrap();
    assert!(
        memory
            .validate_event(
                child,
                &Event::PlotPointer {
                    from: child,
                    pointer: point(PlotPointerPhase::Cancel)
                }
            )
            .is_err()
    );
}

#[test]
fn plot_disabled_fieldset_views_and_ancestors_block_input_but_keep_cleanup_and_recovery() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = add(&mut memory, &mut ids, None);
    let child = add(&mut memory, &mut ids, Some(root));
    for blocked in [root, child] {
        memory
            .push_event(
                child,
                Event::PlotPointer {
                    from: child,
                    pointer: point(PlotPointerPhase::Move),
                },
            )
            .unwrap();
        memory
            .apply(&[Mutation::Patch {
                id: blocked,
                patch: Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    field_part: Some(FieldPart::Fieldset),
                    disabled: true,
                    ..Default::default()
                })))),
            }])
            .unwrap();
        assert_eq!(
            memory.validate_event(
                child,
                &Event::PlotPointer {
                    from: child,
                    pointer: point(PlotPointerPhase::Move)
                }
            ),
            Ok(false)
        );
        let mut drained = vec![];
        memory.drain(&mut drained);
        assert!(drained.is_empty());
        for phase in [PlotPointerPhase::Leave, PlotPointerPhase::Cancel] {
            assert_eq!(
                memory.validate_event(
                    child,
                    &Event::PlotPointer {
                        from: child,
                        pointer: point(phase)
                    }
                ),
                Ok(true)
            );
        }
        memory
            .apply(&[Mutation::Patch {
                id: blocked,
                patch: Patch::Semantics(Change::Clear),
            }])
            .unwrap();
        assert_eq!(
            memory.validate_event(
                child,
                &Event::PlotPointer {
                    from: child,
                    pointer: point(PlotPointerPhase::Move)
                }
            ),
            Ok(true)
        );
    }
}

#[test]
fn invalid_plain_view_disabled_declaration_keeps_the_accepted_input_queue_and_descriptor() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let id = add(&mut memory, &mut ids, None);
    let event = Event::PlotPointer {
        from: id,
        pointer: point(PlotPointerPhase::Down),
    };
    memory.push_event(id, event.clone()).unwrap();
    let before = memory.node(id).unwrap().descriptor.to_owned();
    let stats = memory.stats();
    assert!(matches!(
        memory.apply(&[Mutation::Patch {
            id,
            patch: Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                disabled: true,
                ..Default::default()
            })))),
        }]),
        Err(Error::InvalidProps(_))
    ));
    assert_eq!(memory.node(id).unwrap().descriptor, before);
    assert_eq!(memory.stats(), stats);
    let mut drained = vec![];
    memory.drain(&mut drained);
    assert_eq!(drained, [Input::Event { node: id, event }]);
}
