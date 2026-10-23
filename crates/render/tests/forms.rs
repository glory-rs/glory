use glory_render::memory::Memory;
use glory_render::*;

fn form() -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.semantics = Some(Box::new(Semantics {
        field_part: Some(FieldPart::Form),
        ..Default::default()
    }));
    node
}
fn submit() -> Descriptor {
    let mut node = Descriptor::new(Props::Button(ButtonProps::default()));
    node.semantics = Some(Box::new(Semantics {
        form_action: Some(FormAction::Submit),
        ..Default::default()
    }));
    node
}
#[test]
fn successful_control_metadata_and_form_requests_keep_intrinsic_targets() {
    for props in [
        Props::View,
        Props::Input(InputProps::default()),
        Props::Check(CheckProps::default()),
    ] {
        let mut node = Descriptor::new(props);
        node.semantics = submit().semantics;
        assert!(node.validate().is_err());
    }
    let mut input = Descriptor::new(Props::Input(InputProps::default()));
    input.semantics = Some(Box::new(Semantics {
        form_control: Some(FormControl {
            name: "email".into(),
            value: None,
        }),
        ..Default::default()
    }));
    input.validate().unwrap();
    input
        .semantics
        .as_mut()
        .unwrap()
        .form_control
        .as_mut()
        .unwrap()
        .value = Some("override".into());
    assert!(input.validate().is_err());
    assert!(EventKind::Submit.validate_target(&input).is_err());
    assert!(EventKind::Reset.validate_target(&submit()).is_err());
    EventKind::Submit.validate_target(&form()).unwrap();
}
#[test]
fn nested_form_moves_are_transactional_and_submit_origins_are_live_descendants() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let outer = ids.allocate();
    let inner = ids.allocate();
    let wrapper = ids.allocate();
    let button = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id: outer,
            node: Box::new(form()),
        },
        Mutation::Place {
            id: outer,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: wrapper,
            node: Box::new(Descriptor::new(Props::Group)),
        },
        Mutation::Place {
            id: wrapper,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: inner,
            node: Box::new(form()),
        },
        Mutation::Place {
            id: inner,
            parent: Some(wrapper),
            before: None,
        },
        Mutation::Create {
            id: button,
            node: Box::new(submit()),
        },
        Mutation::Place {
            id: button,
            parent: Some(inner),
            before: None,
        },
    ])
    .unwrap();
    let receipt = tree.receipt();
    let stats = tree.stats();
    assert!(
        tree.apply(&[Mutation::Place {
            id: wrapper,
            parent: Some(outer),
            before: None
        }])
        .is_err()
    );
    assert_eq!(tree.receipt(), receipt);
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.node(wrapper).unwrap().parent, None);
    assert!(
        tree.validate_event(
            outer,
            &Event::Submit {
                submitter: Some(button)
            }
        )
        .is_err()
    );
    assert!(
        tree.validate_event(
            inner,
            &Event::Submit {
                submitter: Some(button)
            }
        )
        .unwrap()
    );
    tree.apply(&[Mutation::Dispose { id: button }]).unwrap();
    assert!(ids.release(button));
    assert!(
        !tree
            .validate_event(
                inner,
                &Event::Submit {
                    submitter: Some(button)
                }
            )
            .unwrap()
    );
    let new_button = ids.allocate();
    assert_eq!(new_button.slot(), button.slot());
    assert_ne!(new_button, button);
    tree.apply(&[
        Mutation::Create {
            id: new_button,
            node: Box::new(submit()),
        },
        Mutation::Place {
            id: new_button,
            parent: Some(inner),
            before: None,
        },
    ])
    .unwrap();
    assert!(
        !tree
            .validate_event(
                inner,
                &Event::Submit {
                    submitter: Some(button)
                }
            )
            .unwrap()
    );
}
