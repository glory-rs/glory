use super::*;
use glory_render::{
    Action, ButtonProps, ClipboardText, Descriptor, EventKind, NodeIds, Props, Semantics,
};
#[test]
fn stream_private_renderer_refuses_clipboard_actions_and_nonphysical_source_patch_emits_no_edits() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut renderer = Renderer {
        mapper: Rc::new(RefCell::new(Mapper::new(runtime))),
        edits: Rc::new(RefCell::new(edits::Edits::new(4096))),
        inputs: Vec::new(),
        receipt: Receipt::default(),
    };
    let mut node = Descriptor::new(Props::Button(ButtonProps {
        label: "Copy".into(),
        ..Default::default()
    }));
    node.semantics = Some(Box::new(Semantics {
        clipboard: Some(ClipboardText {
            text: "private old".into(),
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
    assert!(
        renderer
            .action(Action::CopyText {
                node: id,
                revision: 0,
                request: 1
            })
            .is_err()
    );
    let before = renderer.mapper.borrow().html().unwrap();
    renderer
        .apply(&[Mutation::Patch {
            id,
            patch: glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(
                Semantics {
                    clipboard: Some(ClipboardText {
                        text: "private new".into(),
                        revision: 1,
                    }),
                    ..Default::default()
                },
            )))),
        }])
        .unwrap();
    assert_eq!(renderer.mapper.borrow().html().unwrap(), before);
    assert!(!before.contains("private") && !before.contains("clipboard"));
    assert_eq!(renderer.receipt.applied, 3);
}
