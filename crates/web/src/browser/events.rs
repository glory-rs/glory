use std::rc::Rc;

use glory_html::EditKind;
use glory_render::{Event, EventKind, KeyboardCommand, KeyboardInput, Navigation, NodeId};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, EventTarget, KeyboardEvent, Node, ResizeObserver};

use super::{backend, dom::Dom};
#[path = "context_menu.rs"]
mod context_menu;
#[path = "drag.rs"]
mod drag;
#[path = "tags.rs"]
mod tags;

fn queue_commit(
    dom: &Dom,
    id: NodeId,
    entry: &super::dom::Entry,
    element: &Element,
    sync_value: bool,
) -> Result<(), glory_render::Error> {
    let revision = entry.editing.borrow().revision;
    if let Some(event) = super::input_edit::event(entry, element, false)? {
        let value = match &event {
            Event::InputEdit(glory_render::InputEdit::Value { value, .. }) => Some(value.clone()),
            _ => None,
        };
        dom.queue(id, event);
        if let Some(value) = value {
            dom.queue(id, Event::Commit { value, revision });
        }
    } else {
        let value = entry.value()?;
        if sync_value {
            dom.queue(
                id,
                Event::ValueChanged {
                    value: value.clone(),
                    composing: false,
                    revision,
                },
            );
        }
        if entry.control()?.tag_name() != "TEXTAREA" {
            dom.queue(id, Event::Commit { value, revision });
        }
    }
    Ok(())
}

struct Listener {
    target: EventTarget,
    name: &'static str,
    // Focus containment can synchronously dispatch another native focusin.
    // Captures are immutable Weak handles; mutable state lives in Dom and is
    // released before host calls. Fn permits that legitimate native reentry.
    callback: Closure<dyn Fn(web_sys::Event)>,
}
pub(super) struct Listeners {
    listeners: Vec<Listener>,
    _resize: Option<Closure<dyn FnMut(js_sys::Array)>>,
}
impl Listeners {
    pub fn new(dom: Rc<Dom>) -> Self {
        let mut listeners = Vec::new();
        if let Some(media) = dom.document.default_view().and_then(|window| {
            window
                .match_media("(prefers-color-scheme: dark)")
                .ok()
                .flatten()
        }) {
            let weak = Rc::downgrade(&dom);
            let callback = Closure::new(move |event: web_sys::Event| {
                let Some(dom) = weak.upgrade() else { return };
                let Some(event) = event.dyn_ref::<web_sys::MediaQueryListEvent>() else {
                    return;
                };
                let ids = dom
                    .entries
                    .borrow()
                    .iter()
                    .filter_map(|(id, entry)| {
                        entry
                            .interests
                            .borrow()
                            .contains(&EventKind::ColorSchemeChanged)
                            .then_some(*id)
                    })
                    .collect::<Vec<_>>();
                for id in ids {
                    dom.queue(
                        id,
                        Event::ColorSchemeChanged {
                            dark: event.matches(),
                        },
                    );
                }
            });
            let target: EventTarget = media.into();
            if let Err(error) = target.add_event_listener_with_callback_and_bool(
                "change",
                callback.as_ref().unchecked_ref(),
                true,
            ) {
                dom.error.borrow_mut().get_or_insert(backend(error));
            }
            listeners.push(Listener {
                target,
                name: "change",
                callback,
            });
        }
        for name in [
            "dragenter",
            "dragover",
            "drop",
            "submit",
            "reset",
            "click",
            "input",
            "selectionchange",
            "beforeinput",
            "paste",
            "contextmenu",
            "change",
            "compositionstart",
            "compositionend",
            "keydown",
            "focusin",
            "focusout",
            "scroll",
            "wheel",
            "touchmove",
            "pointerdown",
            "pointermove",
            "pointerup",
            "pointercancel",
            "pointerover",
            "pointerout",
            "lostpointercapture",
            "blur",
            "resize",
        ] {
            let weak = Rc::downgrade(&dom);
            let callback = Closure::new(move |event: web_sys::Event| {
                let Some(dom) = weak.upgrade() else { return };
                if let Err(error) = receive(&dom, name, event) {
                    dom.error.borrow_mut().get_or_insert(error);
                    dom.frame.input();
                }
            });
            let target: EventTarget = if matches!(name, "blur" | "resize") {
                dom.document
                    .default_view()
                    .map(Into::into)
                    .unwrap_or_else(|| dom.document.clone().into())
            } else if matches!(
                name,
                "keydown"
                    | "selectionchange"
                    | "pointerdown"
                    | "click"
                    | "focusin"
                    | "pointermove"
                    | "pointerup"
                    | "pointercancel"
                    | "scroll"
                    | "wheel"
                    | "touchmove"
            ) {
                dom.document.clone().into()
            } else {
                dom.root.clone().into()
            };
            let registration = if matches!(name, "wheel" | "touchmove") {
                let options = web_sys::AddEventListenerOptions::new();
                options.set_capture(true);
                options.set_passive(false);
                target.add_event_listener_with_callback_and_add_event_listener_options(
                    name,
                    callback.as_ref().unchecked_ref(),
                    &options,
                )
            } else {
                target.add_event_listener_with_callback_and_bool(
                    name,
                    callback.as_ref().unchecked_ref(),
                    true,
                )
            };
            if let Err(error) = registration {
                dom.error.borrow_mut().get_or_insert(backend(error));
            }
            listeners.push(Listener {
                target,
                name,
                callback,
            });
        }
        let weak = Rc::downgrade(&dom);
        if let Some(viewport) = dom
            .document
            .default_view()
            .and_then(|window| window.visual_viewport())
        {
            for name in ["resize", "scroll"] {
                let weak = Rc::downgrade(&dom);
                let callback = Closure::new(move |event: web_sys::Event| {
                    let Some(dom) = weak.upgrade() else { return };
                    super::swipe::cancel(&dom);
                    if let Err(error) = super::sheet::receive(&dom, "resize", &event) {
                        dom.error.borrow_mut().get_or_insert(error);
                    }
                });
                let target: EventTarget = viewport.clone().into();
                if let Err(error) = target.add_event_listener_with_callback_and_bool(
                    name,
                    callback.as_ref().unchecked_ref(),
                    true,
                ) {
                    dom.error.borrow_mut().get_or_insert(backend(error));
                }
                listeners.push(Listener {
                    target,
                    name,
                    callback,
                });
            }
        }
        let resize = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            let Some(dom) = weak.upgrade() else { return };
            for entry in entries {
                if let Ok(entry) = entry.dyn_into::<web_sys::ResizeObserverEntry>()
                    && let Some((id, _)) = target(&dom, &entry.target())
                {
                    if dom
                        .swipe
                        .borrow()
                        .as_ref()
                        .is_some_and(|current| current.node == id)
                    {
                        super::swipe::cancel(&dom);
                    }
                    if let Some(size) = border_size(&dom, &entry) {
                        dom.sizes.borrow_mut().insert(id, size);
                    }
                    dom.layout.borrow_mut().insert((id, EventKind::Resize));
                }
            }
            dom.frame.input();
        });
        // A missing observer is diagnosed only if a component requests resize.
        let resize = match ResizeObserver::new(resize.as_ref().unchecked_ref()) {
            Ok(observer) => {
                *dom.observer.borrow_mut() = Some(observer);
                *dom.viewport_observer.borrow_mut() =
                    ResizeObserver::new(resize.as_ref().unchecked_ref()).ok();
                Some(resize)
            }
            Err(_) => None,
        };
        Self {
            listeners,
            _resize: resize,
        }
    }
}
fn border_size(dom: &Dom, entry: &web_sys::ResizeObserverEntry) -> Option<glory_render::Rect> {
    // Observer dimensions are layout pixels, unaffected by CSS transforms.
    // Keep the last fractional border box, rather than forcing one rect query
    // per row in the next frame. Legacy observers fall back in the read phase.
    let sizes = js_sys::Reflect::get(entry, &"borderBoxSize".into()).ok()?;
    let size = if js_sys::Array::is_array(&sizes) {
        js_sys::Array::from(&sizes).get(0)
    } else {
        sizes
    };
    let inline = js_sys::Reflect::get(&size, &"inlineSize".into())
        .ok()?
        .as_f64()?;
    let block = js_sys::Reflect::get(&size, &"blockSize".into())
        .ok()?
        .as_f64()?;
    if !inline.is_finite() || !block.is_finite() {
        return None;
    }
    let mode = dom
        .document
        .default_view()?
        .get_computed_style(&entry.target())
        .ok()??
        .get_property_value("writing-mode")
        .ok()?;
    let vertical = mode.starts_with("vertical") || mode.starts_with("sideways");
    Some(glory_render::Rect {
        width: if vertical { block } else { inline },
        height: if vertical { inline } else { block },
        ..Default::default()
    })
}
impl Drop for Listeners {
    fn drop(&mut self) {
        for listener in &self.listeners {
            let _ = listener
                .target
                .remove_event_listener_with_callback_and_bool(
                    listener.name,
                    listener.callback.as_ref().unchecked_ref(),
                    true,
                );
        }
    }
}
pub(super) fn target(dom: &Dom, element: &Element) -> Option<(NodeId, Element)> {
    let boundary = element.closest("[data-gly-app]").ok().flatten()?;
    if boundary != dom.root {
        return None;
    }
    let target = element.closest("[data-gly-node]").ok().flatten()?;
    let id = dom.id_for_token(&target.get_attribute("data-gly-node")?)?;
    Some((id, target))
}
fn receive(dom: &Dom, name: &str, event: web_sys::Event) -> Result<(), glory_render::Error> {
    if name == "selectionchange" {
        if let Some(element) = dom.document.active_element()
            && let Some((id, _)) = target(dom, &element)
            && let Ok(entry) = dom.entry(id)
            && entry.edit == EditKind::Rich
        {
            entry
                .rich
                .borrow_mut()
                .get_or_insert_with(Default::default)
                .selected = true;
            super::rich::mark(dom, id);
        }
        return Ok(());
    }
    if super::swipe::receive(dom, name, &event)? {
        return Ok(());
    }
    if name == "pointerdown" && dom.swipe.borrow().is_some() {
        drag::cancel(dom);
    }
    if super::files::receive(dom, name, &event)? {
        return Ok(());
    }
    if super::layers::receive(dom, name, &event)? {
        return Ok(());
    }
    if super::sheet::receive(dom, name, &event)? {
        return Ok(());
    }
    if matches!(
        name,
        "pointerdown"
            | "pointermove"
            | "pointerup"
            | "pointercancel"
            | "lostpointercapture"
            | "blur"
    ) && dom.swipe.borrow().is_none()
        && drag::receive(dom, name, &event)?
    {
        return Ok(());
    }
    let Some(node) = event
        .target()
        .and_then(|target| target.dyn_into::<Node>().ok())
    else {
        return Ok(());
    };
    let element = node
        .dyn_ref::<Element>()
        .cloned()
        .or_else(|| node.parent_element());
    let origin = element.as_ref().and_then(|element| target(dom, element));
    // An unfocused document sends keyboard input to body. Only the first app
    // owns that empty-document context; a focused sibling app stays isolated.
    let origin = origin.or_else(|| {
        if name != "keydown" {
            return None;
        }
        let element = element.as_ref()?;
        if element != &dom.root && element.tag_name() != "BODY" && element.tag_name() != "HTML" {
            return None;
        }
        if dom
            .document
            .query_selector("[data-gly-app]")
            .ok()
            .flatten()
            .as_ref()
            != Some(&dom.root)
        {
            return None;
        }
        dom.root
            .query_selector("[data-gly-node]")
            .ok()
            .flatten()
            .and_then(|element| target(dom, &element))
    });
    let Some((id, element)) = origin else {
        return Ok(());
    };
    let entry = dom.entry(id)?;
    if entry.edit == EditKind::Rich && super::rich::receive(dom, id, &entry, name, &event)? {
        return Ok(());
    }
    if context_menu::receive(dom, name, &event, id, &element)? {
        return Ok(());
    }
    if matches!(name, "submit" | "reset") {
        if !element.has_attribute("data-gly-form") {
            return Ok(());
        }
        event.prevent_default();
        if name == "reset" {
            dom.queue(id, Event::Reset);
        } else {
            let submitter = event
                .dyn_ref::<web_sys::SubmitEvent>()
                .and_then(|event| event.submitter());
            let submitter = match submitter {
                Some(element) => {
                    let Some((submitter, _)) = target(dom, element.unchecked_ref()) else {
                        return Ok(());
                    };
                    Some(submitter)
                }
                None => None,
            };
            dom.queue(id, Event::Submit { submitter });
        }
        return Ok(());
    }
    if name == "click"
        && element
            .closest("fieldset[disabled]")
            .map_err(backend)?
            .is_some()
    {
        event.prevent_default();
        return Ok(());
    }
    if name == "click"
        && entry.edit == EditKind::Check
        && entry.control()?.has_attribute("readonly")
    {
        event.prevent_default();
        return Ok(());
    }
    if name == "pointerdown"
        && (element.has_attribute("data-gly-option-owner")
            || element.has_attribute("data-gly-menubar-item"))
        && let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>()
        && pointer.is_primary()
        && pointer.button() == 0
    {
        event.prevent_default();
    }
    match name {
        "click" => {
            if element.has_attribute("data-gly-form-action")
                || element.has_attribute("data-gly-link-request")
            {
                event.prevent_default();
            }
            if dom.suppress_click.replace(false) {
                event.prevent_default();
                return Ok(());
            }
            // A custom slider remains a physical focusable View. Native label
            // association does not focus divs, so perform only this host default;
            // resulting focus events still queue before business delivery.
            if let Some(label) = element.closest("label[for]").map_err(backend)?
                && let Some(control) = label
                    .get_attribute("for")
                    .and_then(|id| dom.document.get_element_by_id(&id))
                && control.has_attribute("data-gly-slider")
                && control.get_attribute("aria-disabled").as_deref() != Some("true")
            {
                event.prevent_default();
                super::dom::focus(&control)?;
            }
            // Label default activation dispatches a second click to its input.
            // Only that control click represents checkbox activation.
            if entry.edit == EditKind::Check && entry.control()? != element {
                return Ok(());
            }
            let mut ancestor = Some(element.clone());
            let mut prevent = false;
            while let Some(element) = ancestor {
                if element == dom.root {
                    break;
                }
                if let Some((id, _)) = target(dom, &element)
                    && let Ok(entry) = dom.entry(id)
                {
                    prevent |= entry.prevent_default;
                }
                ancestor = element.parent_element();
            }
            if prevent {
                event.prevent_default();
            }
            // Logical Group ancestors have no DOM element. Always deliver the
            // origin; App resolves semantic bubbling and disabled ancestors.
            dom.queue(id, Event::Activate);
        }
        "input" if entry.edit == EditKind::Text => {
            if event
                .dyn_ref::<web_sys::InputEvent>()
                .is_some_and(|event| event.is_composing())
            {
                entry.editing.borrow_mut().composing = true;
            }
            let composing = entry.editing.borrow().composing;
            if let Some(event) = super::input_edit::event(&entry, &element, composing)? {
                dom.queue(id, event);
                return Ok(());
            }
            let edit = entry.editing.borrow();
            let event = Event::ValueChanged {
                value: entry.value()?,
                composing: edit.composing,
                revision: edit.revision,
            };
            drop(edit);
            dom.queue(id, event);
        }
        "beforeinput" if entry.edit == EditKind::Text => {
            super::input_edit::before(&entry, &element, &event)?;
        }
        "change" if entry.edit == EditKind::Select => {
            let keys = super::select::read(&entry.control()?)?;
            let revision = entry.editing.borrow().revision;
            dom.queue(id, Event::SelectChanged { keys, revision });
        }
        "change" if entry.edit == EditKind::Check => {
            let control = entry.control()?;
            let checked = js_sys::Reflect::get(&control, &"checked".into())
                .map_err(backend)?
                .as_bool()
                .unwrap_or(false);
            dom.queue(id, Event::CheckedChanged(checked));
        }
        "paste" if entry.edit == EditKind::Text => {
            tags::paste(dom, id, &entry, &element, &node, &event)?;
        }
        "compositionstart" if entry.edit == EditKind::Text => {
            entry.editing.borrow_mut().composing = true;
            dom.queue(id, Event::Composition { active: true });
        }
        "compositionend" if entry.edit == EditKind::Text => {
            js_sys::Reflect::set(
                &entry.control()?.into(),
                &js_sys::Symbol::for_("glory.preload.composition").into(),
                &false.into(),
            )
            .map_err(backend)?;
            dom.queue(id, Event::Composition { active: false });
            if let Some(event) = super::input_edit::event(&entry, &element, false)? {
                dom.queue(id, event);
            } else {
                let revision = entry.editing.borrow().revision;
                let value = entry.value()?;
                dom.queue(
                    id,
                    Event::ValueChanged {
                        value,
                        composing: false,
                        revision,
                    },
                );
            }
            entry.composition_end()?;
        }
        "keydown" => {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return Ok(());
            };
            if key.is_composing() || entry.editing.borrow().composing || event.default_prevented() {
                return Ok(());
            }
            let editable = entry.edit == EditKind::Text
                || entry.edit != EditKind::Check
                    && element
                        .closest("input,textarea,select,[contenteditable=true],[role=textbox]")
                        .map_err(backend)?
                        .is_some();
            if tags::key(dom, id, &entry, &element, &node, &event, key)? {
                return Ok(());
            }
            if element.has_attribute("data-gly-tree-item")
                && node == Node::from(entry.control()?)
                && dom.document.active_element().as_ref() == Some(&element)
                && !editable
            {
                let mut cursor = dom.parent(id);
                while let Some(tree) = cursor {
                    let root = dom.entry(tree)?;
                    if root.parts.get(&glory_html::Part::Main).is_none() {
                        cursor = dom.parent(tree);
                        continue;
                    }
                    let root_element = root.element()?;
                    if root_element.get_attribute("role").as_deref() == Some("tree") {
                        let direction = match root_element
                            .get_attribute("data-gly-tree-navigation")
                            .as_deref()
                        {
                            Some("ltr") => Some(glory_render::Direction::Ltr),
                            Some("rtl") => Some(glory_render::Direction::Rtl),
                            _ => None,
                        };
                        let clock = key.time_stamp();
                        if root.interests.borrow().contains(&EventKind::TreeNavigate)
                            && clock.is_finite()
                            && clock >= 0.0
                            && clock <= glory_render::MAX_TYPEAHEAD_MILLIS as f64
                            && let Some(command) = direction.and_then(|direction| {
                                glory_render::TreeCommand::key(
                                    glory_render::TreeNavigation {
                                        direction,
                                        typeahead: root_element
                                            .get_attribute("data-gly-tree-typeahead")
                                            .as_deref()
                                            == Some("true"),
                                        multiple: root_element
                                            .get_attribute("aria-multiselectable")
                                            .as_deref()
                                            == Some("true"),
                                    },
                                    KeyboardInput {
                                        key: &key.key(),
                                        control: key.ctrl_key(),
                                        meta: key.meta_key(),
                                        alt: key.alt_key(),
                                        shift: key.shift_key(),
                                        repeat: key.repeat(),
                                        composing: false,
                                        editable,
                                    },
                                    clock as u64,
                                )
                            })
                        {
                            event.prevent_default();
                            dom.queue(tree, Event::TreeNavigate { from: id, command });
                            return Ok(());
                        }
                        break;
                    }
                    cursor = dom.parent(tree);
                }
            }
            if node == Node::from(entry.control()?)
                && dom.document.active_element().as_ref() == Some(&element)
                && entry.interests.borrow().contains(&EventKind::Choice)
            {
                let kind = match element.get_attribute("data-gly-combobox").as_deref() {
                    Some("select") => Some(glory_render::ComboboxKind::Select),
                    Some("editable") => Some(glory_render::ComboboxKind::Editable),
                    Some("autocomplete") => Some(glory_render::ComboboxKind::Autocomplete),
                    Some("multiple") => Some(glory_render::ComboboxKind::Multiple),
                    _ => None,
                };
                if let Some(kind) = kind {
                    let input = KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: false,
                        editable,
                    };
                    if kind == glory_render::ComboboxKind::Multiple {
                        let value =
                            js_sys::Reflect::get(entry.control()?.as_ref(), &"value".into())
                                .map_err(backend)?;
                        if let Some(command) = input
                            .choice_remove_last(kind, js_sys::JsString::from(value).length() == 0)
                        {
                            event.prevent_default();
                            dom.queue(id, Event::Choice { from: id, command });
                            return Ok(());
                        }
                    }
                    if let Some(command) = input.choice(
                        kind,
                        element.get_attribute("aria-expanded").as_deref() == Some("true"),
                    ) {
                        event.prevent_default();
                        dom.queue(id, Event::Choice { from: id, command });
                        return Ok(());
                    }
                    let text = key.key();
                    let clock = key.time_stamp();
                    let at_millis = clock as u64;
                    if kind == glory_render::ComboboxKind::Select
                        && element.has_attribute("data-gly-typeahead")
                        && entry.interests.borrow().contains(&EventKind::Typeahead)
                        && clock.is_finite()
                        && clock >= 0.0
                        && clock <= glory_render::MAX_TYPEAHEAD_MILLIS as f64
                        && input.typeahead(at_millis).is_some()
                    {
                        event.prevent_default();
                        dom.queue(
                            id,
                            Event::Typeahead {
                                from: id,
                                text,
                                at_millis,
                            },
                        );
                        return Ok(());
                    }
                }
            }
            if element.tag_name() == "BUTTON"
                && dom.document.active_element().as_ref() == Some(&element)
                && !editable
            {
                let mut cursor = dom.parent(id);
                while let Some(grid) = cursor {
                    let root = dom.entry(grid)?;
                    // Logical component/when/each Groups have no Main part.
                    // They do not terminate the nearest boxed Grid search.
                    if root.parts.get(&glory_html::Part::Main).is_none() {
                        cursor = dom.parent(grid);
                        continue;
                    }
                    let grid_element = root.element()?;
                    if grid_element.get_attribute("role").as_deref() == Some("grid") {
                        let direction = match grid_element
                            .get_attribute("data-gly-grid-navigation")
                            .as_deref()
                        {
                            Some("ltr") => Some(glory_render::Direction::Ltr),
                            Some("rtl") => Some(glory_render::Direction::Rtl),
                            _ => None,
                        };
                        if root.interests.borrow().contains(&EventKind::GridNavigate)
                            && let Some(command) = direction.and_then(|direction| {
                                glory_render::GridCommand::key(
                                    direction,
                                    KeyboardInput {
                                        key: &key.key(),
                                        control: key.ctrl_key(),
                                        meta: key.meta_key(),
                                        alt: key.alt_key(),
                                        shift: key.shift_key(),
                                        repeat: key.repeat(),
                                        composing: false,
                                        editable,
                                    },
                                )
                            })
                        {
                            event.prevent_default();
                            dom.queue(grid, Event::GridNavigate { from: id, command });
                            return Ok(());
                        }
                        break;
                    }
                    cursor = dom.parent(grid);
                }
            }
            if node == Node::from(entry.element()?)
                && dom.document.active_element().as_ref() == Some(&element)
                && element.has_attribute("data-gly-spinbutton")
                && entry.interests.borrow().contains(&EventKind::Spin)
                && element.get_attribute("aria-disabled").as_deref() != Some("true")
                && element.get_attribute("aria-readonly").as_deref() != Some("true")
                && let Some(command) = glory_render::RangeCommand::spin_key(
                    element.has_attribute("aria-valuemin"),
                    element.has_attribute("aria-valuemax"),
                    glory_render::KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: key.is_composing(),
                        editable: true,
                    },
                )
            {
                event.prevent_default();
                dom.queue(id, Event::Spin { from: id, command });
                return Ok(());
            }
            if node == Node::from(entry.element()?)
                && dom.document.active_element().as_ref() == Some(&element)
                && element.has_attribute("data-gly-slider")
                && entry.interests.borrow().contains(&EventKind::Range)
            {
                let axis =
                    if element.get_attribute("aria-orientation").as_deref() == Some("vertical") {
                        glory_render::Orientation::Vertical
                    } else {
                        glory_render::Orientation::Horizontal
                    };
                let direction = if element.get_attribute("dir").as_deref() == Some("rtl") {
                    glory_render::Direction::Rtl
                } else {
                    glory_render::Direction::Ltr
                };
                if let Some(command) = glory_render::RangeCommand::slider_key(
                    axis,
                    direction,
                    glory_render::KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: key.is_composing(),
                        editable: false,
                    },
                ) {
                    event.prevent_default();
                    if element.get_attribute("aria-disabled").as_deref() != Some("true")
                        && element.get_attribute("aria-readonly").as_deref() != Some("true")
                    {
                        dom.queue(id, Event::Range { from: id, command });
                    }
                    return Ok(());
                }
            }
            if node == Node::from(entry.element()?)
                && dom.document.active_element().as_ref() == Some(&element)
                && element.has_attribute("data-gly-splitter")
                && element.get_attribute("aria-disabled").as_deref() != Some("true")
                && entry.interests.borrow().contains(&EventKind::Range)
            {
                let line =
                    if element.get_attribute("aria-orientation").as_deref() == Some("vertical") {
                        glory_render::Orientation::Vertical
                    } else {
                        glory_render::Orientation::Horizontal
                    };
                if let Some(command) = glory_render::RangeCommand::splitter_key(
                    line,
                    KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: false,
                        editable,
                    },
                ) {
                    event.prevent_default();
                    dom.queue(id, Event::Range { from: id, command });
                    return Ok(());
                }
            }
            if node == Node::from(entry.element()?)
                && dom.document.active_element().as_ref() == Some(&element)
                && entry
                    .interests
                    .borrow()
                    .contains(&EventKind::ScrollKeyboard)
            {
                let axes = match element.get_attribute("data-gly-scroll-keyboard").as_deref() {
                    Some("both") => Some(glory_render::ScrollAxes::Both),
                    Some("horizontal") => Some(glory_render::ScrollAxes::Horizontal),
                    Some("vertical") => Some(glory_render::ScrollAxes::Vertical),
                    _ => None,
                };
                if let Some(command) = axes.and_then(|axes| {
                    axes.key(KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: false,
                        editable,
                    })
                }) {
                    event.prevent_default();
                    dom.queue(id, Event::ScrollKeyboard { from: id, command });
                    return Ok(());
                }
            }
            let command = KeyboardInput {
                key: &key.key(),
                control: key.ctrl_key(),
                meta: key.meta_key(),
                alt: key.alt_key(),
                shift: key.shift_key(),
                repeat: key.repeat(),
                composing: false,
                editable,
            }
            .command();
            if let Some(command) = command {
                let form_submit = command == KeyboardCommand::Submit
                    && entry.edit == EditKind::Text
                    && entry.control()?.tag_name() != "TEXTAREA"
                    && element
                        .closest("form[data-gly-form]")
                        .map_err(backend)?
                        .is_some();
                let own_interest = entry.interests.borrow().contains(&EventKind::Keyboard);
                let palette = command == glory_render::KeyboardCommand::Palette;
                let mut cursor = Some(id);
                let mut interested = false;
                let mut palette_interest = false;
                while let Some(node) = cursor {
                    if palette
                        && dom.entry(node).is_ok_and(|entry| {
                            entry.interests.borrow().contains(&EventKind::Palette)
                        })
                    {
                        interested = true;
                        palette_interest = true;
                        break;
                    }
                    if dom
                        .entry(node)
                        .is_ok_and(|entry| entry.interests.borrow().contains(&EventKind::Keyboard))
                    {
                        interested = true;
                        break;
                    }
                    cursor = dom.parent(node);
                }
                if interested && (!editable || own_interest) || form_submit {
                    if command == KeyboardCommand::Submit {
                        queue_commit(dom, id, &entry, &element, true)?;
                    }
                    event.prevent_default();
                    dom.queue(
                        id,
                        if palette_interest {
                            Event::Palette { from: id }
                        } else {
                            Event::Keyboard { from: id, command }
                        },
                    );
                    return Ok(());
                }
            }
            if entry.edit == EditKind::Text {
                if key.key() == "Enter" && entry.control()?.tag_name() != "TEXTAREA" {
                    queue_commit(dom, id, &entry, &element, false)?;
                }
                return Ok(());
            }
            let panel_trigger = element
                .get_attribute("data-gly-navigation-panel")
                .as_deref()
                == Some("trigger");
            let panel_content = element
                .closest("[data-gly-navigation-panel=content]")
                .map_err(backend)?;
            if node == Node::from(entry.control()?)
                && (panel_trigger
                    || panel_content.is_some() && element.has_attribute("data-gly-navigation-item"))
            {
                let boundary = panel_content.as_ref();
                let direction = boundary
                    .and_then(|root| root.get_attribute("dir"))
                    .or_else(|| {
                        element
                            .closest("[data-gly-navigation]")
                            .ok()
                            .flatten()
                            .and_then(|root| root.get_attribute("dir"))
                    });
                let direction = if direction.as_deref() == Some("rtl") {
                    glory_render::Direction::Rtl
                } else {
                    glory_render::Direction::Ltr
                };
                if let Some(command) = (KeyboardInput {
                    key: &key.key(),
                    control: key.ctrl_key(),
                    meta: key.meta_key(),
                    alt: key.alt_key(),
                    shift: key.shift_key(),
                    repeat: key.repeat(),
                    composing: false,
                    editable,
                })
                .navigation_panel(
                    direction,
                    panel_trigger,
                    panel_content.is_some(),
                ) {
                    let interested = if command == glory_render::NavigationPanelCommand::Close {
                        panel_content
                            .as_ref()
                            .and_then(|root| target(dom, root))
                            .and_then(|(id, _)| dom.entry(id).ok())
                            .is_some_and(|entry| {
                                entry
                                    .interests
                                    .borrow()
                                    .contains(&EventKind::NavigationPanel)
                            })
                    } else {
                        entry
                            .interests
                            .borrow()
                            .contains(&EventKind::NavigationPanel)
                    };
                    if interested {
                        event.prevent_default();
                        dom.queue(id, Event::NavigationPanel { from: id, command });
                        return Ok(());
                    }
                }
            }
            let menu_trigger = element.get_attribute("aria-haspopup").as_deref() == Some("menu")
                && (!element.has_attribute("data-gly-menu-item")
                    || element.has_attribute("data-gly-menubar-item"));
            if entry.edit == EditKind::Check
                && matches!(
                    element.get_attribute("role").as_deref(),
                    Some("menuitemcheckbox" | "menuitemradio")
                )
                && (KeyboardInput {
                    key: &key.key(),
                    control: key.ctrl_key(),
                    meta: key.meta_key(),
                    alt: key.alt_key(),
                    shift: key.shift_key(),
                    repeat: key.repeat(),
                    composing: false,
                    editable,
                })
                .menu_check_activation()
            {
                event.prevent_default();
                entry
                    .control()?
                    .dyn_into::<web_sys::HtmlElement>()
                    .map_err(|_| glory_render::Error::Unsupported("menu check activation"))?
                    .click();
                return Ok(());
            }
            let menu_root = element.closest("[role=menu]").map_err(backend)?;
            let menubar_root = element.closest("[role=menubar]").map_err(backend)?;
            if menu_trigger || menu_root.is_some() {
                let direction = if menu_root
                    .as_ref()
                    .or(menubar_root.as_ref())
                    .and_then(|root| root.get_attribute("dir"))
                    .as_deref()
                    == Some("rtl")
                {
                    glory_render::Direction::Rtl
                } else {
                    glory_render::Direction::Ltr
                };
                let interested = if menu_trigger {
                    entry.interests.borrow().contains(&EventKind::Menu)
                } else {
                    menu_root
                        .as_ref()
                        .and_then(|root| target(dom, root))
                        .and_then(|(id, _)| dom.entry(id).ok())
                        .is_some_and(|entry| entry.interests.borrow().contains(&EventKind::Menu))
                };
                if interested
                    && let Some(command) = (KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: false,
                        editable,
                    })
                    .menu(direction, menu_trigger)
                {
                    event.prevent_default();
                    dom.queue(id, Event::Menu { from: id, command });
                    return Ok(());
                }
            }
            // Explicit composite triggers and the navigation boundary itself opt in.
            // Descendant editors and ordinary buttons retain native behavior.
            if (element.has_attribute("data-gly-navigation-item")
                || element.has_attribute("data-gly-navigation"))
                && node == Node::from(entry.control()?)
                && let Some(root) = element.closest("[data-gly-navigation]").map_err(backend)?
                && let Some((root_id, _)) = target(dom, &root)
                && dom
                    .entry(root_id)?
                    .interests
                    .borrow()
                    .contains(&EventKind::Navigate)
            {
                let axis = match root.get_attribute("data-gly-navigation").as_deref() {
                    Some("horizontal") => glory_render::Orientation::Horizontal,
                    Some("vertical") => glory_render::Orientation::Vertical,
                    _ => return Ok(()),
                };
                let direction = if root.get_attribute("dir").as_deref() == Some("rtl") {
                    glory_render::Direction::Rtl
                } else {
                    glory_render::Direction::Ltr
                };
                if let Some(command) = axis.key_with_direction(
                    direction,
                    KeyboardInput {
                        key: &key.key(),
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: false,
                        editable,
                    },
                ) {
                    event.prevent_default();
                    dom.queue(id, Event::Navigate { from: id, command });
                    return Ok(());
                }
                let text = key.key();
                let clock = key.time_stamp();
                let at_millis = clock as u64;
                if clock.is_finite()
                    && clock >= 0.0
                    && clock <= glory_render::MAX_TYPEAHEAD_MILLIS as f64
                    && root.has_attribute("data-gly-typeahead")
                    && dom
                        .entry(root_id)?
                        .interests
                        .borrow()
                        .contains(&EventKind::Typeahead)
                    && (KeyboardInput {
                        key: &text,
                        control: key.ctrl_key(),
                        meta: key.meta_key(),
                        alt: key.alt_key(),
                        shift: key.shift_key(),
                        repeat: key.repeat(),
                        composing: key.is_composing(),
                        editable,
                    })
                    .typeahead(at_millis)
                    .is_some()
                {
                    event.prevent_default();
                    dom.queue(
                        id,
                        Event::Typeahead {
                            from: id,
                            text,
                            at_millis,
                        },
                    );
                    return Ok(());
                }
            }
            // Navigation belongs to the list/row shell only. Descendant inputs,
            // buttons and links keep native cursor/activation/Tab behavior.
            if key.alt_key()
                || key.ctrl_key()
                || key.meta_key()
                || key.shift_key()
                || !matches!(
                    element.get_attribute("data-gly-virtual").as_deref(),
                    Some("root" | "row")
                )
            {
                return Ok(());
            }
            if node != Node::from(entry.element()?) {
                return Ok(());
            }
            let command = match key.key().as_str() {
                "ArrowUp" => Navigation::Previous,
                "ArrowDown" => Navigation::Next,
                "Home" => Navigation::First,
                "End" => Navigation::Last,
                "PageUp" => Navigation::PagePrevious,
                "PageDown" => Navigation::PageNext,
                _ => return Ok(()),
            };
            let listener = element
                .closest("[data-gly-virtual=root]")
                .map_err(backend)?
                .and_then(|root| target(dom, &root))
                .and_then(|(id, _)| dom.entry(id).ok());
            if listener.is_some_and(|entry| entry.interests.borrow().contains(&EventKind::Navigate))
            {
                event.prevent_default();
                dom.queue(id, Event::Navigate { from: id, command });
            }
        }
        // App also derives FocusWithin for semantic ancestors. The origin need
        // not have its own listener (nor a physical Group parent).
        "focusin" | "focusout" => dom.queue(id, Event::FocusChanged(name == "focusin")),
        "pointermove" => {
            if event
                .dyn_ref::<web_sys::PointerEvent>()
                .is_some_and(|pointer| matches!(pointer.pointer_type().as_str(), "mouse" | "pen"))
            {
                dom.queue(
                    id,
                    Event::Pointer {
                        from: id,
                        related: None,
                        phase: glory_render::PointerPhase::Move,
                    },
                );
            }
        }
        "pointerover" | "pointerout" => {
            let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>() else {
                return Ok(());
            };
            if pointer.pointer_type() != "mouse" && pointer.pointer_type() != "pen" {
                return Ok(());
            }
            let related = pointer
                .related_target()
                .and_then(|target| target.dyn_into::<Node>().ok())
                .and_then(|node| {
                    node.dyn_ref::<Element>()
                        .cloned()
                        .or_else(|| node.parent_element())
                })
                .and_then(|element| target(dom, &element))
                .map(|(id, _)| id);
            if related != Some(id) {
                dom.queue(
                    id,
                    Event::Pointer {
                        from: id,
                        related,
                        phase: if name == "pointerover" {
                            glory_render::PointerPhase::Enter
                        } else {
                            glory_render::PointerPhase::Leave
                        },
                    },
                );
            }
        }
        "scroll" if entry.interests.borrow().contains(&EventKind::Scroll) => {
            dom.layout.borrow_mut().insert((id, EventKind::Scroll));
            dom.frame.input();
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn cancel_blocked_capture(dom: &Dom) -> Result<(), glory_render::Error> {
    let from = dom.drag.borrow().as_ref().map(|drag| drag.from);
    if let Some(from) = from
        && let Ok(element) = dom.entry(from).and_then(|entry| entry.element())
        && !dom.document_layers.allows(Some(element.as_ref()))
    {
        drag::cancel(dom);
    }
    super::sheet::cancel_outside_document_modal(dom)
}
