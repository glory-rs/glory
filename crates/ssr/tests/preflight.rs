use glory_html::{HtmlTextOptions, TextTag};
use glory_render::*;
use glory_ssr::Renderer;
use glory_ui::{App, input, text};

fn app() -> (App, Renderer) {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    (app, output.unwrap())
}

#[test]
fn structural_patches_fail_before_changing_the_app_or_poisoning_later_updates() {
    let cases = [
        (
            Descriptor::new(Props::Text(TextProps::default())),
            Patch::TextKind(Change::Set(TextKind::Paragraph)),
        ),
        (
            Descriptor::new(Props::View),
            Patch::Role(Change::Set(Some(Role::Main))),
        ),
        (
            Descriptor::new(Props::Input(InputProps::default())),
            Patch::Multiline(Change::Set(true)),
        ),
    ];
    for (descriptor, patch) in cases {
        let (app, renderer) = app();
        let id = app.scope().create(descriptor.clone()).unwrap();
        app.flush().unwrap();
        let html = renderer.html().unwrap();
        let stats = app.stats();
        assert_eq!(
            app.scope().patch(id, patch),
            Err(glory_ui::Error::Render(Error::InvalidProps(
                "HTML tag changes require remount"
            )))
        );
        assert_eq!(app.tree().node(id).unwrap().descriptor, descriptor);
        assert_eq!(app.stats(), stats);
        let update = if matches!(descriptor.props, Props::Text(_)) {
            Patch::Text(Change::Set("still works".into()))
        } else {
            Patch::Name(Change::Set(Some("still works".into())))
        };
        app.scope().patch(id, update).unwrap();
        app.flush().unwrap();
        assert_ne!(renderer.html().unwrap(), html);
        assert!(renderer.html().unwrap().contains("still works"));
    }
}

#[test]
fn mapping_preflight_uses_staged_descriptors_and_allows_changes_with_the_same_tag() {
    let (app, renderer) = app();
    let mut descriptor = Descriptor::new(Props::Text(TextProps::default()));
    descriptor
        .options
        .insert(HtmlTextOptions::default().tag(TextTag::P));
    let id = app.scope().create(descriptor).unwrap();
    // The renderer has not seen Create. The explicit override already maps to p.
    app.scope()
        .patch(id, Patch::TextKind(Change::Set(TextKind::Paragraph)))
        .unwrap();
    app.scope()
        .patch(id, Patch::TextKind(Change::Clear))
        .unwrap();
    app.scope()
        .patch(id, Patch::Text(Change::Set("body".into())))
        .unwrap();
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains(">body</p>"));

    let id = app.scope().create(Descriptor::new(Props::View)).unwrap();
    app.scope()
        .patch(id, Patch::Role(Change::Set(Some(Role::List))))
        .unwrap();
    app.scope()
        .patch(id, Patch::Role(Change::Set(Some(Role::Status))))
        .unwrap();
    let stats = app.stats();
    assert!(
        app.scope()
            .patch(id, Patch::Role(Change::Set(Some(Role::Main))))
            .is_err()
    );
    assert_eq!(app.stats(), stats);
    assert_eq!(
        app.tree().node(id).unwrap().descriptor.view().role,
        Some(Role::Status)
    );
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains("role=\"status\""));
}

#[test]
fn invalid_html_options_do_not_allocate_a_semantic_node_and_bound_values_keep_updating() {
    let (app, renderer) = app();
    let stats = app.stats();
    let mut invalid = Descriptor::new(Props::Text(TextProps::default()));
    invalid
        .options
        .insert(HtmlTextOptions::default().tag(TextTag::H1));
    assert!(app.scope().create(invalid).is_err());
    assert_eq!(app.stats(), stats);
    let content = app.scope().cage(String::from("text"));
    app.mount(text().content(content)).unwrap();
    let id = app.tree().roots()[0];
    let before = renderer.html().unwrap();
    assert!(
        app.scope()
            .patch(id, Patch::TextKind(Change::Set(TextKind::Heading(1))))
            .is_err()
    );
    assert_eq!(renderer.html().unwrap(), before);
    content.revise(|content| *content = "updated".into());
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains(">updated</span>"));
    app.mount(
        input()
            .purpose(Purpose::Password)
            .value("web retains password support"),
    )
    .unwrap();
    assert!(renderer.html().unwrap().contains("type=\"password\""));
}

#[test]
fn rejected_direct_batch_retains_the_successful_prefix_query_watermark() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let id = NodeIds::new(runtime).allocate();
    assert!(
        renderer
            .apply(&[
                Mutation::Create {
                    id,
                    node: Box::new(Descriptor::new(Props::View))
                },
                Mutation::Place {
                    id,
                    parent: None,
                    before: None
                },
                Mutation::Patch {
                    id,
                    patch: Patch::Role(Change::Set(Some(Role::Main)))
                },
            ])
            .is_err()
    );
    assert!(renderer.html().unwrap().contains("<div"));
    renderer.request(Query::Bounds(id), QueryToken(1)).unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(1),
            applied: 2,
            result: Err(Error::Unsupported(_))
        }]
    ));
    let receipt = renderer
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Name(Change::Set(Some("recovered".into()))),
        }])
        .unwrap();
    assert_eq!(receipt.applied, 3);
}

#[test]
fn rejected_child_creation_keeps_identity_model_queue_and_later_updates_intact() {
    let (app, renderer) = app();
    let button = app
        .scope()
        .create(Descriptor::new(Props::Button(ButtonProps::default())))
        .unwrap();
    let children = app.scope().child_in(button).unwrap();
    let group = children.create(Descriptor::new(Props::Group)).unwrap();
    let grouped = children.child_in(group).unwrap();
    // Both ancestors and Create are still staged, absent from the adapter tree.
    let stats = app.stats();
    let model = app.tree().stats();
    let mut paragraph = Descriptor::new(Props::Text(TextProps::default()));
    paragraph
        .options
        .insert(HtmlTextOptions::default().tag(TextTag::P));
    assert_eq!(
        grouped.create(paragraph),
        Err(glory_ui::Error::Render(Error::InvalidProps(
            "button text cannot map to an HTML paragraph"
        )))
    );
    assert_eq!(
        grouped.create(Descriptor::new(Props::View)),
        Err(glory_ui::Error::Render(Error::InvalidProps(
            "button children must be inline text, images or groups"
        )))
    );
    assert_eq!(app.stats(), stats);
    assert_eq!(app.tree().stats(), model);
    let text = grouped
        .create(Descriptor::new(Props::Text(TextProps {
            content: "valid".into(),
            ..TextProps::default()
        })))
        .unwrap();
    assert_eq!(text.slot(), group.slot() + 1);
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains(">valid</span>"));
    grouped
        .patch(text, Patch::Text(Change::Set("updated".into())))
        .unwrap();
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains(">updated</span>"));
}
