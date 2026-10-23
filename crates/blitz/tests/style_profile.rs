#![cfg(not(target_arch = "wasm32"))]

use glory_blitz::Renderer;
use glory_html::Part;
use glory_render::style::{Css, Length, Overflow, Position, Property, Style, Unsupported, Value};
use glory_render::*;
use glory_ui::{App, Error as UiError, input, view};
use std::cell::RefCell;
use std::rc::Rc;

fn app() -> (App, Renderer) {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    (app, output.unwrap())
}
fn unsupported(property: Property, value: Value) -> Error {
    Error::UnsupportedStyle {
        profile: "blitz",
        version: 4,
        requirement: Unsupported {
            property,
            value: Some(value),
        },
    }
}
fn rejected() -> [(Style, Error); 2] {
    [
        (
            Style::new().position(Position::Sticky).top(Length::Px(0.0)),
            unsupported(Property::Position, Value::Sticky),
        ),
        (
            Style::new().scroll_anchor(true),
            unsupported(Property::ScrollAnchor, Value::AnchorAuto),
        ),
    ]
}
fn query(app: &App, query: Query) -> QueryValue {
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(query, move |value| {
        *reply.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    result.borrow_mut().take().unwrap()
}

#[test]
fn native_profile_distinguishes_unimplemented_values_from_the_html_dialect() {
    let (app, renderer) = app();
    let profile = renderer.style_profile();
    assert_eq!(app.style_profile(), profile);
    assert_eq!(app.scope().style_profile().unwrap(), profile);
    for property in Property::ALL {
        assert!(profile.inline.supports_property(*property), "{property:?}");
    }
    for value in Value::ALL {
        assert_eq!(
            profile.inline.supports_value(*value),
            !matches!(value, Value::Sticky | Value::AnchorAuto),
            "{value:?}"
        );
    }
    assert_eq!(
        (profile.name, profile.version, profile.css),
        ("blitz", 4, Css::Html)
    );
    assert!(
        glory_render::style::Profile::HTML
            .inline
            .supports_value(Value::Sticky)
    );
    assert!(
        glory_render::style::Profile::HTML
            .inline
            .supports_value(Value::AnchorAuto)
    );
}

#[test]
fn native_creation_and_explicit_preflight_reject_before_identity_queue_or_layout_changes() {
    let (app, renderer) = app();
    for (style, error) in rejected() {
        let mut node = Descriptor::new(Props::View);
        node.style = style;
        let before = (app.stats(), app.tree().stats(), renderer.stats());
        assert_eq!(renderer.preflight(&node, None), Err(error.clone()));
        assert_eq!(app.scope().create(node), Err(UiError::Render(error)));
        assert_eq!((app.stats(), app.tree().stats(), renderer.stats()), before);
    }
    let id = app.scope().create(Descriptor::new(Props::View)).unwrap();
    assert_eq!(id.slot(), 0);
    app.flush().unwrap();
    assert!(renderer.engine_id(id, Part::Main).is_ok());
}

#[test]
fn staged_style_rejection_keeps_input_identity_value_and_subsequent_updates_usable() {
    let (app, renderer) = app();
    app.mount(
        input()
            .value("still editable")
            .style(Style::new().width(Length::Px(120.0))),
    )
    .unwrap();
    app.settle(8).unwrap();
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, Part::Main).unwrap();
    for (style, error) in rejected() {
        let patch = Patch::Style(Change::Set(Box::new(style)));
        let before = (
            app.stats(),
            app.tree().stats(),
            renderer.stats(),
            app.tree().node(id).unwrap().descriptor.clone(),
        );
        assert_eq!(
            renderer.preflight(&before.3, Some(&patch)),
            Err(error.clone())
        );
        assert_eq!(app.scope().patch(id, patch), Err(UiError::Render(error)));
        assert_eq!(
            (
                app.stats(),
                app.tree().stats(),
                renderer.stats(),
                app.tree().node(id).unwrap().descriptor.clone()
            ),
            before
        );
        app.flush().unwrap();
        assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
    }
    app.scope().patch(id, Patch::Style(Change::Keep)).unwrap();
    app.scope().patch(id, Patch::Style(Change::Clear)).unwrap();
    app.scope()
        .patch(id, Patch::Name(Change::Set(Some("usable".into()))))
        .unwrap();
    app.settle(8).unwrap();
    assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
    renderer.inspect(|doc| {
        assert_eq!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .raw_text(),
            "still editable"
        )
    });
}

#[test]
fn direct_rejection_retains_successful_prefix_receipt_and_recovery() {
    for (style, error) in rejected() {
        let runtime = RuntimeId::new();
        let mut renderer = Renderer::new(runtime);
        let mut ids = NodeIds::new(runtime);
        let id = ids.allocate();
        let later = ids.allocate();
        assert_eq!(
            renderer.apply(&[
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
                    patch: Patch::Style(Change::Set(Box::new(style.clone())))
                },
                Mutation::Create {
                    id: later,
                    node: Box::new(Descriptor::new(Props::View))
                },
            ]),
            Err(error.clone())
        );
        assert_eq!(renderer.stats().applied, 2);
        assert_eq!(renderer.stats().nodes, 1);
        assert!(renderer.engine_id(later, Part::Main).is_err());
        renderer.request(Query::Bounds(id), QueryToken(1)).unwrap();
        let mut inputs = Vec::new();
        renderer.drain(&mut inputs);
        assert!(matches!(
            inputs.as_slice(),
            [Input::Response {
                applied: 2,
                token: QueryToken(1),
                result: Ok(QueryValue::Bounds(_))
            }]
        ));
        let before = renderer.stats();
        let mut node = Descriptor::new(Props::View);
        node.style = style;
        assert_eq!(
            renderer.apply(&[Mutation::Create {
                id: later,
                node: Box::new(node)
            }]),
            Err(error)
        );
        assert_eq!(renderer.stats(), before);
        let receipt = renderer
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::Style(Change::Set(Box::new(
                    Style::new()
                        .position(Position::Relative)
                        .scroll_anchor(false),
                ))),
            }])
            .unwrap();
        assert_eq!(receipt.applied, 3);
        assert_eq!(renderer.apply(&[]).unwrap(), receipt);
    }
}

#[test]
fn supported_relative_offsets_and_explicit_no_anchor_reach_real_layout() {
    let (app, renderer) = app();
    app.mount(
        view().style(
            Style::new()
                .position(Position::Relative)
                .left(Length::Px(12.0))
                .top(Length::Px(7.0))
                .width(Length::Px(80.0))
                .height(Length::Px(20.0))
                .scroll_anchor(false),
        ),
    )
    .unwrap();
    let id = app.tree().roots()[0];
    let QueryValue::Bounds(before) = query(&app, Query::Bounds(id)) else {
        panic!("bounds")
    };
    let physical = renderer.engine_id(id, Part::Main).unwrap();
    app.scope()
        .patch(
            id,
            Patch::Style(Change::Set(Box::new(
                Style::new()
                    .position(Position::Relative)
                    .left(Length::Px(25.0))
                    .top(Length::Px(-3.0))
                    .width(Length::Px(120.0))
                    .height(Length::Px(30.0))
                    .scroll_anchor(false),
            ))),
        )
        .unwrap();
    let QueryValue::Bounds(after) = query(&app, Query::Bounds(id)) else {
        panic!("bounds")
    };
    for (actual, expected) in [
        (after.x - before.x, 13.0),
        (after.y - before.y, -10.0),
        (after.width, 120.0),
        (after.height, 30.0),
    ] {
        assert!((actual - expected).abs() < 0.03, "{actual} != {expected}");
    }
    assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
}

#[test]
fn advertised_overflow_modes_preserve_scroll_container_and_clip_semantics() {
    for overflow in [
        Overflow::Visible,
        Overflow::Hidden,
        Overflow::Clip,
        Overflow::Auto,
        Overflow::Scroll,
    ] {
        let (app, renderer) = app();
        app.mount(
            view()
                .style(
                    Style::new()
                        .width(Length::Px(100.0))
                        .height(Length::Px(80.0))
                        .overflow_x(overflow)
                        .overflow_y(overflow),
                )
                .fill(
                    view().style(
                        Style::new()
                            .width(Length::Px(500.0))
                            .height(Length::Px(400.0)),
                    ),
                ),
        )
        .unwrap();
        let id = app.tree().roots()[0];
        let child = app.tree().node(id).unwrap().children[0];
        let QueryValue::Bounds(before) = query(&app, Query::Bounds(child)) else {
            panic!("bounds")
        };
        let physical = renderer.engine_id(id, Part::Main).unwrap();
        app.scope()
            .action(Action::ScrollTo {
                node: id,
                x: 60.0,
                y: 60.0,
            })
            .unwrap();
        app.flush().unwrap();
        let expected = if matches!(overflow, Overflow::Visible | Overflow::Clip) {
            0.0
        } else {
            60.0
        };
        assert_eq!(
            query(&app, Query::Scroll(id)),
            QueryValue::Scroll {
                x: expected,
                y: expected
            },
            "{overflow:?}"
        );
        let QueryValue::Bounds(after) = query(&app, Query::Bounds(child)) else {
            panic!("bounds")
        };
        assert!(
            (after.x - before.x + expected).abs() < 0.03,
            "{overflow:?}: {before:?} -> {after:?}"
        );
        assert!(
            (after.y - before.y + expected).abs() < 0.03,
            "{overflow:?}: {before:?} -> {after:?}"
        );
        assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
    }
}

#[cfg(all(
    feature = "shell",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
#[test]
fn interactive_host_uses_the_same_profile_and_preserves_focus_after_rejection() {
    let host = glory_blitz::shell::WindowApp::new();
    assert_eq!(host.app().style_profile(), glory_blitz::STYLE_PROFILE);
    host.mount(input().value("editable")).unwrap();
    let app = host.app();
    let id = app.tree().roots()[0];
    app.scope().action(Action::Focus(id)).unwrap();
    assert_eq!(query(app, Query::Focus(id)), QueryValue::Focus(true));
    let physical = host.renderer().engine_id(id, Part::Main).unwrap();
    for (style, error) in rejected() {
        assert_eq!(
            app.scope()
                .patch(id, Patch::Style(Change::Set(Box::new(style)))),
            Err(UiError::Render(error))
        );
        assert_eq!(query(app, Query::Focus(id)), QueryValue::Focus(true));
        assert_eq!(host.renderer().engine_id(id, Part::Main).unwrap(), physical);
    }
    let revision = match &app.tree().node(id).unwrap().descriptor.props {
        Props::Input(current) => current.revision + 1,
        _ => panic!("input"),
    };
    app.scope()
        .patch(
            id,
            Patch::Value {
                value: Change::Set("next".into()),
                revision,
            },
        )
        .unwrap();
    app.settle(8).unwrap();
    assert_eq!(query(app, Query::Focus(id)), QueryValue::Focus(true));
    host.renderer().inspect(|doc| {
        assert_eq!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .raw_text(),
            "next"
        )
    });
}
