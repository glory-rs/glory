use glory_render::{memory::Memory, *};

#[test]
fn collected_retirement_matches_apply_including_parked_descendants_and_focus_names() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let (outer, root, first, parked, last, survivor) = (
        ids.allocate(),
        ids.allocate(),
        ids.allocate(),
        ids.allocate(),
        ids.allocate(),
        ids.allocate(),
    );
    let view = || Box::new(Descriptor::new(Props::View));
    let mut scope = view();
    scope.focus = FocusMode::Programmatic;
    scope.semantics = Some(Box::new(Semantics {
        focus_scope: Some(FocusScopeConfig::default()),
        ..Default::default()
    }));
    let mut named = view();
    named.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("retired").unwrap()),
        ..Default::default()
    }));
    let commands = [
        Mutation::Create {
            id: outer,
            node: scope,
        },
        Mutation::Create {
            id: root,
            node: view(),
        },
        Mutation::Create {
            id: first,
            node: named,
        },
        Mutation::Create {
            id: parked,
            node: view(),
        },
        Mutation::Create {
            id: last,
            node: view(),
        },
        Mutation::Create {
            id: survivor,
            node: view(),
        },
        Mutation::Place {
            id: outer,
            parent: None,
            before: None,
        },
        Mutation::Place {
            id: root,
            parent: Some(outer),
            before: None,
        },
        Mutation::Place {
            id: first,
            parent: Some(root),
            before: None,
        },
        Mutation::Place {
            id: parked,
            parent: Some(first),
            before: None,
        },
        Mutation::Place {
            id: last,
            parent: Some(root),
            before: None,
        },
        Mutation::Place {
            id: survivor,
            parent: Some(outer),
            before: None,
        },
        Mutation::Suspend { id: parked },
    ];
    let mut original = Memory::new(runtime);
    let mut collected = Memory::new(runtime);
    original.apply(&commands).unwrap();
    collected.apply(&commands).unwrap();
    let before = collected.receipt();
    let expected = original.apply(&[Mutation::Dispose { id: root }]).unwrap();
    let (actual, removed) = collected.dispose(root).unwrap();
    assert_eq!(removed, [root, last, first, parked]);
    assert_eq!(actual, expected);
    assert_eq!(actual.applied, before.applied + 1);
    assert_eq!(collected.stats(), original.stats());
    assert_eq!(collected.len(), 2);
    assert_eq!(collected.focus_scope_nodes(outer).unwrap(), Some(2));
    assert_eq!(collected.named(&SemanticId::new("retired").unwrap()), None);
    for id in removed {
        assert!(collected.node(id).is_err());
    }
    assert_eq!(
        collected.node(outer).unwrap().children.as_ref(),
        &[survivor]
    );
    assert!(collected.dispose(root).is_err());
    assert_eq!(collected.receipt(), actual);
    // Both forms keep the generation watermark after removal.
    let stale = Mutation::Create {
        id: first,
        node: view(),
    };
    assert!(original.apply(std::slice::from_ref(&stale)).is_err());
    assert!(collected.apply(&[stale]).is_err());
}

#[test]
fn collected_retirement_preserves_queued_reply_watermarks_and_rejects_foreign_nodes() {
    let runtime = RuntimeId::new();
    let root = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[Mutation::Create {
            id: root,
            node: Box::new(Descriptor::new(Props::View)),
        }])
        .unwrap();
    let before = memory.receipt();
    memory.request(Query::Bounds(root), QueryToken(1)).unwrap();
    let (accepted, ids) = memory.dispose(root).unwrap();
    assert_eq!(ids, [root]);
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert!(memory.dispose(foreign).is_err());
    assert_eq!(memory.receipt(), accepted);
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(matches!(&inputs[..], [Input::Response { applied, .. }] if *applied == before.applied));
}
