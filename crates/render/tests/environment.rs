use glory_render::{memory::Memory, *};
fn environment(direction: Direction, language: &str) -> Environment {
    Environment {
        direction,
        language: LanguageTag::new(language).unwrap(),
    }
}
fn create(
    tree: &mut Memory,
    ids: &mut NodeIds,
    props: Props,
    parent: Option<NodeId>,
    environment: Option<Environment>,
) -> NodeId {
    let id = ids.allocate();
    let mut node = Descriptor::new(props);
    node.semantics = environment.map(|environment| {
        Box::new(Semantics {
            environment: Some(environment),
            ..Default::default()
        })
    });
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(node),
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
fn bounded_language_syntax_is_checked_before_copying_the_input() {
    for valid in ["en", "en-US", "zh-Hans-CN", "ar", "de-1996"] {
        assert!(LanguageTag::new(valid).is_ok());
    }
    for invalid in [
        "",
        "e",
        "en_US",
        "en--US",
        "-en",
        "en-",
        "en-abcdefghi",
        "中文",
        "en\0",
        "en US",
    ] {
        assert!(LanguageTag::new(invalid).is_err(), "{invalid}");
    }
    let huge = "a".repeat(1_000_000);
    assert!(LanguageTag::new(huge.as_str()).is_err());
}
#[test]
fn environment_inherits_through_groups_and_tracks_moves_resumes_and_clear() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let english = create(
        &mut tree,
        &mut ids,
        Props::View,
        None,
        Some(environment(Direction::Ltr, "en-US")),
    );
    let arabic = create(
        &mut tree,
        &mut ids,
        Props::View,
        None,
        Some(environment(Direction::Rtl, "ar")),
    );
    let group = create(&mut tree, &mut ids, Props::Group, Some(english), None);
    let control = create(
        &mut tree,
        &mut ids,
        Props::Input(Default::default()),
        Some(group),
        None,
    );
    assert_eq!(
        tree.inherited_environment(control)
            .unwrap()
            .unwrap()
            .language
            .as_str(),
        "en-US"
    );
    tree.apply(&[Mutation::Suspend { id: control }]).unwrap();
    assert_eq!(
        tree.inherited_environment(control)
            .unwrap()
            .unwrap()
            .direction,
        Direction::Ltr
    );
    tree.apply(&[Mutation::Resume {
        id: control,
        parent: Some(arabic),
        before: None,
    }])
    .unwrap();
    assert_eq!(
        tree.inherited_environment(control)
            .unwrap()
            .unwrap()
            .direction,
        Direction::Rtl
    );
    drop(
        tree.prepare_patch(arabic, &Patch::Semantics(Change::Clear))
            .unwrap(),
    );
    assert_eq!(
        tree.inherited_environment(control)
            .unwrap()
            .unwrap()
            .language
            .as_str(),
        "ar"
    );
    tree.prepare_patch(arabic, &Patch::Semantics(Change::Clear))
        .unwrap()
        .commit();
    assert_eq!(tree.inherited_environment(control).unwrap(), None);
    let before = (tree.receipt(), tree.stats());
    assert!(
        tree.prepare_patch(
            control,
            &Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                environment: Some(environment(Direction::Rtl, "ar")),
                ..Default::default()
            }))))
        )
        .is_err()
    );
    assert_eq!((tree.receipt(), tree.stats()), before);
}
#[test]
fn an_environment_cannot_contradict_its_own_keyboard_navigation() {
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.semantics = Some(Box::new(Semantics {
        environment: Some(environment(Direction::Rtl, "ar")),
        navigation: Some(Orientation::Horizontal),
        navigation_direction: Direction::Ltr,
        ..Default::default()
    }));
    assert!(descriptor.validate().is_err());
    descriptor.semantics.as_mut().unwrap().navigation_direction = Direction::Rtl;
    descriptor.validate().unwrap();
}
