use super::*;
use accesskit_consumer::{NodeRef, Tree as Consumer, TreeChangeHandler};
use glory_render::style::{Length, Style};
use glory_ui::{ItemPosition, Widget, button, check, group, image, input, link, list, text, view};
use std::cell::Cell;
#[path = "accessibility_alert_tests.rs"]
mod alerts;
#[path = "accessibility_current_tests.rs"]
mod current;
#[path = "accessibility_display_tests.rs"]
mod display;
#[path = "accessibility_document_tests.rs"]
mod document;
#[path = "accessibility_hint_tests.rs"]
mod hints;
#[path = "accessibility_inert_tests.rs"]
mod inert;
#[path = "accessibility_layer_tests.rs"]
mod layers;
#[path = "accessibility_menu_tests.rs"]
mod menu;
#[path = "accessibility_progress_tests.rs"]
mod progress;
#[path = "accessibility_select_tests.rs"]
mod select;
#[path = "accessibility_separator_tests.rs"]
mod separator;
#[path = "accessibility_table_tests.rs"]
mod tables;
#[path = "accessibility_tags_tests.rs"]
mod tags;
#[path = "accessibility_translation_tests.rs"]
mod translation;
#[cfg(target_os = "windows")]
#[path = "accessibility_windows_tests.rs"]
mod windows;

struct Changes;
impl TreeChangeHandler for Changes {
    fn node_added(&mut self, _: &NodeRef) {}
    fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
    fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {}
    fn node_removed(&mut self, _: &NodeRef) {}
}
fn build(app: &WindowApp, full: bool) -> TreeUpdate {
    app.app.settle(16).unwrap();
    app.accessibility
        .borrow_mut()
        .build(
            &mut app.renderer.0.borrow_mut(),
            full,
            (0.0, 0.0),
            "test window",
        )
        .unwrap()
}
fn id(app: &WindowApp, node: NodeId) -> AxId {
    app.accessibility.borrow().ids[&Key::Main(node)]
}
fn request(target: AxId, action: AxAction, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: target,
        data,
    }
}
fn action(app: &WindowApp, request: &ActionRequest) -> Result<bool, Error> {
    app.accessibility
        .borrow_mut()
        .action(&mut app.renderer.0.borrow_mut(), request)
}
fn nodes(tree: &Consumer) -> Vec<NodeRef<'_>> {
    let mut all = Vec::new();
    let mut pending = vec![tree.state().root()];
    while let Some(node) = pending.pop() {
        pending.extend(node.children());
        all.push(node);
    }
    all
}

#[test]
fn mixed_checks_publish_native_mixed_then_boolean_state_after_assistive_activation() {
    use glory_render::{Change, CheckMode, Event, Patch};
    for initial in [false, true] {
        let app = WindowApp::new();
        let checked = app.app.scope().cage(initial);
        let calls = Rc::new(Cell::new(0));
        let changed = calls.clone();
        app.mount(
            check()
                .label("Mixed checkbox")
                .mode(CheckMode::Mixed)
                .checked(checked)
                .on(glory_ui::events::checked_changed, move |event| {
                    if let Event::CheckedChanged(next) = event {
                        changed.set(changed.get() + 1);
                        checked.revise(|v| *v = next);
                    }
                }),
        )
        .unwrap();
        let node = app.app.tree().roots()[0];
        let update = build(&app, true);
        let target = id(&app, node);
        let native = update.nodes.iter().find(|(id, _)| *id == target).unwrap();
        assert_eq!(native.1.role(), AxRole::CheckBox);
        assert_eq!(native.1.toggled(), Some(Toggled::Mixed));
        assert!(action(&app, &request(target, AxAction::Click, None)).unwrap());
        let update = build(&app, true);
        let native = update.nodes.iter().find(|(id, _)| *id == target).unwrap();
        assert_eq!(
            native.1.toggled(),
            Some(if initial {
                Toggled::False
            } else {
                Toggled::True
            })
        );
        assert_eq!(checked.get(), !initial);
        assert_eq!(calls.get(), 1);
        for mode in [CheckMode::Checkbox, CheckMode::Mixed] {
            app.app
                .scope()
                .patch(node, Patch::CheckMode(Change::Set(mode)))
                .unwrap();
        }
        app.app
            .scope()
            .patch(node, Patch::Enabled(Change::Set(false)))
            .unwrap();
        let update = build(&app, true);
        let native = update.nodes.iter().find(|(id, _)| *id == target).unwrap();
        assert_eq!(native.1.toggled(), Some(Toggled::Mixed));
        assert!(!action(&app, &request(target, AxAction::Click, None)).unwrap());
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn radio_accesskit_click_replaces_the_peer_state_without_duplicate_changes_or_identity_changes() {
    use glory_render::{CheckMode, Event, Orientation, SemanticId, Semantics};
    let app = WindowApp::new();
    let calls = Rc::new(Cell::new(0));
    let output = calls.clone();
    let member = Some(Box::new(Semantics {
        check_group: Some(SemanticId::new("ax-radio").unwrap()),
        navigation_item: true,
        ..Default::default()
    }));
    app.mount(
        view()
            .name("Delivery")
            .role(glory_render::Role::RadioGroup)
            .semantics(Some(Box::new(Semantics {
                navigation: Some(Orientation::Vertical),
                ..Default::default()
            })))
            .fill((
                check()
                    .label("A")
                    .mode(CheckMode::Radio)
                    .checked(true)
                    .semantics(member.clone()),
                check()
                    .label("B")
                    .mode(CheckMode::Radio)
                    .semantics(member)
                    .on(glory_ui::events::checked_changed, move |event| {
                        assert_eq!(event, Event::CheckedChanged(true));
                        output.set(output.get() + 1);
                    }),
            )),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let children = app.app.tree().node(root).unwrap().children.clone();
    let mut tree = Consumer::new(build(&app, true), true);
    let a = id(&app, children[0]);
    let b = id(&app, children[1]);
    assert_eq!(
        tree.state()
            .node_by_tree_local_id(a, TreeId::ROOT)
            .unwrap()
            .role(),
        AxRole::RadioButton
    );
    assert_eq!(
        tree.state()
            .node_by_tree_local_id(a, TreeId::ROOT)
            .unwrap()
            .data()
            .toggled(),
        Some(Toggled::True)
    );
    assert_eq!(
        tree.state()
            .node_by_tree_local_id(b, TreeId::ROOT)
            .unwrap()
            .data()
            .toggled(),
        Some(Toggled::False)
    );
    assert!(action(&app, &request(b, AxAction::Click, None)).unwrap());
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_eq!(
        tree.state()
            .node_by_tree_local_id(a, TreeId::ROOT)
            .unwrap()
            .data()
            .toggled(),
        Some(Toggled::False)
    );
    assert_eq!(
        tree.state()
            .node_by_tree_local_id(b, TreeId::ROOT)
            .unwrap()
            .data()
            .toggled(),
        Some(Toggled::True)
    );
    assert_eq!(calls.get(), 1);
    assert!(action(&app, &request(b, AxAction::Click, None)).unwrap());
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_eq!(calls.get(), 1);
    assert_eq!(id(&app, children[0]), a);
    assert_eq!(id(&app, children[1]), b);
}

#[test]
fn composite_roles_states_and_live_named_relations_reach_accesskit() {
    use glory_render::{Change, Orientation, Patch, SemanticId, Semantics};
    let app = WindowApp::new();
    let trigger = app.app.scope().cage(None);
    let panel = app.app.scope().cage(None);
    let trigger_name = SemanticId::new("native-tab").unwrap();
    let panel_name = SemanticId::new("native-panel").unwrap();
    app.mount(
        view().fill((
            view()
                .role(Role::TabList)
                .name("Pages")
                .semantics(Some(Box::new(Semantics {
                    navigation: Some(Orientation::Horizontal),
                    ..Default::default()
                })))
                .fill(
                    button()
                        .label("Profile")
                        .node_ref(trigger)
                        .semantics(Some(Box::new(Semantics {
                            id: Some(trigger_name.clone()),
                            controls: Some(panel_name.clone()),
                            tab: true,
                            selected: Some(true),
                            navigation_item: true,
                            ..Default::default()
                        }))),
                ),
            view()
                .role(Role::TabPanel)
                .node_ref(panel)
                .semantics(Some(Box::new(Semantics {
                    id: Some(panel_name.clone()),
                    labelled_by: Some(trigger_name.clone()),
                    described_by: Some(trigger_name),
                    ..Default::default()
                })))
                .fill(text().content("Panel body")),
            view()
                .role(Role::Heading(2))
                .fill(
                    button()
                        .label("Disclosure")
                        .semantics(Some(Box::new(Semantics {
                            expanded: Some(true),
                            popup: Some(glory_render::PopupKind::Dialog),
                            ..Default::default()
                        }))),
                ),
        )),
    )
    .unwrap();
    let mut consumer = Consumer::new(build(&app, true), true);
    let trigger_id = id(&app, trigger.get().unwrap());
    let panel_id = id(&app, panel.get().unwrap());
    let tab_full = nodes(&consumer)
        .into_iter()
        .find(|node| node.role() == AxRole::Tab)
        .unwrap()
        .id();
    let panel_full = nodes(&consumer)
        .into_iter()
        .find(|node| node.role() == AxRole::TabPanel)
        .unwrap()
        .id();
    let tab = consumer.state().node_by_id(tab_full).unwrap();
    assert_eq!(tab.role(), AxRole::Tab);
    assert_eq!(tab.data().is_selected(), Some(true));
    assert_eq!(tab.data().controls(), &[panel_id]);
    let panel_node = consumer.state().node_by_id(panel_full).unwrap();
    assert_eq!(panel_node.role(), AxRole::TabPanel);
    assert_eq!(panel_node.data().labelled_by(), &[trigger_id]);
    assert_eq!(panel_node.data().described_by(), &[trigger_id]);
    assert!(nodes(&consumer).iter().any(|n| n.role() == AxRole::Button
        && n.data().is_expanded() == Some(true)
        && n.data().has_popup() == Some(accesskit::HasPopup::Dialog)));
    app.app
        .scope()
        .patch(
            panel.get().unwrap(),
            Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                id: Some(panel_name),
                hidden: true,
                ..Default::default()
            })))),
        )
        .unwrap();
    consumer.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(consumer.state().node_by_id(panel_full).is_none());
    assert!(
        consumer
            .state()
            .node_by_id(tab_full)
            .unwrap()
            .data()
            .controls()
            .is_empty()
    );
}

#[test]
fn field_states_label_description_error_and_disabled_group_reach_accesskit() {
    use glory_render::{Change, FieldPart, Patch, SemanticId, Semantics};
    let app = WindowApp::new();
    let field = app.app.scope().cage(None);
    let group = app.app.scope().cage(None);
    let label = SemanticId::new("field-label").unwrap();
    let help = SemanticId::new("field-help").unwrap();
    let error = SemanticId::new("field-error").unwrap();
    let states = Semantics {
        id: Some(SemanticId::new("field-control").unwrap()),
        labelled_by: Some(label.clone()),
        described_by: Some(help.clone()),
        error_message: Some(error.clone()),
        required: true,
        readonly: true,
        invalid: true,
        ..Default::default()
    };
    app.mount(
        view()
            .field_part(FieldPart::Fieldset)
            .node_ref(group)
            .fill((
                view()
                    .field_part(FieldPart::Legend)
                    .fill(text().content("Profile")),
                view()
                    .field_part(FieldPart::Label)
                    .semantics(Some(Box::new(Semantics {
                        field_part: Some(FieldPart::Label),
                        id: Some(label),
                        label_for: states.id.clone(),
                        ..Default::default()
                    })))
                    .fill(text().content("Email")),
                view()
                    .semantics(Some(Box::new(Semantics {
                        id: Some(help),
                        ..Default::default()
                    })))
                    .fill(text().content("Help")),
                view()
                    .semantics(Some(Box::new(Semantics {
                        id: Some(error),
                        ..Default::default()
                    })))
                    .fill(text().content("Error")),
                input()
                    .node_ref(field)
                    .value("value")
                    .semantics(Some(Box::new(states))),
            )),
    )
    .unwrap();
    let mut consumer = Consumer::new(build(&app, true), true);
    let axid = nodes(&consumer)
        .into_iter()
        .find(|n| n.role() == AxRole::TextInput)
        .unwrap()
        .id();
    let data = consumer.state().node_by_id(axid).unwrap();
    assert_eq!(data.label().as_deref(), Some("Email"));
    assert_eq!(data.description().as_deref(), Some("Help Error"));
    assert!(data.data().is_required() && data.data().is_read_only());
    assert_eq!(data.data().invalid(), Some(accesskit::Invalid::True));
    assert_eq!(data.data().described_by().len(), 2);
    assert!(data.data().error_message().is_some());
    assert_eq!(data.data().labelled_by().len(), 1);
    assert!(!data.data().supports_action(AxAction::SetValue));
    app.app
        .scope()
        .patch(
            group.get().unwrap(),
            Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                field_part: Some(FieldPart::Fieldset),
                disabled: true,
                ..Default::default()
            })))),
        )
        .unwrap();
    consumer.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        consumer
            .state()
            .node_by_id(axid)
            .unwrap()
            .data()
            .is_disabled()
    );
}

#[test]
fn semantic_roles_labels_passwords_and_list_totals_form_a_valid_consumer_tree() {
    let app = WindowApp::new();
    app.mount(
        group().fill((
            view().name("Panel"),
            text()
                .kind(TextKind::Heading(3))
                .name("Heading name")
                .content("Visible heading"),
            button().label("Prefix").fill(text().content("Suffix")),
            input().name("Editor").value("a🙂"),
            input()
                .name("Secret")
                .purpose(Purpose::Password)
                .value("never-export🙂"),
            check().label("Check label").checked(true),
            image().source("").alt("Image label"),
            image().source("").decorative(),
            link().target("/next").label("Link label"),
            view()
                .role(Role::List)
                .fill(view().role(Role::ListItem).item(Some(ItemPosition {
                    index: 99_999_999,
                    count: Some(100_000_000),
                }))),
            view().role(Role::Status).fill(text().content("Ready")),
            view().role(Role::Alert).fill(text().content("Attention")),
        )),
    )
    .unwrap();
    let update = build(&app, true);
    assert!(!format!("{update:?}").contains("never-export"));
    let tree = Consumer::new(update, true);
    let all = nodes(&tree);
    let find = |role, label: &str| {
        all.iter()
            .find(|n| n.role() == role && n.label().as_deref() == Some(label))
            .expect(label)
    };
    find(AxRole::Group, "Panel");
    assert_eq!(
        find(AxRole::Heading, "Heading name").data().level(),
        Some(2)
    );
    find(AxRole::Button, "Prefix Suffix");
    assert_eq!(find(AxRole::TextInput, "Editor").raw_value(), Some("a🙂"));
    assert_eq!(find(AxRole::PasswordInput, "Secret").raw_value(), None);
    assert_eq!(
        find(AxRole::CheckBox, "Check label").data().toggled(),
        Some(Toggled::True)
    );
    assert_eq!(all.iter().filter(|n| n.role() == AxRole::Image).count(), 1);
    find(AxRole::Image, "Image label");
    assert_eq!(find(AxRole::Link, "Link label").data().url(), Some("/next"));
    assert_eq!(
        all.iter()
            .find(|n| n.role() == AxRole::List)
            .unwrap()
            .data()
            .size_of_set(),
        Some(100_000_000)
    );
    assert_eq!(
        all.iter()
            .find(|n| n.role() == AxRole::ListItem)
            .unwrap()
            .data()
            .position_in_set(),
        Some(99_999_999)
    );
    assert_eq!(
        all.iter()
            .find(|n| n.role() == AxRole::Status)
            .unwrap()
            .data()
            .live(),
        Some(Live::Polite)
    );
    assert_eq!(
        all.iter()
            .find(|n| n.role() == AxRole::Alert)
            .unwrap()
            .data()
            .live(),
        Some(Live::Assertive)
    );
}

#[test]
fn platform_actions_queue_once_and_report_actual_uncontrolled_and_controlled_values() {
    let app = WindowApp::new();
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let value = app.app.scope().cage(String::from("old"));
    app.mount(
        group().fill((
            button()
                .label("Go")
                .on(EventKind::Activate, move |_| counter.set(counter.get() + 1)),
            check().label("Toggle"),
            input()
                .value(value)
                .on(EventKind::ValueChanged, move |event| {
                    if let Event::ValueChanged { value: next, .. } = event {
                        value.revise(|v| *v = next);
                    }
                }),
        )),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let roots = app.app.tree().node(root).unwrap().children.clone();
    let mut tree = Consumer::new(build(&app, true), true);
    assert!(action(&app, &request(id(&app, roots[0]), AxAction::Click, None)).unwrap());
    assert_eq!(
        clicks.get(),
        0,
        "no business reentry from the borrowed adapter"
    );
    app.app.settle(16).unwrap();
    assert_eq!(clicks.get(), 1);
    let checkbox = id(&app, roots[1]);
    assert!(action(&app, &request(checkbox, AxAction::Click, None)).unwrap());
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .any(|n| n.role() == AxRole::CheckBox && n.data().toggled() == Some(Toggled::True))
    );
    let editor = id(&app, roots[2]);
    assert!(action(&app, &request(editor, AxAction::Focus, None)).unwrap());
    assert!(
        action(
            &app,
            &request(
                editor,
                AxAction::SetValue,
                Some(ActionData::Value("new\r\n🙂\0".into()))
            )
        )
        .unwrap()
    );
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_eq!(value.get(), "new🙂�");
    assert_eq!(tree.state().focus().unwrap().role(), AxRole::TextInput);
    assert!(
        nodes(&tree)
            .iter()
            .any(|n| n.role() == AxRole::TextInput && n.raw_value() == Some("new🙂�"))
    );
}

#[test]
fn suspended_hidden_disabled_retired_and_reactivated_ids_cannot_receive_late_actions() {
    let app = WindowApp::new();
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let mounted = button()
        .label("Go")
        .on(EventKind::Activate, move |_| counter.set(counter.get() + 1))
        .mount(app.app.scope())
        .unwrap();
    build(&app, true);
    let node = mounted.roots()[0];
    let old = id(&app, node);
    app.app
        .scope()
        .patch(
            node,
            glory_render::Patch::Enabled(glory_render::Change::Set(false)),
        )
        .unwrap();
    app.app.flush().unwrap();
    assert!(!action(&app, &request(old, AxAction::Click, None)).unwrap());
    app.app
        .scope()
        .patch(
            node,
            glory_render::Patch::Enabled(glory_render::Change::Set(true)),
        )
        .unwrap();
    app.app
        .scope()
        .patch(
            node,
            glory_render::Patch::Style(glory_render::Change::Set(Box::new(
                Style::new().display(glory_render::style::Display::None),
            ))),
        )
        .unwrap();
    app.app.flush().unwrap();
    assert!(!action(&app, &request(old, AxAction::Click, None)).unwrap());
    mounted.suspend().unwrap();
    app.app.flush().unwrap();
    assert!(!action(&app, &request(old, AxAction::Click, None)).unwrap());
    drop(mounted);
    app.app.clear().unwrap();
    build(&app, false);
    assert!(!action(&app, &request(old, AxAction::Click, None)).unwrap());
    app.mount(button().label("New")).unwrap();
    build(&app, false);
    let new = app.app.tree().roots()[0];
    let next = id(&app, new);
    assert!(next.0 > old.0);
    assert!(!action(&app, &request(old, AxAction::Click, None)).unwrap());
    app.accessibility.borrow_mut().clear();
    build(&app, true);
    assert!(id(&app, new).0 > next.0);
    assert!(!action(&app, &request(next, AxAction::Click, None)).unwrap());
    assert_eq!(clicks.get(), 0);
}

#[test]
fn unchanged_trees_emit_no_nodes_and_geometry_scales_once() {
    let app = WindowApp::new();
    app.mount(
        view().style(
            Style::new()
                .width(Length::Px(80.0))
                .height(Length::Px(20.0)),
        ),
    )
    .unwrap();
    app.renderer.set_viewport(800, 600, 2.0).unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let all = nodes(&tree);
    let node = all
        .iter()
        .find(|n| n.role() == AxRole::GenericContainer)
        .unwrap();
    let rect = node.bounding_box().unwrap();
    assert!((rect.width() - 160.0).abs() < 0.04 && (rect.height() - 40.0).abs() < 0.04);
    drop(all);
    let update = build(&app, false);
    assert!(update.nodes.is_empty() && update.tree.is_none());
    tree.update_and_process_changes(update, &mut Changes);
}

#[test]
fn hundred_million_row_exports_keep_live_metadata_bounded_without_source_reads() {
    for adaptive in [false, true] {
        let app = WindowApp::new();
        let position = app.app.scope().cage(0.0);
        let reads = Rc::new(Cell::new(0));
        let observed = reads.clone();
        let widget = list(
            100_000_000usize,
            move |index| {
                observed.set(observed.get() + 1);
                index
            },
            move |_, key| {
                view()
                    .style(Style::new().height(Length::Px(if adaptive {
                        [24.0, 28.0, 32.0][key % 3]
                    } else {
                        24.0
                    })))
                    .fill(text().content(key.to_string()))
            },
        )
        .height(180.0)
        .overscan(2)
        .position(position)
        .locate(|key| Some(*key));
        app.mount(if adaptive {
            widget.measure(24.0).measure_limit(64)
        } else {
            widget.row_height(24.0)
        })
        .unwrap();
        let mut tree = Consumer::new(build(&app, true), true);
        for step in 0..40 {
            position.revise(|p| *p = ((step * 2_365_177) % 99_999_900) as f64 * 24.0);
            app.app.settle(16).unwrap();
            let before = reads.get();
            let update = build(&app, false);
            assert_eq!(reads.get(), before);
            tree.update_and_process_changes(update, &mut Changes);
            let retained = app.accessibility.borrow();
            assert!(
                retained.ids.len() < 100
                    && retained.reverse.len() < 100
                    && retained.previous.len() < 100
            );
            assert!(nodes(&tree).len() < 100);
            assert!(
                nodes(&tree).iter().any(
                    |n| n.role() == AxRole::List && n.data().size_of_set() == Some(100_000_000)
                )
            );
        }
        app.app.clear().unwrap();
        tree.update_and_process_changes(build(&app, false), &mut Changes);
        assert_eq!(nodes(&tree).len(), 1);
        assert!(app.accessibility.borrow().ids.is_empty());
    }
}

#[test]
fn an_offscreen_focus_pin_keeps_its_accessible_identity_and_logical_position() {
    let app = WindowApp::new();
    let position = app.app.scope().cage(0.0);
    let count = app.app.scope().cage(100_000_000usize);
    let reads = Rc::new(Cell::new(0));
    let observed = reads.clone();
    app.mount(
        list(
            count,
            move |index| {
                observed.set(observed.get() + 1);
                index
            },
            |_, key| input().value(format!("draft {key}")),
        )
        .height(180.0)
        .row_height(24.0)
        .overscan(2)
        .position(position)
        .locate(move |key| (*key < count.get()).then_some(*key)),
    )
    .unwrap();
    build(&app, true);
    let mut pending = app.app.tree().roots().to_vec();
    let editor = loop {
        let id = pending.pop().unwrap();
        let tree = app.app.tree();
        let node = tree.node(id).unwrap();
        if matches!(node.descriptor.view().props, Props::Input(_)) {
            break id;
        }
        pending.extend(node.children.iter().rev().copied());
    };
    app.app
        .scope()
        .action(glory_render::Action::Focus(editor))
        .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let focused = id(&app, editor);
    for step in 1..31 {
        position.revise(|p| *p = (step * 3_000_000) as f64 * 24.0);
        app.app.settle(16).unwrap();
        let before = reads.get();
        let update = build(&app, false);
        assert_eq!(reads.get(), before);
        assert_eq!(update.focus, focused);
        tree.update_and_process_changes(update, &mut Changes);
        assert!(nodes(&tree).len() < 100);
        assert!(
            nodes(&tree)
                .iter()
                .any(|n| n.role() == AxRole::ListItem && n.data().position_in_set() == Some(0))
        );
        assert_eq!(id(&app, editor), focused);
    }
    count.revise(|n| *n = 0);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        !action(
            &app,
            &request(
                focused,
                AxAction::SetValue,
                Some(ActionData::Value("late".into()))
            )
        )
        .unwrap()
    );
    assert!(!app.accessibility.borrow().reverse.contains_key(&focused));
}

#[cfg(target_os = "windows")]
#[test]
fn uia_positions_at_the_one_based_i32_boundary_are_unknown_instead_of_overflowing() {
    let app = WindowApp::new();
    let limit = i32::MAX as usize;
    app.mount(view().role(Role::List).fill((
        view().role(Role::ListItem).item(Some(ItemPosition {
            index: limit - 1,
            count: None,
        })),
        view().role(Role::ListItem).item(Some(ItemPosition {
            index: limit,
            count: None,
        })),
        view().role(Role::ListItem).item(Some(ItemPosition {
            index: limit + 1,
            count: None,
        })),
        view().role(Role::ListItem).item(Some(ItemPosition {
            index: 99_999_999,
            count: Some(100_000_000),
        })),
    )))
    .unwrap();
    let tree = Consumer::new(build(&app, true), true);
    let list = tree.state().root().children().next().unwrap();
    let positions: Vec<_> = list
        .children()
        .map(|n| n.data().position_in_set())
        .collect();
    assert_eq!(positions, [Some(limit - 1), None, None, Some(99_999_999)]);
}

#[test]
fn a_platform_scroll_item_request_reveals_a_pinned_row_without_changing_its_editor() {
    for measured in [false, true] {
        let app = WindowApp::new();
        let position = app.app.scope().cage(0.0);
        let reads = Rc::new(Cell::new(0));
        let observed = reads.clone();
        let source = list(
            100_000_000usize,
            move |index| {
                observed.set(observed.get() + 1);
                index
            },
            |_, key| {
                input()
                    .name(format!("Editor {key}"))
                    .value(format!("draft {key}"))
            },
        )
        .height(180.0)
        .overscan(2)
        .position(position)
        .locate(|key| Some(*key));
        app.mount(if measured {
            source.measure(24.0)
        } else {
            source.row_height(24.0)
        })
        .unwrap();
        let tree = Consumer::new(build(&app, true), true);
        let row = nodes(&tree)
            .into_iter()
            .find(|n| n.role() == AxRole::ListItem && n.data().position_in_set() == Some(0))
            .unwrap()
            .id();
        let editor = nodes(&tree)
            .into_iter()
            .find(|n| n.label().as_deref() == Some("Editor 0"))
            .unwrap()
            .id();
        let row_ax = *app
            .accessibility
            .borrow()
            .previous
            .iter()
            .find(|(_, n)| n.role() == AxRole::ListItem)
            .unwrap()
            .0;
        let editor_ax = *app
            .accessibility
            .borrow()
            .previous
            .iter()
            .find(|(_, n)| n.label() == Some("Editor 0"))
            .unwrap()
            .0;
        assert!(action(&app, &request(editor_ax, AxAction::Focus, None)).unwrap());
        build(&app, false);
        position.revise(|p| *p = 90_000_000.0 * 24.0);
        let tree = Consumer::new(build(&app, true), true);
        let pinned = tree.state().node_by_id(row).unwrap();
        assert!(pinned.data().supports_action(AxAction::ScrollIntoView));
        let before = reads.get();
        assert!(action(&app, &request(row_ax, AxAction::ScrollIntoView, None)).unwrap());
        assert_eq!(
            reads.get(),
            before,
            "the platform callback only queues the intent"
        );
        let tree = Consumer::new(build(&app, true), true);
        assert_eq!(tree.state().focus().unwrap().id(), editor);
        assert_eq!(
            tree.state().node_by_id(editor).unwrap().raw_value(),
            Some("draft 0")
        );
        let bounds = tree
            .state()
            .node_by_id(row)
            .unwrap()
            .data()
            .bounds()
            .unwrap();
        assert!(
            bounds.y0 >= -0.1 && bounds.y0 < 180.0,
            "row was not revealed: {bounds:?}"
        );
        assert!(reads.get() - before < 100 && nodes(&tree).len() < 100);
        assert!(
            action(
                &app,
                &request(
                    row_ax,
                    AxAction::ScrollIntoView,
                    Some(ActionData::ScrollHint(::accesskit::ScrollHint::TopEdge))
                )
            )
            .unwrap()
        );
        build(&app, false);
        assert!(
            action(&app, &request(editor_ax, AxAction::ScrollIntoView, None)).is_err(),
            "unadvertised descendant scrolling is not claimed"
        );
        app.app.clear().unwrap();
        build(&app, false);
        assert!(!action(&app, &request(row_ax, AxAction::ScrollIntoView, None)).unwrap());
    }
}

#[test]
fn row_reveal_scrolls_native_ancestors_and_the_document_viewport_after_logical_layout() {
    use glory_render::style::Overflow;
    let app = WindowApp::new();
    app.renderer.set_css(
        ".outer { border:3px solid black } .outer [role=list] { width:400px; margin-left:100px }",
    );
    app.mount(
        group().fill((
            view().style(Style::new().height(Length::Px(1200.0))),
            view()
                .name("Outer")
                .class("outer")
                .style(
                    Style::new()
                        .width(Length::Px(240.0))
                        .height(Length::Px(100.0))
                        .overflow_x(Overflow::Hidden)
                        .overflow_y(Overflow::Hidden),
                )
                .fill((
                    view().style(Style::new().height(Length::Px(200.0))),
                    list(
                        100_000_000usize,
                        |index| index,
                        |_, key| text().content(format!("row {key}")),
                    )
                    .height(180.0)
                    .row_height(24.0)
                    .overscan(2),
                )),
        )),
    )
    .unwrap();
    let tree = Consumer::new(build(&app, true), true);
    let row_full = nodes(&tree)
        .into_iter()
        .find(|n| n.role() == AxRole::ListItem && n.data().position_in_set() == Some(0))
        .unwrap()
        .id();
    let outer_full = nodes(&tree)
        .into_iter()
        .find(|n| n.label().as_deref() == Some("Outer"))
        .unwrap()
        .id();
    let row_ax = *app
        .accessibility
        .borrow()
        .previous
        .iter()
        .find(|(_, n)| n.role() == AxRole::ListItem)
        .unwrap()
        .0;
    let row_node = match app.accessibility.borrow().reverse[&row_ax] {
        Key::Main(id) => id,
        _ => unreachable!(),
    };
    // Repeating the same live target keeps one pending record, never a request history.
    for _ in 0..32 {
        assert!(action(&app, &request(row_ax, AxAction::ScrollIntoView, None)).unwrap());
    }
    assert_eq!(app.accessibility.borrow().pending_reveals, [row_node]);
    let tree = Consumer::new(build(&app, true), true);
    let row = tree
        .state()
        .node_by_id(row_full)
        .unwrap()
        .data()
        .bounds()
        .unwrap();
    let outer = tree
        .state()
        .node_by_id(outer_full)
        .unwrap()
        .data()
        .bounds()
        .unwrap();
    assert!(
        row.y0 >= outer.y0 + 2.9 && row.y0 < outer.y1 - 3.0,
        "row={row:?}, outer={outer:?}"
    );
    assert!(
        row.x0 >= outer.x0 + 2.9 && row.x0 < outer.x1 - 3.0,
        "row={row:?}, outer={outer:?}"
    );
    assert!(
        row.y0 >= 0.0
            && row.y0 < f64::from(app.renderer.0.borrow().dom.doc.viewport().window_size.1)
    );
    assert!(app.renderer.0.borrow().dom.doc.viewport_scroll().y > 0.0);
    assert!(app.accessibility.borrow().pending_reveals.is_empty());
    assert!(
        action(
            &app,
            &request(
                row_ax,
                AxAction::ScrollIntoView,
                Some(ActionData::NumericValue(3.0))
            )
        )
        .is_err()
    );
    assert!(app.accessibility.borrow().pending_reveals.is_empty());
    assert!(action(&app, &request(row_ax, AxAction::ScrollIntoView, None)).unwrap());
    app.app.clear().unwrap();
    build(&app, false);
    assert!(app.accessibility.borrow().pending_reveals.is_empty());
}

#[test]
fn switch_exports_independent_role_actual_engine_checked_and_live_name() {
    let app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let checked = app.app.scope().cage(false);
    app.mount(
        check()
            .mode(glory_render::CheckMode::Switch)
            .label("Notifications")
            .checked(checked)
            .node_ref(reference),
    )
    .unwrap();
    let semantic = reference.get().unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let ax = nodes(&tree)
        .into_iter()
        .find(|node| node.role() == AxRole::Switch)
        .unwrap()
        .id();
    let switch = tree.state().node_by_id(ax).unwrap();
    assert_eq!(switch.role(), AxRole::Switch);
    assert_eq!(switch.label().as_deref(), Some("Notifications"));
    assert_eq!(switch.data().toggled(), Some(Toggled::False));
    checked.revise(|checked| *checked = true);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_eq!(
        tree.state().node_by_id(ax).unwrap().data().toggled(),
        Some(Toggled::True)
    );
    app.app
        .scope()
        .patch(
            semantic,
            glory_render::Patch::Name(glory_render::Change::Set(Some("Sounds".into()))),
        )
        .unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let switch = tree.state().node_by_id(ax).unwrap();
    assert_eq!(switch.label().as_deref(), Some("Sounds"));
    assert_eq!(switch.data().toggled(), Some(Toggled::True));
}

#[test]
fn toggle_button_state_updates_accesskit_without_replacing_native_button() {
    use glory_render::{Change, Patch, Semantics};
    let app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    app.mount(
        button()
            .label("Bold")
            .semantics(Some(Box::new(Semantics {
                pressed: Some(false),
                ..Default::default()
            })))
            .node_ref(reference),
    )
    .unwrap();
    let update = build(&app, true);
    let tree = Consumer::new(update, true);
    let button = reference.get().unwrap();
    let identity = nodes(&tree)
        .into_iter()
        .find(|node| node.role() == AxRole::Button)
        .unwrap()
        .id();
    let node = nodes(&tree)
        .into_iter()
        .find(|node| node.id() == identity)
        .unwrap();
    assert_eq!(node.role(), AxRole::Button);
    assert_eq!(node.toggled(), Some(Toggled::False));
    app.app
        .scope()
        .patch(
            button,
            Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                pressed: Some(true),
                ..Default::default()
            })))),
        )
        .unwrap();
    let update = build(&app, false);
    let mut tree = tree;
    tree.update_and_process_changes(update, &mut Changes);
    let node = nodes(&tree)
        .into_iter()
        .find(|node| node.id() == identity)
        .unwrap();
    assert_eq!(node.toggled(), Some(Toggled::True));
    assert!(node.data().supports_action(AxAction::Click));
}
