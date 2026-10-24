use glory_render::Render;
use glory_render::memory::Memory;
use glory_render::{
    Change, Descriptor, FocusMode, Layer, LayerFocus, Mutation, NodeIds, Patch, Props, Role,
    RuntimeId, SemanticId, Semantics,
};
fn alert() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::AlertDialog);
    d.focus = FocusMode::Programmatic;
    d.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("decision").unwrap()),
        layer: Some(Layer {
            initial_focus: Some(SemanticId::new("cancel").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    d
}
#[test]
fn alert_layer_requires_managed_modality_and_an_explicit_initial_action() {
    let d = alert();
    d.validate().unwrap();
    for layer in [
        Layer {
            modal: false,
            ..d.semantics.as_ref().unwrap().layer.clone().unwrap()
        },
        Layer {
            initial_focus: None,
            ..Default::default()
        },
        Layer {
            focus: LayerFocus::Preserve,
            ..Default::default()
        },
    ] {
        let mut bad = d.clone();
        bad.semantics.as_mut().unwrap().layer = Some(layer);
        assert!(bad.validate().is_err());
    }
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(d.clone()),
        }])
        .unwrap();
    let before = memory.node(id).unwrap().descriptor.view().to_owned();
    let invalid = Semantics {
        layer: Some(Layer {
            modal: false,
            ..d.semantics.as_ref().unwrap().layer.clone().unwrap()
        }),
        ..*d.semantics.unwrap()
    };
    assert!(
        memory
            .prepare_patch(id, &Patch::Semantics(Change::Set(Some(Box::new(invalid)))))
            .is_err()
    );
    assert_eq!(
        memory.node(id).unwrap().descriptor.view().to_owned(),
        before
    );
}
