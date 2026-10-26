use crate::*;
use glory_render::*;

#[test]
fn placeholder_patch_matches_full_shape_without_editor_writes() {
    for multiline in [false, true] {
        let mut node = Descriptor::new(Props::Input(InputProps {
            value: "keep editor and revision".into(),
            revision: 9,
            multiline,
            ..Default::default()
        }));
        node.name = Some("Name stays independent".into());
        for change in [
            Change::Set(Some(Box::new(Semantics {
                placeholder: Some("<&\" hint".into()),
                ..Default::default()
            }))),
            Change::Keep,
            Change::Set(Some(Box::new(Semantics {
                placeholder: Some(String::new()),
                ..Default::default()
            }))),
            Change::Clear,
        ] {
            let patch = Patch::Semantics(change);
            let next = node.patched(patch.clone()).unwrap();
            let expected = difference(
                &shape(&node).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            let runtime = RuntimeId::new();
            let id = NodeIds::new(runtime).allocate();
            let mut memory = glory_render::memory::Memory::new(runtime);
            memory
                .apply(&[Mutation::Create {
                    id,
                    node: Box::new(node.clone()),
                }])
                .unwrap();
            let guard = memory.prepare_patch(id, &patch).unwrap();
            let actual = crate::patch::lower(guard.descriptor(), guard.previous());
            assert_eq!(actual, expected);
            assert!(actual.iter().all(|edit| matches!(
                edit,
                Edit::Attribute {
                    name: "placeholder",
                    ..
                }
            )));
            assert_eq!(next.props, node.props);
            assert_eq!(next.name, node.name);
            node = next;
        }
    }
}
