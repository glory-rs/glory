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
use std::sync::Arc;

pub mod command;
pub mod command_dom;
pub mod ssr_dom;
pub use command::{
    BoundingRect, ClipboardData, Command, CommandInsertPosition, CommandNode, CommandQueue, CommandRenderer, CurrentQueueGuard, EventData,
    KeyboardData, MediaData, NodeQuery, PointerData, QueryError, QueryResponse, QueryValue, ResizeData, ScrollOffset, SelectionData, TargetData,
    coalesce,
};

/// The renderer the shared (non-wasm) `Element` widget instantiates.
/// Every non-browser target speaks the command stream; what a batch means
/// is decided by the consumer (SSR replay, webview IPC, native, ...).
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub type BackendRenderer = CommandRenderer;

/// Relative child placement used by [`Renderer::insert_child`].
#[derive(Clone, Copy, Debug)]
pub enum InsertPosition<'a, N> {
    Head,
    Tail,
    Before(&'a N),
    After(&'a N),
}

/// Renderer-level rich attribute/property payload.
#[derive(Clone)]
pub enum AttributeValue {
    Text(String),
    Float(f64),
    Int(i64),
    Bool(bool),
    Any(Arc<dyn Any + Send + Sync>),
    None,
}

impl AttributeValue {
    pub fn as_text(&self) -> Option<Cow<'_, str>> {
        match self {
            AttributeValue::Text(value) => Some(Cow::Borrowed(value)),
            AttributeValue::Float(value) => Some(Cow::Owned(value.to_string())),
            AttributeValue::Int(value) => Some(Cow::Owned(value.to_string())),
            AttributeValue::Bool(value) => Some(Cow::Borrowed(if *value { "true" } else { "false" })),
            AttributeValue::Any(_) | AttributeValue::None => None,
        }
    }
}

impl fmt::Debug for AttributeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributeValue::Text(value) => f.debug_tuple("Text").field(value).finish(),
            AttributeValue::Float(value) => f.debug_tuple("Float").field(value).finish(),
            AttributeValue::Int(value) => f.debug_tuple("Int").field(value).finish(),
            AttributeValue::Bool(value) => f.debug_tuple("Bool").field(value).finish(),
            AttributeValue::Any(_) => f.write_str("Any(..)"),
            AttributeValue::None => f.write_str("None"),
        }
    }
}

impl From<String> for AttributeValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for AttributeValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<f64> for AttributeValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<i64> for AttributeValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<bool> for AttributeValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
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
    fn set_attribute_value(&self, node: &Self::Node, name: Cow<'static, str>, value: AttributeValue) {
        match value.as_text() {
            Some(value) => self.set_attribute(node, name, Cow::Owned(value.into_owned())),
            None => self.remove_attribute(node, &name),
        }
    }

    fn set_property(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>);
    fn remove_property(&self, node: &Self::Node, name: &str);
    fn set_property_value(&self, node: &Self::Node, name: Cow<'static, str>, value: AttributeValue) {
        match value.as_text() {
            Some(value) => self.set_property(node, name, Cow::Owned(value.into_owned())),
            None => self.remove_property(node, &name),
        }
    }

    fn add_class(&self, node: &Self::Node, value: Cow<'static, str>);
    fn remove_class(&self, node: &Self::Node, value: &str);

    fn set_text(&self, node: &Self::Node, value: Cow<'static, str>);
    fn set_html(&self, node: &Self::Node, value: Cow<'static, str>);

    fn insert_child(&self, parent: &Self::Node, child: &Self::Node, position: InsertPosition<'_, Self::Node>);
    fn remove_child(&self, parent: &Self::Node, child: &Self::Node);
    fn node_identity_eq(&self, left: &Self::Node, right: &Self::Node) -> bool;

    fn attach_event(&self, node: &Self::Node, name: Cow<'static, str>, bubbles: bool, handler: Box<dyn FnMut(Self::Event)>);
}

#[derive(Debug)]
pub struct RenderedElement<R: Renderer> {
    pub name: Cow<'static, str>,
    pub is_void: bool,
    pub node: R::Node,
    renderer: R,
}

impl<R: Renderer> RenderedElement<R> {
    pub fn new(renderer: R, name: impl Into<Cow<'static, str>>, is_void: bool) -> Self {
        let name = name.into();
        let node = renderer.create_element(name.clone(), is_void);
        Self {
            name,
            is_void,
            node,
            renderer,
        }
    }

    pub fn renderer(&self) -> &R {
        &self.renderer
    }
}

/// Historical aliases. `MockRenderer` was the original test-only command
/// recorder; it is now the canonical [`CommandRenderer`] from
/// [`command`](self::command), which buffers identically but is also the
/// production renderer for command-stream backends.
pub type MockRenderer = CommandRenderer;
pub type MockCommand = Command;
pub type MockInsertPosition = CommandInsertPosition;
pub type MockNode = CommandNode;

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

    /// Former `ssr_renderer_mutates_node_tree`: the same operation sequence
    /// through the command renderer + SSR replay must produce the exact
    /// legacy HTML.
    #[test]
    fn command_stream_replay_matches_legacy_ssr_html() {
        let renderer = CommandRenderer::new();
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

        let doc = ssr_dom::SsrDocument::replay(&renderer.commands());
        assert_eq!(
            doc.inner_html(parent.id()),
            "<li></li><li></li><li value=\"1\" data-id=\"a\" class=\" selected\">A</li>"
        );

        renderer.remove_class(&first, "selected");
        renderer.remove_property(&first, "value");
        renderer.remove_attribute(&first, "data-id");
        renderer.remove_child(&parent, &third);

        let doc = ssr_dom::SsrDocument::replay(&renderer.commands());
        assert_eq!(doc.inner_html(parent.id()), "<li></li><li>A</li>");
    }

    #[test]
    fn attribute_value_preserves_rich_types() {
        assert_eq!(AttributeValue::from("hello").as_text().unwrap(), "hello");
        assert_eq!(AttributeValue::from(42_i64).as_text().unwrap(), "42");
        assert_eq!(AttributeValue::from(true).as_text().unwrap(), "true");
        assert!(AttributeValue::None.as_text().is_none());
    }

    #[test]
    fn mock_renderer_records_command_sequence() {
        let renderer = MockRenderer::default();
        let parent = RenderedElement::new(renderer.clone(), "ul", false);
        let child = RenderedElement::new(renderer.clone(), "li", false);

        renderer.set_attribute_value(&child.node, "data-count".into(), 3_i64.into());
        renderer.insert_child(&parent.node, &child.node, InsertPosition::Tail);

        assert_eq!(
            renderer.commands(),
            vec![
                MockCommand::Create {
                    id: 1,
                    name: "ul".to_string(),
                    is_void: false
                },
                MockCommand::Create {
                    id: 2,
                    name: "li".to_string(),
                    is_void: false
                },
                MockCommand::SetAttribute {
                    id: 2,
                    name: "data-count".to_string(),
                    value: "3".to_string()
                },
                MockCommand::Insert {
                    parent: 1,
                    child: 2,
                    position: MockInsertPosition::Tail
                }
            ]
        );
    }
}
