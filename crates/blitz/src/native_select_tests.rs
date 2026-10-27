use super::focus_tests::{focus, key, settle};
use super::tests::pointer;
use super::*;
use glory_core::Cage;
use glory_render::{SelectEntry, SelectGroup, SelectOption, SelectProps, SemanticId};
use glory_ui::{button, select, view};

pub(super) fn name(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
pub(super) fn config(multiple: bool) -> SelectProps {
    let option = |key: &str, label: &str, disabled| SelectOption {
        key: name(key),
        label: label.into(),
        value: format!("private-{key}"),
        disabled,
    };
    SelectProps {
        multiple,
        placeholder: "请选择".into(),
        entries: Rc::new(vec![
            SelectEntry::Option(option("alpha", "Alpha", false)),
            SelectEntry::Option(option("disabled", "Disabled", true)),
            SelectEntry::Group(SelectGroup {
                label: "Group".into(),
                disabled: false,
                options: vec![
                    option("beta", "Beta", false),
                    option("chinese", "中文", false),
                ],
            }),
            SelectEntry::Group(SelectGroup {
                label: "Unavailable group".into(),
                disabled: true,
                options: vec![option("group-disabled", "No", false)],
            }),
            SelectEntry::Option(option("delta", "Delta", false)),
        ]),
        ..Default::default()
    }
}
pub(super) struct Fixture {
    pub host: WindowApp,
    pub id: NodeId,
    pub selected: Cage<Vec<SemanticId>>,
    pub accept: Cage<bool>,
    pub events: Rc<RefCell<Vec<(Vec<SemanticId>, u64)>>>,
}
pub(super) fn fixture(multiple: bool) -> Fixture {
    let host = WindowApp::new();
    host.renderer.set_viewport(400, 320, 1.).unwrap();
    host.renderer
        .set_css("body{background:white}select{width:180px}button{width:90px;height:30px}");
    let scope = host.app.scope();
    let selected = scope.cage(Vec::<SemanticId>::new());
    let accept = scope.cage(true);
    let node = scope.cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let reassert = scope.clone();
    host.mount(
        view().fill((
            select()
                .configuration(config(multiple))
                .name("Choose a value")
                .selected(selected)
                .node_ref(node)
                .on(EventKind::SelectChanged, move |event| {
                    if let Event::SelectChanged { keys, revision } = event {
                        output.borrow_mut().push((keys.clone(), revision));
                        if accept.get() {
                            selected.revise(|value| *value = keys);
                        } else {
                            reassert
                                .patch(
                                    node.get().expect("mounted"),
                                    glory_render::Patch::SelectSelection {
                                        keys: glory_render::Change::Set(selected.get()),
                                        revision,
                                    },
                                )
                                .unwrap();
                        }
                    }
                }),
            button().label("After"),
        )),
    )
    .unwrap();
    let id = node.get().unwrap();
    Fixture {
        host,
        id,
        selected,
        accept,
        events,
    }
}
fn selected(f: &Fixture) -> Vec<SemanticId> {
    f.host
        .renderer
        .0
        .borrow()
        .dom
        .range(f.id)
        .unwrap()
        .select
        .as_ref()
        .unwrap()
        .selected_keys()
}
fn row_rect(f: &mut Fixture, index: usize) -> Rect {
    let mut state = f.host.renderer.0.borrow_mut();
    state.dom.resolve();
    let row = state.dom.range(f.id).unwrap().select.as_ref().unwrap().rows[index].physical;
    let r = state.dom.doc.get_client_bounding_rect(row).unwrap();
    Rect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
fn click_row(f: &mut Fixture, index: usize) {
    let r = row_rect(f, index);
    let (x, y) = ((r.x + 10.) as f32, (r.y + r.height / 2.) as f32);
    f.host
        .handle_ui_event(UiEvent::PointerMove(pointer(x, y, false)));
    f.host
        .handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    f.host
        .handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    settle(&mut f.host);
}
#[test]
fn native_select_keyboard_popup_cancel_typeahead_and_single_selection_skip_unavailable_rows() {
    let mut f = fixture(false);
    focus(&mut f.host, f.id);
    assert!(selected(&f).is_empty());
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(f.selected.get(), vec![name("alpha")]);
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(f.selected.get(), vec![name("beta")]);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, Some(f.id));
    key(&mut f.host, Key::Named(NamedKey::End), Modifiers::empty());
    assert_eq!(
        f.selected.get(),
        vec![name("beta")],
        "popup cursor does not commit"
    );
    key(
        &mut f.host,
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
    );
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, None);
    key(&mut f.host, Key::Character("中".into()), Modifiers::empty());
    assert_eq!(f.selected.get(), vec![name("chinese")]);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    key(&mut f.host, Key::Named(NamedKey::Home), Modifiers::empty());
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(f.selected.get(), vec![name("alpha")]);
    key(
        &mut f.host,
        Key::Named(NamedKey::PageDown),
        Modifiers::empty(),
    );
    assert_eq!(f.selected.get(), vec![name("delta")]);
    assert_eq!(f.events.borrow().len(), 5);
    assert_eq!(f.host.renderer.0.borrow().focused, Some(f.id));
}
#[test]
fn native_select_pointer_group_disabled_and_equal_rejection_preserve_options_and_revisions() {
    let mut f = fixture(false);
    focus(&mut f.host, f.id);
    let (physical, declaration) = {
        let state = f.host.renderer.0.borrow();
        let c = state.dom.range(f.id).unwrap().select.as_ref().unwrap();
        (
            c.rows.iter().map(|r| r.physical).collect::<Vec<_>>(),
            c.options.entries.clone(),
        )
    };
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    click_row(&mut f, 1);
    assert!(f.selected.get().is_empty());
    // A disabled row does not dismiss the popup.
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, Some(f.id));
    let group = row_rect(&mut f, 2);
    let (x, y) = ((group.x + 10.) as f32, (group.y - 10.) as f32);
    f.host
        .handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    f.host
        .handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    settle(&mut f.host);
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, Some(f.id));
    assert!(f.events.borrow().is_empty());
    click_row(&mut f, 2);
    assert_eq!(f.selected.get(), vec![name("beta")]);
    assert_eq!(f.host.renderer.0.borrow().focused, Some(f.id));
    f.accept.revise(|v| *v = false);
    key(&mut f.host, Key::Named(NamedKey::End), Modifiers::empty());
    assert_eq!(selected(&f), vec![name("beta")]);
    assert_eq!(f.events.borrow().len(), 2);
    let state = f.host.renderer.0.borrow();
    let c = state.dom.range(f.id).unwrap().select.as_ref().unwrap();
    assert!(Rc::ptr_eq(&declaration, &c.options.entries));
    assert_eq!(
        physical,
        c.rows.iter().map(|r| r.physical).collect::<Vec<_>>()
    );
    assert!(f.events.borrow()[1].1 > f.events.borrow()[0].1);
}
#[test]
fn native_select_multiple_toggle_range_disabled_values_and_cursor_only_control_navigation() {
    let mut f = fixture(true);
    focus(&mut f.host, f.id);
    click_row(&mut f, 0);
    click_row(&mut f, 2);
    assert_eq!(f.selected.get(), vec![name("alpha"), name("beta")]);
    click_row(&mut f, 0);
    assert_eq!(f.selected.get(), vec![name("beta")]);
    key(&mut f.host, Key::Named(NamedKey::End), Modifiers::CONTROL);
    assert_eq!(f.selected.get(), vec![name("beta")]);
    key(&mut f.host, Key::Character(" ".into()), Modifiers::empty());
    assert_eq!(f.selected.get(), vec![name("beta"), name("delta")]);
    f.selected
        .revise(|v| *v = vec![name("disabled"), name("group-disabled")]);
    f.host.app.flush().unwrap();
    key(&mut f.host, Key::Named(NamedKey::Home), Modifiers::empty());
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::SHIFT,
    );
    assert_eq!(
        f.selected.get(),
        vec![
            name("alpha"),
            name("disabled"),
            name("beta"),
            name("group-disabled")
        ]
    );
    key(&mut f.host, Key::Character("a".into()), Modifiers::CONTROL);
    assert_eq!(
        f.selected.get(),
        vec![
            name("alpha"),
            name("disabled"),
            name("beta"),
            name("chinese"),
            name("group-disabled"),
            name("delta")
        ]
    );
}
#[test]
fn native_select_popup_lifetime_readonly_disabled_outside_focus_and_disposal_retire_input() {
    use glory_render::{Change, Patch, Semantics};
    let mut f = fixture(false);
    focus(&mut f.host, f.id);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert!(
        f.host
            .app
            .scope()
            .patch(
                f.id,
                Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    readonly: true,
                    ..Default::default()
                }))))
            )
            .is_err()
    );
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, Some(f.id));
    let after = f
        .host
        .app
        .tree()
        .node(f.host.app.tree().roots()[0])
        .unwrap()
        .children[1];
    focus(&mut f.host, after);
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, None);
    focus(&mut f.host, f.id);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    f.host
        .app
        .scope()
        .patch(f.id, Patch::Enabled(Change::Set(false)))
        .unwrap();
    f.host.app.flush().unwrap();
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, None);
    f.host.app.clear().unwrap();
    assert!(f.host.renderer.0.borrow().dom.ranges.is_empty());
    assert!(f.host.renderer.0.borrow().dom.doc.top_layers().is_empty());
}
#[cfg(feature = "raster")]
#[test]
fn native_select_real_cpu_fonts_and_selected_rows_render_at_one_and_two_scales() {
    for scale in [1., 2.] {
        let mut f = fixture(false);
        f.host
            .renderer
            .set_viewport((400. * scale) as u32, (320. * scale) as u32, scale)
            .unwrap();
        focus(&mut f.host, f.id);
        let mut raster = crate::Raster::new();
        raster.draw(&f.host.renderer).unwrap();
        let closed = raster.pixels().to_vec();
        key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
        raster.draw(&f.host.renderer).unwrap();
        assert_ne!(closed, raster.pixels());
        if let Ok(dir) = std::env::var("GLORY_SELECT_ARTIFACTS") {
            std::fs::create_dir_all(&dir).unwrap();
            image::save_buffer(
                format!("{dir}/popup-{scale}.png"),
                raster.pixels(),
                (400. * scale) as u32,
                (320. * scale) as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        click_row(&mut f, 2);
        raster.draw(&f.host.renderer).unwrap();
        assert_ne!(closed, raster.pixels());
        let r = row_rect(&mut f, 2);
        assert!(r.width > 100.);
        if let Ok(dir) = std::env::var("GLORY_SELECT_ARTIFACTS") {
            std::fs::create_dir_all(&dir).unwrap();
            image::save_buffer(
                format!("{dir}/single-{scale}.png"),
                raster.pixels(),
                (400. * scale) as u32,
                (320. * scale) as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}

#[test]
fn native_select_suspension_resume_and_drop_keep_only_current_popup_lease() {
    let mut host = WindowApp::new();
    let mounted = host
        .app
        .scope()
        .mount_with(|| select().configuration(config(false)))
        .unwrap();
    host.app.flush().unwrap();
    let id = mounted.roots()[0];
    focus(&mut host, id);
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    let row = host
        .renderer
        .0
        .borrow()
        .dom
        .range(id)
        .unwrap()
        .select
        .as_ref()
        .unwrap()
        .rows[0]
        .physical;
    mounted.suspend().unwrap();
    host.app.flush().unwrap();
    assert_eq!(host.renderer.0.borrow().dom.open_select, None);
    assert!(host.renderer.0.borrow().dom.doc.top_layers().is_empty());
    mounted.place_before(None).unwrap();
    host.app.flush().unwrap();
    focus(&mut host, id);
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        host.renderer
            .0
            .borrow()
            .dom
            .range(id)
            .unwrap()
            .select
            .as_ref()
            .unwrap()
            .rows[0]
            .physical,
        row
    );
    drop(mounted);
    host.app.flush().unwrap();
    assert!(host.renderer.0.borrow().dom.doc.top_layers().is_empty());
    assert!(host.renderer.0.borrow().dom.ranges.is_empty());
}

#[test]
fn native_select_touch_pan_and_wheel_stay_in_popup_without_accidental_selection() {
    use blitz_traits::events::{BlitzPointerId, BlitzWheelDelta, BlitzWheelEvent};
    let mut f = fixture(false);
    f.host.app.clear().unwrap();
    let entries = (0..60)
        .map(|i| {
            SelectEntry::Option(SelectOption {
                key: name(&format!("row-{i}")),
                label: format!("Row {i}"),
                value: i.to_string(),
                disabled: false,
            })
        })
        .collect();
    let selected = f.selected;
    f.host
        .mount(
            select()
                .configuration(SelectProps {
                    entries: Rc::new(entries),
                    ..Default::default()
                })
                .selected(selected),
        )
        .unwrap();
    f.id = f.host.app.tree().roots()[0];
    focus(&mut f.host, f.id);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    let r = row_rect(&mut f, 4);
    let x = (r.x + 15.) as f32;
    let y = (r.y + 10.) as f32;
    let before = f.host.renderer.0.borrow().dom.doc.viewport_scroll();
    let touch = |x, y, down| {
        let mut p = pointer(x, y, down);
        p.id = BlitzPointerId::Finger(7);
        p
    };
    f.host
        .handle_ui_event(UiEvent::PointerDown(touch(x, y, true)));
    f.host
        .handle_ui_event(UiEvent::PointerMove(touch(x, y - 70., true)));
    f.host
        .handle_ui_event(UiEvent::PointerMove(touch(x, y - 110., true)));
    f.host
        .handle_ui_event(UiEvent::PointerUp(touch(x, y - 110., false)));
    settle(&mut f.host);
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, Some(f.id));
    assert!(selected.get().is_empty());
    let panel = f
        .host
        .renderer
        .0
        .borrow()
        .dom
        .range(f.id)
        .unwrap()
        .select
        .as_ref()
        .unwrap()
        .panel;
    assert!(
        f.host
            .renderer
            .0
            .borrow()
            .dom
            .doc
            .get_node(panel)
            .unwrap()
            .scroll_offset()
            .y
            > 0.
    );
    f.host.handle_ui_event(UiEvent::Wheel(BlitzWheelEvent {
        coords: pointer(x, y - 70., false).coords,
        buttons: Default::default(),
        element: Default::default(),
        delta: BlitzWheelDelta::Pixels(0., -10000.),
        mods: Default::default(),
    }));
    settle(&mut f.host);
    assert_eq!(f.host.renderer.0.borrow().dom.doc.viewport_scroll(), before);
    key(&mut f.host, Key::Named(NamedKey::End), Modifiers::empty());
    assert!(
        f.host
            .renderer
            .0
            .borrow()
            .dom
            .doc
            .get_node(panel)
            .unwrap()
            .scroll_offset()
            .y
            > 0.
    );
    key(&mut f.host, Key::Named(NamedKey::Home), Modifiers::empty());
    assert_eq!(
        f.host
            .renderer
            .0
            .borrow()
            .dom
            .doc
            .get_node(panel)
            .unwrap()
            .scroll_offset()
            .y,
        0.
    );
    assert_eq!(f.host.renderer.0.borrow().dom.doc.viewport_scroll(), before);
}

#[test]
fn native_select_popup_inside_modal_does_not_dismiss_parent_and_escape_closes_only_popup() {
    use glory_render::style::{Length, Style};
    use glory_render::{FocusMode, Layer, Role, Semantics};
    let host = WindowApp::new();
    host.renderer.set_viewport(400, 320, 1.).unwrap();
    let scope = host.app.scope();
    let target = scope.cage(None);
    let dismissed = scope.cage(0usize);
    let chosen = scope.cage(Vec::<SemanticId>::new());
    host.mount(
        view()
            .name("Selection dialog")
            .role(Role::Dialog)
            .focus(FocusMode::Programmatic)
            .style(Style::new().width(Length::Px(220.)).height(Length::Px(48.)))
            .semantics(Some(Box::new(Semantics {
                id: Some(name("selection-dialog")),
                layer: Some(Layer::default()),
                ..Default::default()
            })))
            .on(EventKind::Dismiss, move |_| dismissed.revise(|v| *v += 1))
            .fill(
                select()
                    .configuration(config(false))
                    .node_ref(target)
                    .selected(chosen)
                    .on(EventKind::SelectChanged, move |event| {
                        if let Event::SelectChanged { keys, .. } = event {
                            chosen.revise(|v| *v = keys)
                        }
                    }),
            ),
    )
    .unwrap();
    let id = target.get().unwrap();
    let accept = scope.cage(true);
    let mut f = Fixture {
        host,
        id,
        selected: chosen,
        accept,
        events: Rc::default(),
    };
    focus(&mut f.host, id);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    click_row(&mut f, 2);
    assert_eq!(chosen.get(), vec![name("beta")]);
    assert_eq!(dismissed.get(), 0);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    key(
        &mut f.host,
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
    );
    assert_eq!(dismissed.get(), 0);
    assert_eq!(f.host.renderer.0.borrow().dom.open_select, None);
    key(
        &mut f.host,
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
    );
    assert_eq!(dismissed.get(), 1);
}

#[test]
fn native_select_current_popup_repositions_after_resize_and_offscreen_anchor_without_replacement() {
    let mut f = fixture(false);
    focus(&mut f.host, f.id);
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    let panel = f
        .host
        .renderer
        .0
        .borrow()
        .dom
        .range(f.id)
        .unwrap()
        .select
        .as_ref()
        .unwrap()
        .panel;
    f.host.renderer.set_viewport(260, 160, 2.).unwrap();
    f.host
        .renderer
        .set_css("select{position:relative;top:800px;width:180px}");
    let mut state = f.host.renderer.0.borrow_mut();
    state.dom.resolve();
    let r = state.dom.doc.get_client_bounding_rect(panel).unwrap();
    assert!(
        r.x >= 0. && r.y >= 0. && r.x + r.width <= 130.1 && r.y + r.height <= 80.1,
        "{r:?}"
    );
    assert_eq!(
        state
            .dom
            .range(f.id)
            .unwrap()
            .select
            .as_ref()
            .unwrap()
            .panel,
        panel
    );
}

#[test]
fn native_select_empty_popup_closes_without_selection_and_tab_skips_private_options() {
    let mut host = WindowApp::new();
    let received = Rc::new(Cell::new(0));
    let output = received.clone();
    host.mount(
        view().fill((
            select()
                .name("Empty")
                .on(EventKind::SelectChanged, move |_| {
                    output.set(output.get() + 1)
                }),
            button().label("After"),
        )),
    )
    .unwrap();
    let root = host.app.tree().roots()[0];
    let children = host.app.tree().node(root).unwrap().children.clone();
    let id = children[0];
    focus(&mut host, id);
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(host.renderer.0.borrow().dom.open_select, Some(id));
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(host.renderer.0.borrow().dom.open_select, None);
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    key(&mut host, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(host.renderer.0.borrow().dom.open_select, None);
    assert_eq!(host.renderer.0.borrow().focused, Some(children[1]));
    assert_eq!(received.get(), 0);
}
