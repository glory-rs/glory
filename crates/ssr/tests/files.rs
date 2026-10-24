use glory_render::*;
#[test]
fn synchronous_file_shape_is_pure_has_one_real_button_and_no_ambient_file_value() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut renderer = glory_ssr::Renderer::new(runtime);
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::File(FileProps {
                    label: "Select bounded files".into(),
                    ..Default::default()
                }))),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let html = renderer.snapshot().unwrap();
    assert!(html.contains("data-gly-part=\"control\" style=\"display:contents;\""));
    assert_eq!(html.matches("<button ").count(), 1);
    assert!(html.contains("Select bounded files</button><input "));
    assert!(html.contains("type=\"file\""));
    assert!(!html.contains("value="));
    renderer
        .apply(&[
            Mutation::Patch {
                id,
                patch: Patch::Label(Change::Set("Changed label".into())),
            },
            Mutation::Patch {
                id,
                patch: Patch::Enabled(Change::Set(false)),
            },
        ])
        .unwrap();
    let html = renderer.snapshot().unwrap();
    assert!(html.contains("Changed label"));
    assert_eq!(html.matches(" disabled=\"\"").count(), 2);
}
