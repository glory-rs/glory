use super::*;
use blitz_traits::events::{
    BlitzPointerEvent, BlitzPointerId, BlitzWheelDelta, BlitzWheelEvent, MouseEventButton,
    MouseEventButtons, Point, PointerCoords,
};
use glory_core::{Cage, Host};
use glory_ui::{Widget, button, component, events::activate, input, list, text, view};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Poll, Waker};
use std::time::{Duration, Instant};

pub(super) fn tick(app: &mut WindowApp) -> bool {
    app.poll(Some(Context::from_waker(Waker::noop())))
}
pub(super) fn pointer(x: f32, y: f32, pressed: bool) -> BlitzPointerEvent {
    BlitzPointerEvent {
        id: BlitzPointerId::Mouse,
        is_primary: true,
        coords: PointerCoords {
            page_x: x,
            page_y: y,
            client_x: x,
            client_y: y,
            screen_x: x,
            screen_y: y,
        },
        button: MouseEventButton::Main,
        buttons: if pressed {
            MouseEventButtons::Primary
        } else {
            MouseEventButtons::empty()
        },
        mods: Default::default(),
        details: Default::default(),
        element: Point::default(),
        active_pointers: Default::default(),
    }
}
fn native_key(app: &mut WindowApp, value: Key) {
    let text = match &value {
        Key::Character(value) => Some(value.as_str().into()),
        _ => None,
    };
    app.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: value,
        code: keyboard_types::Code::Unidentified,
        modifiers: Modifiers::empty(),
        location: keyboard_types::Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: blitz_traits::events::KeyState::Pressed,
        text,
    }));
}
pub(super) fn native_focus(app: &mut WindowApp, node: NodeId) {
    app.app
        .scope()
        .action(glory_render::Action::Focus(node))
        .unwrap();
    app.app.flush().unwrap();
}
fn selection_now(app: &WindowApp, node: NodeId) -> (usize, usize) {
    let QueryValue::Selection { start, end } = app.renderer.0.borrow().dom.selection(node).unwrap()
    else {
        panic!("selection")
    };
    (start, end)
}
fn native_focused(app: &WindowApp) -> NodeId {
    let state = app.renderer.0.borrow();
    state
        .dom
        .owner(state.dom.doc.get_focussed_node_id().unwrap())
        .unwrap()
}
pub(super) fn bounds_now(app: &WindowApp, node: NodeId) -> Rect {
    let output = Rc::new(RefCell::new(None));
    let result = output.clone();
    app.app
        .request(Query::Bounds(node), move |value| {
            *result.borrow_mut() = Some(value)
        })
        .unwrap();
    app.app.pump().unwrap();
    let value = output.borrow_mut().take().unwrap().unwrap();
    let QueryValue::Bounds(rect) = value else {
        panic!("bounds")
    };
    rect
}
fn visible_focus(app: &WindowApp) {
    let root = app.app.tree().roots()[1];
    let outer = bounds_now(app, root);
    let inner = bounds_now(app, native_focused(app));
    assert!(
        inner.y >= outer.y - 0.03 && inner.y + inner.height <= outer.y + outer.height + 0.03,
        "focused {inner:?} outside {outer:?}"
    );
}
fn click(app: &mut WindowApp) {
    let root = app.app.tree().roots()[0];
    let node = app.renderer.engine_id(root, Part::Main).unwrap();
    let rect = app
        .renderer
        .inspect(|doc| doc.get_client_bounding_rect(node).unwrap());
    let (x, y) = (
        (rect.x + rect.width / 2.0) as f32,
        (rect.y + rect.height / 2.0) as f32,
    );
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
}
#[test]
fn native_activation_is_queued_outside_engine_borrows_and_idle_poll_does_not_dirty_frames() {
    let mut app = WindowApp::new();
    let count = app.app.scope().cage(0);
    app.mount(
        button()
            .label("Count")
            .fill(text().content(count.map(|n| n.to_string())))
            .on(activate, move |_| count.revise(|n| *n += 1)),
    )
    .unwrap();
    tick(&mut app);
    assert!(!tick(&mut app));
    let revision = app.renderer.stats().visual_revision;
    for _ in 0..50 {
        assert!(!tick(&mut app));
    }
    assert_eq!(revision, app.renderer.stats().visual_revision);
    click(&mut app);
    assert_eq!(count.get(), 0, "engine callbacks only enqueue");
    assert!(tick(&mut app));
    assert_eq!(count.get(), 1);
    assert!(!tick(&mut app));
}

#[test]
fn window_observation_turn_has_a_separate_budget_and_yields_then_recovers() {
    use glory_render::profile::{Phase, Profile};
    use glory_render::style::{Length, Style};
    let mut app = WindowApp::new();
    let profile = Profile::native();
    app.app.set_profile(Some(profile.clone()));
    let active = app.app.scope().cage(true);
    let width = app.app.scope().cage(100.);
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    app.mount(
        view()
            .style(width.map(|w| Style::new().width(Length::Px(*w)).height(Length::Px(20.))))
            .on(glory_ui::events::resize, move |event| {
                if let Event::Resize { size, .. } = event {
                    seen.set(seen.get() + 1);
                    if active.get() {
                        width.revise(|v| *v = if size.width < 100.5 { 101. } else { 100. });
                    }
                }
            }),
    )
    .unwrap();
    for _ in 0..2 {
        assert!(profile.reset());
        tick(&mut app);
        assert_eq!(
            profile.snapshot().sample(Phase::Observe).calls,
            App::DEFAULT_SETTLE_PASSES as u64
        );
        assert!(app.renderer.needs_observe());
    }
    assert_eq!(calls.get(), 2 * App::DEFAULT_SETTLE_PASSES);
    active.revise(|v| *v = false);
    tick(&mut app);
    assert!(!app.renderer.needs_observe());
    let before = calls.get();
    assert!(!tick(&mut app));
    assert_eq!(calls.get(), before);
}
#[test]
fn self_waking_local_tasks_yield_and_window_drop_cancels_before_executor_close() {
    let mut app = WindowApp::new();
    let polls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    struct Cleanup(Rc<Cell<usize>>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let cleanup = Cleanup(drops.clone());
    app.app
        .scope()
        .owner()
        .spawn(std::future::poll_fn({
            let polls = polls.clone();
            move |cx| {
                let _keep = &cleanup;
                polls.set(polls.get() + 1);
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }))
        .unwrap();
    assert_eq!(polls.get(), 0);
    tick(&mut app);
    assert_eq!(polls.get(), 64, "one host tick has a task budget");
    assert_eq!(app.executor.len(), 1);
    let executor = app.executor.clone();
    drop(app);
    assert_eq!(drops.get(), 1);
    assert!(executor.is_empty());
    executor.close();
    assert!(executor.spawn(Box::pin(async {})).is_err());
}
#[test]
fn cpu_surface_guards_run_before_native_allocation() {
    let mut renderer = CpuRenderer::with_limit(4096);
    renderer.set_size(u32::MAX, u32::MAX);
    assert!(matches!(
        renderer.error.take(),
        Some(ShellError::Surface { .. })
    ));
    renderer.set_size(100, 100);
    assert!(matches!(
        renderer.error.take(),
        Some(ShellError::Surface { .. })
    ));
    renderer.set_size(0, u32::MAX);
    assert!(!renderer.is_active());
    assert_eq!(renderer.frames(), 0);
}

struct Live(Rc<Cell<usize>>);
impl Live {
    fn new(count: Rc<Cell<usize>>) -> Self {
        count.set(count.get() + 1);
        Self(count)
    }
}
impl Drop for Live {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}
struct Data {
    runtime: RuntimeId,
    count: Cage<usize>,
    position: Cage<f64>,
    reads: Rc<Cell<usize>>,
    live: Rc<Cell<usize>>,
    async_ready: Rc<Cell<bool>>,
    logical: Rc<Cell<f64>>,
}
fn fixture(adaptive: bool) -> (WindowApp, Data) {
    let app = WindowApp::new();
    app.renderer
        .set_css("body{font:14px sans-serif}button{display:block;width:100px;height:30px} [data-gly-virtual=row]{box-sizing:border-box}.entry{display:flex;align-items:center;gap:4px}.editor{width:120px;height:20px;padding:0;box-sizing:border-box}");
    let count = app.app.scope().cage(0);
    let position = app.app.scope().cage(0.0);
    let reads = Rc::new(Cell::new(0));
    let live = Rc::new(Cell::new(0));
    let logical = Rc::new(Cell::new(0.0));
    let observed = logical.clone();
    app.mount(
        button()
            .label(count.map(|n| format!("Count {n}")))
            .on(activate, move |_| count.revise(|n| *n += 1)),
    )
    .unwrap();
    let source_reads = reads.clone();
    let row_live = live.clone();
    let widget = list(
        100_000_000_usize,
        move |index| {
            source_reads.set(source_reads.get() + 1);
            index
        },
        move |index, key| {
            let live = row_live.clone();
            component(move |scope| {
                let guard = Live::new(live.clone());
                scope.owner().spawn(async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                })?;
                let draft = scope.cage(format!("draft🙂:{key}"));
                view()
                    .class("entry")
                    .style(glory_render::style::Style::new().height(
                        glory_render::style::Length::Px(if adaptive {
                            [24., 28., 32.][key % 3]
                        } else {
                            24.
                        }),
                    ))
                    .fill((
                        text().content(index.map(|n| format!("row {n}"))),
                        input()
                            .purpose(if adaptive {
                                glory_render::Purpose::Password
                            } else {
                                glory_render::Purpose::Text
                            })
                            .class("editor")
                            .value(draft)
                            .on(glory_ui::events::value_changed, move |event| {
                                if let Event::ValueChanged { value, .. } = event {
                                    draft.revise(|draft| *draft = value);
                                }
                            }),
                    ))
                    .show_in(scope)
            })
        },
    )
    .height(180.0)
    .overscan(2)
    .position(position)
    .on_scroll(move |position| observed.set(position))
    .locate(|key| Some(*key));
    app.mount(if adaptive {
        widget.measure(24.0).measure_limit(64)
    } else {
        widget.row_height(24.0)
    })
    .unwrap();
    let runtime = app.app.runtime().id();
    (
        app,
        Data {
            runtime,
            count,
            position,
            reads,
            live,
            async_ready: Rc::default(),
            logical,
        },
    )
}

#[test]
fn native_wheel_bursts_observe_the_final_offset_without_scaling_or_scanning_the_source() {
    for adaptive in [false, true] {
        let (mut app, data) = fixture(adaptive);
        data.position.revise(|position| *position = 24_000_000.0);
        tick(&mut app);
        assert!(!app.renderer.needs_observe());
        let root = app.app.tree().roots()[1];
        let (rect, _) = app.renderer.0.borrow_mut().dom.geometry(root).unwrap();
        let coords = pointer((rect.x + 10.0) as f32, (rect.y + 10.0) as f32, false).coords;
        app.handle_ui_event(UiEvent::PointerMove(pointer(
            coords.page_x,
            coords.page_y,
            false,
        )));
        let wheel = || {
            UiEvent::Wheel(BlitzWheelEvent {
                delta: BlitzWheelDelta::Pixels(0., -1.),
                coords,
                buttons: MouseEventButtons::empty(),
                mods: Default::default(),
                element: Point::default(),
            })
        };
        let before = data.logical.get();
        app.handle_ui_event(wheel());
        tick(&mut app);
        assert!(!app.renderer.needs_observe());
        assert!(
            (data.logical.get() - before - 1.).abs() < 0.03,
            "adaptive={adaptive}: {}",
            data.logical.get() - before
        );
        let before = data.logical.get();
        let reads = data.reads.get();
        for _ in 0..200 {
            app.handle_ui_event(wheel());
        }
        assert_eq!(
            data.reads.get(),
            reads,
            "native handlers do not enter row factories"
        );
        tick(&mut app);
        assert!(!app.renderer.needs_observe());
        assert!((data.logical.get() - before - 200.).abs() < 0.03);
        assert!(data.reads.get() - reads < 100);
        assert!(app.app.runtime_stats().tasks <= 16);
        drop(app);
        assert_eq!(data.live.get(), 0);
    }
}
struct Driver {
    shell: Shell,
    data: Vec<Data>,
    phase: usize,
    started: Instant,
    done: Rc<Cell<bool>>,
    maxima: Rc<Cell<(usize, usize, usize)>>,
    frames_before: Vec<u64>,
    pinned: Vec<(RuntimeId, NodeId, EngineId, String, (usize, usize))>,
}
impl ApplicationHandler for Driver {
    fn can_create_surfaces(&mut self, el: &dyn ActiveEventLoop) {
        self.shell.can_create_surfaces(el);
    }
    fn destroy_surfaces(&mut self, el: &dyn ActiveEventLoop) {
        self.shell.destroy_surfaces(el);
    }
    fn window_event(&mut self, el: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.shell.window_event(el, id, event);
    }
    fn proxy_wake_up(&mut self, el: &dyn ActiveEventLoop) {
        self.shell.proxy_wake_up(el);
    }
    fn about_to_wait(&mut self, el: &dyn ActiveEventLoop) {
        assert!(
            self.started.elapsed() < Duration::from_secs(30),
            "window acceptance stalled in phase {}",
            self.phase
        );
        assert!(
            self.shell.error.borrow().is_none(),
            "{:?}",
            self.shell.error.borrow()
        );
        if self.shell.inner.windows.len() != 2 {
            return;
        }
        let mut changed = false;
        for view in self.shell.inner.windows.values_mut() {
            // Before ResumeReady there is no poll context yet; View::poll is
            // intentionally a no-op, not a completed observation turn.
            let observed = self.phase != 25 && view.waker.is_some();
            if self.phase != 25 {
                changed |= view.poll();
            }
            let app = view.downcast_doc_mut::<WindowApp>();
            if observed {
                let state = app.renderer.0.borrow();
                assert!(
                    !app.renderer.needs_observe(),
                    "finite window geometry did not settle before paint: phase={}, error={:?}, observed={}, visual={}, pending={}, inputs={}, dirty={}, completions={}",
                    self.phase,
                    app.error,
                    state.observed_revision,
                    state.dom.revision,
                    state.observation_pending,
                    state.inputs.len(),
                    state.dom.dirty,
                    state.dom.doc.resource_stats().completions
                );
            }
            let (rows, nodes, reads) = self.maxima.get();
            self.maxima.set((
                rows.max(app.app.runtime_stats().tasks),
                nodes.max(app.renderer.stats().engine_nodes),
                reads,
            ));
            assert!(app.app.runtime_stats().tasks <= 16);
            assert!(app.renderer.stats().engine_nodes < 100);
        }
        if changed {
            return;
        }
        if self
            .shell
            .inner
            .windows
            .values()
            .any(|v| v.renderer.frames() == 0)
        {
            return;
        }
        match self.phase {
            0 => {
                for v in self.shell.inner.windows.values_mut() {
                    click(v.downcast_doc_mut::<WindowApp>());
                }
            }
            1 => {
                for data in &self.data {
                    assert_eq!(data.count.get(), 1);
                }
            }
            2..=21 => {
                for data in &self.data {
                    data.position.revise(|p| {
                        *p = if self.phase.is_multiple_of(2) {
                            3e9
                        } else {
                            self.phase as f64 * 71923.0
                        }
                    });
                }
            }
            22 => {
                for data in &self.data {
                    assert!(data.reads.get() < 1500);
                }
                self.frames_before = self
                    .shell
                    .inner
                    .windows
                    .values()
                    .map(|v| v.renderer.frames())
                    .collect();
                let counts: Vec<_> = self
                    .shell
                    .inner
                    .windows
                    .values_mut()
                    .map(|v| v.downcast_doc_mut::<WindowApp>().app.stats().nodes)
                    .collect();
                self.shell.destroy_surfaces(el);
                assert!(
                    self.shell
                        .inner
                        .windows
                        .values()
                        .all(|v| !v.renderer.is_active())
                );
                for (v, count) in self.shell.inner.windows.values_mut().zip(counts) {
                    assert_eq!(v.downcast_doc_mut::<WindowApp>().app.stats().nodes, count);
                }
                self.shell.can_create_surfaces(el);
            }
            23 => {
                if self
                    .shell
                    .inner
                    .windows
                    .values()
                    .any(|v| !v.renderer.is_active())
                {
                    return;
                }
                // Native surface activation can precede the queued ResumeReady
                // callback and its first present. Wait for that actual frame.
                if self
                    .shell
                    .inner
                    .windows
                    .values()
                    .zip(&self.frames_before)
                    .any(|(v, before)| v.renderer.frames() <= *before)
                {
                    return;
                }
                for (v, before) in self
                    .shell
                    .inner
                    .windows
                    .values_mut()
                    .zip(&self.frames_before)
                {
                    assert!(v.renderer.frames() > *before);
                    v.renderer.set_size(0, 0);
                    assert!(!v.renderer.is_active());
                    let (w, h) = v.doc.inner().viewport().window_size;
                    v.renderer.set_size(w, h);
                    assert!(v.renderer.is_active());
                }
                let windows: Vec<_> = self.shell.inner.windows.keys().copied().collect();
                for id in windows {
                    let applied = self
                        .shell
                        .request_size(id, winit::dpi::PhysicalSize::new(360, 280).into())
                        .unwrap();
                    eprintln!("resize request: applied={applied:?}");
                }
            }
            24 => {
                if self
                    .shell
                    .inner
                    .windows
                    .values()
                    .any(|v| v.window.surface_size() != winit::dpi::PhysicalSize::new(360, 280))
                {
                    return;
                }
                if self.shell.inner.windows.values().any(|view| {
                    let size = view.window.surface_size();
                    let insets = view.safe_area_insets;
                    view.doc.inner().viewport().window_size
                        != (
                            size.width.saturating_sub(insets.left + insets.right),
                            size.height.saturating_sub(insets.top + insets.bottom),
                        )
                }) {
                    return;
                }
                for view in self.shell.inner.windows.values() {
                    let size = view.window.surface_size();
                    let insets = view.safe_area_insets;
                    assert_eq!(
                        view.doc.inner().viewport().window_size,
                        (
                            size.width.saturating_sub(insets.left + insets.right),
                            size.height.saturating_sub(insets.top + insets.bottom)
                        )
                    );
                }
                for v in self.shell.inner.windows.values_mut() {
                    let app = v.downcast_doc_mut::<WindowApp>();
                    assert!(app.executor.len() < 16);
                    let data = self
                        .data
                        .iter()
                        .find(|d| d.runtime == app.app.runtime().id())
                        .unwrap();
                    start_timer(app, data.count, data.async_ready.clone());
                }
            }
            25 => {
                // No manual document polling in this phase. Only the real
                // cross-thread waker can cause the host to poll and publish.
                if self.data.iter().any(|data| !data.async_ready.get()) {
                    return;
                }
                for data in &self.data {
                    assert_eq!(data.count.get(), 11);
                }
                for view in self.shell.inner.windows.values_mut() {
                    let app = view.downcast_doc_mut::<WindowApp>();
                    let root = app.app.tree().roots()[1];
                    let surface = app.app.tree().node(root).unwrap().children[0];
                    let rows = app.app.tree().node(surface).unwrap().children.clone();
                    let row = rows[rows.len() / 2];
                    let content = app.app.tree().node(row).unwrap().children[0];
                    let editor = app.app.tree().node(content).unwrap().children[1];
                    native_focus(app, editor);
                    native_key(app, Key::Named(NamedKey::End));
                    native_key(app, Key::Character("X".into()));
                    app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit(
                        "中".into(),
                        Some((3, 3)),
                    )));
                    let physical = app.renderer.engine_id(editor, Part::Main).unwrap();
                    let value = app.renderer.inspect(|doc| {
                        doc.get_node(physical)
                            .unwrap()
                            .element_data()
                            .unwrap()
                            .text_input_data()
                            .unwrap()
                            .editor
                            .raw_text()
                            .to_string()
                    });
                    assert!(value.ends_with("X中"));
                    let selection = selection_now(app, editor);
                    self.pinned
                        .push((app.app.runtime().id(), editor, physical, value, selection));
                }
            }
            26 => {
                for data in &self.data {
                    data.position.revise(|position| *position = 0.);
                }
            }
            27 => {
                for view in self.shell.inner.windows.values_mut() {
                    let app = view.downcast_doc_mut::<WindowApp>();
                    let (_, editor, physical, value, selection) = self
                        .pinned
                        .iter()
                        .find(|entry| entry.0 == app.app.runtime().id())
                        .unwrap();
                    assert_eq!(native_focused(app), *editor);
                    assert_eq!(
                        app.renderer.engine_id(*editor, Part::Main).unwrap(),
                        *physical
                    );
                    assert_eq!(selection_now(app, *editor), *selection);
                    assert!(app.renderer.inspect(|doc| {
                        doc.get_node(*physical)
                            .unwrap()
                            .element_data()
                            .unwrap()
                            .text_input_data()
                            .unwrap()
                            .editor
                            .raw_text()
                            == value
                    }));
                    assert!(
                        matches!(&app.app.tree().node(*editor).unwrap().descriptor.props, glory_render::Props::Input(input) if input.value == *value)
                    );
                    app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Commit("文".into())));
                    app.app.pump().unwrap();
                    assert!(
                        app.renderer
                            .0
                            .borrow()
                            .dom
                            .text_value(*editor)
                            .unwrap()
                            .ends_with("X文")
                    );
                    let outside = app.app.tree().roots()[0];
                    native_focus(app, outside);
                }
            }
            28 => {
                for view in self.shell.inner.windows.values_mut() {
                    let app = view.downcast_doc_mut::<WindowApp>();
                    let (_, _, physical, _, _) = self
                        .pinned
                        .iter()
                        .find(|entry| entry.0 == app.app.runtime().id())
                        .unwrap();
                    assert!(
                        app.renderer
                            .inspect(|doc| doc.get_node(*physical).is_none())
                    );
                    let root = app.app.tree().roots()[1];
                    native_focus(app, root);
                    native_key(app, Key::Named(NamedKey::End));
                }
            }
            29 => {
                for view in self.shell.inner.windows.values_mut() {
                    let app = view.downcast_doc_mut::<WindowApp>();
                    let focused = native_focused(app);
                    assert_eq!(
                        app.app
                            .tree()
                            .node(focused)
                            .unwrap()
                            .descriptor
                            .item
                            .unwrap()
                            .index,
                        99_999_999
                    );
                    visible_focus(app);
                    native_key(app, Key::Named(NamedKey::Home));
                }
            }
            30 => {
                for view in self.shell.inner.windows.values_mut() {
                    let app = view.downcast_doc_mut::<WindowApp>();
                    let focused = native_focused(app);
                    assert_eq!(
                        app.app
                            .tree()
                            .node(focused)
                            .unwrap()
                            .descriptor
                            .item
                            .unwrap()
                            .index,
                        0
                    );
                    visible_focus(app);
                    native_key(app, Key::Named(NamedKey::Tab));
                }
            }
            31 => {
                for view in self.shell.inner.windows.values_mut() {
                    let app = view.downcast_doc_mut::<WindowApp>();
                    let focused = native_focused(app);
                    assert!(matches!(
                        app.app.tree().node(focused).unwrap().descriptor.props,
                        glory_render::Props::Input(_)
                    ));
                    native_key(app, Key::Named(NamedKey::End));
                    let (start, end) = selection_now(app, focused);
                    assert_eq!(start, end);
                    assert!(end > 0);
                    visible_focus(app);
                }
            }
            32 => {
                let (rows, nodes, _) = self.maxima.get();
                let reads = self.data.iter().map(|d| d.reads.get()).max().unwrap();
                self.maxima.set((rows, nodes, reads));
                self.done.set(true);
                let windows: Vec<_> = self.shell.inner.windows.keys().copied().collect();
                for id in windows {
                    self.shell.window_event(el, id, WindowEvent::CloseRequested);
                }
                return;
            }
            _ => unreachable!(),
        }
        self.phase += 1;
        for v in self.shell.inner.windows.values_mut() {
            v.poll();
        }
        self.shell.inner.proxy.wake_up();
    }
}

fn start_timer(app: &WindowApp, count: Cage<usize>, ready: Rc<Cell<bool>>) {
    let done = Arc::new(AtomicBool::new(false));
    let mut started = false;
    app.app
        .scope()
        .owner()
        .spawn(async move {
            std::future::poll_fn(move |cx| {
                if done.load(Ordering::Acquire) {
                    return Poll::Ready(());
                }
                if !started {
                    started = true;
                    let done = done.clone();
                    let waker = cx.waker().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(10));
                        done.store(true, Ordering::Release);
                        waker.wake();
                    });
                }
                Poll::Pending
            })
            .await;
            count.revise(|n| *n += 10);
            ready.set(true);
        })
        .unwrap();
}

#[test]
#[ignore = "requires a real Windows/X11/Wayland display; run explicitly"]
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn real_windows_present_scroll_resume_and_release_owned_tasks() {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "windows")]
    winit::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    #[cfg(target_os = "linux")]
    winit::platform::x11::EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    #[cfg(target_os = "linux")]
    match std::env::var("GLORY_WINDOW_BACKEND").as_deref() {
        Ok("x11") => {
            winit::platform::x11::EventLoopBuilderExtX11::with_x11(&mut builder);
        }
        Ok("wayland") => {
            winit::platform::wayland::EventLoopBuilderExtWayland::with_wayland(&mut builder);
        }
        Err(_) => {}
        value => panic!("invalid GLORY_WINDOW_BACKEND: {value:?}"),
    }
    let event_loop = builder.build().unwrap();
    let (proxy, queue) = BlitzShellProxy::new(event_loop.create_proxy());
    let mut shell = Shell {
        #[cfg(feature = "accesskit")]
        accessibility: Default::default(),
        inner: BlitzApplication::new(proxy.clone(), queue),
        event_loop: None,
        error: Rc::default(),
    };
    let mut data = Vec::new();
    for adaptive in [false, true] {
        let (app, entry) = fixture(adaptive);
        data.push(entry);
        shell.add_window(
            app,
            WindowAttributes::default()
                .with_title("Glory acceptance")
                .with_surface_size(winit::dpi::PhysicalSize::new(320, 240)),
        );
    }
    let live: Vec<_> = data.iter().map(|d| d.live.clone()).collect();
    let counts: Vec<_> = data.iter().map(|d| d.count).collect();
    let (stop, rx) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if rx.recv_timeout(Duration::from_secs(30)).is_err() {
            proxy.wake_up();
        }
    });
    let done = Rc::new(Cell::new(false));
    let maxima = Rc::new(Cell::new((0, 0, 0)));
    event_loop
        .run_app(Driver {
            shell,
            data,
            phase: 0,
            started: Instant::now(),
            done: done.clone(),
            maxima: maxima.clone(),
            frames_before: Vec::new(),
            pinned: Vec::new(),
        })
        .unwrap();
    let _ = stop.send(());
    watchdog.join().unwrap();
    assert!(done.get());
    assert!(live.iter().all(|n| n.get() == 0));
    assert!(counts.iter().all(|count| count.try_get().is_err()));
    eprintln!(
        "native windows: max_tasks={}, max_engine_nodes={}, max_key_reads={}",
        maxima.get().0,
        maxima.get().1,
        maxima.get().2
    );
}
