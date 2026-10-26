//! Optional native window host. UI state remains local to the event-loop thread.
//! CPU presentation and queued native input share the semantic document. OS IME
//! and accessibility acceptance remain distinct from the protocol tests.
use super::*;
#[path = "editing.rs"]
mod editing;
use editing::ime_event;
#[cfg(all(
    feature = "accesskit",
    not(any(target_os = "android", target_os = "ios"))
))]
#[path = "accessibility.rs"]
mod accessibility;
#[cfg(all(
    feature = "accesskit",
    not(any(target_os = "android", target_os = "ios"))
))]
#[path = "accesskit_platform.rs"]
mod accesskit_platform;
use std::cell::{Cell, Ref, RefMut};
use std::sync::Arc;
use std::task::Context;

use anyrender::{WindowHandle, WindowRenderer};
type VelloCpuWindowRenderer = softbuffer_window_renderer::SoftbufferWindowRenderer<
    anyrender_vello_cpu::VelloCpuImageRenderer,
>;
use blitz_dom::{DocGuard, DocGuardMut, Document, EventDriver, EventHandler};
use blitz_shell::{BlitzApplication, BlitzShellEvent, BlitzShellProxy, WindowConfig};
use blitz_traits::events::{
    BlitzImeEvent, BlitzKeyEvent, DomEvent, DomEventData, EventState, UiEvent,
};
use glory_core::executor::Executor;
use glory_ui::{App, Widget};
use keyboard_types::{Key, Modifiers, NamedKey};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;
pub use winit::window::WindowAttributes;
use winit::window::WindowId;

#[derive(Debug)]
pub enum ShellError {
    Ui(glory_ui::Error),
    Window(String),
    Surface {
        width: u32,
        height: u32,
        limit: usize,
    },
}
impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ShellError {}
impl From<glory_ui::Error> for ShellError {
    fn from(error: glory_ui::Error) -> Self {
        Self::Ui(error)
    }
}

/// Owns one App/document and its local tasks. Move it into Shell::add_window.
/// Dropping the window drops App before the executor, cancelling owned tasks.
pub struct WindowApp {
    app: App,
    renderer: Renderer,
    executor: Rc<Executor>,
    revision: Cell<u64>,
    error: Option<glory_ui::Error>,
    #[cfg(all(
        feature = "accesskit",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    accessibility: RefCell<accessibility::Tree>,
    #[cfg(all(
        feature = "accesskit",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    requested_visible: bool,
}
impl Default for WindowApp {
    fn default() -> Self {
        Self::new()
    }
}
impl WindowApp {
    pub fn new() -> Self {
        Self::with_task_limit(4096)
    }
    pub fn with_task_limit(limit: usize) -> Self {
        Self::with_options(Config::default(), limit)
    }
    pub fn with_config(config: Config) -> Self {
        Self::with_options(config, 4096)
    }
    pub fn with_options(config: Config, limit: usize) -> Self {
        let executor = Rc::new(Executor::new(limit));
        let mut renderer = None;
        let app = App::with_host(executor.clone(), |runtime| {
            let value = Renderer::configured_with(runtime, true, config);
            renderer = Some(value.clone());
            value
        });
        Self {
            app,
            renderer: renderer.expect("renderer factory"),
            executor,
            revision: Cell::new(0),
            error: None,
            #[cfg(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            accessibility: RefCell::default(),
            #[cfg(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            requested_visible: true,
        }
    }
    pub fn app(&self) -> &App {
        &self.app
    }
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }
    pub fn mount(&self, widget: impl Widget) -> glory_ui::Result {
        self.app.mount(widget)
    }
    pub fn mount_with<W: Widget>(&self, factory: impl FnOnce() -> W) -> glory_ui::Result {
        self.app.mount_with(factory)
    }
    /// Observe one bounded native host turn: poll ready tasks, ingest resources
    /// and settle semantic work. The caller supplies its event-loop waker; this
    /// does not create an OS window or confirm that pixels were displayed.
    pub fn observe(&self, context: &Context<'_>) -> glory_ui::Result<bool> {
        self.renderer.observe_resources(context.waker());
        self.executor.observe(context.waker());
        self.executor.dirty();
        // Bound one event-loop turn, including a future that wakes itself.
        for _ in 0..64 {
            if !self.executor.poll_one() {
                break;
            }
        }
        let turn = self.app.settle(App::DEFAULT_SETTLE_PASSES)?;
        // Source writes during observation have now been flushed. Ready task
        // wakes remain queued and retain the separate 64-poll turn budget.
        self.executor.dirty();
        let revision = self.renderer.stats().visual_revision;
        let changed = self.revision.replace(revision) != revision;
        if turn.pending || self.executor.work() {
            // Budget exhaustion yields; converged geometry needs no extra pass.
            // Only ids/wakers cross the event loop; never borrow UI from wake().
            context.waker().wake_by_ref();
        }
        Ok(changed)
    }
}
impl Document for WindowApp {
    fn inner(&self) -> DocGuard<'_> {
        DocGuard::RefCell(Ref::map(self.renderer.0.borrow(), |state| &state.dom.doc))
    }
    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        // The shell explicitly invalidates on viewport/resource/input changes.
        // Invalidating every mutable borrow would make redraw/poll loop forever.
        DocGuardMut::RefCell(RefMut::map(self.renderer.0.borrow_mut(), |state| {
            &mut state.dom.doc
        }))
    }
    fn handle_ui_event(&mut self, mut event: UiEvent) {
        editing::capture_before(&mut self.renderer.0.borrow_mut(), &event);
        editing::sanitize_ime(&self.renderer.0.borrow(), &mut event);
        self.renderer.invalidate();
        let collector = Events(self.renderer.clone());
        EventDriver::new(self, collector).handle_ui_event(event);
        let mut state = self.renderer.0.borrow_mut();
        layers::lock_viewport(&mut state);
        state.sync_focus();
        if let Some(error) = state.input_error.take() {
            self.error = Some(error.into());
        }
    }
    fn poll(&mut self, context: Option<Context>) -> bool {
        if self.error.is_some() {
            return false;
        }
        let Some(context) = context else { return false };
        match self.observe(&context) {
            Ok(changed) => changed,
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }
}
struct Events(Renderer);
impl EventHandler for Events {
    fn handle_event(
        &mut self,
        chain: &[EngineId],
        event: &mut DomEvent,
        _doc: &mut dyn Document,
        event_state: &mut EventState,
    ) {
        let mut state = self.0.0.borrow_mut();
        if matches!(event.data, DomEventData::Click(_)) && std::mem::take(&mut state.suppress_click)
        {
            event_state.prevent_default();
            return;
        }
        if focus_scope::prevent_pointer_focus(&state, event.target, &event.data) {
            event_state.prevent_default();
        }
        if focus_scope::receive(&mut state, event.target, &event.data)
            || layers::receive(&mut state, event.target, &event.data)
        {
            event_state.prevent_default();
            return;
        }
        // The event chain starts at the target. Do not copy the whole node map
        // for each input: work is proportional to event ancestry, not list size.
        let node = chain
            .iter()
            .find_map(|node| {
                state
                    .dom
                    .controls
                    .get(node)
                    .or_else(|| state.dom.owners.get(node))
                    .copied()
            })
            .or_else(|| state.dom.owner(event.target));
        if let Some(node) = node {
            // Engine defaults run after this callback. Inert content must not
            // acquire a caret/selection or a context action before UI routing.
            // Scroll/wheel and terminal loss retain their independent behavior.
            if state.mapper.tree().inert(node)
                && matches!(
                    event.data,
                    DomEventData::PointerDown(_)
                        | DomEventData::PointerMove(_)
                        | DomEventData::MouseDown(_)
                        | DomEventData::TouchStart(_)
                        | DomEventData::DoubleClick(_)
                        | DomEventData::ContextMenu(_)
                        | DomEventData::KeyPress(_)
                        | DomEventData::AppleStandardKeybinding(_)
                )
            {
                event_state.prevent_default();
                return;
            }
            let enabled = state.mapper.tree().active(node)
                && state
                    .mapper
                    .tree()
                    .node(node)
                    .is_ok_and(|_| state.mapper.tree().enabled(node));
            if !enabled
                && matches!(
                    event.data,
                    DomEventData::KeyDown(_)
                        | DomEventData::KeyUp(_)
                        | DomEventData::Ime(_)
                        | DomEventData::Input(_)
                        | DomEventData::Click(_)
                )
            {
                event_state.prevent_default();
                return;
            }
            if let DomEventData::Ime(ime) = &event.data
                && state
                    .dom
                    .ranges
                    .get(&node)
                    .is_some_and(|range| range.edit == EditKind::Text)
            {
                event_state.prevent_default();
                if state
                    .mapper
                    .tree()
                    .node(node)
                    .is_ok_and(|n| n.descriptor.view().readonly())
                {
                    if (matches!(ime, BlitzImeEvent::Disabled | BlitzImeEvent::Commit(_))
                        || matches!(ime, BlitzImeEvent::Preedit(text, _) if text.is_empty()))
                        && let Err(error) = state.end_editing(node)
                    {
                        state.input_error = Some(error);
                    }
                    return;
                }
                if let Err(error) = ime_event(&mut state, node, ime) {
                    state.input_error = Some(error);
                }
                return;
            }
            if let DomEventData::Input(input) = &event.data {
                if state
                    .mapper
                    .tree()
                    .node(node)
                    .is_ok_and(|n| n.descriptor.view().readonly())
                {
                    event_state.prevent_default();
                    return;
                }
                let edit = state.dom.ranges.get(&node).expect("mapped input").edit;
                let value = match edit {
                    EditKind::Rich => {
                        state.input_error = Some(Error::Unsupported("native rich editing"));
                        None
                    }
                    EditKind::Select => {
                        state.input_error =
                            Some(glory_render::Error::Unsupported("native select controls"));
                        None
                    }
                    EditKind::Text => match editing::text_event(&mut state, node, &input.value) {
                        Ok(event) => Some(event),
                        Err(error) => {
                            state.input_error = Some(error);
                            None
                        }
                    },
                    EditKind::Check => state
                        .dom
                        .doc
                        .get_node(event.target)
                        .and_then(|node| node.element_data())
                        .and_then(|element| element.checkbox_input_checked())
                        .map(Event::CheckedChanged),
                    EditKind::None => None,
                };
                if let Some(event) = value {
                    state.inputs.push(Input::Event { node, event });
                }
                return;
            }
        }
        if swipe::input(&mut state, event.target, &event.data) {
            event_state.prevent_default();
            return;
        }
        if matches!(event.data, DomEventData::PointerDown(_)) && state.swipe.is_some() {
            cancel_drag(&mut state);
        }
        if state.swipe.is_none() && drag_input(&mut state, event.target, &event.data) {
            event_state.prevent_default();
            return;
        }
        match &event.data {
            DomEventData::KeyDown(key) | DomEventData::KeyUp(key)
                if keyboard(&mut state, event.target, key) =>
            {
                event_state.prevent_default();
            }
            DomEventData::Focus(_) => {
                state.focus_changed(node);
            }
            DomEventData::Blur(_) if node == state.focused => {
                state.focus_changed(None);
            }
            DomEventData::Click(_) if node.is_some() => {
                let node = node.expect("mapped click");
                let mut cursor = Some(node);
                let mut label_for = None;
                let mut button = None;
                while let Some(id) = cursor {
                    let Ok(current) = state.mapper.tree().node(id) else {
                        break;
                    };
                    if button.is_none()
                        && (matches!(
                            current.descriptor.view().props,
                            glory_render::Props::Button(_)
                        ) || current.descriptor.view().role
                            == Some(glory_render::Role::TreeItem))
                    {
                        button = Some(id);
                    }
                    label_for = current
                        .descriptor
                        .view()
                        .semantics
                        .as_ref()
                        .and_then(|s| s.label_for.clone());
                    if label_for.is_some() {
                        break;
                    }
                    cursor = current.parent;
                }
                if let Some(target) = label_for.and_then(|name| state.mapper.tree().named(&name)) {
                    event_state.prevent_default();
                    state.inputs.push(Input::Event {
                        node,
                        event: Event::Activate,
                    });
                    if state.mapper.tree().enabled(target) && state.mapper.tree().active(target) {
                        let _ = state.focus(target);
                        if state.mapper.tree().node(target).is_ok_and(|n| {
                            matches!(n.descriptor.view().props, glory_render::Props::Check(_))
                                && !n.descriptor.view().readonly()
                        }) {
                            activate_control(&mut state, target);
                        }
                    }
                    return;
                }
                if state.mapper.tree().node(node).is_ok_and(|n| {
                    n.descriptor.view().readonly()
                        && matches!(n.descriptor.view().props, glory_render::Props::Check(_))
                }) {
                    event_state.prevent_default();
                    return;
                }
                if let Some(button) = button
                    && state.mapper.tree().enabled(button)
                    && state.layers.allowed(state.mapper.tree(), button)
                {
                    // beta.4 otherwise treats a Button click as selectable text
                    // and clears focus. Keep the actual intrinsic control as
                    // the pointer focus/return target, including visual slots.
                    event_state.prevent_default();
                    let _ = state.focus(button);
                }
                state.inputs.push(Input::Event {
                    node,
                    event: Event::Activate,
                });
            }
            _ => {}
        }
    }
}

fn drag_observer(state: &State, mut node: NodeId) -> Option<NodeId> {
    loop {
        if state.subscriptions.contains(&(node, EventKind::Drag))
            && state.mapper.tree().active(node)
        {
            return Some(node);
        }
        node = state.mapper.tree().node(node).ok()?.parent?;
    }
}
fn cancel_drag(state: &mut State) {
    if let Some(drag) = state.drag.take()
        && drag.started
    {
        state.inputs.push(Input::Event {
            node: drag.from,
            event: Event::Drag {
                from: drag.from,
                phase: glory_render::DragPhase::Cancel,
                x: drag.x,
                y: drag.y,
            },
        });
    }
}
fn drag_input(state: &mut State, target: EngineId, event: &DomEventData) -> bool {
    use glory_render::DragPhase;
    if matches!(event, DomEventData::Click(_)) {
        return std::mem::take(&mut state.suppress_click);
    }
    let (pointer, phase) = match event {
        DomEventData::PointerDown(pointer) => (pointer, 0),
        DomEventData::PointerMove(pointer) => (pointer, 1),
        DomEventData::PointerUp(pointer) => (pointer, 2),
        DomEventData::PointerCancel(pointer) => (pointer, 3),
        _ => return false,
    };
    if !pointer.is_primary {
        return false;
    }
    let x = f64::from(pointer.client_x());
    let y = f64::from(pointer.client_y());
    if phase == 0 {
        state.suppress_click = false;
        cancel_drag(state);
        if pointer.button != blitz_traits::events::MouseEventButton::Main {
            return false;
        }
        let Some(origin) = state.dom.owner(target) else {
            return false;
        };
        let mut cursor = Some(origin);
        while let Some(id) = cursor {
            let Ok(node) = state.mapper.tree().node(id) else {
                return false;
            };
            if matches!(
                node.descriptor.view().props,
                glory_render::Props::Input(_)
                    | glory_render::Props::Button(_)
                    | glory_render::Props::Link(_)
                    | glory_render::Props::Check(_)
            ) {
                return false;
            }
            cursor = node.parent;
        }
        if let Some(from) = drag_observer(state, origin) {
            let metadata = state
                .mapper
                .tree()
                .node(from)
                .ok()
                .and_then(|node| node.descriptor.view().semantics.as_ref());
            let slider = metadata.is_some_and(|s| s.slider.is_some());
            let capture = metadata.and_then(|s| s.drag_capture);
            let captured =
                slider || capture.is_some() || metadata.is_some_and(|s| s.splitter.is_some());
            let immediate = slider || capture.is_some();
            if immediate {
                if !state.mapper.tree().enabled(from)
                    || state
                        .mapper
                        .tree()
                        .node(from)
                        .is_ok_and(|node| node.descriptor.view().readonly())
                {
                    return false;
                }
                if slider || capture == Some(glory_render::DragCapture::PressFocus) {
                    let _ = state.focus(from);
                }
                state.inputs.push(Input::Event {
                    node: from,
                    event: Event::Drag {
                        from,
                        phase: DragPhase::Start,
                        x,
                        y,
                    },
                });
            }
            state.drag = Some(DragSession {
                from,
                pointer: pointer.id,
                x,
                y,
                started: immediate,
                captured,
            });
        }
        return state.drag.as_ref().is_some_and(|drag| drag.started);
    }
    let Some(mut drag) = state.drag.take() else {
        return false;
    };
    if drag.pointer != pointer.id {
        state.drag = Some(drag);
        return false;
    }
    let capture_live = state
        .mapper
        .tree()
        .node(drag.from)
        .ok()
        .and_then(|node| node.descriptor.view().semantics.as_ref())
        .is_some_and(|s| s.slider.is_some() || s.splitter.is_some() || s.drag_capture.is_some());
    if !state.mapper.tree().active(drag.from)
        || !state.mapper.tree().enabled(drag.from)
        || drag.captured && !capture_live
        || phase == 3
    {
        let started = drag.started;
        state.drag = Some(drag);
        cancel_drag(state);
        return started;
    }
    if phase == 1 {
        if !drag.started && (x - drag.x).hypot(y - drag.y) >= 6.0 {
            drag.started = true;
            state.inputs.push(Input::Event {
                node: drag.from,
                event: Event::Drag {
                    from: drag.from,
                    phase: DragPhase::Start,
                    x: drag.x,
                    y: drag.y,
                },
            });
        }
        if drag.started {
            let next = Input::Event {
                node: drag.from,
                event: Event::Drag {
                    from: drag.from,
                    phase: DragPhase::Move,
                    x,
                    y,
                },
            };
            if matches!(state.inputs.last(),Some(Input::Event {node,event:Event::Drag {phase:DragPhase::Move,..}}) if *node==drag.from)
            {
                *state.inputs.last_mut().expect("last move") = next;
            } else {
                state.inputs.push(next);
            }
        }
        let started = drag.started;
        state.drag = Some(drag);
        return started;
    }
    if drag.started {
        state.suppress_click = true;
        state.dom.resolve();
        let drop = state
            .dom
            .doc
            .element_from_point(x as f32, y as f32)
            .and_then(|engine| state.dom.owner(engine))
            .and_then(|id| drag_observer(state, id));
        let (node, phase) = (if drag.captured { Some(drag.from) } else { drop })
            .map_or((drag.from, DragPhase::Cancel), |target| {
                (target, DragPhase::Drop)
            });
        state.inputs.push(Input::Event {
            node,
            event: Event::Drag {
                from: drag.from,
                phase,
                x,
                y,
            },
        });
    }
    drag.started
}

fn keyboard(state: &mut State, target: EngineId, key: &BlitzKeyEvent) -> bool {
    let owner = state.dom.owner(target);
    if owner.is_some_and(|id| {
        state
            .mapper
            .tree()
            .node(id)
            .is_ok_and(|n| n.descriptor.view().readonly())
    }) {
        state.space = None;
        let value = key.key.to_string();
        let chord = key
            .modifiers
            .intersects(Modifiers::CONTROL | Modifiers::META);
        if matches!(&key.key, Key::Character(_)) && !chord
            || matches!(value.as_str(), "Backspace" | "Delete" | "Enter")
            || chord && matches!(value.to_ascii_lowercase().as_str(), "x" | "v" | "z" | "y")
        {
            return true;
        }
    }
    let space = matches!(&key.key, Key::Character(value) if value == " ");
    if !key.state.is_pressed() {
        if space && let Some(armed) = state.space.take() {
            if Some(armed) == owner && state.focused == owner && key.modifiers.is_empty() {
                activate_control(state, armed);
            }
            return true;
        }
        return false;
    }
    let composing = state
        .dom
        .doc
        .get_node(target)
        .and_then(|node| node.element_data())
        .and_then(|element| element.text_input_data())
        .is_some_and(|input| input.editor.is_composing());
    if key.is_composing || composing {
        // IME owns text/caret keys until its terminal update; beta.4's default
        // key handler otherwise inserts text a second time during composition.
        return true;
    }
    let origin = owner.or_else(|| {
        state.mapper.tree().roots().iter().copied().find(|id| {
            state.mapper.tree().node(*id).is_ok_and(|node| {
                !matches!(node.descriptor.view().props, glory_render::Props::Group)
            })
        })
    });
    if let Some(origin) = origin {
        let editable = state.mapper.tree().node(origin).is_ok_and(|node| {
            matches!(node.descriptor.view().props, glory_render::Props::Input(_))
        });
        let key_name = key.key.to_string();
        if state.focused == Some(origin)
            && !editable
            && state.dom.control(origin).ok() == Some(target)
            && state.mapper.tree().enabled(origin)
            && state.mapper.tree().node(origin).is_ok_and(|node| {
                node.descriptor.view().role == Some(glory_render::Role::TreeItem)
                    && node
                        .descriptor
                        .view()
                        .semantics
                        .as_ref()
                        .is_some_and(|s| s.tree_item.is_some())
            })
        {
            let mut cursor = state
                .mapper
                .tree()
                .node(origin)
                .ok()
                .and_then(|node| node.parent);
            while let Some(tree) = cursor {
                let Ok(root) = state.mapper.tree().node(tree) else {
                    break;
                };
                if root.descriptor.view().role == Some(glory_render::Role::Tree) {
                    if state
                        .subscriptions
                        .contains(&(tree, EventKind::TreeNavigate))
                        && let Some(command) = root
                            .descriptor
                            .view()
                            .semantics
                            .as_ref()
                            .and_then(|s| s.tree_navigation)
                            .and_then(|config| {
                                glory_render::TreeCommand::key(
                                    config,
                                    glory_render::KeyboardInput {
                                        key: &key_name,
                                        control: key.modifiers.contains(Modifiers::CONTROL),
                                        meta: key.modifiers.contains(Modifiers::META),
                                        alt: key.modifiers.contains(Modifiers::ALT),
                                        shift: key.modifiers.contains(Modifiers::SHIFT),
                                        repeat: key.is_auto_repeating,
                                        composing: false,
                                        editable,
                                    },
                                    state
                                        .keyboard_clock
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(glory_render::MAX_TYPEAHEAD_MILLIS))
                                        as u64,
                                )
                            })
                    {
                        state.inputs.push(Input::Event {
                            node: tree,
                            event: Event::TreeNavigate {
                                from: origin,
                                command,
                            },
                        });
                        return true;
                    }
                    break;
                }
                cursor = root.parent;
            }
        }
        if state.focused == Some(origin)
            && !editable
            && state.mapper.tree().enabled(origin)
            && state.mapper.tree().node(origin).is_ok_and(|node| {
                matches!(node.descriptor.view().props, glory_render::Props::Button(_))
            })
        {
            let mut cursor = state
                .mapper
                .tree()
                .node(origin)
                .ok()
                .and_then(|node| node.parent);
            while let Some(grid) = cursor {
                let Ok(root) = state.mapper.tree().node(grid) else {
                    break;
                };
                if root.descriptor.view().role == Some(glory_render::Role::Grid) {
                    let command = root
                        .descriptor
                        .view()
                        .semantics
                        .as_ref()
                        .and_then(|s| s.grid_navigation)
                        .and_then(|direction| {
                            glory_render::GridCommand::key(
                                direction,
                                glory_render::KeyboardInput {
                                    key: &key_name,
                                    control: key.modifiers.contains(Modifiers::CONTROL),
                                    meta: key.modifiers.contains(Modifiers::META),
                                    alt: key.modifiers.contains(Modifiers::ALT),
                                    shift: key.modifiers.contains(Modifiers::SHIFT),
                                    repeat: key.is_auto_repeating,
                                    composing: false,
                                    editable,
                                },
                            )
                        });
                    if state
                        .subscriptions
                        .contains(&(grid, EventKind::GridNavigate))
                        && let Some(command) = command
                    {
                        state.inputs.push(Input::Event {
                            node: grid,
                            event: Event::GridNavigate {
                                from: origin,
                                command,
                            },
                        });
                        return true;
                    }
                    break;
                }
                cursor = root.parent;
            }
        }
        if state.focused == Some(origin)
            && state.dom.control(origin).ok() == Some(target)
            && state.mapper.tree().enabled(origin)
            && state.mapper.tree().node(origin).is_ok_and(|node| {
                node.descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.navigation_item)
            })
        {
            let trigger = state.mapper.tree().node(origin).is_ok_and(|node| {
                node.descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.navigation_panel)
                    == Some(glory_render::NavigationPanel::Trigger)
            });
            let mut content = None;
            let mut cursor = Some(origin);
            while let Some(id) = cursor {
                let Ok(node) = state.mapper.tree().node(id) else {
                    break;
                };
                if node
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.navigation_panel)
                    == Some(glory_render::NavigationPanel::Content)
                {
                    content = Some(id);
                    break;
                }
                cursor = node.parent;
            }
            let mut direction = glory_render::Direction::Ltr;
            let mut cursor = content.or(Some(origin));
            while let Some(id) = cursor {
                let Ok(node) = state.mapper.tree().node(id) else {
                    break;
                };
                if let Some(semantics) = node.descriptor.view().semantics.as_ref()
                    && semantics.navigation.is_some()
                {
                    direction = semantics.navigation_direction;
                    break;
                }
                cursor = node.parent;
            }
            if let Some(command) = (glory_render::KeyboardInput {
                key: &key_name,
                control: key.modifiers.contains(Modifiers::CONTROL),
                meta: key.modifiers.contains(Modifiers::META),
                alt: key.modifiers.contains(Modifiers::ALT),
                shift: key.modifiers.contains(Modifiers::SHIFT),
                repeat: key.is_auto_repeating,
                composing: false,
                editable,
            })
            .navigation_panel(direction, trigger, content.is_some())
            {
                let interested = if command == glory_render::NavigationPanelCommand::Close {
                    content.is_some_and(|id| {
                        state
                            .subscriptions
                            .contains(&(id, EventKind::NavigationPanel))
                    })
                } else {
                    state
                        .subscriptions
                        .contains(&(origin, EventKind::NavigationPanel))
                };
                if interested {
                    state.inputs.push(Input::Event {
                        node: origin,
                        event: Event::NavigationPanel {
                            from: origin,
                            command,
                        },
                    });
                    return true;
                }
            }
        }
        if state.focused == Some(origin)
            && state.dom.control(origin).ok() == Some(target)
            && state.mapper.tree().enabled(origin)
            && state.subscriptions.contains(&(origin, EventKind::Spin))
            && let Some(spin) = state
                .mapper
                .tree()
                .node(origin)
                .ok()
                .filter(|n| !n.descriptor.view().readonly())
                .and_then(|n| n.descriptor.view().semantics.as_ref())
                .and_then(|s| s.spinbutton.as_ref())
            && let Some(command) = glory_render::RangeCommand::spin_key(
                spin.min.is_some(),
                spin.max.is_some(),
                glory_render::KeyboardInput {
                    key: &key_name,
                    control: key.modifiers.contains(Modifiers::CONTROL),
                    meta: key.modifiers.contains(Modifiers::META),
                    alt: key.modifiers.contains(Modifiers::ALT),
                    shift: key.modifiers.contains(Modifiers::SHIFT),
                    repeat: key.is_auto_repeating,
                    composing: false,
                    editable: true,
                },
            )
        {
            state.inputs.push(Input::Event {
                node: origin,
                event: Event::Spin {
                    from: origin,
                    command,
                },
            });
            return true;
        }
        if state.focused == Some(origin)
            && state.dom.control(origin).ok() == Some(target)
            && state.subscriptions.contains(&(origin, EventKind::Range))
            && let Some(slider) = state
                .mapper
                .tree()
                .node(origin)
                .ok()
                .and_then(|node| node.descriptor.view().semantics.as_ref())
                .and_then(|s| s.slider.as_ref())
            && let Some(command) = glory_render::RangeCommand::slider_key(
                slider.axis,
                slider.direction,
                glory_render::KeyboardInput {
                    key: &key_name,
                    control: key.modifiers.contains(Modifiers::CONTROL),
                    meta: key.modifiers.contains(Modifiers::META),
                    alt: key.modifiers.contains(Modifiers::ALT),
                    shift: key.modifiers.contains(Modifiers::SHIFT),
                    repeat: key.is_auto_repeating,
                    composing: false,
                    editable,
                },
            )
        {
            if state.mapper.tree().enabled(origin)
                && state
                    .mapper
                    .tree()
                    .node(origin)
                    .is_ok_and(|node| !node.descriptor.view().readonly())
            {
                state.inputs.push(Input::Event {
                    node: origin,
                    event: Event::Range {
                        from: origin,
                        command,
                    },
                });
            }
            return true;
        }
        if state.focused == Some(origin)
            && state.dom.control(origin).ok() == Some(target)
            && state.mapper.tree().enabled(origin)
            && state.subscriptions.contains(&(origin, EventKind::Range))
            && let Some(command) = state
                .mapper
                .tree()
                .node(origin)
                .ok()
                .and_then(|node| node.descriptor.view().semantics.as_ref())
                .filter(|s| s.splitter.is_some())
                .and_then(|s| s.separator_axis)
                .and_then(|line| {
                    glory_render::RangeCommand::splitter_key(
                        line,
                        glory_render::KeyboardInput {
                            key: &key_name,
                            control: key.modifiers.contains(Modifiers::CONTROL),
                            meta: key.modifiers.contains(Modifiers::META),
                            alt: key.modifiers.contains(Modifiers::ALT),
                            shift: key.modifiers.contains(Modifiers::SHIFT),
                            repeat: key.is_auto_repeating,
                            composing: false,
                            editable,
                        },
                    )
                })
        {
            state.inputs.push(Input::Event {
                node: origin,
                event: Event::Range {
                    from: origin,
                    command,
                },
            });
            return true;
        }
        if state.focused == Some(origin)
            && state.dom.control(origin).ok() == Some(target)
            && state
                .subscriptions
                .contains(&(origin, EventKind::ScrollKeyboard))
            && let Some(command) = state
                .mapper
                .tree()
                .node(origin)
                .ok()
                .and_then(|node| node.descriptor.view().semantics.as_ref())
                .and_then(|semantics| semantics.scroll_keyboard)
                .and_then(|axes| {
                    axes.key(glory_render::KeyboardInput {
                        key: &key_name,
                        control: key.modifiers.contains(Modifiers::CONTROL),
                        meta: key.modifiers.contains(Modifiers::META),
                        alt: key.modifiers.contains(Modifiers::ALT),
                        shift: key.modifiers.contains(Modifiers::SHIFT),
                        repeat: key.is_auto_repeating,
                        composing: false,
                        editable,
                    })
                })
        {
            state.inputs.push(Input::Event {
                node: origin,
                event: Event::ScrollKeyboard {
                    from: origin,
                    command,
                },
            });
            return true;
        }
        let command = glory_render::KeyboardInput {
            key: &key_name,
            control: key.modifiers.contains(Modifiers::CONTROL),
            meta: key.modifiers.contains(Modifiers::META),
            alt: key.modifiers.contains(Modifiers::ALT),
            shift: key.modifiers.contains(Modifiers::SHIFT),
            repeat: key.is_auto_repeating,
            composing: false,
            editable,
        }
        .command();
        if let Some(command) = command {
            let own_interest = state.subscriptions.contains(&(origin, EventKind::Keyboard));
            let mut cursor = Some(origin);
            let mut interested = false;
            let mut in_form = false;
            while let Some(id) = cursor {
                in_form |= state.mapper.tree().node(id).is_ok_and(|node| {
                    node.descriptor.view().field_part() == Some(glory_render::FieldPart::Form)
                });
                if state.subscriptions.contains(&(id, EventKind::Keyboard)) {
                    interested = true;
                }
                cursor = state
                    .mapper
                    .tree()
                    .node(id)
                    .ok()
                    .and_then(|node| node.parent);
            }
            let form_submit=in_form && command==glory_render::KeyboardCommand::Submit
                && state.mapper.tree().node(origin).is_ok_and(|node| matches!(&node.descriptor.view().props,glory_render::Props::Input(input) if !input.multiline));
            if interested && (!editable || own_interest) || form_submit {
                if command == glory_render::KeyboardCommand::Submit
                    && let Err(error) = editing::queue_commit(state, origin, true)
                {
                    state.input_error = Some(error);
                    return true;
                }
                state.inputs.push(Input::Event {
                    node: origin,
                    event: Event::Keyboard {
                        from: origin,
                        command,
                    },
                });
                return true;
            }
        }
    }
    if let Some(id) = owner
        && key.modifiers.is_empty()
        && let Ok(node) = state.mapper.tree().node(id)
    {
        if space
            && matches!(
                node.descriptor.view().props,
                glory_render::Props::Button(_) | glory_render::Props::Check(_)
            )
        {
            state.space = Some(id);
            return true;
        }
        if key.key == Key::Named(NamedKey::Enter)
            && matches!(
                node.descriptor.view().props,
                glory_render::Props::Button(_) | glory_render::Props::Link(_)
            )
        {
            activate_control(state, id);
            return true;
        }
    }
    if let Some(id) = state.dom.owner(target)
        && let Some(range) = state.dom.ranges.get_mut(&id)
        && range.edit == EditKind::Text
    {
        range.editing.after_preedit = None;
        if key.key == Key::Named(NamedKey::Enter)
            && key.modifiers.is_empty()
            && state
                .mapper
                .tree()
                .node(id)
                .is_ok_and(|node| matches!(&node.descriptor.view().props, glory_render::Props::Input(input) if !input.multiline))
        {
            if let Err(error)=editing::queue_commit(state,id,false){state.input_error=Some(error);}
            return true;
        }
    }
    if key.key == Key::Named(NamedKey::Tab) {
        // beta.4's default Tab path checks only Shift. Other modifiers belong
        // to window/system shortcuts and must not traverse document controls.
        if !(key.modifiers & !Modifiers::SHIFT).is_empty() {
            return true;
        }
        state.dom.resolve();
        // Blitz beta.4 needs an existing focus for its Tab helper. Traverse only
        // the live physical tree, including the non-tab-stop current origin;
        // no source/virtual-row scan or focus search index is constructed.
        let current = state.dom.doc.get_focussed_node_id();
        let mut nodes = Vec::new();
        let mut pending = vec![state.dom.doc.root_node().id];
        while let Some(id) = pending.pop() {
            let Some(node) = state.dom.doc.get_node(id) else {
                continue;
            };
            nodes.push(id);
            pending.extend(node.children.iter().rev().copied());
        }
        let eligible = |id: EngineId| {
            state.dom.doc.get_node(id).is_some_and(|node| {
                node.flags.is_in_document()
                    && state.dom.focus_visible(id)
                    && node.is_focussable()
                    && node.element_data().is_some_and(|element| {
                        element
                            .attr(blitz_dom::LocalName::from("disabled"))
                            .is_none()
                    })
                    && state.dom.owner(id).is_some_and(|owner| {
                        state.mapper.tree().active(owner)
                            && state.mapper.tree().enabled(owner)
                            && state.layers.allowed(state.mapper.tree(), owner)
                            && focus_scope::allowed(state, owner)
                    })
            })
        };
        let current = current.and_then(|id| nodes.iter().position(|candidate| *candidate == id));
        let backwards = key.modifiers.contains(Modifiers::SHIFT);
        let indices: Box<dyn Iterator<Item = usize>> = if backwards {
            let pivot = current.unwrap_or(nodes.len());
            Box::new((0..pivot).rev().chain((pivot..nodes.len()).rev()))
        } else {
            let pivot = current.map_or(0, |index| index + 1);
            Box::new((pivot..nodes.len()).chain(0..pivot))
        };
        if let Some(id) = indices.map(|index| nodes[index]).find(|id| eligible(*id)) {
            if state.dom.doc.set_focus_to(id) {
                state.dom.invalidate();
            }
            state.sync_focus();
            return true;
        }
        return false;
    }
    let Some(origin) = state.dom.owner(target) else {
        return false;
    };
    if !key.modifiers.is_empty() {
        return false;
    }
    let Ok(node) = state.mapper.tree().node(origin) else {
        return false;
    };
    if state.dom.control(origin).ok() == Some(target)
        && let Some((root, axis)) = state.mapper.tree().navigation(origin)
        && state.subscriptions.contains(&(root, EventKind::Navigate))
        && let Some(command) = axis.key_with_direction(
            state
                .mapper
                .tree()
                .node(root)
                .ok()
                .and_then(|node| node.descriptor.view().semantics.as_ref())
                .map(|s| s.navigation_direction)
                .unwrap_or_default(),
            glory_render::KeyboardInput {
                key: &key.key.to_string(),
                ..Default::default()
            },
        )
    {
        state.inputs.push(Input::Event {
            node: origin,
            event: Event::Navigate {
                from: origin,
                command,
            },
        });
        return true;
    }
    if !matches!(
        node.descriptor
            .view()
            .options
            .get::<glory_render::VirtualPart>(),
        Some(glory_render::VirtualPart::Root | glory_render::VirtualPart::Row)
    ) || state.dom.control(origin).ok() != Some(target)
    {
        return false;
    }
    let command = match key.key {
        Key::Named(NamedKey::ArrowUp) => glory_render::Navigation::Previous,
        Key::Named(NamedKey::ArrowDown) => glory_render::Navigation::Next,
        Key::Named(NamedKey::Home) => glory_render::Navigation::First,
        Key::Named(NamedKey::End) => glory_render::Navigation::Last,
        Key::Named(NamedKey::PageUp) => glory_render::Navigation::PagePrevious,
        Key::Named(NamedKey::PageDown) => glory_render::Navigation::PageNext,
        _ => return false,
    };
    let mut cursor = Some(origin);
    while let Some(id) = cursor {
        if state.subscriptions.contains(&(id, EventKind::Navigate)) {
            state.inputs.push(Input::Event {
                node: origin,
                event: Event::Navigate {
                    from: origin,
                    command,
                },
            });
            return true;
        }
        cursor = state
            .mapper
            .tree()
            .node(id)
            .ok()
            .and_then(|node| node.parent);
    }
    false
}

fn activate_control(state: &mut State, id: NodeId) {
    if !state.mapper.tree().enabled(id)
        || state.mapper.tree().node(id).is_ok_and(|n| {
            n.descriptor.view().readonly()
                && matches!(n.descriptor.view().props, glory_render::Props::Check(_))
        })
    {
        return;
    }
    state.inputs.push(Input::Event {
        node: id,
        event: Event::Activate,
    });
    if let Ok(control) = state.dom.control(id)
        && let Some(checked) = state
            .dom
            .doc
            .get_node(control)
            .and_then(|node| node.element_data())
            .and_then(|element| element.checkbox_input_checked())
    {
        let mut mutation = state.dom.doc.mutate();
        if checked {
            mutation.clear_attribute(control, attr("checked"));
        } else {
            mutation.set_attribute(control, attr("checked"), "true");
        }
        drop(mutation);
        state.inputs.push(Input::Event {
            node: id,
            event: Event::CheckedChanged(!checked),
        });
    }
    state.dom.invalidate();
}

/// Guard the pinned CPU backend's u16 dimensions and its RGBA allocation.
/// Zero-size releases the surface; a later resize resumes the same document.
pub struct CpuRenderer {
    inner: VelloCpuWindowRenderer,
    window: Option<Arc<dyn WindowHandle>>,
    error: Option<ShellError>,
    limit: usize,
    frames: u64,
}
impl Default for CpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}
impl CpuRenderer {
    pub fn new() -> Self {
        Self::with_limit(64 * 1024 * 1024)
    }
    pub fn with_limit(limit: usize) -> Self {
        Self {
            inner: VelloCpuWindowRenderer::new(),
            window: None,
            error: None,
            limit,
            frames: 0,
        }
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.inner.suspend();
            self.inner = VelloCpuWindowRenderer::new();
            return;
        }
        // Check dimensions first: arbitrary u32 width*height*4 can overflow u64.
        if width > u32::from(u16::MAX)
            || height > u32::from(u16::MAX)
            || u64::from(width) * u64::from(height) * 4 > self.limit as u64
        {
            self.inner.suspend();
            self.error = Some(ShellError::Surface {
                width,
                height,
                limit: self.limit,
            });
            return;
        }
        if self.inner.is_active() {
            self.inner.set_size(width, height);
        } else if let Some(window) = &self.window {
            self.inner.resume(window.clone(), width, height, || {});
            self.inner.complete_resume();
        }
    }
}
impl anyrender::RenderContext for CpuRenderer {}
impl WindowRenderer for CpuRenderer {
    type ScenePainter<'a> = <VelloCpuWindowRenderer as WindowRenderer>::ScenePainter<'a>;
    fn resume<F: FnOnce() + 'static>(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        self.window = Some(window);
        self.resize(width, height);
        on_ready();
    }
    fn complete_resume(&mut self) -> bool {
        // This synchronous CPU implementation has no pending initialization.
        // Install the poll waker even while a zero-size surface is inactive.
        true
    }
    fn suspend(&mut self) {
        self.inner.suspend();
        self.inner = VelloCpuWindowRenderer::new();
        self.window = None;
    }
    fn is_active(&self) -> bool {
        self.inner.is_active()
    }
    fn set_size(&mut self, width: u32, height: u32) {
        self.resize(width, height);
    }
    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw: F) {
        if self.is_active() {
            let mut painted = false;
            self.inner.render(|scene| {
                painted = true;
                draw(scene);
            });
            // The pinned softbuffer backend skips draw if buffer acquisition
            // fails, and returns normally after a successful present otherwise.
            if painted {
                self.frames += 1;
            }
        }
    }
}

/// One native event-loop owner; supports multiple independently owned Apps.
pub struct Shell {
    // Platform subclasses must be removed before their window handles die.
    #[cfg(all(
        feature = "accesskit",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    accessibility: std::collections::HashMap<WindowId, accesskit_platform::Platform>,
    inner: BlitzApplication<CpuRenderer>,
    event_loop: Option<EventLoop>,
    error: Rc<RefCell<Option<ShellError>>>,
}
fn prepare(view: &mut blitz_shell::View<CpuRenderer>) {
    if let Some(theme) = view.window.theme() {
        queue_color_scheme(
            &mut view.downcast_doc_mut::<WindowApp>().renderer.0.borrow_mut(),
            theme == winit::window::Theme::Dark,
        );
    }
    let seconds = view.current_animation_time();
    let app = view.downcast_doc_mut::<WindowApp>();
    if let Err(error) = app.renderer.advance_time(seconds) {
        app.error.get_or_insert(error.into());
    }
}
fn queue_color_scheme(state: &mut State, dark: bool) {
    if state.color_scheme.replace(dark) == Some(dark) {
        return;
    }
    let nodes = state
        .subscriptions
        .iter()
        .filter_map(|(node, kind)| (*kind == EventKind::ColorSchemeChanged).then_some(*node))
        .collect::<Vec<_>>();
    for node in nodes {
        state.inputs.push(Input::Event {
            node,
            event: Event::ColorSchemeChanged { dark },
        });
    }
}
impl Shell {
    #[cfg(not(target_os = "android"))]
    pub fn new() -> Result<Self, ShellError> {
        let event_loop = EventLoop::builder()
            .build()
            .map_err(|error| ShellError::Window(error.to_string()))?;
        Ok(Self::from_event_loop(event_loop))
    }

    /// Use an application-configured event loop. The caller follows winit's
    /// platform thread/one-loop rules; UI state stays on that same thread.
    pub fn from_event_loop(event_loop: EventLoop) -> Self {
        let (proxy, queue) = BlitzShellProxy::new(event_loop.create_proxy());
        Self {
            #[cfg(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            accessibility: Default::default(),
            inner: BlitzApplication::new(proxy, queue),
            event_loop: Some(event_loop),
            error: Rc::default(),
        }
    }

    /// Build the native-activity event loop from this invocation's AndroidApp.
    /// Never discover or store a process-global Activity.
    #[cfg(target_os = "android")]
    pub fn android(app: AndroidApp) -> Result<Self, ShellError> {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        let event_loop = EventLoop::builder()
            .with_android_app(app)
            .build()
            .map_err(|error| ShellError::Window(error.to_string()))?;
        Ok(Self::from_event_loop(event_loop))
    }
    pub fn add_window(&mut self, app: WindowApp, attributes: WindowAttributes) {
        #[cfg(all(
            feature = "accesskit",
            not(any(target_os = "android", target_os = "ios"))
        ))]
        let (app, attributes) = {
            let mut app = app;
            app.requested_visible = attributes.visible;
            (app, attributes.with_visible(false))
        };
        self.inner.add_window(WindowConfig::with_attributes(
            Box::new(app),
            CpuRenderer::new(),
            attributes,
        ));
    }
    /// Some platforms resize synchronously and emit no SurfaceResized event.
    /// Apply that returned size immediately; None waits for the native event.
    pub fn request_size(
        &mut self,
        id: WindowId,
        size: winit::dpi::Size,
    ) -> Result<Option<winit::dpi::PhysicalSize<u32>>, ShellError> {
        let view = self
            .inner
            .windows
            .get_mut(&id)
            .ok_or_else(|| ShellError::Window("window closed".into()))?;
        let applied = view.window.request_surface_size(size);
        if let Some(size) = applied {
            prepare(view);
            view.downcast_doc_mut::<WindowApp>().renderer.invalidate();
            view.handle_winit_event(WindowEvent::SurfaceResized(size));
            view.poll();
        }
        Ok(applied)
    }
    pub fn run(mut self) -> Result<(), ShellError> {
        if self.inner.pending_windows.is_empty() {
            return Err(ShellError::Window("no windows configured".into()));
        }
        let event_loop = self.event_loop.take().expect("event loop owned");
        let error = self.error.clone();
        event_loop
            .run_app(self)
            .map_err(|error| ShellError::Window(error.to_string()))?;
        error.borrow_mut().take().map_or(Ok(()), Err)
    }
    fn check(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(all(
            feature = "accesskit",
            not(any(target_os = "android", target_os = "ios"))
        ))]
        for (id, view) in &mut self.inner.windows {
            let platform = self
                .accessibility
                .entry(*id)
                .or_insert_with(|| accesskit_platform::Platform::new(view));
            if let Err(error) = platform.update(view) {
                view.downcast_doc_mut::<WindowApp>()
                    .error
                    .get_or_insert(error);
            }
        }
        for view in self.inner.windows.values_mut() {
            prepare(view);
            if let Some(error) = view.renderer.error.take() {
                self.error.borrow_mut().get_or_insert(error);
            }
            if let Some(error) = view.downcast_doc_mut::<WindowApp>().error.take() {
                self.error.borrow_mut().get_or_insert(ShellError::Ui(error));
            }
        }
        if self.error.borrow().is_some() {
            #[cfg(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            self.accessibility.clear();
            self.inner.windows.clear();
            event_loop.exit();
        }
    }
}
impl ApplicationHandler for Shell {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        for view in self.inner.windows.values_mut() {
            view.downcast_doc_mut::<WindowApp>().renderer.invalidate();
        }
        self.inner.can_create_surfaces(event_loop);
        self.check(event_loop);
    }
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.destroy_surfaces(event_loop);
    }
    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        #[cfg(all(
            feature = "accesskit",
            not(any(target_os = "android", target_os = "ios"))
        ))]
        if let Some(platform) = self.accessibility.get_mut(&id) {
            if matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed) {
                self.accessibility.remove(&id);
            } else {
                platform.window_event(&event);
            }
        }
        if let Some(view) = self.inner.windows.get_mut(&id) {
            prepare(view);
            if let WindowEvent::ThemeChanged(theme) = &event {
                queue_color_scheme(
                    &mut view.downcast_doc_mut::<WindowApp>().renderer.0.borrow_mut(),
                    *theme == winit::window::Theme::Dark,
                );
            }
            if matches!(
                event,
                WindowEvent::SurfaceResized(_) | WindowEvent::ScaleFactorChanged { .. }
            ) {
                swipe::cancel(&mut view.downcast_doc_mut::<WindowApp>().renderer.0.borrow_mut());
            }
            if matches!(event, WindowEvent::Focused(false)) {
                let mut state = view.downcast_doc_mut::<WindowApp>().renderer.0.borrow_mut();
                cancel_drag(&mut state);
                swipe::cancel(&mut state);
            }
            if matches!(event, WindowEvent::RedrawRequested) {
                // Read queued scroll/resize inputs and commit the current
                // virtual window before the shell paints this frame.
                view.poll();
            } else {
                view.downcast_doc_mut::<WindowApp>().renderer.invalidate();
            }
        }
        self.inner.window_event(event_loop, id, event);
        self.check(event_loop);
    }
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        // A self-waking future cannot monopolize the window/input event loop.
        let mut polled = std::collections::HashSet::new();
        for index in 0..64 {
            let Ok(event) = self.inner.event_queue.try_recv() else {
                break;
            };
            if let BlitzShellEvent::Poll { window_id } = &event
                && !polled.insert(*window_id)
            {
                // Defer a fresh self-wake to another native event-loop turn.
                self.inner.proxy.send_event(event);
                break;
            }
            let resumed = matches!(event, BlitzShellEvent::ResumeReady { .. });
            if let BlitzShellEvent::Poll { window_id } = &event
                && let Some(view) = self.inner.windows.get_mut(window_id)
            {
                prepare(view);
            }
            if let BlitzShellEvent::RequestRedraw { doc_id } = &event {
                for view in self
                    .inner
                    .windows
                    .values_mut()
                    .filter(|view| view.doc.id() == *doc_id)
                {
                    view.downcast_doc_mut::<WindowApp>().renderer.invalidate();
                }
            }
            #[cfg(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            if let BlitzShellEvent::CloseWindow { window_id } = &event {
                self.accessibility.remove(window_id);
            }
            #[cfg(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            let accessibility = if let BlitzShellEvent::Embedder(value) = &event
                && let Some(message) = value.downcast_ref::<accesskit_platform::Message>()
            {
                if let Some(view) = self.inner.windows.get_mut(&message.window)
                    && let Some(platform) = self.accessibility.get_mut(&message.window)
                    && view.downcast_doc_mut::<WindowApp>().app.runtime().id() == message.runtime
                {
                    platform.event(view, &message.event);
                }
                true
            } else {
                false
            };
            #[cfg(not(all(
                feature = "accesskit",
                not(any(target_os = "android", target_os = "ios"))
            )))]
            let accessibility = false;
            if !accessibility {
                self.inner.handle_blitz_shell_event(event_loop, event);
            }
            if resumed {
                for view in self.inner.windows.values_mut() {
                    prepare(view);
                    view.poll();
                    view.request_redraw();
                }
            }
            if index == 63 {
                self.inner.proxy.wake_up();
            }
        }
        self.check(event_loop);
    }
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "command_tests.rs"]
mod command_tests;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "editing_tests.rs"]
mod editing_tests;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "environment_tests.rs"]
mod environment_tests;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "focus_scope_tests.rs"]
mod focus_scope_tests;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "focus_tests.rs"]
mod focus_tests;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "layer_tests.rs"]
mod layer_tests;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "shell_tests.rs"]
mod tests;
