use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Node {
    name: Rc<RefCell<Cow<'static, str>>>,
    is_void: Rc<RefCell<bool>>,
    classes: Rc<RefCell<BTreeSet<Cow<'static, str>>>>,
    attributes: Rc<RefCell<BTreeMap<Cow<'static, str>, Cow<'static, str>>>>,
    properties: Rc<RefCell<BTreeMap<Cow<'static, str>, Option<Cow<'static, str>>>>>,
    children: Rc<RefCell<Vec<Node>>>,
}

impl Node {
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

    /// Identity comparison. Two Node values are considered the same DOM
    /// instance iff they share the underlying `Rc` allocation (i.e. one
    /// was produced from the other via `Clone`). Content equality (via
    /// `PartialEq`) is not appropriate for child-of operations because
    /// distinct nodes can carry identical content.
    pub fn ptr_eq(&self, other: &Node) -> bool {
        Rc::ptr_eq(&self.children, &other.children)
    }

    pub fn remove_child(&self, node: &Node) {
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
    pub fn remove_attribute(&self, key: &str) {
        self.attributes.borrow_mut().remove(key);
    }
    pub fn set_property(&self, key: impl Into<Cow<'static, str>>, value: impl Into<Option<Cow<'static, str>>>) {
        self.properties.borrow_mut().insert(key.into(), value.into());
    }
    pub fn remove_property(&self, key: &str) {
        self.properties.borrow_mut().remove(key);
    }

    /// Move `node` to be the first child of `self`. If `node` already lives
    /// in `self`'s children, it is removed from its previous position first.
    pub fn prepend_with_node(&self, node: &Node) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(node));
        children.insert(0, node.clone());
    }
    /// Move `node` to be the last child of `self`. If `node` already lives
    /// in `self`'s children, it is removed from its previous position first.
    pub fn append_with_node(&self, node: &Node) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(node));
        children.push(node.clone());
    }

    /// Move `new_node` to the position immediately AFTER `anchor` among
    /// `self`'s children. If `new_node` is already a child of `self`, it
    /// is removed from its previous slot first. If `anchor` is not a
    /// child of `self`, `new_node` is appended.
    pub fn insert_after(&self, anchor: &Node, new_node: &Node) {
        let mut children = self.children.borrow_mut();
        children.retain(|n| !n.ptr_eq(new_node));
        let pos = children.iter().position(|n| n.ptr_eq(anchor));
        match pos {
            Some(idx) => children.insert(idx + 1, new_node.clone()),
            None => children.push(new_node.clone()),
        }
    }
    /// Move `new_node` to the position immediately BEFORE `anchor` among
    /// `self`'s children. If `new_node` is already a child of `self`, it
    /// is removed from its previous slot first. If `anchor` is not a
    /// child of `self`, `new_node` is appended.
    pub fn insert_before(&self, anchor: &Node, new_node: &Node) {
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
        let class = if !self.classes.borrow().is_empty() {
            format!(
                " class=\"{}\"",
                self.classes.borrow().deref().iter().fold("".to_string(), |mut acc, k| {
                    acc.push_str(&format!(" {k}"));
                    acc
                })
            )
        } else {
            "".to_string()
        };

        let properties = if !self.properties.borrow().is_empty() {
            self.properties.borrow().iter().fold("".to_string(), |mut acc, (k, v)| {
                if k != "text" {
                    if let Some(v) = v {
                        acc.push_str(&format!(" {k}=\"{v}\""));
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
                if k != "inner_html" && k != "inner_text" {
                    write!(&mut value, " {k}=\"{v}\"").unwrap();
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
    pub fn inner_html(&self) -> String {
        let mut html = "".to_string();
        if !self.children.borrow().is_empty() {
            for child in self.children.borrow().iter() {
                write!(&mut html, "{}", child.outer_html()).unwrap();
            }
        } else {
            let properties = self.properties.borrow();
            let attributes = self.attributes.borrow();
            let inner_html = attributes.get("inner_html");
            let inner_text = attributes.get("inner_text");
            if let Some(Some(text)) = properties.get("text") {
                write!(&mut html, "{}", &*text).unwrap();
            } else if let Some(inner_html) = inner_html {
                write!(&mut html, "{}", inner_html).unwrap();
            } else if let Some(inner_text) = inner_text {
                write!(&mut html, "{}", inner_text).unwrap();
            }
        }
        html
    }
}
