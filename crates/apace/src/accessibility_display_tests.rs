use super::*;
use glory_html::view::{Attribute, Click, Field, Node as HtmlNode, Outlet, Template};
use glory_web::view::{lazy, view as display};
const VALUE: Field = Field::new("value");
const CLICK: Click = Click::new("click");
const BODY: Outlet = Outlet::new("body");
static TEMPLATE: Template = Template::new(&[HtmlNode::Element {
    tag: "div",
    attrs: &[],
    children: &[
        HtmlNode::Element {
            tag: "button",
            attrs: &[Attribute::Click(CLICK)],
            children: &[HtmlNode::Text("raw accessible")],
        },
        HtmlNode::Field(VALUE),
        HtmlNode::Outlet(BODY),
    ],
}]);
#[test]
fn raw_template_and_semantic_outlet_share_ordered_accesskit_tree_focus_and_actions() {
    let app = WindowApp::new();
    let value = app.app.scope().cage(0usize);
    app.mount(
        display(&TEMPLATE)
            .bind(VALUE, value)
            .on_click(CLICK, move || value.revise(|v| *v += 1))
            .child(BODY, lazy(|| button().label("semantic accessible"))),
    )
    .unwrap();
    let initial = build(&app, true);
    let raw_ax = initial
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("raw accessible"))
        .unwrap()
        .0;
    let mut tree = Consumer::new(initial, true);
    let raw = nodes(&tree)
        .iter()
        .find(|node| node.label().as_deref() == Some("raw accessible"))
        .unwrap()
        .id();
    let semantic = nodes(&tree)
        .iter()
        .find(|node| node.label().as_deref() == Some("semantic accessible"))
        .unwrap()
        .id();
    assert_ne!(raw, semantic);
    assert!(action(&app, &request(raw_ax, AxAction::Focus, None)).unwrap());
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_eq!(tree.state().focus_id(), Some(raw));
    assert!(action(&app, &request(raw_ax, AxAction::Click, None)).unwrap());
    app.app.settle(8).unwrap();
    assert_eq!(value.get(), 1);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .any(|node| node.data().value() == Some("1"))
    );
    let parent = tree.state().node_by_id(raw).unwrap().parent().unwrap();
    let order: Vec<_> = parent.children().map(|node| node.id()).collect();
    assert!(order.iter().position(|id| *id == raw) < order.iter().position(|id| *id == semantic));
    app.app.clear().unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(tree.state().node_by_id(raw).is_none());
    assert!(!action(&app, &request(raw_ax, AxAction::Click, None)).unwrap());
}

#[test]
fn mixed_native_modal_autofocus_tab_loop_and_return_keep_physical_raw_targets() {
    use glory_render::{FocusMode, Layer, Semantics};
    let mut app = WindowApp::new();
    let open = app.app.scope().cage(false);
    app.mount(
        view().fill((
            display(&TEMPLATE).bind(VALUE, "outside"),
            view()
                .role(Role::Dialog)
                .name("Mixed modal")
                .focus(FocusMode::Programmatic)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(180.)),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(glory_render::SemanticId::new("mixed-modal").unwrap()),
                        hidden: !*open,
                        layer: Some(Layer::default()),
                        ..Default::default()
                    }))
                }))
                .fill(
                    display(&TEMPLATE)
                        .bind(VALUE, "inside")
                        .child(BODY, lazy(|| button().label("modal semantic"))),
                ),
        )),
    )
    .unwrap();
    let initial = build(&app, true);
    let raw = initial
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("raw accessible"))
        .unwrap()
        .0;
    assert!(action(&app, &request(raw, AxAction::Focus, None)).unwrap());
    let previous = app
        .renderer
        .inspect(|doc| doc.get_focussed_node_id())
        .unwrap();
    open.revise(|value| *value = true);
    build(&app, false);
    let first = app
        .renderer
        .inspect(|doc| doc.get_focussed_node_id())
        .unwrap();
    assert_ne!(first, previous);
    assert!(
        app.renderer
            .0
            .borrow()
            .dom
            .display_controls
            .contains_key(&first)
    );
    super::super::super::focus_tests::key(
        &mut app,
        keyboard_types::Key::Named(keyboard_types::NamedKey::Tab),
        keyboard_types::Modifiers::empty(),
    );
    let second = app
        .renderer
        .inspect(|doc| doc.get_focussed_node_id())
        .unwrap();
    assert_ne!(second, first);
    super::super::super::focus_tests::key(
        &mut app,
        keyboard_types::Key::Named(keyboard_types::NamedKey::Tab),
        keyboard_types::Modifiers::empty(),
    );
    assert_eq!(
        app.renderer.inspect(|doc| doc.get_focussed_node_id()),
        Some(first)
    );
    open.revise(|value| *value = false);
    build(&app, false);
    assert_eq!(
        app.renderer.inspect(|doc| doc.get_focussed_node_id()),
        Some(previous)
    );
}

#[test]
fn nonmodal_scope_limits_raw_focus_but_keeps_background_activation() {
    use glory_render::{FocusMode, FocusScopeConfig, Semantics};
    let app = WindowApp::new();
    let active = app.app.scope().cage(false);
    let count = app.app.scope().cage(0usize);
    app.mount(
        view().fill((
            display(&TEMPLATE)
                .bind(VALUE, "outside")
                .on_click(CLICK, move || count.revise(|value| *value += 1)),
            view()
                .focus(FocusMode::Programmatic)
                .semantics(active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(display(&TEMPLATE).bind(VALUE, "inside")),
        )),
    )
    .unwrap();
    let initial = build(&app, true);
    let outside = initial
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("raw accessible"))
        .unwrap()
        .0;
    assert!(action(&app, &request(outside, AxAction::Focus, None)).unwrap());
    let previous = app.renderer.inspect(|doc| doc.get_focussed_node_id());
    active.revise(|value| *value = true);
    build(&app, false);
    assert!(!action(&app, &request(outside, AxAction::Focus, None)).unwrap());
    assert!(action(&app, &request(outside, AxAction::Click, None)).unwrap());
    app.app.settle(8).unwrap();
    assert_eq!(count.get(), 1);
    active.revise(|value| *value = false);
    build(&app, false);
    assert_eq!(
        app.renderer.inspect(|doc| doc.get_focussed_node_id()),
        previous
    );
}
