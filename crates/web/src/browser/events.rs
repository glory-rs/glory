use std::rc::Rc;

use glory_html::EditKind;
use glory_render::{Event, EventKind, KeyboardCommand, KeyboardInput, Navigation, NodeId};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, EventTarget, KeyboardEvent, Node, ResizeObserver};

use super::{backend, dom::Dom};
#[path = "drag.rs"]
mod drag;

struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(web_sys::Event)>,
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
            "click",
            "input",
            "beforeinput",
            "change",
            "compositionstart",
            "compositionend",
            "paste",
            "drop",
            "keydown",
            "focusin",
            "focusout",
            "scroll",
            "pointerdown",
            "pointermove",
            "pointerup",
            "pointercancel",
            "lostpointercapture",
            "blur",
        ] {
            let weak = Rc::downgrade(&dom);
            let callback = Closure::new(move |event: web_sys::Event| {
                let Some(dom) = weak.upgrade() else { return };
                if let Err(error) = receive(&dom, name, event) {
                    dom.error.borrow_mut().get_or_insert(error);
                    dom.frame.input();
                }
            });
            let target: EventTarget = if name == "blur" {
                dom.document
                    .default_view()
                    .map(Into::into)
                    .unwrap_or_else(|| dom.document.clone().into())
            } else if matches!(
                name,
                "keydown" | "pointermove" | "pointerup" | "pointercancel"
            ) {
                dom.document.clone().into()
            } else {
                dom.root.clone().into()
            };
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
        let weak = Rc::downgrade(&dom);
        let resize = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            let Some(dom) = weak.upgrade() else { return };
            for entry in entries {
                if let Ok(entry) = entry.dyn_into::<web_sys::ResizeObserverEntry>()
                    && let Some((id, _)) = target(&dom, &entry.target())
                {
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
    let id = dom
        .tokens
        .borrow()
        .get(&target.get_attribute("data-gly-node")?)
        .copied()?;
    Some((id, target))
}
fn receive(dom: &Dom, name: &str, event: web_sys::Event) -> Result<(), glory_render::Error> {
    if matches!(
        name,
        "pointerdown"
            | "pointermove"
            | "pointerup"
            | "pointercancel"
            | "lostpointercapture"
            | "blur"
    ) && drag::receive(dom, name, &event)?
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
    if entry.edit == EditKind::RichText
        && entry.control()?.get_attribute("contenteditable").as_deref() != Some("true")
        && matches!(
            name,
            "keydown" | "input" | "beforeinput" | "compositionstart" | "paste" | "drop"
        )
    {
        return Ok(());
    }
    match name {
        "click"
            if entry.edit == EditKind::RichText
                && super::rich_text::task(&entry.control()?, &event)? =>
        {
            let (document, selection) = super::rich_text::read(&entry.control()?)?;
            dom.queue(
                id,
                Event::RichTextChanged {
                    document,
                    selection,
                    revision: entry.editing.borrow().revision,
                    composing: entry.editing.borrow().composing,
                },
            );
        }
        "click" => {
            if dom.suppress_click.replace(false) {
                event.prevent_default();
                return Ok(());
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
        "paste" | "drop" if entry.edit == EditKind::RichText => {
            if super::rich_text::paste(&entry.control()?, &event)? {
                let (document, selection) = super::rich_text::read(&entry.control()?)?;
                dom.queue(
                    id,
                    Event::RichTextChanged {
                        document,
                        selection,
                        revision: entry.editing.borrow().revision,
                        composing: false,
                    },
                );
            }
        }
        "beforeinput" if entry.edit == EditKind::RichText => {
            if super::rich_text::before(&entry.control()?, &event)? {
                let (document, selection) = super::rich_text::read(&entry.control()?)?;
                dom.queue(
                    id,
                    Event::RichTextChanged {
                        document,
                        selection,
                        revision: entry.editing.borrow().revision,
                        composing: false,
                    },
                );
            }
        }
        "input" if entry.edit == EditKind::RichText => {
            if event
                .dyn_ref::<web_sys::InputEvent>()
                .is_some_and(|event| event.is_composing())
            {
                entry.editing.borrow_mut().composing = true;
            }
            let (document, selection) = super::rich_text::read(&entry.control()?)?;
            let edit = entry.editing.borrow();
            let event = Event::RichTextChanged {
                document,
                selection,
                revision: edit.revision,
                composing: edit.composing,
            };
            drop(edit);
            dom.queue(id, event);
        }
        "compositionstart" if entry.edit == EditKind::RichText => {
            super::rich_text::compose(&entry.control()?)?;
            entry.editing.borrow_mut().composing = true;
            dom.queue(id, Event::Composition { active: true });
        }
        "compositionend" if entry.edit == EditKind::RichText => {
            let (document, selection) = super::rich_text::read(&entry.control()?)?;
            let revision = entry.editing.borrow().revision;
            dom.queue(id, Event::Composition { active: false });
            dom.queue(
                id,
                Event::RichTextChanged {
                    document,
                    selection,
                    revision,
                    composing: false,
                },
            );
            entry.composition_end()?;
        }
        "input" if entry.edit == EditKind::Text => {
            if event
                .dyn_ref::<web_sys::InputEvent>()
                .is_some_and(|event| event.is_composing())
            {
                entry.editing.borrow_mut().composing = true;
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
        "change" if entry.edit == EditKind::Check => {
            let control = entry.control()?;
            let checked = js_sys::Reflect::get(&control, &"checked".into())
                .map_err(backend)?
                .as_bool()
                .unwrap_or(false);
            dom.queue(id, Event::CheckedChanged(checked));
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
            let revision = entry.editing.borrow().revision;
            let value = entry.value()?;
            dom.queue(id, Event::Composition { active: false });
            dom.queue(
                id,
                Event::ValueChanged {
                    value,
                    composing: false,
                    revision,
                },
            );
            entry.composition_end()?;
        }
        "keydown" if entry.edit == EditKind::RichText => {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return Ok(());
            };
            if !key.is_composing() && !entry.editing.borrow().composing {
                if let Some(command) = glory_render::rich_text::Command::shortcut(
                    &key.key(),
                    key.ctrl_key() || key.meta_key(),
                    key.shift_key(),
                    key.alt_key(),
                ) {
                    event.prevent_default();
                    let (document, selection) = super::rich_text::read(&entry.control()?)?;
                    dom.queue(
                        id,
                        Event::RichTextCommand {
                            document,
                            selection,
                            revision: entry.editing.borrow().revision,
                            command,
                        },
                    );
                    return Ok(());
                }
            }
            if !key.is_composing()
                && !entry.editing.borrow().composing
                && key.key() == "Enter"
                && (key.ctrl_key() || key.meta_key())
            {
                event.prevent_default();
                let (document, selection) = super::rich_text::read(&entry.control()?)?;
                dom.queue(
                    id,
                    Event::RichTextCommit {
                        document,
                        selection,
                        revision: entry.editing.borrow().revision,
                    },
                );
            }
        }
        "keydown" => {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return Ok(());
            };
            if key.is_composing() || entry.editing.borrow().composing || event.default_prevented() {
                return Ok(());
            }
            let editable = entry.edit == EditKind::Text
                || element
                    .closest("input,textarea,select,[contenteditable=true],[role=textbox]")
                    .map_err(backend)?
                    .is_some();
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
                let own_interest = entry.interests.borrow().contains(&EventKind::Keyboard);
                let mut cursor = Some(id);
                let mut interested = false;
                while let Some(node) = cursor {
                    if dom
                        .entry(node)
                        .is_ok_and(|entry| entry.interests.borrow().contains(&EventKind::Keyboard))
                    {
                        interested = true;
                        break;
                    }
                    cursor = dom.parents.borrow().get(&node).copied().flatten();
                }
                if interested && (!editable || own_interest) {
                    if command == KeyboardCommand::Submit {
                        dom.queue(
                            id,
                            Event::ValueChanged {
                                value: entry.value()?,
                                composing: false,
                                revision: entry.editing.borrow().revision,
                            },
                        );
                    }
                    event.prevent_default();
                    dom.queue(id, Event::Keyboard { from: id, command });
                    return Ok(());
                }
            }
            if entry.edit == EditKind::Text {
                if key.key() == "Enter" && entry.control()?.tag_name() != "TEXTAREA" {
                    let revision = entry.editing.borrow().revision;
                    dom.queue(
                        id,
                        Event::Commit {
                            value: entry.value()?,
                            revision,
                        },
                    );
                }
                return Ok(());
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
        "scroll" if entry.interests.borrow().contains(&EventKind::Scroll) => {
            dom.layout.borrow_mut().insert((id, EventKind::Scroll));
            dom.frame.input();
        }
        _ => {}
    }
    Ok(())
}
