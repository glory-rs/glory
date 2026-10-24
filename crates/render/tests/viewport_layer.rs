use glory_render::*;

#[test]
fn viewport_policy_validates_every_field_and_resolves_without_an_anchor() {
    let policy = ViewportPlacement::default();
    let panel = Rect {
        width: 100.,
        height: 80.,
        ..Default::default()
    };
    let viewport = Rect {
        width: 320.,
        height: 240.,
        ..Default::default()
    };
    assert_eq!(
        policy.resolve(panel, viewport).unwrap(),
        Rect {
            x: 48.,
            y: 48.,
            ..panel
        }
    );
    let shifted = ViewportPlacement {
        x: 500.,
        y: -100.,
        ..policy
    }
    .resolve(panel, viewport)
    .unwrap();
    assert_eq!((shifted.x, shifted.y), (212., 8.));
    let direct = ViewportPlacement {
        x: 500.,
        y: -100.,
        constrain: false,
        ..policy
    }
    .resolve(panel, viewport)
    .unwrap();
    assert_eq!((direct.x, direct.y), (500., -100.));
    let oversized = policy
        .resolve(
            Rect {
                width: 400.,
                height: 400.,
                ..panel
            },
            viewport,
        )
        .unwrap();
    assert_eq!((oversized.x, oversized.y), (8., 8.));
    for bad in [
        ViewportPlacement {
            x: f64::NAN,
            ..policy
        },
        ViewportPlacement {
            y: 1_000_001.,
            ..policy
        },
        ViewportPlacement {
            viewport_padding: -1.,
            ..policy
        },
        ViewportPlacement {
            viewport_padding: 4097.,
            ..policy
        },
    ] {
        assert!(bad.validate().is_err());
    }
    assert!(
        policy
            .resolve(
                Rect {
                    width: -1.,
                    ..panel
                },
                viewport
            )
            .is_err()
    );
    assert!(
        policy
            .resolve(
                panel,
                Rect {
                    x: f64::MAX,
                    width: f64::MAX,
                    ..viewport
                }
            )
            .is_err()
    );
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.role = Some(Role::Dialog);
    descriptor.focus = FocusMode::Programmatic;
    descriptor.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("viewport-layer").unwrap()),
        layer: Some(Layer {
            placement: LayerPlacement::Viewport(policy),
            ..Default::default()
        }),
        ..Default::default()
    }));
    descriptor.validate().unwrap();
    descriptor
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .anchor = Some(SemanticId::new("wrong-anchor").unwrap());
    assert!(descriptor.validate().is_err());
    descriptor
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .anchor = None;
    descriptor.role = Some(Role::Tooltip);
    assert!(descriptor.validate().is_err());
}
