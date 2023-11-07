use std::borrow::Cow;
use std::collections::BTreeMap;

use educe::Educe;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsValue, UnwrapThrowExt};

use crate::reflow::{Bond, Lotus};
use crate::web::{AttrValue, Classes, PropValue};
use crate::{Node, Scope, Widget};

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub const DEPOT_HTML_META_KEY: &str = "glory::web::html_meta";
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub const DEPOT_BODY_META_KEY: &str = "glory::web::body_meta";

#[derive(Educe)]
#[educe(Debug)]
pub struct NodeMeta {
    pub classes: Classes,
    #[educe(Debug(ignore))]
    pub attrs: BTreeMap<Cow<'static, str>, Box<dyn AttrValue>>,
    #[educe(Debug(ignore))]
    pub props: BTreeMap<Cow<'static, str>, Box<dyn PropValue>>,

    node: Node,
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    truck_key: &'static str,
}

impl Widget for NodeMeta {
    fn build(&mut self, _ctx: &mut Scope) {}

    fn attach(&mut self, ctx: &mut Scope) {
        for (name, value) in &self.props {
            value.inject_to(&ctx.view_id, &mut self.node, name, true);
        }
        for (name, value) in &self.attrs {
            value.inject_to(&ctx.view_id, &mut self.node, name, true);
        }
        self.classes.inject_to(&ctx.view_id, &mut self.node, "class", true);

        #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
        ctx.truck_mut().insert(self.truck_key, self.node.clone());
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn detach(&mut self, _ctx: &mut Scope) {
        for name in self.props.keys() {
            js_sys::Reflect::delete_property(&self.node, &JsValue::from_str(name)).unwrap_throw();
        }
        for name in self.attrs.keys() {
            self.node.remove_attribute(name).unwrap_throw();
        }
        self.node.class_list().remove(&self.classes.to_array()).unwrap_throw();
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
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

impl NodeMeta {
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub(crate) fn new(node: Node) -> Self {
        Self {
            classes: Default::default(),
            attrs: Default::default(),
            props: Default::default(),
            node,
        }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    pub(crate) fn new(node: Node, truck_key: &'static str) -> Self {
        Self {
            classes: Default::default(),
            attrs: Default::default(),
            props: Default::default(),
            node,
            truck_key,
        }
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
    pub fn class<V>(mut self, value: V) -> Self
    where
        V: Into<Lotus<String>>,
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
    pub fn prop<V>(mut self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: PropValue + 'static,
    {
        self.props.insert(name.into(), Box::new(value));
        self
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
}

pub fn html_meta() -> NodeMeta {
    cfg_if! {
        if #[cfg(all(target_arch = "wasm32", feature = "web-csr"))] {
            NodeMeta::new(crate::web::document().document_element().unwrap_throw())
        } else {
            NodeMeta::new(Node::new("html", false), DEPOT_HTML_META_KEY)
        }
    }
}

pub fn body_meta() -> NodeMeta {
    cfg_if! {
        if #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]{
            NodeMeta::new(crate::web::document().body().unwrap_throw().into())
        } else {
            NodeMeta::new(Node::new("body", false), DEPOT_BODY_META_KEY)
        }
    }
}
