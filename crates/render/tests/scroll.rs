use glory_render::*;

#[test]
fn metrics_validate_every_dimension_preserve_negative_offsets_and_allow_small_content() {
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    let metrics = ScrollMetrics {
        x: -50.0,
        y: -2.0,
        viewport_width: 400.0,
        viewport_height: 200.0,
        content_width: 20.0,
        content_height: 10.0,
    };
    let query = Query::ScrollMetrics(id);
    query
        .validate_response(&QueryValue::ScrollMetrics(metrics))
        .unwrap();
    assert!(
        query
            .validate_response(&QueryValue::Scroll { x: 0.0, y: 0.0 })
            .is_err()
    );
    assert!(
        Query::Scroll(id)
            .validate_response(&QueryValue::ScrollMetrics(metrics))
            .is_err()
    );
    for index in 0..6 {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut bad = metrics;
            let values = [
                &mut bad.x,
                &mut bad.y,
                &mut bad.viewport_width,
                &mut bad.viewport_height,
                &mut bad.content_width,
                &mut bad.content_height,
            ];
            *values.into_iter().nth(index).unwrap() = invalid;
            assert!(
                query
                    .validate_response(&QueryValue::ScrollMetrics(bad))
                    .is_err()
            );
        }
    }
    for index in 0..4 {
        let mut bad = metrics;
        let values = [
            &mut bad.viewport_width,
            &mut bad.viewport_height,
            &mut bad.content_width,
            &mut bad.content_height,
        ];
        *values.into_iter().nth(index).unwrap() = -1.0;
        assert!(
            query
                .validate_response(&QueryValue::ScrollMetrics(bad))
                .is_err()
        );
    }
    query
        .validate_target(&Descriptor::new(Props::View))
        .unwrap();
    assert!(
        query
            .validate_target(&Descriptor::new(Props::Group))
            .is_err()
    );
}

#[test]
fn viewport_key_mapping_is_independent_from_collection_navigation_and_editing() {
    for axes in [
        ScrollAxes::Both,
        ScrollAxes::Horizontal,
        ScrollAxes::Vertical,
    ] {
        for key in [
            "ArrowLeft",
            "ArrowRight",
            "ArrowUp",
            "ArrowDown",
            "PageUp",
            "PageDown",
            "Home",
            "End",
        ] {
            let input = KeyboardInput {
                key,
                ..Default::default()
            };
            let command = axes.key(input);
            if let Some(command) = command {
                assert!(axes.allows(command.axis));
            }
            assert!(
                axes.key(KeyboardInput {
                    editable: true,
                    ..input
                })
                .is_none()
            );
            assert!(
                axes.key(KeyboardInput {
                    composing: true,
                    ..input
                })
                .is_none()
            );
            assert!(
                axes.key(KeyboardInput {
                    control: true,
                    ..input
                })
                .is_none()
            );
            assert!(
                axes.key(KeyboardInput {
                    meta: true,
                    ..input
                })
                .is_none()
            );
            assert!(axes.key(KeyboardInput { alt: true, ..input }).is_none());
            assert!(
                axes.key(KeyboardInput {
                    shift: true,
                    ..input
                })
                .is_none()
            );
            assert_eq!(
                axes.key(KeyboardInput {
                    repeat: true,
                    ..input
                }),
                command
            );
        }
        for key in ["Tab", " ", "Enter", "a"] {
            assert!(
                axes.key(KeyboardInput {
                    key,
                    ..Default::default()
                })
                .is_none()
            );
        }
    }
    assert_eq!(
        ScrollAxes::Both
            .key(KeyboardInput {
                key: "PageDown",
                ..Default::default()
            })
            .unwrap(),
        ScrollCommand {
            axis: Orientation::Vertical,
            movement: ScrollMovement::PagePositive
        }
    );
    assert_eq!(
        ScrollAxes::Horizontal
            .key(KeyboardInput {
                key: "End",
                ..Default::default()
            })
            .unwrap()
            .axis,
        Orientation::Horizontal
    );
    assert!(
        Orientation::Vertical
            .key(KeyboardInput {
                key: "PageDown",
                ..Default::default()
            })
            .is_none()
    );
}

#[test]
fn scroll_keyboard_requires_an_independent_view_and_its_exact_physical_origin_axis() {
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    let other = ids.allocate();
    let mut node = Descriptor::new(Props::View);
    node.semantics = Some(Box::new(Semantics {
        scroll_keyboard: Some(ScrollAxes::Vertical),
        ..Default::default()
    }));
    node.validate().unwrap();
    let command = ScrollCommand {
        axis: Orientation::Vertical,
        movement: ScrollMovement::End,
    };
    Event::ScrollKeyboard { from: id, command }
        .validate_payload(id, &node)
        .unwrap();
    assert!(
        Event::ScrollKeyboard {
            from: other,
            command
        }
        .validate_payload(id, &node)
        .is_err()
    );
    assert!(
        Event::ScrollKeyboard {
            from: id,
            command: ScrollCommand {
                axis: Orientation::Horizontal,
                ..command
            }
        }
        .validate_payload(id, &node)
        .is_err()
    );
    assert!(
        Event::ScrollKeyboard { from: id, command }
            .validate_payload(id, &Descriptor::new(Props::View))
            .is_err()
    );
    for props in [
        Props::Group,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
        Props::Check(CheckProps::default()),
    ] {
        let mut invalid = Descriptor::new(props);
        invalid.semantics = node.semantics.clone();
        assert!(invalid.validate().is_err());
    }
    for field in [
        Semantics {
            navigation: Some(Orientation::Vertical),
            ..Default::default()
        },
        Semantics {
            layer: Some(Layer::default()),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Form),
            ..Default::default()
        },
    ] {
        let mut invalid = node.clone();
        let mut semantics = field;
        semantics.scroll_keyboard = Some(ScrollAxes::Vertical);
        invalid.semantics = Some(Box::new(semantics));
        assert!(invalid.validate().is_err());
    }
}
