//! Server-side HTML consumer of the command stream.
//!
//! [`SsrDocument`] replays a recorded [`Command`] batch into an in-memory
//! [`SsrNode`] tree and renders HTML strings from it. `SsrNode` is the
//! former `node::ssr::Node` moved here verbatim, so escaping, attribute
//! ordering and the `inner_text` / `inner_html` pseudo-attribute rules are
//! byte-identical to the legacy direct-mutation SSR renderer (guarded by
//! the widget snapshot test suite).
//!
//! # Parity note
//!
//! Replay reproduces the *legacy SSR semantics*, not browser semantics:
//! `SetText` records the `inner_text` pseudo attribute and does **not**
//! clear children (the legacy renderer behaved the same way, and a node
//! with children renders its children, ignoring `inner_text`). The
//! browser-faithful reference interpreter is
//! [`command_dom`](super::command_dom).

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::ops::Deref;
use std::rc::Rc;

use super::command::{Command, CommandInsertPosition};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct SsrNode {
    name: Rc<RefCell<Cow<'static, str>>>,
    is_void: Rc<RefCell<bool>>,
    classes: Rc<RefCell<BTreeSet<Cow<'static, str>>>>,
    attributes: Rc<RefCell<BTreeMap<Cow<'static, str>, Cow<'static, str>>>>,
    properties: Rc<RefCell<BTreeMap<Cow<'static, str>, Option<Cow<'static, str>>>>>,
    children: Rc<RefCell<Vec<SsrNode>>>,
}

impl SsrNode {
    pub fn new(name: impl Into<Cow<'static, str>>, is_void: bool) -> Self {
        Self {
            name: Rc::new(RefCell::new(name.into())),
            is_void: Rc::new(RefCell::new(is_void)),
            classes: Default::default(),
            attributes: Default::default(),
            properties: Default::default(),
            children: Default::default(),
        }
    }

    /// Identity comparison. Two SsrNode values are considered the same DOM
    /// instance iff they share the underlying `Rc` allocation.
    pub fn ptr_eq(&self, other: &SsrNode) -> bool {
        Rc::ptr_eq(&self.children, &other.children)
    }

    pub fn remove_child(&self, node: &SsrNode) {
        self.children.borrow_mut().retain(|item| !item.ptr_eq(node));
    }

    pub fn add_class(&self, value: impl Into<Cow<'static, str>>) {
        self.classes.borrow_mut().insert(value.into());
    }
    pub fn remove_class(&self, key: &str) {
        self.classes.borrow_mut().remove(key);
    }

    pub fn set_attribute(&self, key: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) {
        self.attributes.borrow_mut().insert(key.into(), value.into());
    }
    /// Reads back a previously-set attribute value. Used by streaming SSR to
    /// recognise Suspense wrapper nodes (tagged `data-glory-suspense`).
    pub fn get_attribute(&self, key: &str) -> Option<String> {
        self.attributes.borrow().get(key).map(|value| value.to_string())
    }
    pub fn remove_attribute(&self, key: &str) {
        self.attributes.borrow_mut().remove(key);
    }
    pub fn set_property(&self, key: impl Into<Cow<'static, str>>, value: impl Into<Option<Cow<'static, str>>>) {
        self.properties.borrow_mut().insert(key.into(), value.into());
    }
    pub fn remove_property(&self, key: &str) {
        self.properties.borrow_mut().remove(key);
    }

    /// Move `node` to be the first child of `self`.
    pub fn prepend_with_node(&self, node: &SsrNode) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(node));
        children.insert(0, node.clone());
    }
    /// Move `node` to be the last child of `self`.
    pub fn append_with_node(&self, node: &SsrNode) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(node));
        children.push(node.clone());
    }

    /// Move `new_node` immediately AFTER `anchor` among `self`'s children;
    /// appends when `anchor` is not a child.
    pub fn insert_after(&self, anchor: &SsrNode, new_node: &SsrNode) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(new_node));
        let pos = children.iter().position(|n| n.ptr_eq(anchor));
        match pos {
            Some(idx) => children.insert(idx + 1, new_node.clone()),
            None => children.push(new_node.clone()),
        }
    }
    /// Move `new_node` immediately BEFORE `anchor` among `self`'s children;
    /// appends when `anchor` is not a child.
    pub fn insert_before(&self, anchor: &SsrNode, new_node: &SsrNode) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(new_node));
        let pos = children.iter().position(|n| n.ptr_eq(anchor));
        match pos {
            Some(idx) => children.insert(idx, new_node.clone()),
            None => children.push(new_node.clone()),
        }
    }

    pub fn html_tag(&self) -> (String, String) {
        let name = self.name.borrow();
        let name = if is_valid_html_name(&name) { name.as_ref() } else { "div" };
        let class = if !self.classes.borrow().is_empty() {
            format!(
                " class=\"{}\"",
                self.classes.borrow().deref().iter().fold("".to_string(), |mut acc, k| {
                    acc.push(' ');
                    acc.push_str(&escape_html_attr(k));
                    acc
                })
            )
        } else {
            "".to_string()
        };

        let properties = if !self.properties.borrow().is_empty() {
            self.properties.borrow().iter().fold("".to_string(), |mut acc, (k, v)| {
                if k != "text" && is_valid_html_name(k) {
                    if let Some(v) = v {
                        acc.push_str(&format!(" {k}=\"{}\"", escape_html_attr(v)));
                    } else {
                        acc.push_str(&format!(" {k}"));
                    }
                }
                acc
            })
        } else {
            "".to_string()
        };

        let attributes = if !self.attributes.borrow().is_empty() {
            let mut value = "".to_string();
            for (k, v) in self.attributes.borrow().iter() {
                if k != "inner_html" && k != "inner_text" && is_valid_html_name(k) {
                    write!(&mut value, " {k}=\"{}\"", escape_html_attr(v)).unwrap();
                }
            }
            value
        } else {
            "".to_string()
        };

        if *self.is_void.borrow() {
            (format!("<{name}{properties}{attributes}{class}>"), "".into())
        } else {
            (format!("<{name}{properties}{attributes}{class}>"), format!("</{name}>"))
        }
    }

    pub fn outer_html(&self) -> String {
        if *self.is_void.borrow() {
            self.html_tag().0
        } else {
            let (tag_open, tag_close) = self.html_tag();
            format!("{tag_open}{}{tag_close}", self.inner_html())
        }
    }

    pub fn outer_html_chunks(&self, chunks: &mut Vec<String>) {
        if *self.is_void.borrow() {
            chunks.push(self.html_tag().0);
            return;
        }

        let children = self.children.borrow();
        if children.is_empty() {
            let (tag_open, tag_close) = self.html_tag();
            chunks.push(format!("{tag_open}{}{tag_close}", self.leaf_inner_html()));
            return;
        }

        let (tag_open, tag_close) = self.html_tag();
        chunks.push(tag_open);
        for child in children.iter() {
            child.outer_html_chunks(chunks);
        }
        chunks.push(tag_close);
    }

    /// Like [`outer_html_chunks`](Self::outer_html_chunks) but lets `replace`
    /// substitute a node's whole serialization. When `replace` returns
    /// `Some(html)` for a node, that string is emitted verbatim and the node's
    /// subtree is *not* walked; returning `None` falls back to normal
    /// serialization. Streaming SSR uses this to swap a pending Suspense
    /// wrapper for a `<template data-glory-placeholder>` marker, or to unwrap a
    /// resolved wrapper into its children.
    pub fn outer_html_chunks_with(&self, chunks: &mut Vec<String>, replace: &dyn Fn(&SsrNode) -> Option<String>) {
        if let Some(replacement) = replace(self) {
            chunks.push(replacement);
            return;
        }

        if *self.is_void.borrow() {
            chunks.push(self.html_tag().0);
            return;
        }

        let children = self.children.borrow();
        if children.is_empty() {
            let (tag_open, tag_close) = self.html_tag();
            chunks.push(format!("{tag_open}{}{tag_close}", self.leaf_inner_html()));
            return;
        }

        let (tag_open, tag_close) = self.html_tag();
        chunks.push(tag_open);
        for child in children.iter() {
            child.outer_html_chunks_with(chunks, replace);
        }
        chunks.push(tag_close);
    }

    /// Children serialization (no enclosing tag), honouring `replace` exactly
    /// as [`outer_html_chunks_with`](Self::outer_html_chunks_with) does.
    pub fn inner_html_with(&self, replace: &dyn Fn(&SsrNode) -> Option<String>) -> String {
        let children = self.children.borrow();
        if children.is_empty() {
            return self.leaf_inner_html();
        }
        let mut chunks = Vec::new();
        for child in children.iter() {
            child.outer_html_chunks_with(&mut chunks, replace);
        }
        chunks.join("")
    }

    pub fn inner_html(&self) -> String {
        let mut html = "".to_string();
        if !self.children.borrow().is_empty() {
            for child in self.children.borrow().iter() {
                write!(&mut html, "{}", child.outer_html()).unwrap();
            }
        } else {
            html.push_str(&self.leaf_inner_html());
        }
        html
    }

    fn leaf_inner_html(&self) -> String {
        let properties = self.properties.borrow();
        let attributes = self.attributes.borrow();
        let inner_html = attributes.get("inner_html");
        let inner_text = attributes.get("inner_text");
        if let Some(Some(text)) = properties.get("text") {
            escape_html_text(text)
        } else if let Some(inner_html) = inner_html {
            inner_html.to_string()
        } else if let Some(inner_text) = inner_text {
            escape_html_text(inner_text)
        } else {
            String::new()
        }
    }
}

fn escape_html_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html_text(value).replace('"', "&quot;")
}

fn is_valid_html_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_') && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
}

/// Command-stream consumer producing legacy-exact SSR HTML.
#[derive(Debug, Default)]
pub struct SsrDocument {
    nodes: HashMap<u64, SsrNode>,
}

impl SsrDocument {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        // Reserved host root (id 0).
        nodes.insert(0, SsrNode::new("body", false));
        Self { nodes }
    }

    /// Builds a document by replaying `commands` in order.
    pub fn replay(commands: &[Command]) -> Self {
        let mut doc = Self::new();
        for command in commands {
            doc.apply(command);
        }
        doc
    }

    pub fn node(&self, id: u64) -> Option<&SsrNode> {
        self.nodes.get(&id)
    }

    /// Rendered children of `id` (empty string for unknown ids).
    pub fn inner_html(&self, id: u64) -> String {
        self.nodes.get(&id).map(SsrNode::inner_html).unwrap_or_default()
    }

    pub fn inner_html_chunks(&self, id: u64) -> Vec<String> {
        let Some(node) = self.nodes.get(&id) else {
            return Vec::new();
        };
        let mut chunks = Vec::new();
        for child in node.children.borrow().iter() {
            child.outer_html_chunks(&mut chunks);
        }
        chunks
    }

    /// Streaming variant of [`inner_html_chunks`](Self::inner_html_chunks):
    /// `replace` may substitute (and prune) individual nodes' serialization.
    pub fn inner_html_chunks_with(&self, id: u64, replace: &dyn Fn(&SsrNode) -> Option<String>) -> Vec<String> {
        let Some(node) = self.nodes.get(&id) else {
            return Vec::new();
        };
        let mut chunks = Vec::new();
        for child in node.children.borrow().iter() {
            child.outer_html_chunks_with(&mut chunks, replace);
        }
        chunks
    }

    /// Children HTML of `id` with the streaming `replace` hook applied.
    pub fn inner_html_with(&self, id: u64, replace: &dyn Fn(&SsrNode) -> Option<String>) -> String {
        self.nodes.get(&id).map(|node| node.inner_html_with(replace)).unwrap_or_default()
    }

    pub fn outer_html(&self, id: u64) -> String {
        self.nodes.get(&id).map(SsrNode::outer_html).unwrap_or_default()
    }

    /// Open/close tag pair of `id`, or `None` for unknown ids.
    pub fn html_tag(&self, id: u64) -> Option<(String, String)> {
        self.nodes.get(&id).map(SsrNode::html_tag)
    }

    pub fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, name, is_void } => {
                self.nodes.insert(*id, SsrNode::new(name.clone(), *is_void));
            }
            Command::SetAttribute { id, name, value } => {
                if let Some(node) = self.nodes.get(id) {
                    node.set_attribute(name.clone(), value.clone());
                }
            }
            Command::RemoveAttribute { id, name } => {
                if let Some(node) = self.nodes.get(id) {
                    node.remove_attribute(name);
                }
            }
            Command::SetProperty { id, name, value } => {
                if let Some(node) = self.nodes.get(id) {
                    node.set_property(name.clone(), Some(Cow::from(value.clone())));
                }
            }
            Command::RemoveProperty { id, name } => {
                if let Some(node) = self.nodes.get(id) {
                    node.remove_property(name);
                }
            }
            Command::AddClass { id, value } => {
                if let Some(node) = self.nodes.get(id) {
                    node.add_class(value.clone());
                }
            }
            Command::RemoveClass { id, value } => {
                if let Some(node) = self.nodes.get(id) {
                    node.remove_class(value);
                }
            }
            // Legacy-parity: SetText/SetHtml record the pseudo attributes
            // the old SSR renderer used; children are not cleared.
            Command::SetText { id, value } => {
                if let Some(node) = self.nodes.get(id) {
                    node.set_attribute("inner_text", value.clone());
                }
            }
            Command::SetHtml { id, value } => {
                if let Some(node) = self.nodes.get(id) {
                    node.set_attribute("inner_html", value.clone());
                }
            }
            Command::Insert { parent, child, position } => {
                let (Some(parent), Some(child)) = (self.nodes.get(parent), self.nodes.get(child)) else {
                    return;
                };
                match position {
                    CommandInsertPosition::Head => parent.prepend_with_node(child),
                    CommandInsertPosition::Tail => parent.append_with_node(child),
                    CommandInsertPosition::Before(anchor) => match self.nodes.get(anchor) {
                        Some(anchor) => parent.insert_before(anchor, child),
                        None => parent.append_with_node(child),
                    },
                    CommandInsertPosition::After(anchor) => match self.nodes.get(anchor) {
                        Some(anchor) => parent.insert_after(anchor, child),
                        None => parent.append_with_node(child),
                    },
                }
            }
            Command::Remove { parent, child } => {
                if let (Some(parent), Some(child)) = (self.nodes.get(parent), self.nodes.get(child)) {
                    parent.remove_child(child);
                }
            }
            // Server rendering records no listeners and answers no queries.
            Command::AttachEvent { .. } | Command::DetachEvent { .. } | Command::Query { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_html_escapes_text_and_attribute_values() {
        let node = SsrNode::new("p", false);
        node.set_attribute("title", "\"<&>");
        node.set_attribute("inner_text", "<b>&</b>");

        assert_eq!(node.outer_html(), r#"<p title="&quot;&lt;&amp;&gt;">&lt;b&gt;&amp;&lt;/b&gt;</p>"#);
    }

    #[test]
    fn outer_html_keeps_explicit_inner_html_raw() {
        let node = SsrNode::new("p", false);
        node.set_attribute("inner_html", "<strong>raw</strong>");

        assert_eq!(node.outer_html(), "<p><strong>raw</strong></p>");
    }

    #[test]
    fn outer_html_skips_invalid_names() {
        let node = SsrNode::new("script onclick=alert(1)", false);
        node.set_attribute("bad name", "x");
        node.set_attribute("data-ok", "y");

        assert_eq!(node.outer_html(), r#"<div data-ok="y"></div>"#);
    }

    #[test]
    fn replay_builds_legacy_exact_tree() {
        use crate::renderer::command::{CommandQueue, CommandRenderer};
        use crate::renderer::{InsertPosition, Renderer};

        let queue = CommandQueue::new();
        let renderer = CommandRenderer::from_queue(queue.clone());
        let host = crate::renderer::command::CommandNode::host(queue);

        let list = renderer.create_element("ul".into(), false);
        let first = renderer.create_element("li".into(), false);
        let second = renderer.create_element("li".into(), false);
        renderer.set_attribute(&first, "data-id".into(), "a".into());
        renderer.set_text(&first, "A".into());
        renderer.add_class(&first, "selected".into());
        renderer.set_text(&second, "B".into());
        renderer.insert_child(&host, &list, InsertPosition::Tail);
        renderer.insert_child(&list, &first, InsertPosition::Tail);
        renderer.insert_child(&list, &second, InsertPosition::Before(&first));

        let doc = SsrDocument::replay(&renderer.take_batch());
        assert_eq!(doc.inner_html(0), r#"<ul><li>B</li><li data-id="a" class=" selected">A</li></ul>"#);
        assert_eq!(
            doc.inner_html_chunks(0).join(""),
            r#"<ul><li>B</li><li data-id="a" class=" selected">A</li></ul>"#
        );
        assert_eq!(doc.html_tag(list.id()).unwrap().0, "<ul>");
    }
}
