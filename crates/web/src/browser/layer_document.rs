//! One live coordinator per Document. The lookup owns only Weak handles; Apps
//! own the coordinator and it owns only weak App registrations/current layers.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::JsValue;
use web_sys::{Document, HtmlElement};
#[path = "focus_scope.rs"]
mod focus_scope;

thread_local! { static DOCUMENTS: RefCell<Vec<Weak<DocumentLayers>>> = const { RefCell::new(Vec::new()) }; }
const MAX_APPS: usize = 64;

#[derive(Clone)]
struct Global {
    owner: Weak<Dom>,
    open: Open,
    fallback: Option<Element>,
    allowed: bool,
}
#[derive(Default)]
struct State {
    apps: Vec<Weak<Dom>>,
    stack: Vec<Global>,
    inert: Vec<Element>,
    scroll: Option<ScrollLock>,
}
pub(in crate::browser) struct DocumentLayers {
    document: Document,
    state: RefCell<State>,
    busy: Cell<bool>,
    event_tag: js_sys::Object,
    scopes: RefCell<Vec<focus_scope::Scope>>,
}
struct Busy<'a>(&'a Cell<bool>);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.set(false)
    }
}
struct Working<'a> {
    coordinator: &'a DocumentLayers,
    state: State,
}
impl std::ops::Deref for Working<'_> {
    type Target = State;
    fn deref(&self) -> &State {
        &self.state
    }
}
impl std::ops::DerefMut for Working<'_> {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.state
    }
}
impl Drop for Working<'_> {
    fn drop(&mut self) {
        *self.coordinator.state.borrow_mut() = std::mem::take(&mut self.state)
    }
}

struct SavedStyle {
    element: HtmlElement,
    name: &'static str,
    value: String,
    priority: String,
    installed: String,
}
impl SavedStyle {
    fn install(element: HtmlElement, name: &'static str, value: &str) -> Result<Self, Error> {
        let style = element.style();
        let mut saved = Self {
            element: element.clone(),
            name,
            value: style.get_property_value(name).map_err(backend)?,
            priority: style.get_property_priority(name),
            installed: String::new(),
        };
        style
            .set_property_with_priority(name, value, "important")
            .map_err(backend)?;
        saved.installed = style.get_property_value(name).map_err(backend)?;
        Ok(saved)
    }
    fn restore(self) {
        let style = self.element.style();
        if style.get_property_value(self.name).ok().as_deref() != Some(self.installed.as_str())
            || style.get_property_priority(self.name) != "important"
        {
            return;
        }
        if self.value.is_empty() {
            let _ = style.remove_property(self.name);
        } else {
            let _ = style.set_property_with_priority(self.name, &self.value, &self.priority);
        }
    }
}
struct ScrollLock {
    styles: Vec<SavedStyle>,
    x: f64,
    y: f64,
}
impl ScrollLock {
    fn acquire(document: &Document) -> Result<Self, Error> {
        let window = document
            .default_view()
            .ok_or(Error::Unsupported("layer document window"))?;
        let x = window.scroll_x().map_err(backend)?;
        let y = window.scroll_y().map_err(backend)?;
        let html = document
            .document_element()
            .ok_or(Error::Unsupported("layer document root"))?;
        let width = window
            .inner_width()
            .map_err(backend)?
            .as_f64()
            .unwrap_or(0.);
        let gutter = (width - f64::from(html.client_width())).max(0.);
        let mut lock = Self {
            styles: Vec::new(),
            x,
            y,
        };
        let result = (|| {
            for element in [Some(html), document.body().map(Into::into)]
                .into_iter()
                .flatten()
            {
                let element = element
                    .dyn_into::<HtmlElement>()
                    .map_err(|_| Error::Unsupported("layer document style"))?;
                for name in ["overflow-x", "overflow-y"] {
                    lock.styles
                        .push(SavedStyle::install(element.clone(), name, "hidden")?);
                }
            }
            if gutter > 0.
                && let Some(body) = document.body()
            {
                let padding = window
                    .get_computed_style(&body)
                    .map_err(backend)?
                    .and_then(|style| style.get_property_value("padding-right").ok())
                    .unwrap_or_else(|| "0px".into());
                lock.styles.push(SavedStyle::install(
                    body,
                    "padding-right",
                    &format!("calc({padding} + {gutter}px)"),
                )?);
            }
            Ok(())
        })();
        if let Err(error) = result {
            lock.release(document);
            return Err(error);
        }
        // Removing a scrollbar changes the viewport and can run native scroll
        // anchoring. Commit that layout, then retain the original viewport.
        if let Some(root) = document.document_element() {
            let _ = root.get_bounding_client_rect();
        }
        window.scroll_to_with_x_and_y(x, y);
        Ok(lock)
    }
    fn release(self, document: &Document) {
        for style in self.styles.into_iter().rev() {
            style.restore()
        }
        if let Some(window) = document.default_view() {
            window.scroll_to_with_x_and_y(self.x, self.y)
        }
    }
}
impl DocumentLayers {
    pub(in crate::browser) fn acquire(document: &Document) -> Result<Rc<Self>, Error> {
        let found = DOCUMENTS.with(|documents| {
            let mut documents = documents.borrow_mut();
            documents.retain(|entry| entry.strong_count() > 0);
            documents
                .iter()
                .filter_map(Weak::upgrade)
                .find(|entry| entry.document == *document)
        });
        let result = if let Some(found) = found {
            found
        } else {
            let created = Rc::new(Self {
                document: document.clone(),
                state: RefCell::new(State::default()),
                busy: Cell::new(false),
                event_tag: js_sys::Object::new(),
                scopes: RefCell::new(Vec::new()),
            });
            DOCUMENTS.with(|documents| documents.borrow_mut().push(Rc::downgrade(&created)));
            created
        };
        let mut state = result.state.borrow_mut();
        state.apps.retain(|app| app.strong_count() > 0);
        if state.apps.len() >= MAX_APPS {
            return Err(Error::Unsupported("document live App layer budget"));
        }
        drop(state);
        Ok(result)
    }
    pub(in crate::browser) fn register(&self, dom: &Rc<Dom>) {
        self.state.borrow_mut().apps.push(Rc::downgrade(dom))
    }
    fn restore_inert(state: &mut State) {
        for element in state.inert.drain(..) {
            if !element.has_attribute("data-gly-inert") {
                let _ = element.remove_attribute("inert");
            }
        }
    }
    /// Release only owned inert leases before reading focusability or changing
    /// a local stack. No DOM callbacks execute under the state borrow.
    pub(in crate::browser) fn prepare(&self) {
        let mut inert = std::mem::take(&mut self.state.borrow_mut().inert);
        for element in inert.drain(..) {
            if !element.has_attribute("data-gly-inert") {
                let _ = element.remove_attribute("inert");
            }
        }
    }
    pub(in crate::browser) fn sync(&self, preferred: Option<Element>) -> Result<(), Error> {
        if self.busy.replace(true) {
            return Ok(());
        }
        let busy = Busy(&self.busy);
        let mut state = Working {
            coordinator: self,
            state: std::mem::take(&mut *self.state.borrow_mut()),
        };
        Self::restore_inert(&mut state);
        state.apps.retain(|app| app.strong_count() > 0);
        let apps = state
            .apps
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        let current = apps
            .iter()
            .filter(|dom| dom.live.get())
            .flat_map(|dom| {
                dom.layers
                    .borrow()
                    .stack
                    .iter()
                    .filter(|open| {
                        open.element.is_connected()
                            && open.element.closest("[hidden]").ok().flatten().is_none()
                    })
                    .cloned()
                    .map(|open| (Rc::downgrade(dom), open))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut removed = Vec::new();
        state.stack.retain_mut(|entry| {
            if let Some((_, open)) = current
                .iter()
                .find(|(owner, open)| Weak::ptr_eq(owner, &entry.owner) && open.id == entry.open.id)
            {
                entry.open = open.clone();
                true
            } else {
                removed.push(entry.clone());
                false
            }
        });
        for (owner, open) in current {
            if !state
                .stack
                .iter()
                .any(|entry| Weak::ptr_eq(&owner, &entry.owner) && entry.open.id == open.id)
            {
                let fallback = owner
                    .upgrade()
                    .and_then(|dom| named_control(&dom, open.config.return_focus.as_ref()))
                    .or_else(|| open.previous.clone());
                state.stack.push(Global {
                    owner,
                    open,
                    fallback,
                    allowed: true,
                });
            }
        }
        // Transfer return paths while the upper layer is still live, then drop
        // the closed entry. Out-of-order teardown does not retain ghost layers.
        for closed in &removed {
            for entry in &mut state.stack {
                if entry
                    .fallback
                    .as_ref()
                    .is_some_and(|target| closed.open.element.contains(Some(target)))
                {
                    entry.fallback = closed.fallback.clone();
                    if let Some(dom) = entry.owner.upgrade()
                        && let Some(open) = dom
                            .layers
                            .borrow_mut()
                            .stack
                            .iter_mut()
                            .find(|open| open.id == entry.open.id)
                    {
                        open.previous = entry.fallback.clone();
                    }
                }
            }
        }
        let modal = state.stack.iter().rposition(|entry| {
            entry.open.config.modal
                && entry
                    .open
                    .element
                    .closest("[inert]")
                    .ok()
                    .flatten()
                    .is_none()
        });
        let mut allowed = Vec::<Element>::new();
        for (index, entry) in state.stack.iter_mut().enumerate() {
            entry.allowed = modal.is_none_or(|modal| {
                index == modal
                    || index > modal
                        && (allowed
                            .iter()
                            .any(|element| element.contains(Some(&entry.open.element)))
                            || entry
                                .owner
                                .upgrade()
                                .and_then(|dom| named(&dom, entry.open.config.anchor.as_ref()))
                                .is_some_and(|anchor| {
                                    allowed
                                        .iter()
                                        .any(|element| element.contains(Some(&anchor)))
                                }))
            });
            if entry.allowed {
                allowed.push(entry.open.element.clone())
            }
        }
        for entry in &state.stack {
            let visible = entry.allowed
                || allowed
                    .iter()
                    .any(|element| entry.open.element.contains(Some(element)));
            let shown = entry.open.element.matches(":popover-open").unwrap_or(false);
            if visible && !shown {
                popover(&entry.open.element, "showPopover")?;
            } else if !visible && shown {
                popover(&entry.open.element, "hidePopover")?;
            }
        }
        if modal.is_some() {
            // Preserve every allowed branch to BODY, inert siblings and any
            // disallowed top-layer node (top layers may escape inert ancestry).
            let mut branches = Vec::<Element>::new();
            for element in &allowed {
                let mut cursor = Some(element.clone());
                while let Some(node) = cursor {
                    if !branches.contains(&node) {
                        branches.push(node.clone());
                    }
                    if node.tag_name() == "BODY" {
                        break;
                    }
                    cursor = node.parent_element();
                }
            }
            let mut blocked = Vec::<Element>::new();
            for branch in &branches {
                let mut child = branch.first_element_child();
                while let Some(node) = child {
                    child = node.next_element_sibling();
                    if !branches.contains(&node)
                        && !allowed.iter().any(|element| element.contains(Some(&node)))
                        && !blocked.contains(&node)
                    {
                        blocked.push(node);
                    }
                }
            }
            blocked.extend(
                state
                    .stack
                    .iter()
                    .filter(|entry| {
                        !entry.allowed
                            && !allowed
                                .iter()
                                .any(|element| entry.open.element.contains(Some(element)))
                    })
                    .map(|entry| entry.open.element.clone()),
            );
            for element in blocked {
                if !element.has_attribute("inert") {
                    element.set_attribute("inert", "").map_err(backend)?;
                    state.inert.push(element);
                }
            }
            if state.scroll.is_none() {
                state.scroll = Some(ScrollLock::acquire(&self.document)?)
            }
        } else if let Some(lock) = state.scroll.take() {
            lock.release(&self.document)
        }
        let restore_requested = preferred.is_some();
        let target = preferred
            .filter(focusable)
            .filter(|target| {
                modal.is_none() || allowed.iter().any(|element| element.contains(Some(target)))
            })
            .or_else(|| {
                if !restore_requested {
                    return None;
                }
                removed
                    .iter()
                    .rev()
                    .flat_map(|entry| [entry.fallback.clone(), entry.open.previous.clone()])
                    .flatten()
                    .find(|target| {
                        focusable(target)
                            && (modal.is_none()
                                || allowed.iter().any(|element| element.contains(Some(target))))
                    })
            });
        let active = self.document.active_element();
        let repair = modal.is_some()
            && !active.as_ref().is_some_and(|target| {
                allowed.iter().any(|element| element.contains(Some(target))) && focusable(target)
            });
        let repair_target = if repair {
            state
                .stack
                .iter()
                .rev()
                .find(|entry| entry.allowed && entry.open.config.focus == LayerFocus::Manage)
                .and_then(|entry| entry.owner.upgrade().map(|dom| first(&dom, &entry.open)))
        } else {
            None
        };
        drop(state);
        drop(busy);
        for app in apps {
            super::super::events::cancel_blocked_capture(&app)?;
        }
        if let Some(target) = target.or(repair_target) {
            dom::focus(&target)?;
        }
        self.sync_scopes()
    }
    pub(in crate::browser) fn unregister(&self, dom: &Dom) {
        self.state
            .borrow_mut()
            .apps
            .retain(|app| !std::ptr::eq(app.as_ptr(), dom) && app.strong_count() > 0);
    }
    pub(in crate::browser) fn modal_scope(&self) -> Option<Element> {
        self.state
            .borrow()
            .stack
            .iter()
            .rev()
            .find(|entry| entry.allowed && entry.open.config.modal)
            .map(|entry| entry.open.element.clone())
    }
    pub(in crate::browser) fn allows(&self, target: Option<&Node>) -> bool {
        let state = self.state.borrow();
        !state.stack.iter().any(|entry| entry.open.config.modal)
            || target.is_some_and(|target| {
                state
                    .stack
                    .iter()
                    .any(|entry| entry.allowed && entry.open.element.contains(Some(target)))
            })
    }
    pub(in crate::browser) fn raise(&self, dom: &Dom, id: NodeId) -> Result<(), Error> {
        self.sync(None)?;
        let mut state = self.state.borrow_mut();
        let root = state
            .stack
            .iter()
            .position(|entry| std::ptr::eq(entry.owner.as_ptr(), dom) && entry.open.id == id)
            .ok_or(Error::Unsupported("layer is no longer open"))?;
        if !state.stack[root].allowed || state.stack[root].open.config.modal {
            return Err(Error::Unsupported(
                "layer raise crosses the current document modal",
            ));
        }
        // Current logical descendants and current anchor-associated popups move
        // together. This temporary membership belongs only to these live leases.
        let mut members = vec![false; state.stack.len()];
        members[root] = true;
        let mut queue = VecDeque::from([root]);
        while let Some(parent) = queue.pop_front() {
            let element = &state.stack[parent].open.element;
            for (index, entry) in state.stack.iter().enumerate() {
                if !members[index]
                    && (element.contains(Some(&entry.open.element))
                        || entry
                            .owner
                            .upgrade()
                            .and_then(|owner| named(&owner, entry.open.config.anchor.as_ref()))
                            .is_some_and(|anchor| element.contains(Some(&anchor))))
                {
                    members[index] = true;
                    queue.push_back(index);
                }
            }
        }
        if !members[..root].iter().any(|member| *member)
            && members[root..].iter().all(|member| *member)
        {
            return Ok(());
        }
        let mut root_entry = None;
        let mut related = Vec::new();
        let mut retained = Vec::new();
        for (index, entry) in std::mem::take(&mut state.stack).into_iter().enumerate() {
            if index == root {
                root_entry = Some(entry)
            } else if members[index] {
                related.push(entry)
            } else {
                retained.push(entry)
            }
        }
        let root_entry = root_entry.expect("current root");
        let mut elements = vec![root_entry.open.element.clone()];
        retained.push(root_entry);
        elements.extend(related.iter().map(|entry| entry.open.element.clone()));
        retained.extend(related);
        state.stack = retained;
        drop(state);
        // Manual popovers retain the actual focused editor while their host
        // top-layer order changes. No descriptor/value/focus write occurs here.
        for element in elements {
            if element.matches(":popover-open").unwrap_or(false) {
                popover(&element, "hidePopover")?;
                popover(&element, "showPopover")?;
            }
        }
        self.sync(None)
    }
    pub(in crate::browser) fn receive(
        &self,
        name: &str,
        event: &web_sys::Event,
    ) -> Result<bool, Error> {
        let symbol = js_sys::Symbol::for_("glory.layers.document.event");
        if js_sys::Reflect::get(event, &symbol)
            .ok()
            .is_some_and(|tag| tag == JsValue::from(self.event_tag.clone()))
        {
            return Ok(event.default_prevented());
        }
        js_sys::Reflect::set(event, &symbol, &self.event_tag).map_err(backend)?;
        if self.receive_scope(name, event)? {
            return Ok(true);
        }
        let target = event
            .target()
            .and_then(|value| value.dyn_into::<Node>().ok());
        if matches!(name, "wheel" | "touchmove") {
            if !self.allows(target.as_ref()) {
                event.prevent_default();
                event.stop_immediate_propagation();
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(name, "scroll" | "resize") {
            let apps = self
                .state
                .borrow()
                .apps
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>();
            for dom in apps {
                if !dom.layers.borrow().stack.is_empty() {
                    dom.layer_layout.set(true);
                    dom.frame.input();
                }
            }
            return Ok(false);
        }
        let stack = self
            .state
            .borrow()
            .stack
            .iter()
            .filter(|entry| entry.allowed)
            .cloned()
            .collect::<Vec<_>>();
        let chosen = if name == "focusin" {
            stack.iter().rev().find(|entry| entry.open.config.modal)
        } else if name == "keydown" {
            stack.iter().rev().find(|entry| {
                entry.open.config.focus == LayerFocus::Manage
                    || entry.owner.upgrade().is_some_and(|dom| {
                        target
                            .as_ref()
                            .is_some_and(|target| dom.root.contains(Some(target)))
                    })
                    || target
                        .as_ref()
                        .and_then(|node| node.dyn_ref::<Element>())
                        .is_some_and(|target| {
                            matches!(target.tag_name().as_str(), "BODY" | "HTML")
                                && (entry.open.element.matches(":hover").unwrap_or(false)
                                    || entry
                                        .owner
                                        .upgrade()
                                        .and_then(|dom| {
                                            named(&dom, entry.open.config.anchor.as_ref())
                                        })
                                        .is_some_and(|anchor| {
                                            anchor.matches(":hover").unwrap_or(false)
                                        }))
                        })
            })
        } else {
            stack.last()
        };
        if let Some(entry) = chosen
            && let Some(owner) = entry.owner.upgrade()
        {
            return super::receive_local(&owner, name, event);
        }
        Ok(false)
    }
}
impl Drop for DocumentLayers {
    fn drop(&mut self) {
        let state = self.state.get_mut();
        Self::restore_inert(state);
        if let Some(lock) = state.scroll.take() {
            lock.release(&self.document)
        }
        DOCUMENTS.with(|documents| {
            documents
                .borrow_mut()
                .retain(|entry| entry.strong_count() > 0)
        });
    }
}
