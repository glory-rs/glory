use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use educe::Educe;
// #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsCast, UnwrapThrowExt};

use crate::reflow::{Bond, Lotus};
use crate::renderer::{InsertPosition, Renderer, WebRenderer};
use crate::view::{ViewId, ViewPlacement};
use crate::web::events::EventDescriptor;
use crate::web::{AttrValue, ClassPart, Classes, PropValue};
use crate::widget::{Filler, IntoFiller};
use crate::{NodeRef, Scope, Widget};

#[derive(Educe)]
#[educe(Debug)]
pub struct Element<T>
where
    T: AsRef<web_sys::Element> + JsCast + fmt::Debug + Clone + 'static,
{
    pub name: Cow<'static, str>,
    pub is_void: bool,
    pub classes: Classes,
    #[educe(Debug(ignore))]
    pub attrs: BTreeMap<Cow<'static, str>, Box<dyn AttrValue>>,
    #[educe(Debug(ignore))]
    pub props: BTreeMap<Cow<'static, str>, Box<dyn PropValue>>,
    #[educe(Debug(ignore))]
    pub fillers: Vec<Filler>,

    #[educe(Debug(ignore))]
    listeners: Vec<Box<dyn FnOnce(&T)>>,

    pub(crate) node: T,
    #[educe(Debug(ignore))]
    pub(crate) renderer: WebRenderer,
}

impl<T> Widget for Element<T>
where
    T: AsRef<web_sys::Element> + JsCast + Clone + fmt::Debug + 'static,
{
    fn flood(&mut self, ctx: &mut Scope) {
        let parent_node = ctx.parent_node.as_ref().unwrap_throw();
        let node = <T as AsRef<web_sys::Element>>::as_ref(&self.node);
        if crate::web::is_hydrating() && node.has_attribute("gly-hydrating") {
            node.remove_attribute("gly-hydrating").unwrap_throw();
        } else {
            match &ctx.placement {
                ViewPlacement::Head => self.renderer.insert_child(parent_node, node, InsertPosition::Head),
                ViewPlacement::Before(next_node) => self.renderer.insert_child(parent_node, node, InsertPosition::Before(next_node)),
                ViewPlacement::After(prev_node) => self.renderer.insert_child(parent_node, node, InsertPosition::After(prev_node)),
                ViewPlacement::Tail => self.renderer.insert_child(parent_node, node, InsertPosition::Tail),
                ViewPlacement::Unset => {
                    crate::warn!("node placement is unset. {:#?}", node);
                    self.renderer.insert_child(parent_node, node, InsertPosition::Tail);
                }
            }
        }

        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.attach_child(&id);
        }
    }
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn hydrate(&mut self, ctx: &mut Scope) {
        let selector = format!("{}[gly-id='{}']", self.name, ctx.view_id);
        let exist_node = if let Some(pnode) = &ctx.parent_node {
            let node = pnode.query_selector(&selector).unwrap_throw();
            if node.is_none() {
                crate::debug_warn!("[hydrating]: node not found: {} {}", selector, pnode.outer_html());
            }
            node
        } else {
            let node = crate::web::document().query_selector(&selector).unwrap_throw();
            if node.is_none() {
                crate::debug_warn!("[hydrating]: node not found2: {}", selector);
            }
            node
        };
        if let Some(exist_node) = exist_node {
            self.node = wasm_bindgen::JsCast::unchecked_into(exist_node);
            crate::debug_warn!("[hydrating]: node exist: {}", selector);
        }
    }
    fn build(&mut self, ctx: &mut Scope) {
        let node = <T as AsRef<web_sys::Element>>::as_ref(&self.node);

        ctx.render_node = Some(node.clone());
        // The element itself is the outermost DOM anchor of this view's
        // subtree, so sibling-positioning logic in `Scope::attach_child`
        // can rely on it (`last_element_child()` returns None for leaf
        // elements such as `<li>text</li>`, which breaks Each reordering).
        ctx.first_child_node = Some(node.clone());
        ctx.last_child_node = Some(node.clone());

        let fillers = std::mem::take(&mut self.fillers);
        for filler in fillers {
            filler.fill(ctx);
        }
        for (name, value) in &self.props {
            value.inject_to(&ctx.view_id, &mut node.clone(), name, true);
        }
        for (name, value) in &self.attrs {
            value.inject_to(&ctx.view_id, &mut node.clone(), name, true);
        }
        self.classes.inject_to(&ctx.view_id, &mut node.clone(), "class", true);

        for listener in std::mem::take(&mut self.listeners) {
            (listener)(&self.node);
        }
    }
    fn detach(&mut self, ctx: &mut Scope) {
        if let Some(parent_node) = ctx.parent_node.as_ref() {
            let node = <T as AsRef<web_sys::Element>>::as_ref(&self.node);
            self.renderer.remove_child(parent_node, node);
        }
        ctx.mark_descendants_dom_detached();
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.detach_child(&id);
        }
    }
    fn patch(&mut self, ctx: &mut Scope) {
        let node = <T as AsRef<web_sys::Element>>::as_ref(&self.node);
        if node.parent_element().is_none() {
            ctx.parent_node.as_ref().unwrap().append_child(node).ok();
        }
        for (name, value) in &self.props {
            value.inject_to(&ctx.view_id, &mut node.clone(), name, false);
        }
        for (name, value) in &self.attrs {
            value.inject_to(&ctx.view_id, &mut node.clone(), name, false);
        }
        self.classes.inject_to(&ctx.view_id, &mut node.clone(), "class", false);
    }
}

type DomSubtreeBuild<State> = Box<dyn FnOnce(&mut Scope) -> (web_sys::Element, State)>;
type DomSubtreeHook<State> = Box<dyn FnMut(&web_sys::Element, &mut State, &mut Scope)>;

/// A CSR-only widget that lets one Glory view own a native DOM subtree.
///
/// This is the low-level path for hot rendering loops that already know their
/// static DOM shape. The build closure may create nodes manually, clone a
/// `<template>` skeleton, bind reactive handles to `ctx.view_id()`, and return
/// the subtree root plus any app-specific state needed for patching.
pub struct DomSubtree<State>
where
    State: 'static,
{
    build: Option<DomSubtreeBuild<State>>,
    patch: DomSubtreeHook<State>,
    detach: DomSubtreeHook<State>,
    root: Option<web_sys::Element>,
    state: Option<State>,
}

impl<State> fmt::Debug for DomSubtree<State>
where
    State: 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DomSubtree")
            .field("has_root", &self.root.is_some())
            .field("has_state", &self.state.is_some())
            .finish_non_exhaustive()
    }
}

/// Construct a [`DomSubtree`] from a native DOM builder closure.
pub fn dom_subtree<State>(build: impl FnOnce(&mut Scope) -> (web_sys::Element, State) + 'static) -> DomSubtree<State>
where
    State: 'static,
{
    DomSubtree::new(build)
}

impl<State> DomSubtree<State>
where
    State: 'static,
{
    pub fn new(build: impl FnOnce(&mut Scope) -> (web_sys::Element, State) + 'static) -> Self {
        Self {
            build: Some(Box::new(build)),
            patch: Box::new(|_, _, _| {}),
            detach: Box::new(|_, _, _| {}),
            root: None,
            state: None,
        }
    }

    pub fn on_patch(mut self, patch: impl FnMut(&web_sys::Element, &mut State, &mut Scope) + 'static) -> Self {
        self.patch = Box::new(patch);
        self
    }

    pub fn on_detach(mut self, detach: impl FnMut(&web_sys::Element, &mut State, &mut Scope) + 'static) -> Self {
        self.detach = Box::new(detach);
        self
    }
}

impl<State> Widget for DomSubtree<State>
where
    State: 'static,
{
    fn build(&mut self, ctx: &mut Scope) {
        let build = self.build.take().expect("DomSubtree::build called more than once");
        let (root, state) = build(ctx);
        ctx.set_single_node_bounds(root.clone());
        self.root = Some(root);
        self.state = Some(state);
    }

    fn flood(&mut self, ctx: &mut Scope) {
        if let Some(root) = self.root.as_ref() {
            ctx.insert_node_at_placement(root);
        }

        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.attach_child(&id);
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        if let Some(root) = self.root.as_ref() {
            if root.parent_element().is_none() && ctx.parent_node().is_some() {
                ctx.insert_node_at_placement(root);
            }
            if let Some(state) = self.state.as_mut() {
                (self.patch)(root, state, ctx);
            }
        }
    }

    fn detach(&mut self, ctx: &mut Scope) {
        if let Some(root) = self.root.as_ref() {
            if let Some(state) = self.state.as_mut() {
                (self.detach)(root, state, ctx);
            }
            ctx.remove_node_from_parent(root);
        }
        ctx.mark_descendants_dom_detached();

        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.detach_child(&id);
        }
    }
}

impl<T> Deref for Element<T>
where
    T: AsRef<web_sys::Element> + JsCast + fmt::Debug + Clone,
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl<T> Element<T>
where
    T: AsRef<web_sys::Element> + JsCast + Clone + fmt::Debug,
{
    pub fn new(name: impl Into<Cow<'static, str>>, is_void: bool) -> Self {
        let name = name.into();
        let node = crate::web::document().create_element(&name).unwrap_throw();
        let node = wasm_bindgen::JsCast::unchecked_into::<T>(node);
        Self::with_node(name, is_void, node)
    }
    pub fn with_node(name: impl Into<Cow<'static, str>>, is_void: bool, node: T) -> Self {
        Self {
            name: name.into(),
            is_void,
            classes: Classes::new(),
            attrs: BTreeMap::new(),
            props: BTreeMap::new(),
            fillers: vec![],
            node,
            listeners: vec![],
            renderer: WebRenderer,
        }
    }

    pub fn node(&self) -> &T {
        &self.node
    }

    pub fn add_filler(&mut self, filler: impl IntoFiller) {
        self.fillers.push(filler.into_filler());
    }
    pub fn fill(mut self, filler: impl IntoFiller) -> Self {
        self.fillers.push(filler.into_filler());
        self
    }

    pub fn then<F>(self, func: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        func(self)
    }

    pub fn is_void(&self) -> bool {
        self.is_void
    }

    #[track_caller]
    pub fn add_id<V>(&mut self, value: V)
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert("id".into(), Box::new(value));
    }

    #[track_caller]
    pub fn id<V>(mut self, value: V) -> Self
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert("id".into(), Box::new(value));
        self
    }

    #[track_caller]
    pub fn add_class<V>(&mut self, value: V)
    where
        V: ClassPart + 'static,
    {
        self.classes.part(value);
    }
    #[track_caller]
    pub fn class<V>(mut self, value: V) -> Self
    where
        V: ClassPart + 'static,
    {
        self.classes.part(value);
        self
    }

    #[track_caller]
    pub fn toggle_class<V, C>(self, value: V, cond: C) -> Self
    where
        V: Into<String>,
        C: Into<Lotus<bool>>,
    {
        self.switch_class(value, "", cond)
    }

    #[track_caller]
    pub fn switch_class<TV, FV, C>(mut self, tv: TV, fv: FV, cond: C) -> Self
    where
        TV: Into<String>,
        FV: Into<String>,
        C: Into<Lotus<bool>>,
    {
        let tv = tv.into();
        let fv = fv.into();
        let cond = cond.into();
        let part = Bond::new(move || if *cond.get() { tv.clone() } else { fv.clone() });
        self.classes.part(part);
        self
    }
    /// Adds an property to this element.
    #[track_caller]
    pub fn add_prop<V>(&mut self, name: impl Into<Cow<'static, str>>, value: V)
    where
        V: PropValue + 'static,
    {
        self.props.insert(name.into(), Box::new(value));
    }
    /// Adds an property to this element.
    #[track_caller]
    pub fn prop<V>(mut self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: PropValue + 'static,
    {
        self.props.insert(name.into(), Box::new(value));
        self
    }

    /// Adds an attribute to this element.
    #[track_caller]
    pub fn add_attr<V>(&mut self, name: impl Into<Cow<'static, str>>, value: V)
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert(name.into(), Box::new(value));
    }

    /// Adds an attribute to this element.
    #[track_caller]
    pub fn attr<V>(mut self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert(name.into(), Box::new(value));
        self
    }

    /// Adds an event listener to this element.
    ///
    /// User-supplied handlers are wrapped in `reflow::batch` so that any
    /// `Cage::revise` calls made during a single event flush re-renders
    /// once at the end of the event tick instead of after every write.
    #[track_caller]
    pub fn add_event_listener<E, H>(&mut self, event: E, handler: H)
    where
        E: EventDescriptor + 'static,
        E::EventType: JsCast,
        H: FnMut(E::EventType) + 'static,
    {
        cfg_if! {
            if #[cfg(debug_assertions)] {
                let onspan = ::tracing::span!(
                    // parent: &self.span,
                    ::tracing::Level::TRACE,
                    "on",
                    event = %event.name()
                );
                let _onguard = onspan.enter();
            }
        }

        let mut handler = handler;
        let wrapped = move |e: E::EventType| {
            crate::reflow::batch(|| handler(e));
        };

        self.listeners.push(Box::new(move |node| {
            let event_name = event.name();
            if event_name == "mounted" {
                let mut wrapped = wrapped;
                let _ = crate::web::set_timeout(
                    move || {
                        let event = web_sys::Event::new("mounted").unwrap_throw();
                        wrapped(event.unchecked_into::<E::EventType>());
                    },
                    Duration::ZERO,
                );
                return;
            }

            if event_name == "visible" {
                let wrapped = Rc::new(std::cell::RefCell::new(wrapped));
                let callback = wasm_bindgen::closure::Closure::wrap(Box::new({
                    let wrapped = wrapped.clone();
                    move |entries: js_sys::Array, _observer: web_sys::IntersectionObserver| {
                        for entry in entries.iter() {
                            let entry = entry.unchecked_into::<web_sys::IntersectionObserverEntry>();
                            if entry.is_intersecting() {
                                let event = web_sys::Event::new("visible").unwrap_throw();
                                wrapped.borrow_mut()(event.unchecked_into::<E::EventType>());
                            }
                        }
                    }
                })
                    as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>);
                let observer = web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref()).unwrap_throw();
                observer.observe(node.as_ref());
                let _ = callback.into_js_value();
                std::mem::forget(observer);
                return;
            }

            if event.bubbles() {
                crate::web::add_event_listener(node.as_ref(), event_name, wrapped);
            } else {
                crate::web::add_event_listener_undelegated(node.as_ref(), &event_name, wrapped);
            }
        }));
    }

    /// Adds an event listener to this element.
    #[track_caller]
    pub fn on<E, H>(mut self, event: E, handler: H) -> Self
    where
        E: EventDescriptor + 'static,
        E::EventType: JsCast,
        H: FnMut(E::EventType) + 'static,
    {
        self.add_event_listener(event, handler);
        self
    }

    /// Sets the inner Text of this element from the provided
    /// string slice.
    ///
    /// Text content is assigned through `textContent`. Use
    /// [`Element::html`] only when the value is trusted markup.
    pub fn set_text<V>(&mut self, text: V)
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert("inner_text".into(), Box::new(text));
    }
    /// Sets the inner Text of this element from the provided
    /// string slice.
    ///
    /// Text content is assigned through `textContent`. Use
    /// [`Element::html`] only when the value is trusted markup.
    pub fn text<V>(mut self, text: V) -> Self
    where
        V: AttrValue + 'static,
    {
        self.set_text(text);
        self
    }

    /// Sets the inner HTML of this element from the provided
    /// string slice.
    ///
    /// # Security
    /// Be very careful when using this method. Always remember to
    /// sanitize the input to avoid a cross-site scripting (XSS)
    /// vulnerability.
    pub fn set_html<V>(&mut self, html: V)
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert("inner_html".into(), Box::new(html));
    }
    /// Sets the inner HTML of this element from the provided
    /// string slice.
    ///
    /// # Security
    /// Be very careful when using this method. Always remember to
    /// sanitize the input to avoid a cross-site scripting (XSS)
    /// vulnerability.
    pub fn html<V>(mut self, html: V) -> Self
    where
        V: AttrValue + 'static,
    {
        self.set_html(html);
        self
    }

    pub fn node_ref(self, node_ref: &NodeRef<T>) {
        node_ref.set(self.node.clone());
    }
}
