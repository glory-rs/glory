use glory_render::{memory::Memory, style::*, *};

fn node(open: bool, revision: u64) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.semantics = Some(Box::new(Semantics {
        collapse: Some(CollapseConfig {
            open,
            duration_millis: 240,
            revision,
        }),
        hidden: !open,
        inert: !open,
        ..Default::default()
    }));
    node
}
fn patch(open: bool, revision: u64) -> Patch {
    Patch::Semantics(Change::Set(node(open, revision).semantics))
}

#[test]
fn collapse_is_a_finite_independent_view_with_immediate_hidden_and_inert_state() {
    for open in [false, true] {
        for duration in [0, 1, 500] {
            let mut descriptor = node(open, 7);
            descriptor
                .semantics
                .as_mut()
                .unwrap()
                .collapse
                .as_mut()
                .unwrap()
                .duration_millis = duration;
            descriptor.validate().unwrap();
        }
        for index in 0..7 {
            let mut descriptor = node(open, 7);
            match index {
                0 => descriptor.props = Props::Text(TextProps::default()),
                1 => descriptor.semantics.as_mut().unwrap().hidden = open,
                2 => descriptor.semantics.as_mut().unwrap().inert = open,
                3 => descriptor.semantics.as_mut().unwrap().layer = Some(Layer::default()),
                4 => {
                    descriptor.semantics.as_mut().unwrap().document_part =
                        Some(DocumentPart::Blockquote)
                }
                5 => descriptor.semantics.as_mut().unwrap().field_part = Some(FieldPart::Fieldset),
                6 => {
                    descriptor
                        .semantics
                        .as_mut()
                        .unwrap()
                        .collapse
                        .as_mut()
                        .unwrap()
                        .duration_millis = 501
                }
                _ => unreachable!(),
            }
            assert!(descriptor.validate().is_err());
        }
    }
    assert!(
        !Memory::new(RuntimeId::new())
            .capabilities()
            .collapse_completion
    );
}

#[test]
fn every_incompatible_outer_geometry_declaration_is_independently_rejected() {
    let mut styles = vec![
        Style::new().height(Length::Auto),
        Style::new().min_height(Length::Px(0.)),
        Style::new().max_height(Limit::None),
        Style::new().padding(Edges::all(1.)),
        Style {
            grow: Some(1.),
            ..Default::default()
        },
        Style::new().position(Position::Absolute),
        Style::new().display(Display::Flex),
        Style::new().overflow_x(Overflow::Auto),
        Style::new().overflow_y(Overflow::Visible),
    ];
    let mut ratio = Style::new();
    ratio.aspect_ratio = Some(AspectRatio::Ratio(1.));
    styles.push(ratio);
    let mut border = Style::new();
    border.border_width = Some(Edges::all(1.));
    styles.push(border);
    let mut shadow = Style::new();
    shadow.box_shadow = Some(BoxShadow::Outset(Shadow {
        x: 1.,
        y: 1.,
        blur: 2.,
        spread: 0.,
        color: Color(0, 0, 0, 255),
    }));
    styles.push(shadow);
    for style in styles {
        let mut descriptor = node(false, 3);
        descriptor.style = style;
        assert!(descriptor.validate().is_err(), "{descriptor:?}");
    }
    let mut descriptor = node(false, 3);
    descriptor.style = Style::new()
        .padding(Edges::all(0.))
        .display(Display::Block)
        .overflow_y(Overflow::Hidden)
        .width(Length::Percent(100.));
    descriptor.validate().unwrap();
}

#[test]
fn raw_live_and_direct_revision_rejection_roll_back_without_stats_or_watermark_changes() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(node(true, 4)),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    for bad in [
        patch(false, 3),
        patch(false, 4),
        Patch::Semantics(Change::Clear),
        Patch::Style(Change::Set(Box::new(Style::new().height(Length::Px(24.))))),
    ] {
        let before = memory.node(id).unwrap().descriptor.view().to_owned();
        let stats = memory.stats();
        let receipt = memory.receipt();
        assert!(before.patched(bad.clone()).is_err());
        assert!(memory.prepare_patch(id, &bad).is_err());
        assert!(memory.apply(&[Mutation::Patch { id, patch: bad }]).is_err());
        assert_eq!(memory.node(id).unwrap().descriptor, before);
        assert_eq!(memory.stats(), stats);
        assert_eq!(memory.receipt(), receipt);
    }
    {
        let prepared = memory.prepare_patch(id, &patch(false, 5)).unwrap();
        assert!(prepared.descriptor().semantics.as_ref().unwrap().hidden);
    }
    assert_eq!(memory.node(id).unwrap().descriptor, node(true, 4));
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: patch(false, 5),
        }])
        .unwrap();
    assert!(!memory.active(id));
    assert!(memory.inert(id));
    assert!(
        !memory
            .validate_event(id, &Event::CollapseComplete { revision: 4 })
            .unwrap()
    );
    assert!(
        memory
            .validate_event(id, &Event::CollapseComplete { revision: 5 })
            .unwrap()
    );
    assert!(matches!(
        memory.validate_event(id, &Event::CollapseComplete { revision: 6 }),
        Err(Error::CollapseRevision {
            current: 5,
            incoming: 6
        })
    ));
    assert!(Event::CollapseComplete { revision: 5 }.is_terminal_loss());
    assert!(!Event::CollapseComplete { revision: 5 }.is_user_interaction());
    memory
        .push_event(id, Event::CollapseComplete { revision: 5 })
        .unwrap();
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert_eq!(
        inputs,
        [Input::Event {
            node: id,
            event: Event::CollapseComplete { revision: 5 }
        }]
    );
    memory.apply(&[Mutation::Suspend { id }]).unwrap();
    assert!(
        memory
            .validate_event(id, &Event::CollapseComplete { revision: 5 })
            .unwrap()
    );
    memory.apply(&[Mutation::Dispose { id }]).unwrap();
    assert!(matches!(
        memory.validate_event(id, &Event::CollapseComplete { revision: 5 }),
        Err(Error::NodeGone(_))
    ));
}

#[test]
fn direct_batches_preserve_a_successful_prefix_and_pending_query_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let bad = ids.allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(node(true, 0)),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    memory.request(Query::Bounds(id), QueryToken(1)).unwrap();
    let query_receipt = memory.receipt();
    let mut invalid = node(false, 0);
    invalid.style = Style::new().padding(Edges::all(1.));
    assert!(
        memory
            .apply(&[
                Mutation::Patch {
                    id,
                    patch: patch(false, 1)
                },
                Mutation::Create {
                    id: bad,
                    node: Box::new(invalid)
                }
            ])
            .is_err()
    );
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(inputs.iter().any(|input|matches!(input,Input::Response{token:QueryToken(1),applied,..}if *applied==query_receipt.applied)));
    assert!(memory.receipt().applied > query_receipt.applied);
}

#[cfg(feature = "conformance")]
#[test]
fn reference_runs_the_optional_adapter_collapse_matrix() {
    let runtime = RuntimeId::new();
    conformance::verify_collapse(Memory::new(runtime), runtime, true);
}
