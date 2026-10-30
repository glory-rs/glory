use glory_render::*;

#[test]
fn link_targets_allow_relative_and_explicit_safe_schemes_only() {
    for target in [
        "",
        "#section",
        "?q=javascript:x",
        "/guide",
        "./guide",
        "../guide",
        "guide/path:part",
        "guide%3Apart",
        "//example.test/path",
        "HTTPS://example.test/",
        "http://example.test",
        "mailto:hello@example.test",
        "tel:+123456",
    ] {
        assert!(
            Descriptor::new(Props::Link(LinkProps {
                target: target.into(),
                label: String::new()
            }))
            .validate()
            .is_ok(),
            "{target:?}"
        );
    }
    for target in [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "data:text/html,body",
        "vbscript:msgbox(1)",
        "file:///secret",
        "myapp:open",
        " javascript:alert(1)",
        "java\nscript:alert(1)",
        "\0javascript:alert(1)",
        "https://example.test/\r",
        "guide with spaces",
    ] {
        let mut descriptor = Descriptor::new(Props::Link(LinkProps {
            target: "/safe".into(),
            label: String::new(),
        }));
        let before = descriptor.clone();
        assert!(
            descriptor
                .patch(Patch::Target(Change::Set(target.into())))
                .is_err(),
            "{target:?}"
        );
        assert_eq!(descriptor, before);
        assert!(
            Descriptor::new(Props::Link(LinkProps {
                target: target.into(),
                label: String::new()
            }))
            .validate()
            .is_err(),
            "{target:?}"
        );
    }
}

#[test]
fn rejected_create_and_prepared_link_patch_preserve_state_and_recover() {
    use glory_render::memory::Memory;
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    let descriptor = |target: &str| {
        Descriptor::new(Props::Link(LinkProps {
            target: target.into(),
            label: "Open".into(),
        }))
    };
    assert!(
        memory
            .apply(&[Mutation::Create {
                id,
                node: Box::new(descriptor("javascript:alert(1)"))
            }])
            .is_err()
    );
    assert!(memory.node(id).is_err());
    memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(descriptor("/safe")),
        }])
        .unwrap();
    let receipt = memory.apply(&[]).unwrap();
    let stats = memory.stats();
    let pointer = match memory.node(id).unwrap().descriptor.view().props {
        Props::Link(link) => link.target.as_ptr(),
        _ => unreachable!(),
    };
    for target in ["javascript:alert(1)", "data:text/html,body", "myapp:open"] {
        assert!(
            memory
                .prepare_patch(id, &Patch::Target(Change::Set(target.into())))
                .is_err()
        );
        assert_eq!(memory.apply(&[]).unwrap(), receipt);
        assert_eq!(memory.stats(), stats);
        let Props::Link(link) = memory.node(id).unwrap().descriptor.view().props else {
            unreachable!()
        };
        assert_eq!(link.target, "/safe");
        assert_eq!(link.target.as_ptr(), pointer);
    }
    let accepted = memory
        .prepare_patch(
            id,
            &Patch::Target(Change::Set("https://example.test/".into())),
        )
        .unwrap()
        .commit();
    assert_eq!(accepted.applied, receipt.applied + 1);
    let Props::Link(link) = memory.node(id).unwrap().descriptor.view().props else {
        unreachable!()
    };
    assert_eq!(link.target, "https://example.test/");
}
#[test]
fn separate_context_metadata_only_belongs_to_links_and_rolls_back_other_nodes() {
    for props in [
        Props::View,
        Props::Group,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
        Props::Check(CheckProps::default()),
    ] {
        let mut node = Descriptor::new(props);
        let before = node.clone();
        assert!(
            node.patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                link_open: Some(LinkOpen::NewContext),
                ..Default::default()
            })))))
            .is_err()
        );
        assert_eq!(node, before);
    }
    let mut link = Descriptor::new(Props::Link(LinkProps {
        target: "/guide".into(),
        label: "Guide".into(),
    }));
    for open in [LinkOpen::Current, LinkOpen::NewContext] {
        link.patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            link_open: Some(open),
            ..Default::default()
        })))))
        .unwrap();
    }
    assert!(
        matches!(&link.props,Props::Link(value) if value.target=="/guide"&&value.label=="Guide")
    );
}
