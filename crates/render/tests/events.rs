use glory_render::memory::Memory;
use glory_render::*;

fn add(memory: &mut Memory, ids: &mut NodeIds, props: Props, parent: Option<NodeId>) -> NodeId {
    let id = ids.allocate();
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(props)),
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
fn payload_geometry_and_single_line_values_are_checked_without_rejecting_negative_native_offsets() {
    let id = NodeIds::new(RuntimeId::new()).allocate();
    let node = Descriptor::new(Props::View);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for event in [
            Event::Scroll {
                x: value,
                y: 0.,
                viewport: Rect::default(),
                ack: None,
            },
            Event::Scroll {
                x: 0.,
                y: 0.,
                viewport: Rect {
                    height: value,
                    ..Rect::default()
                },
                ack: None,
            },
            Event::Resize {
                size: Rect {
                    width: value,
                    ..Rect::default()
                },
                viewport: Rect::default(),
            },
            Event::Resize {
                size: Rect::default(),
                viewport: Rect {
                    y: value,
                    ..Rect::default()
                },
            },
            Event::Anchor {
                row: id,
                within: value,
                viewport: Rect::default(),
            },
            Event::Scroll {
                x: 0.,
                y: 0.,
                viewport: Rect::default(),
                ack: Some(ScrollAck {
                    requested: ScrollRequest { x: 0., y: value },
                    x: 0.,
                    y: 0.,
                }),
            },
            Event::Scroll {
                x: 0.,
                y: 0.,
                viewport: Rect::default(),
                ack: Some(ScrollAck {
                    requested: ScrollRequest { x: 0., y: 0. },
                    x: value,
                    y: 0.,
                }),
            },
        ] {
            assert!(
                matches!(
                    event.validate_payload(id, &node),
                    Err(Error::InvalidEvent(_))
                ),
                "{event:?}"
            );
        }
    }
    for event in [
        Event::Resize {
            size: Rect {
                width: -1.,
                ..Rect::default()
            },
            viewport: Rect::default(),
        },
        Event::Resize {
            size: Rect {
                x: 1.,
                ..Rect::default()
            },
            viewport: Rect::default(),
        },
        Event::Anchor {
            row: id,
            within: -1.,
            viewport: Rect::default(),
        },
        Event::Scroll {
            x: 0.,
            y: 0.,
            viewport: Rect {
                x: -1.,
                ..Rect::default()
            },
            ack: None,
        },
        Event::Scroll {
            x: 0.,
            y: 0.,
            viewport: Rect::default(),
            ack: Some(ScrollAck {
                requested: ScrollRequest { x: -1., y: 0. },
                x: 0.,
                y: 0.,
            }),
        },
    ] {
        assert!(matches!(
            event.validate_payload(id, &node),
            Err(Error::InvalidEvent(_))
        ));
    }
    Event::Scroll {
        x: -3.5,
        y: -1.,
        viewport: Rect::default(),
        ack: Some(ScrollAck {
            requested: ScrollRequest { x: 0., y: 0. },
            x: -2.,
            y: -1.,
        }),
    }
    .validate_payload(id, &node)
    .unwrap();
    let single = Descriptor::new(Props::Input(InputProps::default()));
    let multi = Descriptor::new(Props::Input(InputProps {
        multiline: true,
        ..Default::default()
    }));
    for value in ["line\nbreak", "line\rbreak", "line\r\nbreak"] {
        for event in [
            Event::Commit {
                value: value.into(),
                revision: 0,
            },
            Event::ValueChanged {
                value: value.into(),
                composing: true,
                revision: 0,
            },
        ] {
            assert!(matches!(
                event.validate_payload(id, &single),
                Err(Error::InvalidEvent(_))
            ));
            if value.contains('\r') {
                assert!(matches!(
                    event.validate_payload(id, &multi),
                    Err(Error::InvalidEvent(_))
                ));
            } else {
                event.validate_payload(id, &multi).unwrap();
            }
        }
    }
}

#[test]
fn embedded_origins_require_live_boxed_descendants_and_never_alias_reused_slots() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = add(&mut memory, &mut ids, Props::View, None);
    let group = add(&mut memory, &mut ids, Props::Group, Some(root));
    let row = add(&mut memory, &mut ids, Props::View, Some(group));
    let outside = add(&mut memory, &mut ids, Props::View, None);
    let anchor = |row| Event::Anchor {
        row,
        within: 7.5,
        viewport: Rect::default(),
    };
    assert_eq!(memory.validate_event(root, &anchor(row)), Ok(true));
    for source in [root, outside, group] {
        assert!(matches!(
            memory.validate_event(root, &anchor(source)),
            Err(Error::InvalidEvent(_))
        ));
    }
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        memory.validate_event(root, &anchor(foreign)),
        Err(Error::ForeignRuntime)
    );
    assert!(matches!(
        memory.validate_event(
            root,
            &Event::Navigate {
                from: outside,
                command: Navigation::First
            }
        ),
        Err(Error::InvalidEvent(_))
    ));
    memory.apply(&[Mutation::Suspend { id: row }]).unwrap();
    assert_eq!(memory.validate_event(root, &anchor(row)), Ok(false));
    assert_eq!(
        memory.validate_event(
            root,
            &Event::FocusWithin {
                node: row,
                focused: false
            }
        ),
        Ok(true)
    );
    assert_eq!(
        memory.validate_event(
            root,
            &Event::FocusWithin {
                node: row,
                focused: true
            }
        ),
        Ok(false)
    );
    memory.apply(&[Mutation::Dispose { id: row }]).unwrap();
    assert!(ids.release(row));
    let replacement = add(&mut memory, &mut ids, Props::View, Some(group));
    assert_eq!(replacement.slot(), row.slot());
    assert_ne!(replacement, row);
    assert_eq!(memory.validate_event(root, &anchor(row)), Ok(false));
    assert_eq!(memory.validate_event(root, &anchor(replacement)), Ok(true));
    assert!(matches!(
        memory.validate_event(
            root,
            &Event::Navigate {
                from: row,
                command: Navigation::First
            }
        ),
        Err(Error::InvalidEvent(_))
    ));
}

#[test]
fn queued_origins_are_rechecked_after_moves_and_suspension_preserves_terminal_cleanup() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = add(&mut memory, &mut ids, Props::View, None);
    let outside = add(&mut memory, &mut ids, Props::View, None);
    let source = add(
        &mut memory,
        &mut ids,
        Props::Input(InputProps::default()),
        Some(root),
    );
    memory
        .push_event(
            root,
            Event::FocusWithin {
                node: source,
                focused: true,
            },
        )
        .unwrap();
    memory
        .apply(&[Mutation::Place {
            id: source,
            parent: Some(outside),
            before: None,
        }])
        .unwrap();
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(inputs.is_empty());
    memory
        .apply(&[Mutation::Place {
            id: source,
            parent: Some(root),
            before: None,
        }])
        .unwrap();
    memory
        .push_event(source, Event::FocusChanged(false))
        .unwrap();
    memory
        .push_event(source, Event::Composition { active: false })
        .unwrap();
    memory.apply(&[Mutation::Suspend { id: source }]).unwrap();
    memory.drain(&mut inputs);
    assert_eq!(inputs.len(), 2);
    inputs.clear();
    memory
        .push_event(
            root,
            Event::FocusWithin {
                node: source,
                focused: false,
            },
        )
        .unwrap();
    memory.apply(&[Mutation::Dispose { id: source }]).unwrap();
    memory.drain(&mut inputs);
    assert!(inputs.is_empty());
}
