use glory_render::memory::Memory;
use glory_render::*;

fn layer() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Dialog);
    d.focus = FocusMode::Programmatic;
    d.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("dialog").unwrap()),
        layer: Some(Layer::default()),
        ..Default::default()
    }));
    d
}
#[test]
fn layer_contract_rejects_invalid_targets_and_relations() {
    let d = layer();
    d.validate().unwrap();
    for patch in [Patch::Role(Change::Clear), Patch::Focus(Change::Clear)] {
        assert!(d.clone().patch(patch).is_err());
    }
    for props in [
        Props::Group,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
    ] {
        let mut invalid = d.clone();
        invalid.props = props;
        assert!(invalid.validate().is_err());
    }
    for (placement, anchor) in [
        (LayerPlacement::AnchorStart, None),
        (LayerPlacement::Center, Some("other")),
        (LayerPlacement::AnchorEnd, Some("dialog")),
    ] {
        let mut invalid = d.clone();
        let l = invalid.semantics.as_mut().unwrap().layer.as_mut().unwrap();
        l.placement = placement;
        l.anchor = anchor.map(|s| SemanticId::new(s).unwrap());
        assert!(invalid.validate().is_err());
    }
}
#[test]
fn prepared_layer_patch_rolls_back_cross_field_failure_and_stale_dismissals_are_ignored() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(layer()),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    let stats = tree.stats();
    assert!(tree.prepare_patch(id, &Patch::Role(Change::Clear)).is_err());
    assert_eq!(tree.node(id).unwrap().descriptor.role, Some(Role::Dialog));
    assert_eq!(tree.stats(), stats);
    Event::Dismiss(DismissReason::Escape)
        .validate_payload(id, &tree.node(id).unwrap().descriptor)
        .unwrap();
    let mut metadata = tree.node(id).unwrap().descriptor.semantics.clone().unwrap();
    metadata.layer.as_mut().unwrap().escape = false;
    tree.prepare_patch(id, &Patch::Semantics(Change::Set(Some(metadata))))
        .unwrap()
        .commit();
    assert!(
        Event::Dismiss(DismissReason::Escape)
            .validate_payload(id, &tree.node(id).unwrap().descriptor)
            .is_err()
    );
    Event::Dismiss(DismissReason::Outside)
        .validate_payload(id, &tree.node(id).unwrap().descriptor)
        .unwrap();
    tree.apply(&[Mutation::Dispose { id }]).unwrap();
    assert!(
        tree.validate_event(id, &Event::Dismiss(DismissReason::Outside))
            .is_err()
    );
}
