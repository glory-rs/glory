use glory_render::{memory::Memory, *};
fn node() -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.semantics = Some(Box::new(Semantics {
        translation: Some(TranslationMotion {
            kind: TranslationKind::To {
                target: TranslationOffset::new(20., -3.),
                duration_millis: 200,
            },
            paused: false,
        }),
        inert: true,
        ..Default::default()
    }));
    node
}
#[test]
fn translation_is_independent_inert_passive_view_and_all_incompatible_fields_reject() {
    node().validate().unwrap();
    for index in 0..10 {
        let mut bad = node();
        match index {
            0 => bad.props = Props::Group,
            1 => bad.props = Props::Text(TextProps::default()),
            2 => bad.semantics.as_mut().unwrap().inert = false,
            3 => bad.role = Some(Role::Group),
            4 => bad.name = Some("visual duplicate".into()),
            5 => bad.focus = FocusMode::TabStop,
            6 => bad.semantics.as_mut().unwrap().layer = Some(Layer::default()),
            7 => bad.semantics.as_mut().unwrap().collapse = Some(CollapseConfig::default()),
            8 => bad.semantics.as_mut().unwrap().drag_capture = Some(DragCapture::Press),
            9 => bad.semantics.as_mut().unwrap().document_part = Some(DocumentPart::Blockquote),
            _ => unreachable!(),
        };
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}
#[test]
fn prepared_rollback_clear_and_direct_apply_preflight_share_validation() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory.preflight(node().view(), None).unwrap();
    memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(node()),
        }])
        .unwrap();
    let before = memory.stats();
    let mut bad = node();
    bad.semantics
        .as_mut()
        .unwrap()
        .translation
        .as_mut()
        .unwrap()
        .kind = TranslationKind::To {
        target: TranslationOffset::new(f64::NAN, 0.),
        duration_millis: 200,
    };
    assert!(bad.validate().is_err());
    assert!(
        memory
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::Semantics(Change::Set(bad.semantics))
            }])
            .is_err()
    );
    assert_eq!(memory.stats(), before);
    {
        let _guard = memory
            .prepare_patch(id, &Patch::Semantics(Change::Clear))
            .unwrap();
    }
    assert!(
        memory
            .node(id)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .is_some()
    );
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Semantics(Change::Clear),
        }])
        .unwrap();
    assert!(
        memory
            .node(id)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .is_none()
    );
}
