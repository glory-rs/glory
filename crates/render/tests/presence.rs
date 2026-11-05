use glory_render::{memory::Memory, style::*, *};
#[test]
fn custom_closed_styles_validate_each_independent_finite_field() {
    for index in 0..10 {
        let mut style = PresenceStyle::default();
        match index {
            0 => style.opacity = 1001,
            1 => style.scale_x = 2001,
            2 => style.scale_y = 2001,
            3 => style.translate_x = -201,
            4 => style.translate_y = 201,
            5 => style.rotate = -181,
            6 => style.skew_x = 46,
            7 => style.skew_y = -46,
            8 => style.origin_x = 101,
            9 => style.origin_y = 101,
            _ => unreachable!(),
        }
        let config = PresenceConfig {
            effect: PresenceEffect::Custom(style),
            ..Default::default()
        };
        assert!(config.validate().is_err(), "{index}");
    }
    PresenceConfig {
        effect: PresenceEffect::Custom(PresenceStyle {
            opacity: 1000,
            scale_x: 2000,
            scale_y: 0,
            translate_x: -200,
            translate_y: 200,
            rotate: -180,
            skew_x: 45,
            skew_y: -45,
            origin_x: 0,
            origin_y: 100,
        }),
        ..Default::default()
    }
    .validate()
    .unwrap();
}
fn node(open: bool, revision: u64) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.semantics = Some(Box::new(Semantics {
        hidden: !open,
        inert: !open,
        presence: Some(Box::new(PresenceConfig {
            open,
            duration_millis: 200,
            distance: 16,
            revision,
            ..Default::default()
        })),
        ..Default::default()
    }));
    node
}
fn patch(open: bool, revision: u64) -> Patch {
    Patch::Semantics(Change::Set(node(open, revision).semantics))
}
#[test]
fn independent_finite_presence_and_each_owned_presentation_field_are_checked() {
    for open in [false, true] {
        node(open, 0).validate().unwrap();
        for index in 0..9 {
            let mut d = node(open, 0);
            let s = d.semantics.as_mut().unwrap();
            match index {
                0 => s.hidden = open,
                1 => s.inert = open,
                2 => {
                    s.collapse = Some(CollapseConfig {
                        open,
                        ..Default::default()
                    })
                }
                3 => s.presence.as_mut().unwrap().duration_millis = 2001,
                4 => s.presence.as_mut().unwrap().delay_millis = 2001,
                5 => s.presence.as_mut().unwrap().distance = 201,
                6 => d.style.opacity = Some(0.5),
                7 => d.style.display = Some(Display::Flex),
                8 => d.props = Props::Text(TextProps::default()),
                _ => unreachable!(),
            }
            assert!(d.validate().is_err(), "{index}");
        }
    }
}
#[test]
fn rejected_revision_guards_and_direct_batches_preserve_fields_prefix_and_watermarks() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let bad = ids.allocate();
    let mut m = Memory::new(runtime);
    m.apply(&[
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
    for patch in [
        patch(false, 3),
        patch(false, 4),
        Patch::Semantics(Change::Clear),
        Patch::Style(Change::Set(Box::new(Style::new().opacity(0.5)))),
    ] {
        let before = m.node(id).unwrap().descriptor.view().to_owned();
        let stats = m.stats();
        let receipt = m.receipt();
        assert!(before.patched(patch.clone()).is_err());
        assert!(m.prepare_patch(id, &patch).is_err());
        assert!(m.apply(&[Mutation::Patch { id, patch }]).is_err());
        assert_eq!(m.node(id).unwrap().descriptor, before);
        assert_eq!(m.stats(), stats);
        assert_eq!(m.receipt(), receipt);
    }
    {
        let _guard = m.prepare_patch(id, &patch(false, 5)).unwrap();
    }
    assert!(m.active(id));
    m.request(Query::Bounds(id), QueryToken(1)).unwrap();
    let receipt = m.receipt();
    let mut invalid = node(false, 0);
    invalid.style.opacity = Some(0.5);
    assert!(
        m.apply(&[
            Mutation::Patch {
                id,
                patch: patch(false, 5)
            },
            Mutation::Create {
                id: bad,
                node: Box::new(invalid)
            }
        ])
        .is_err()
    );
    assert!(!m.active(id) && m.inert(id));
    assert!(m.node(bad).is_err());
    let mut inputs = Vec::new();
    m.drain(&mut inputs);
    assert!(
        inputs
            .iter()
            .any(|i| matches!(i,Input::Response{applied,..}if *applied==receipt.applied))
    );
    assert!(
        !m.validate_event(id, &Event::PresenceComplete { revision: 4 })
            .unwrap()
    );
    assert!(
        m.validate_event(id, &Event::PresenceComplete { revision: 5 })
            .unwrap()
    );
    assert!(matches!(
        m.validate_event(id, &Event::PresenceComplete { revision: 6 }),
        Err(Error::PresenceRevision { .. })
    ));
    assert!(!Event::PresenceComplete { revision: 5 }.is_user_interaction());
    m.apply(&[Mutation::Suspend { id }]).unwrap();
    assert!(
        m.validate_event(id, &Event::PresenceComplete { revision: 5 })
            .unwrap()
    );
    m.apply(&[Mutation::Dispose { id }]).unwrap();
    assert!(
        m.validate_event(id, &Event::PresenceComplete { revision: 5 })
            .is_err()
    );
}
