use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Debug;

use educe::Educe;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::UnwrapThrowExt;

use crate::node::{Node, NodeRef};
use crate::reflow::{Bond, Lotus};
use crate::renderer::{BackendRenderer, InsertPosition, Renderer};
use crate::view::{ViewId, ViewPlacement};
use crate::web::events::EventDescriptor;
use crate::web::{AttrValue, ClassPart, Classes, PropValue};
use crate::widget::{Filler, IntoFiller};
use crate::{Scope, Widget};

#[derive(Educe)]
#[educe(Debug)]
pub struct Element {
    pub name: Cow<'static, str>,
    pub is_void: bool,
    pub classes: Classes,
    #[educe(Debug(ignore))]
    pub attrs: BTreeMap<Cow<'static, str>, Box<dyn AttrValue>>,
    #[educe(Debug(ignore))]
    pub props: BTreeMap<Cow<'static, str>, Box<dyn PropValue>>,
    #[educe(Debug(ignore))]
    #[allow(clippy::type_complexity)]
    pub fillers: Vec<Filler>,

    pub(crate) node: Node,
    pub(crate) renderer: BackendRenderer,
    /// Event names attached via the command backend; used by `Drop` to
    /// release the queue's handler registrations when the widget (not just
    /// its view attachment — cached views re-attach) goes away.
    #[cfg(feature = "backend-command")]
    #[educe(Debug(ignore))]
    pub(crate) listener_names: std::cell::RefCell<Vec<Cow<'static, str>>>,
}

#[cfg(feature = "backend-command")]
impl Drop for Element {
    fn drop(&mut self) {
        for name in self.listener_names.borrow().iter() {
            self.node.detach_event(name);
        }
    }
}

impl Widget for Element {
    fn build(&mut self, ctx: &mut Scope) {
        ctx.render_node = Some(self.node.clone());
        // For an Element view, the entire subtree is anchored on this
        // element's node. Expose it as both the first- and last-child
        // anchor so sibling-positioning logic in `Scope::attach_child`
        // can find a real DOM neighbour to insert relative to.
        ctx.first_child_node = Some(self.node.clone());
        ctx.last_child_node = Some(self.node.clone());

        let fillers = std::mem::take(&mut self.fillers);
        for filler in fillers {
            filler.fill(ctx);
        }
        for (name, value) in &self.props {
            value.inject_to(&ctx.view_id, &mut self.node, name, true);
        }

        self.attrs.insert("gly-id".into(), Box::new(ctx.view_id.clone()));
        for (name, value) in &self.attrs {
            value.inject_to(&ctx.view_id, &mut self.node, name, true);
        }
        self.classes.inject_to(&ctx.view_id, &mut self.node, "class", true);
    }

    fn flood(&mut self, ctx: &mut Scope) {
        let parent_node = ctx.parent_node.as_ref().unwrap();
        match &ctx.placement {
            ViewPlacement::Head => self.renderer.insert_child(parent_node, &self.node, InsertPosition::Head),
            ViewPlacement::Before(next_node) => self.renderer.insert_child(parent_node, &self.node, InsertPosition::Before(next_node)),
            ViewPlacement::After(prev_node) => self.renderer.insert_child(parent_node, &self.node, InsertPosition::After(prev_node)),
            ViewPlacement::Tail => self.renderer.insert_child(parent_node, &self.node, InsertPosition::Tail),
            ViewPlacement::Unset => {
                crate::warn!("node placement is unset. {:#?}", &self.node);
                self.renderer.insert_child(parent_node, &self.node, InsertPosition::Tail);
            }
        }

        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.attach_child(&id);
        }
    }
    fn detach(&mut self, ctx: &mut Scope) {
        if let Some(parent_node) = ctx.parent_node.as_ref() {
            self.renderer.remove_child(parent_node, &self.node);
        }
        ctx.mark_descendants_dom_detached();
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.detach_child(&id);
        }
    }
    fn patch(&mut self, ctx: &mut Scope) {
        for (name, value) in &self.props {
            value.inject_to(&ctx.view_id, &mut self.node, name, false);
        }
        for (name, value) in &self.attrs {
            value.inject_to(&ctx.view_id, &mut self.node, name, false);
        }
        self.classes.inject_to(&ctx.view_id, &mut self.node, "class", false);
    }
}

impl Element {
    pub fn new(name: impl Into<Cow<'static, str>>, is_void: bool) -> Self {
        let name = name.into();
        let renderer = BackendRenderer::default();
        Self {
            name: name.clone(),
            is_void,
            classes: Default::default(),
            attrs: Default::default(),
            props: Default::default(),
            fillers: vec![],
            node: renderer.create_element(name, is_void),
            renderer,
            #[cfg(feature = "backend-command")]
            listener_names: Default::default(),
        }
    }

    pub fn node(&self) -> &Node {
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
    /// Server rendering records no live listeners; this is a no-op.
    #[cfg(not(feature = "backend-command"))]
    #[track_caller]
    pub fn add_event_listener<E: EventDescriptor>(
        &self,
        _event: E,
        #[allow(unused_mut)] // used for tracing in debug
        mut _event_handler: impl FnMut(E::EventType) + 'static,
    ) {
    }

    /// Adds an event listener to this element.
    ///
    /// Under the command-stream backend the handler is registered in the
    /// queue's event registry and an `AttachEvent` command is emitted; the
    /// host delivers deserialized [`EventData`](crate::renderer::EventData)
    /// back through `CommandRenderer::dispatch_event`.
    #[cfg(feature = "backend-command")]
    #[track_caller]
    pub fn add_event_listener<E>(&self, event: E, event_handler: impl FnMut(E::EventType) + 'static)
    where
        E: EventDescriptor<EventType = crate::renderer::EventData>,
    {
        self.listener_names.borrow_mut().push(event.name());
        self.renderer
            .attach_event(&self.node, event.name(), event.bubbles(), Box::new(event_handler));
    }

    /// Adds an event listener to this element.
    #[cfg(not(feature = "backend-command"))]
    #[track_caller]
    pub fn on<E: EventDescriptor>(
        self,
        _event: E,
        #[allow(unused_mut)] // used for tracing in debug
        mut _event_handler: impl FnMut(E::EventType) + 'static,
    ) -> Self {
        self
    }

    /// Adds an event listener to this element.
    #[cfg(feature = "backend-command")]
    #[track_caller]
    pub fn on<E>(self, event: E, event_handler: impl FnMut(E::EventType) + 'static) -> Self
    where
        E: EventDescriptor<EventType = crate::renderer::EventData>,
    {
        self.add_event_listener(event, event_handler);
        self
    }

    /// Sets the inner Text of this element from the provided
    /// string slice.
    ///
    /// Text content is escaped during SSR. Use [`Element::html`] only
    /// when the value is trusted markup.
    pub fn set_text<V>(&mut self, text: V)
    where
        V: AttrValue + 'static,
    {
        self.attrs.insert("inner_text".into(), Box::new(text));
    }
    /// Sets the inner Text of this element from the provided
    /// string slice.
    ///
    /// Text content is escaped during SSR. Use [`Element::html`] only
    /// when the value is trusted markup.
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

    pub fn node_ref<T>(self, _node_ref: &NodeRef<T>) -> Self
    where
        T: Debug,
    {
        self
    }
}
