//! Shared template-instance data. Static elements have no semantic identities.
use crate::{
    template::Kind,
    view::{Attribute, Compiled, Node, Template},
};
use glory_render::{Error, NodeId};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Flag(bool),
    None,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tree {
    Element {
        tag: &'static str,
        attrs: BTreeMap<&'static str, String>,
        children: Vec<Tree>,
    },
    Text(String),
    Outlet(u16),
}
#[derive(Default, Debug)]
pub struct Queue(RefCell<Vec<NodeId>>);
impl Queue {
    pub fn take(&self) -> Vec<NodeId> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}
#[derive(Debug)]
pub struct Frame {
    template: &'static Template,
    compiled: Arc<Compiled>,
    values: RefCell<Vec<Value>>,
    changed: RefCell<BTreeMap<u16, Value>>,
    queue: RefCell<Option<(Weak<Queue>, NodeId)>>,
    queued: Cell<bool>,
}
#[derive(Clone, Debug)]
pub enum Placement {
    Root(Rc<Frame>),
    Outlet { frame: Rc<Frame>, slot: u16 },
}
impl PartialEq for Placement {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Root(a), Self::Root(b)) => Rc::ptr_eq(a, b),
            (Self::Outlet { frame: a, slot: x }, Self::Outlet { frame: b, slot: y }) => {
                Rc::ptr_eq(a, b) && x == y
            }
            _ => false,
        }
    }
}
impl Eq for Placement {}
impl Frame {
    pub fn compiled(&self) -> &Compiled {
        &self.compiled
    }
    pub fn phrasing(&self) -> bool {
        fn nodes(input: &[Node]) -> bool {
            input.iter().all(|node| match node {
                Node::Text(_) | Node::Field(_) => true,
                Node::Element {
                    tag: "span",
                    attrs,
                    children,
                } => {
                    attrs
                        .iter()
                        .all(|attr| !matches!(attr, Attribute::Children(_)))
                        && nodes(children)
                }
                _ => false,
            })
        }
        nodes(self.template.roots())
    }
    pub fn context(&self, slot: u16) -> Option<&'static str> {
        let path = self.compiled.paths.get(slot as usize)?;
        let path = if self.compiled.slots[slot as usize].kind == Kind::Children {
            path.as_slice()
        } else {
            path.get(..path.len().checked_sub(1)?)?
        };
        let (first, rest) = path.split_first()?;
        let mut node = self.template.roots().get(*first as usize)?;
        for index in rest {
            let Node::Element { children, .. } = node else {
                return None;
            };
            node = children.get(*index as usize)?;
        }
        if let Node::Element { tag, .. } = node {
            Some(tag)
        } else {
            None
        }
    }
    pub fn accepts(&self, parent: &str) -> bool {
        if matches!(
            parent,
            "p" | "span" | "button" | "a" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        ) {
            return self.phrasing();
        }
        self.template.roots().iter().all(|node| match node {
            Node::Text(text) => {
                !matches!(parent, "table" | "tbody" | "tr") || text.trim().is_empty()
            }
            Node::Field(_) => !matches!(parent, "table" | "tbody" | "tr"),
            Node::Outlet(_) => true,
            Node::Element { tag, .. } => crate::template::context(Some(parent), tag, false),
        })
    }
    pub fn new(template: &'static Template) -> Result<Rc<Self>, Error> {
        let compiled = template
            .cached()
            .map_err(|e| Error::Backend(e.to_string()))?;
        let values = compiled
            .slots
            .iter()
            .map(|slot| match slot.kind {
                Kind::Text | Kind::Attribute(_) => Value::Text(String::new()),
                Kind::Class(_, initial) => Value::Flag(initial),
                _ => Value::None,
            })
            .collect();
        Ok(Rc::new(Self {
            template,
            compiled,
            values: RefCell::new(values),
            changed: RefCell::new(BTreeMap::new()),
            queue: RefCell::new(None),
            queued: Cell::new(false),
        }))
    }
    pub fn slot(&self, name: &str) -> Result<(u16, Kind), Error> {
        self.compiled
            .slot(name)
            .ok_or(Error::InvalidProps("unknown display slot"))
    }
    pub fn value(&self, slot: u16) -> Result<Value, Error> {
        self.values
            .borrow()
            .get(slot as usize)
            .cloned()
            .ok_or(Error::InvalidProps("display slot"))
    }
    pub fn text(&self, slot: u16, text: String) -> Result<(), Error> {
        if text.len() > 65536 {
            return Err(Error::InvalidProps("display field payload budget"));
        }
        if !matches!(
            self.compiled.slots.get(slot as usize).map(|s| s.kind),
            Some(Kind::Text | Kind::Attribute(_))
        ) {
            return Err(Error::InvalidProps("display text slot kind"));
        }
        self.write(slot, Value::Text(crate::normalize(&text)))
    }
    pub fn flag(&self, slot: u16, value: bool) -> Result<(), Error> {
        if !matches!(
            self.compiled.slots.get(slot as usize).map(|s| s.kind),
            Some(Kind::Class(..))
        ) {
            return Err(Error::InvalidProps("display class slot kind"));
        }
        self.write(slot, Value::Flag(value))
    }
    fn write(&self, slot: u16, value: Value) -> Result<(), Error> {
        let mut values = self.values.borrow_mut();
        let previous = values
            .get_mut(slot as usize)
            .ok_or(Error::InvalidProps("display field slot"))?;
        if *previous == value {
            return Ok(());
        }
        *previous = value.clone();
        drop(values);
        self.changed.borrow_mut().insert(slot, value);
        if !self.queued.replace(true)
            && let Some((queue, id)) = self.queue.borrow().as_ref()
            && let Some(queue) = queue.upgrade()
        {
            queue.0.borrow_mut().push(*id);
        }
        Ok(())
    }
    pub fn bind(&self, queue: &Rc<Queue>, id: NodeId) {
        *self.queue.borrow_mut() = Some((Rc::downgrade(queue), id));
        self.take_changes();
    }
    pub fn take_changes(&self) -> BTreeMap<u16, Value> {
        self.queued.set(false);
        std::mem::take(&mut *self.changed.borrow_mut())
    }
    pub fn tree(&self) -> Vec<Tree> {
        fn nodes(frame: &Frame, input: &[Node]) -> Vec<Tree> {
            input
                .iter()
                .map(|node| match node {
                    Node::Text(text) => Tree::Text(crate::normalize(text)),
                    Node::Field(field) => {
                        let (slot, _) = frame.slot(field.0).expect("validated field");
                        let Value::Text(value) = frame.value(slot).unwrap() else {
                            unreachable!()
                        };
                        Tree::Text(value)
                    }
                    Node::Outlet(outlet) => Tree::Outlet(frame.slot(outlet.0).unwrap().0),
                    Node::Element {
                        tag,
                        attrs,
                        children,
                    } => {
                        let mut values = BTreeMap::new();
                        for attr in *attrs {
                            match attr {
                                Attribute::Static(name, value) => {
                                    values.insert(*name, crate::normalize(value));
                                }
                                Attribute::Field(name, field) => {
                                    let Value::Text(value) =
                                        frame.value(frame.slot(field.0).unwrap().0).unwrap()
                                    else {
                                        unreachable!()
                                    };
                                    values.insert(*name, value);
                                }
                                Attribute::Click(click) => {
                                    values.insert(
                                        "data-gly-dclick",
                                        frame.slot(click.0).unwrap().0.to_string(),
                                    );
                                }
                                _ => {}
                            }
                        }
                        for attr in *attrs {
                            if let Attribute::Class(token, field) = attr {
                                let Value::Flag(enabled) =
                                    frame.value(frame.slot(field.0).unwrap().0).unwrap()
                                else {
                                    unreachable!()
                                };
                                let class = values.entry("class").or_insert_with(String::new);
                                let present =
                                    class.split_ascii_whitespace().any(|part| part == *token);
                                if present != enabled {
                                    let mut parts: Vec<_> = class
                                        .split_ascii_whitespace()
                                        .filter(|part| *part != *token)
                                        .collect();
                                    if enabled {
                                        parts.push(token);
                                    }
                                    *class = parts.join(" ");
                                }
                            }
                        }
                        let mut children = nodes(frame, children);
                        for attr in *attrs {
                            if let Attribute::Children(outlet) = attr {
                                children.push(Tree::Outlet(frame.slot(outlet.0).unwrap().0));
                            }
                        }
                        Tree::Element {
                            tag,
                            attrs: values,
                            children,
                        }
                    }
                })
                .collect()
        }
        nodes(self, self.template.roots())
    }
}
