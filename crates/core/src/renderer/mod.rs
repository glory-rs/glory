//! Renderer abstraction for non-DOM backends.
//!
//! This module is the migration target for Glory's current split between
//! browser DOM (`web_sys::Element`) and SSR's in-memory [`Node`]. It is
//! intentionally command-shaped rather than VDOM-shaped: widgets should
//! eventually emit "create this element, set this attribute, insert it
//! before that sibling" operations through a [`Renderer`] implementation.

use std::any::Any;
use std::borrow::Cow;
use std::fmt;

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
use crate::node::Node;

/// Relative child placement used by [`Renderer::insert_child`].
#[derive(Clone, Copy, Debug)]
pub enum InsertPosition<'a, N> {
    Head,
    Tail,
    Before(&'a N),
    After(&'a N),
}

/// Type-erased event payload surfaced by non-web renderers.
pub trait EventPayload: fmt::Debug + 'static {
    fn as_any(&self) -> &dyn Any;
    fn name(&self) -> Cow<'_, str>;
}

/// Command surface every platform renderer must provide.
pub trait Renderer: Clone + fmt::Debug + 'static {
    type Node: Clone + fmt::Debug + 'static;
    type Event: EventPayload;

    fn create_element(&self, name: Cow<'static, str>, is_void: bool) -> Self::Node;

    fn set_attribute(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>);
    fn remove_attribute(&self, node: &Self::Node, name: &str);

    fn set_property(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>);
    fn remove_property(&self, node: &Self::Node, name: &str);

    fn add_class(&self, node: &Self::Node, value: Cow<'static, str>);
    fn remove_class(&self, node: &Self::Node, value: &str);

    fn set_text(&self, node: &Self::Node, value: Cow<'static, str>);
    fn set_html(&self, node: &Self::Node, value: Cow<'static, str>);

    fn insert_child(&self, parent: &Self::Node, child: &Self::Node, position: InsertPosition<'_, Self::Node>);
    fn remove_child(&self, parent: &Self::Node, child: &Self::Node);
    fn node_identity_eq(&self, left: &Self::Node, right: &Self::Node) -> bool;

    fn attach_event(&self, node: &Self::Node, name: Cow<'static, str>, bubbles: bool, handler: Box<dyn FnMut(Self::Event)>);
}

/// SSR event placeholder. Server rendering records no live event
/// listeners, but the renderer trait needs a concrete payload type.
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
#[derive(Clone, Debug)]
pub struct SsrEventPayload {
    name: Cow<'static, str>,
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
impl SsrEventPayload {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self { name: name.into() }
    }
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
impl EventPayload for SsrEventPayload {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.name)
    }
}

/// Renderer backed by Glory's in-memory SSR node tree.
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
#[derive(Clone, Copy, Debug, Default)]
pub struct SsrRenderer;

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
impl Renderer for SsrRenderer {
    type Event = SsrEventPayload;
    type Node = Node;

    fn create_element(&self, name: Cow<'static, str>, is_void: bool) -> Self::Node {
        Node::new(name, is_void)
    }

    fn set_attribute(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        node.set_attribute(name, value);
    }

    fn remove_attribute(&self, node: &Self::Node, name: &str) {
        node.remove_attribute(name);
    }

    fn set_property(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        node.set_property(name, Some(value));
    }

    fn remove_property(&self, node: &Self::Node, name: &str) {
        node.remove_property(name);
    }

    fn add_class(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.add_class(value);
    }

    fn remove_class(&self, node: &Self::Node, value: &str) {
        node.remove_class(value);
    }

    fn set_text(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.set_attribute("inner_text", value);
    }

    fn set_html(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.set_attribute("inner_html", value);
    }

    fn insert_child(&self, parent: &Self::Node, child: &Self::Node, position: InsertPosition<'_, Self::Node>) {
        match position {
            InsertPosition::Head => parent.prepend_with_node(child),
            InsertPosition::Tail => parent.append_with_node(child),
            InsertPosition::Before(anchor) => parent.insert_before(anchor, child),
            InsertPosition::After(anchor) => parent.insert_after(anchor, child),
        }
    }

    fn remove_child(&self, parent: &Self::Node, child: &Self::Node) {
        parent.remove_child(child);
    }

    fn node_identity_eq(&self, left: &Self::Node, right: &Self::Node) -> bool {
        left.ptr_eq(right)
    }

    fn attach_event(&self, _node: &Self::Node, _name: Cow<'static, str>, _bubbles: bool, _handler: Box<dyn FnMut(Self::Event)>) {}
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
#[derive(Clone, Debug)]
pub struct WebEventPayload {
    event: web_sys::Event,
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
impl WebEventPayload {
    pub fn new(event: web_sys::Event) -> Self {
        Self { event }
    }

    pub fn event(&self) -> &web_sys::Event {
        &self.event
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
impl EventPayload for WebEventPayload {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> Cow<'_, str> {
        Cow::Owned(self.event.type_())
    }
}

/// Renderer backed by browser DOM APIs.
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
#[derive(Clone, Copy, Debug, Default)]
pub struct WebRenderer;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
impl Renderer for WebRenderer {
    type Event = WebEventPayload;
    type Node = web_sys::Element;

    fn create_element(&self, name: Cow<'static, str>, _is_void: bool) -> Self::Node {
        use wasm_bindgen::UnwrapThrowExt;

        crate::web::document().create_element(&name).unwrap_throw()
    }

    fn set_attribute(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        use wasm_bindgen::UnwrapThrowExt;

        node.set_attribute(&name, &value).unwrap_throw();
    }

    fn remove_attribute(&self, node: &Self::Node, name: &str) {
        use wasm_bindgen::UnwrapThrowExt;

        node.remove_attribute(name).unwrap_throw();
    }

    fn set_property(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        crate::web::helpers::set_property(node, &name, &Some(wasm_bindgen::JsValue::from_str(&value)));
    }

    fn remove_property(&self, node: &Self::Node, name: &str) {
        crate::web::helpers::set_property(node, name, &None);
    }

    fn add_class(&self, node: &Self::Node, value: Cow<'static, str>) {
        use wasm_bindgen::UnwrapThrowExt;

        node.class_list().add_1(&value).unwrap_throw();
    }

    fn remove_class(&self, node: &Self::Node, value: &str) {
        use wasm_bindgen::UnwrapThrowExt;

        node.class_list().remove_1(value).unwrap_throw();
    }

    fn set_text(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.set_text_content(Some(&value));
    }

    fn set_html(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.set_inner_html(&value);
    }

    fn insert_child(&self, parent: &Self::Node, child: &Self::Node, position: InsertPosition<'_, Self::Node>) {
        use wasm_bindgen::UnwrapThrowExt;

        match position {
            InsertPosition::Head => parent.prepend_with_node_1(child).unwrap_throw(),
            InsertPosition::Tail => parent.append_with_node_1(child).unwrap_throw(),
            InsertPosition::Before(anchor) => anchor.before_with_node_1(child).unwrap_throw(),
            InsertPosition::After(anchor) => anchor.after_with_node_1(child).unwrap_throw(),
        }
    }

    fn remove_child(&self, parent: &Self::Node, child: &Self::Node) {
        let _ = parent.remove_child(child);
    }

    fn node_identity_eq(&self, left: &Self::Node, right: &Self::Node) -> bool {
        left.is_same_node(Some(right))
    }

    fn attach_event(&self, node: &Self::Node, name: Cow<'static, str>, bubbles: bool, mut handler: Box<dyn FnMut(Self::Event)>) {
        let wrapped = move |event: web_sys::Event| {
            handler(WebEventPayload::new(event));
        };

        if bubbles {
            crate::web::helpers::add_event_listener(node, name, wrapped);
        } else {
            crate::web::helpers::add_event_listener_undelegated(node, &name, wrapped);
        }
    }
}

#[cfg(test)]
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
mod tests {
    use super::*;

    #[test]
    fn ssr_renderer_mutates_node_tree() {
        let renderer = SsrRenderer;
        let parent = renderer.create_element("ul".into(), false);
        let first = renderer.create_element("li".into(), false);
        let second = renderer.create_element("li".into(), false);
        let third = renderer.create_element("li".into(), false);

        renderer.set_attribute(&first, "data-id".into(), "a".into());
        renderer.set_property(&first, "value".into(), "1".into());
        renderer.add_class(&first, "selected".into());
        renderer.set_text(&first, "A".into());

        renderer.insert_child(&parent, &first, InsertPosition::Tail);
        renderer.insert_child(&parent, &second, InsertPosition::Head);
        renderer.insert_child(&parent, &third, InsertPosition::After(&second));

        assert!(renderer.node_identity_eq(&first, &first.clone()));
        assert!(!renderer.node_identity_eq(&first, &second));
        assert_eq!(
            parent.inner_html(),
            "<li></li><li></li><li value=\"1\" data-id=\"a\" class=\" selected\">A</li>"
        );

        renderer.remove_class(&first, "selected");
        renderer.remove_property(&first, "value");
        renderer.remove_attribute(&first, "data-id");
        renderer.remove_child(&parent, &third);

        assert_eq!(parent.inner_html(), "<li></li><li>A</li>");
    }
}
