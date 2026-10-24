use glory_render::*;
#[test]
fn ssr_records_button_interest_without_payload_leak_and_explicitly_refuses_physical_write() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut renderer = glory_ssr::Renderer::new(runtime);
    let mut node = Descriptor::new(Props::Button(ButtonProps {
        label: "Copy".into(),
        ..Default::default()
    }));
    node.semantics = Some(Box::new(Semantics {
        clipboard: Some(ClipboardText {
            text: "private <& 😀".into(),
            revision: 0,
        }),
        ..Default::default()
    }));
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(node),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    renderer.listen(id, EventKind::Clipboard, true).unwrap();
    assert!(!renderer.capabilities().clipboard_write);
    assert!(matches!(
        renderer.action(Action::CopyText {
            node: id,
            revision: 0,
            request: 1
        }),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        renderer.action(Action::CopyText {
            node: id,
            revision: 9,
            request: 1
        }),
        Err(Error::InvalidProps(_))
    ));
    let html = renderer.snapshot().unwrap();
    assert!(html.contains("Copy"));
    assert!(!html.contains("private") && !html.contains("clipboard"));
}
