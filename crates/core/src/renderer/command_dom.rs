//! In-memory reference interpreter for the [`Command`] stream.
//!
//! `CommandDom` applies command batches to a simple tree exactly the way
//! `crates/desktop/src/wry_interpreter.js` applies them to the real DOM.
//! It serves two purposes:
//!
//! 1. **Conformance testing** — widget scenarios run against the command
//!    backend assert on the resulting `CommandDom` tree, mirroring the SSR
//!    snapshot tests.
//! 2. **Reference semantics** — any new command consumer (TUI, native)
//!    should reproduce the behaviour encoded here, in particular the
//!    move-on-insert rule.
//!
//! # Semantics notes
//!
//! - `Insert` always *moves*: if the child is already attached anywhere it
//!   is detached first (matching `Node.insertBefore` in the browser).
//! - `SetText` clears children (matching `textContent = ...`).
//! - `SetHtml` stores the raw markup and clears children; the raw string is
//!   exposed verbatim by [`CommandDom::inner_html`].
//! - Node id `0` is pre-registered as the `body` root.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

use super::command::{Command, CommandInsertPosition, NodeQuery, QueryError, QueryResponse, QueryValue};

#[derive(Debug, Default, Clone)]
pub struct DomNode {
    pub name: String,
    pub is_void: bool,
    pub attributes: BTreeMap<String, String>,
    pub properties: BTreeMap<String, String>,
    pub classes: BTreeSet<String>,
    pub text: Option<String>,
    pub raw_html: Option<String>,
    pub children: Vec<u64>,
    pub listeners: BTreeSet<String>,
}

/// See the module docs. Panics on malformed batches (unknown node ids):
/// a malformed batch is a framework bug, and tests should fail loudly.
#[derive(Debug)]
pub struct CommandDom {
    nodes: HashMap<u64, DomNode>,
    /// child id -> parent id, for the move-on-insert rule.
    parents: HashMap<u64, u64>,
    /// Answers produced for `Command::Query`, in arrival order. Hosts/tests
    /// drain these and feed them back via `CommandRenderer::resolve_query`,
    /// modelling the async response leg of a remote consumer.
    query_responses: Vec<QueryResponse>,
}

pub const ROOT_ID: u64 = 0;

impl Default for CommandDom {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandDom {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            ROOT_ID,
            DomNode {
                name: "body".to_owned(),
                ..Default::default()
            },
        );
        Self {
            nodes,
            parents: HashMap::new(),
            query_responses: Vec::new(),
        }
    }

    /// Drains the answers produced for `Command::Query` so far.
    pub fn take_query_responses(&mut self) -> Vec<QueryResponse> {
        std::mem::take(&mut self.query_responses)
    }

    pub fn node(&self, id: u64) -> Option<&DomNode> {
        self.nodes.get(&id)
    }

    pub fn root(&self) -> &DomNode {
        &self.nodes[&ROOT_ID]
    }

    fn node_mut(&mut self, id: u64) -> &mut DomNode {
        self.nodes.get_mut(&id).unwrap_or_else(|| panic!("CommandDom: unknown node id {id}"))
    }

    fn detach(&mut self, child: u64) {
        if let Some(parent) = self.parents.remove(&child) {
            self.node_mut(parent).children.retain(|id| *id != child);
        }
    }

    fn attach(&mut self, parent: u64, child: u64, position: CommandInsertPosition) {
        self.detach(child);
        let children = &mut self.node_mut(parent).children;
        match position {
            CommandInsertPosition::Head => children.insert(0, child),
            CommandInsertPosition::Tail => children.push(child),
            CommandInsertPosition::Before(anchor) => {
                let idx = children.iter().position(|id| *id == anchor).unwrap_or(children.len());
                children.insert(idx, child);
            }
            CommandInsertPosition::After(anchor) => {
                let idx = children.iter().position(|id| *id == anchor).map(|i| i + 1).unwrap_or(children.len());
                children.insert(idx, child);
            }
        }
        self.parents.insert(child, parent);
    }

    pub fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, name, is_void } => {
                self.nodes.insert(
                    *id,
                    DomNode {
                        name: name.clone(),
                        is_void: *is_void,
                        ..Default::default()
                    },
                );
            }
            Command::SetAttribute { id, name, value } => {
                self.node_mut(*id).attributes.insert(name.clone(), value.clone());
            }
            Command::RemoveAttribute { id, name } => {
                self.node_mut(*id).attributes.remove(name);
            }
            Command::SetProperty { id, name, value } => {
                self.node_mut(*id).properties.insert(name.clone(), value.clone());
            }
            Command::RemoveProperty { id, name } => {
                self.node_mut(*id).properties.remove(name);
            }
            Command::AddClass { id, value } => {
                let node = self.node_mut(*id);
                for part in value.split_whitespace() {
                    node.classes.insert(part.to_owned());
                }
            }
            Command::RemoveClass { id, value } => {
                let node = self.node_mut(*id);
                for part in value.split_whitespace() {
                    node.classes.remove(part);
                }
            }
            Command::SetText { id, value } => {
                let children: Vec<u64> = std::mem::take(&mut self.node_mut(*id).children);
                for child in children {
                    self.parents.remove(&child);
                }
                let node = self.node_mut(*id);
                node.raw_html = None;
                node.text = Some(value.clone());
            }
            Command::SetHtml { id, value } => {
                let children: Vec<u64> = std::mem::take(&mut self.node_mut(*id).children);
                for child in children {
                    self.parents.remove(&child);
                }
                let node = self.node_mut(*id);
                node.text = None;
                node.raw_html = Some(value.clone());
            }
            Command::Insert { parent, child, position } => {
                self.attach(*parent, *child, *position);
            }
            Command::Remove { parent, child } => {
                if self.parents.get(child) == Some(parent) {
                    self.detach(*child);
                }
            }
            Command::AttachEvent { id, name, .. } => {
                self.node_mut(*id).listeners.insert(name.clone());
            }
            Command::DetachEvent { id, name } => {
                self.node_mut(*id).listeners.remove(name);
            }
            Command::Query { id, token, kind } => {
                let result = match self.nodes.get(id) {
                    None => Err(QueryError::NodeGone),
                    Some(node) => match kind {
                        NodeQuery::Value => Ok(QueryValue::Value(node.properties.get("value").cloned().unwrap_or_default())),
                        // The in-memory tree has no layout engine.
                        NodeQuery::BoundingRect | NodeQuery::ScrollOffset => Err(QueryError::Unsupported),
                    },
                };
                self.query_responses.push(QueryResponse { token: *token, result });
            }
        }
    }

    pub fn apply_batch(&mut self, batch: &[Command]) {
        for command in batch {
            self.apply(command);
        }
    }

    /// Depth-first text contents of all `tag` elements, in document order.
    /// The conformance counterpart of parsing `<li>` sequences out of SSR
    /// snapshots.
    pub fn texts_of(&self, tag: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_texts(ROOT_ID, tag, &mut out);
        out
    }

    fn collect_texts(&self, id: u64, tag: &str, out: &mut Vec<String>) {
        let Some(node) = self.nodes.get(&id) else { return };
        if node.name == tag {
            out.push(node.text.clone().unwrap_or_default());
        }
        for child in &node.children {
            self.collect_texts(*child, tag, out);
        }
    }

    /// Simple HTML-ish serialization for assertions and demos. Not
    /// escape-exact with the SSR renderer — compare structure, not bytes.
    pub fn inner_html(&self, id: u64) -> String {
        let mut html = String::new();
        if let Some(node) = self.nodes.get(&id) {
            if let Some(raw) = &node.raw_html {
                return raw.clone();
            }
            if let Some(text) = &node.text {
                return text.clone();
            }
            for child in &node.children {
                self.write_outer_html(*child, &mut html);
            }
        }
        html
    }

    fn write_outer_html(&self, id: u64, out: &mut String) {
        let Some(node) = self.nodes.get(&id) else { return };
        write!(out, "<{}", node.name).unwrap();
        for (key, value) in &node.attributes {
            write!(out, " {key}=\"{value}\"").unwrap();
        }
        if !node.classes.is_empty() {
            let classes: Vec<&str> = node.classes.iter().map(String::as_str).collect();
            write!(out, " class=\"{}\"", classes.join(" ")).unwrap();
        }
        out.push('>');
        if !node.is_void {
            out.push_str(&self.inner_html(id));
            write!(out, "</{}>", node.name).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::command::{CommandQueue, CommandRenderer};
    use crate::renderer::{InsertPosition, Renderer};

    #[test]
    fn batch_replay_builds_expected_tree() {
        let queue = CommandQueue::new();
        let renderer = CommandRenderer::from_queue(queue.clone());
        let host = crate::renderer::command::CommandNode::host(queue);

        let list = renderer.create_element("ul".into(), false);
        let a = renderer.create_element("li".into(), false);
        let b = renderer.create_element("li".into(), false);
        renderer.set_text(&a, "A".into());
        renderer.set_text(&b, "B".into());
        renderer.insert_child(&host, &list, InsertPosition::Tail);
        renderer.insert_child(&list, &a, InsertPosition::Tail);
        renderer.insert_child(&list, &b, InsertPosition::Tail);
        // Move A after B (LIS-style reorder produces this shape).
        renderer.insert_child(&list, &a, InsertPosition::After(&b));

        let mut dom = CommandDom::new();
        dom.apply_batch(&renderer.take_batch());

        assert_eq!(dom.texts_of("li"), vec!["B".to_string(), "A".to_string()]);
        assert_eq!(dom.inner_html(ROOT_ID), "<ul><li>B</li><li>A</li></ul>");
    }

    #[test]
    fn set_text_clears_children_like_text_content() {
        let renderer = CommandRenderer::new();
        let parent = renderer.create_element("div".into(), false);
        let child = renderer.create_element("span".into(), false);
        renderer.insert_child(&parent, &child, InsertPosition::Tail);
        renderer.set_text(&parent, "flat".into());

        let mut dom = CommandDom::new();
        dom.apply_batch(&renderer.take_batch());
        let parent_node = dom.node(1).unwrap();
        assert!(parent_node.children.is_empty());
        assert_eq!(parent_node.text.as_deref(), Some("flat"));
    }

    #[test]
    fn remove_ignores_stale_parent() {
        let mut dom = CommandDom::new();
        dom.apply_batch(&[
            Command::Create {
                id: 1,
                name: "div".into(),
                is_void: false,
            },
            Command::Create {
                id: 2,
                name: "div".into(),
                is_void: false,
            },
            Command::Insert {
                parent: ROOT_ID,
                child: 2,
                position: CommandInsertPosition::Tail,
            },
            // Stale remove: node 2 is not a child of node 1.
            Command::Remove { parent: 1, child: 2 },
        ]);
        assert_eq!(dom.root().children, vec![2]);
    }
}
