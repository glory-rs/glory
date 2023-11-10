use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;

use educe::Educe;
// #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsCast, UnwrapThrowExt};

use crate::reflow::{Bond, Lotus};
use crate::view::{ViewId, ViewPosition};
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

    pub(crate) node: T,
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
            match &ctx.position {
                ViewPosition::Head => parent_node.prepend_with_node_1(node).unwrap_throw(),
                ViewPosition::Prev(prev_node) => prev_node.after_with_node_1(node).unwrap_throw(),
                ViewPosition::Next(next_node) => next_node.before_with_node_1(node).unwrap_throw(),
                ViewPosition::Tail => parent_node.append_with_node_1(node).unwrap_throw(),
                ViewPosition::Unset => {
                    crate::warn!("node position is unset. {:#?}", node);
                    parent_node.append_with_node_1(node).unwrap_throw();
                }
            }
        }

        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.attach_child(&id);
        }
    }
    fn build(&mut self, ctx: &mut Scope) {
        if crate::web::is_hydrating() {
            let selector = format!("{}[gly-id='{}']", self.name, ctx.view_id);
            let exist_node = if let Some(pnode) = &ctx.parent_node {
                let node = pnode.query_selector(&selector).unwrap_throw();
                if node.is_none() {
                    crate::warn!("[hydrating]: node not found: {} {}", selector, pnode.outer_html());
                }
                node
            } else {
                let node = crate::web::document().query_selector(&selector).unwrap_throw();
                if node.is_none() {
                    crate::warn!("[hydrating]: node not found2: {}", selector);
                }
                node
            };
            if let Some(exist_node) = exist_node {
                self.node = wasm_bindgen::JsCast::unchecked_into(exist_node);
                crate::info!("[hydrating]: node exist: {}", selector);
            }
        }

        let node = <T as AsRef<web_sys::Element>>::as_ref(&self.node);

        ctx.graff_node = Some(node.clone());
        ctx.first_child_node = node.first_element_child();
        ctx.last_child_node = node.last_element_child();

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
    }
    fn detach(&mut self, ctx: &mut Scope) {
        if let Some(parent_node) = ctx.parent_node.as_ref() {
            let node = <T as AsRef<web_sys::Element>>::as_ref(&self.node);
            parent_node.remove_child(node).ok();
        }
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
        self.classes.part(Bond::new(move || if *cond.get() { tv.clone() } else { fv.clone() }));
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
    #[track_caller]
    pub fn add_event_listener<E: EventDescriptor>(
        &self,
        event: E,
        #[allow(unused_mut)] // used for tracing in debug
        mut event_handler: impl FnMut(E::EventType) + 'static,
    ) {
        #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
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
            let event_name = event.name();

            if event.bubbles() {
                crate::web::add_event_listener(self.node.as_ref(), event_name, event_handler);
            } else {
                crate::web::add_event_listener_undelegated(self.node.as_ref(), &event_name, event_handler);
            }
        }

        #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
        {
            _ = event;
            _ = event_handler;
        }
    }

    /// Adds an event listener to this element.
    #[track_caller]
    pub fn on<E: EventDescriptor>(
        self,
        event: E,
        #[allow(unused_mut)] // used for tracing in debug
        mut event_handler: impl FnMut(E::EventType) + 'static,
    ) -> Self {
        self.add_event_listener(event, event_handler);
        self
    }

    /// Sets the inner Text of this element from the provided
    /// string slice.
    ///
    /// # Security
    /// Be very careful when using this method. Always remember to
    /// sanitize the input to avoid a cross-site scripting (XSS)
    /// vulnerability.
    pub fn set_text<V>(&mut self, text: V)
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert("inner_text".into(), Box::new(text));
    }
    /// Sets the inner Text of this element from the provided
    /// string slice.
    ///
    /// # Security
    /// Be very careful when using this method. Always remember to
    /// sanitize the input to avoid a cross-site scripting (XSS)
    /// vulnerability.
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
