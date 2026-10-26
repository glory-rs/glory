use glory_render::{memory::Memory, *};

fn input(multiline: bool, hint: Option<String>) -> Descriptor {
    let mut node = Descriptor::new(Props::Input(InputProps {
        value: "business text".into(),
        revision: 7,
        multiline,
        ..Default::default()
    }));
    node.name = Some("Message".into());
    node.semantics = Some(Box::new(Semantics {
        placeholder: hint,
        ..Default::default()
    }));
    node
}

#[test]
fn hint_validation_is_local_bounded_and_independent_from_value_and_name() {
    assert!(input(false, Some("Search…".into())).validate().is_ok());
    assert!(
        input(true, Some("First line\nSecond line".into()))
            .validate()
            .is_ok()
    );
    for hint in [
        "a\nb".to_owned(),
        "a\rb".into(),
        "a\0b".into(),
        "x".repeat(MAX_INPUT_PLACEHOLDER_BYTES + 1),
    ] {
        assert!(input(false, Some(hint)).validate().is_err());
    }
    for props in [
        Props::View,
        Props::Button(Default::default()),
        Props::Select(Default::default()),
    ] {
        let mut node = Descriptor::new(props);
        node.semantics = input(false, Some("hint".into())).semantics;
        assert!(node.validate().is_err());
    }
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(input(false, Some("Old hint".into()))),
        }])
        .unwrap();
    let stats = memory.stats();
    let patch = Patch::Semantics(Change::Set(
        input(false, Some("bad\nhint".into())).semantics,
    ));
    assert!(memory.prepare_patch(id, &patch).is_err());
    assert_eq!(memory.stats(), stats);
    let before = memory.node(id).unwrap().descriptor.view().to_owned();
    {
        let _guard = memory
            .prepare_patch(id, &Patch::Semantics(Change::Clear))
            .unwrap();
    }
    assert_eq!(
        memory.node(id).unwrap().descriptor.view().to_owned(),
        before
    );
    let guard = memory
        .prepare_patch(
            id,
            &Patch::Semantics(Change::Set(input(false, Some("New hint".into())).semantics)),
        )
        .unwrap();
    guard.commit();
    let after = memory.node(id).unwrap().descriptor.view();
    assert_eq!(after.name, &before.name);
    assert_eq!(after.props, &before.props);
    assert_eq!(
        after.semantics.as_ref().unwrap().placeholder.as_deref(),
        Some("New hint")
    );
}
