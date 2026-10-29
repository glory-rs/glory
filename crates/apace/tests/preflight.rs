#![cfg(not(target_arch = "wasm32"))]

use glory_apace::Renderer;
use glory_html::Part;
use glory_render::*;
use glory_ui::{App, input};

fn password() -> Descriptor {
    Descriptor::new(Props::Input(InputProps {
        purpose: Purpose::Password,
        value: "secret".into(),
        ..Default::default()
    }))
}

#[test]
fn password_creation_and_purpose_changes_preserve_the_control_and_raw_value() {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let renderer = output.unwrap();
    app.mount(input().purpose(Purpose::Password).value("secret"))
        .unwrap();
    let id = app.tree().roots()[0];
    app.settle(8).unwrap();
    let physical = renderer.engine_id(id, Part::Main).unwrap();
    renderer.inspect(|doc| {
        let input = doc
            .get_node(physical)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap();
        assert!(input.is_password());
        assert_eq!(input.display_editor().raw_text(), "••••••");
        assert_eq!(input.editor.raw_text(), "secret");
    });
    app.scope()
        .patch(id, Patch::Purpose(Change::Set(Purpose::Email)))
        .unwrap();
    app.settle(8).unwrap();
    assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
    renderer.inspect(|doc| {
        let input = doc
            .get_node(physical)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap();
        assert!(!input.is_password());
        assert_eq!(input.display_editor().raw_text(), "secret");
    });
}

#[test]
fn direct_password_calls_and_rejected_structure_keep_prefix_query_watermarks() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let rejected = ids.allocate();
    assert!(
        renderer
            .apply(&[
                Mutation::Create {
                    id: rejected,
                    node: Box::new(password())
                },
                Mutation::Place {
                    id: rejected,
                    parent: None,
                    before: None
                },
                Mutation::Patch {
                    id: rejected,
                    patch: Patch::Multiline(Change::Set(true))
                },
            ])
            .is_err()
    );
    assert_eq!(renderer.stats().applied, 2);
    renderer
        .request(Query::Bounds(rejected), QueryToken(1))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(1),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
    let receipt = renderer
        .apply(&[Mutation::Patch {
            id: rejected,
            patch: Patch::Purpose(Change::Set(Purpose::Search)),
        }])
        .unwrap();
    assert_eq!(receipt.applied, 3);
}

#[test]
fn native_structural_preflight_rejects_a_staged_tag_change_without_layout_or_property_writes() {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let renderer = output.unwrap();
    let id = app
        .scope()
        .create(Descriptor::new(Props::Input(InputProps::default())))
        .unwrap();
    let before = (app.stats(), renderer.stats());
    assert!(
        app.scope()
            .patch(id, Patch::Multiline(Change::Set(true)))
            .is_err()
    );
    assert_eq!((app.stats(), renderer.stats()), before);
    app.scope()
        .patch(id, Patch::Purpose(Change::Set(Purpose::Tel)))
        .unwrap();
    app.flush().unwrap();
    assert!(renderer.engine_id(id, Part::Main).is_ok());
}

#[test]
fn native_child_preflight_uses_staged_ancestors_without_engine_work_or_identity_allocation() {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let renderer = output.unwrap();
    let button = app
        .scope()
        .create(Descriptor::new(Props::Button(ButtonProps::default())))
        .unwrap();
    let children = app.scope().child_in(button).unwrap();
    let group = children.create(Descriptor::new(Props::Group)).unwrap();
    let children = children.child_in(group).unwrap();
    let before = (app.stats(), app.tree().stats(), renderer.stats());
    let mut paragraph = Descriptor::new(Props::Text(TextProps::default()));
    paragraph
        .options
        .insert(glory_html::HtmlTextOptions::default().tag(glory_html::TextTag::P));
    assert_eq!(
        children.create(paragraph),
        Err(glory_ui::Error::Render(Error::InvalidProps(
            "button text cannot map to an HTML paragraph"
        )))
    );
    assert_eq!((app.stats(), app.tree().stats(), renderer.stats()), before);
    let text = children
        .create(Descriptor::new(Props::Text(TextProps {
            content: "safe".into(),
            ..Default::default()
        })))
        .unwrap();
    assert_eq!(text.slot(), group.slot() + 1);
    app.settle(8).unwrap();
    let physical = renderer.engine_id(text, Part::Main).unwrap();
    children
        .patch(text, Patch::Text(Change::Set("still usable".into())))
        .unwrap();
    app.settle(8).unwrap();
    assert_eq!(renderer.engine_id(text, Part::Main).unwrap(), physical);
}
