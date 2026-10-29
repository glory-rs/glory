//! Shared display data and named slots used by handwritten code and XML macros.
use crate::template::{Error, Kind};
use std::fmt::Write;

macro_rules! part {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(pub &'static str);
        impl $name {
            pub const fn new(name: &'static str) -> Self {
                Self(name)
            }
        }
    };
}
part!(Field);
part!(Click);
part!(Outlet);

#[derive(Clone, Copy, Debug)]
pub enum Attribute {
    Static(&'static str, &'static str),
    Field(&'static str, Field),
    Click(Click),
    Children(Outlet),
    Class(&'static str, Field),
}
#[derive(Clone, Copy, Debug)]
pub enum Node {
    Element {
        tag: &'static str,
        attrs: &'static [Attribute],
        children: &'static [Node],
    },
    Text(&'static str),
    Field(Field),
    Outlet(Outlet),
}
#[derive(Debug)]
pub struct Template {
    roots: &'static [Node],
    compiled: std::sync::OnceLock<Result<std::sync::Arc<Compiled>, Error>>,
}
impl Template {
    pub fn roots(&self) -> &'static [Node] {
        self.roots
    }
    pub const fn new(roots: &'static [Node]) -> Self {
        Self {
            roots,
            compiled: std::sync::OnceLock::new(),
        }
    }
    pub fn cached(&self) -> Result<std::sync::Arc<Compiled>, Error> {
        self.compiled
            .get_or_init(|| self.compile().map(std::sync::Arc::new))
            .clone()
    }
}
pub const fn element(
    tag: &'static str,
    attrs: &'static [Attribute],
    children: &'static [Node],
) -> Node {
    Node::Element {
        tag,
        attrs,
        children,
    }
}
pub const fn div(attrs: &'static [Attribute], children: &'static [Node]) -> Node {
    element("div", attrs, children)
}
pub const fn span(attrs: &'static [Attribute], children: &'static [Node]) -> Node {
    element("span", attrs, children)
}
pub const fn button(attrs: &'static [Attribute], children: &'static [Node]) -> Node {
    element("button", attrs, children)
}
pub const fn text(value: &'static str) -> Node {
    Node::Text(value)
}
pub const fn field(value: Field) -> Node {
    Node::Field(value)
}
pub const fn outlet(value: Outlet) -> Node {
    Node::Outlet(value)
}
pub const fn attr(name: &'static str, value: &'static str) -> Attribute {
    Attribute::Static(name, value)
}
pub const fn class(value: &'static str) -> Attribute {
    attr("class", value)
}
pub const fn bind_attr(name: &'static str, value: Field) -> Attribute {
    Attribute::Field(name, value)
}
pub const fn on_click(value: Click) -> Attribute {
    Attribute::Click(value)
}
pub const fn children(value: Outlet) -> Attribute {
    Attribute::Children(value)
}
pub const fn toggle_class(token: &'static str, value: Field) -> Attribute {
    Attribute::Class(token, value)
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub name: Option<&'static str>,
    pub kind: Kind,
}
/// Compiled once per shared declaration, never once per static element instance.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub wire: String,
    pub slots: Vec<Slot>,
    pub node_count: usize,
    pub paths: Vec<Vec<u16>>,
}
fn string(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c < ' ' => {
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
impl Compiled {
    pub fn slot(&self, name: &str) -> Option<(u16, Kind)> {
        self.slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.name == Some(name))
            .map(|(i, s)| (i as u16, s.kind))
    }
}
impl Template {
    pub fn compile(&self) -> Result<Compiled, Error> {
        struct Compiler {
            roots: String,
            targets: Vec<(Vec<u16>, Slot)>,
            nodes: usize,
            bytes: usize,
        }
        impl Compiler {
            fn slot(&mut self, path: &[u16], name: &'static str, kind: Kind) -> Result<(), Error> {
                if name.is_empty()
                    || name.len() > 128
                    || self.targets.len() >= 256
                    || self.targets.iter().any(|(_, t)| t.name == Some(name))
                {
                    return Err(Error("empty, duplicate or over-budget slot name"));
                }
                self.bytes += name.len();
                self.targets.push((
                    path.to_vec(),
                    Slot {
                        name: Some(name),
                        kind,
                    },
                ));
                Ok(())
            }
            fn nodes(
                &mut self,
                nodes: &[Node],
                path: &mut Vec<u16>,
                parent: Option<&str>,
                in_button: bool,
            ) -> Result<(), Error> {
                if path.len() > 64 {
                    return Err(Error("template depth budget"));
                }
                self.roots.push('[');
                for (index, node) in nodes.iter().enumerate() {
                    self.nodes += 1;
                    if self.nodes > 256 {
                        return Err(Error("template node budget"));
                    }
                    if index > 0 {
                        self.roots.push(',');
                    }
                    path.push(index as u16);
                    match node {
                        Node::Text(value) => {
                            if parent.is_some_and(|p| matches!(p, "table" | "tbody" | "tr"))
                                && !value.trim().is_empty()
                            {
                                return Err(Error("table structure cannot contain direct text"));
                            }
                            self.bytes += value.len();
                            self.roots.push_str("[0,");
                            string(&mut self.roots, value);
                            self.roots.push(']');
                        }
                        Node::Field(value) => {
                            if parent.is_some_and(|p| matches!(p, "table" | "tbody" | "tr")) {
                                return Err(Error("table structure cannot contain text fields"));
                            }
                            self.slot(path, value.0, Kind::Text)?;
                            self.roots.push_str("[0,\"\"]");
                        }
                        Node::Outlet(value) => {
                            if parent.is_some_and(|p| !matches!(p, "div" | "tbody" | "td")) {
                                return Err(Error(
                                    "dynamic outlets require a div parent in this profile",
                                ));
                            }
                            self.slot(path, value.0, Kind::Range)?;
                            if path.len() == 1 {
                                self.nodes += 1;
                            }
                            if self.nodes > 256 {
                                return Err(Error("template node budget"));
                            }
                            self.roots.push_str("[2]");
                        }
                        Node::Element {
                            tag,
                            attrs,
                            children,
                        } => {
                            if attrs.iter().any(|a| matches!(a, Attribute::Class(..)))
                                && attrs
                                    .iter()
                                    .any(|a| matches!(a, Attribute::Field("class", _)))
                            {
                                return Err(Error(
                                    "class toggles cannot share a dynamic whole class attribute",
                                ));
                            }
                            if !crate::template::context(parent, tag, in_button) {
                                return Err(Error("unsupported HTML tag or content context"));
                            }
                            self.roots.push_str("[1,");
                            string(&mut self.roots, tag);
                            self.roots.push_str(",[");
                            let mut names = Vec::new();
                            let mut written = 0;
                            let mut toggles = Vec::new();
                            for attribute in *attrs {
                                match *attribute {
                                    Attribute::Static(name, value) => {
                                        if !crate::template::attribute(name)
                                            || !name
                                                .bytes()
                                                .all(|b| b.is_ascii_lowercase() || b == b'-')
                                            || names.contains(&name)
                                            || (name == "type"
                                                && (*tag != "button" || value != "button"))
                                            || (name == "disabled" && *tag != "button")
                                        {
                                            return Err(Error(
                                                "unsupported, duplicate or invalid HTML attribute",
                                            ));
                                        }
                                        names.push(name);
                                        self.bytes += name.len() + value.len();
                                        if written > 0 {
                                            self.roots.push(',');
                                        }
                                        written += 1;
                                        self.roots.push('[');
                                        string(&mut self.roots, name);
                                        self.roots.push(',');
                                        string(&mut self.roots, value);
                                        self.roots.push(']');
                                    }
                                    Attribute::Field(name, value) => {
                                        if !matches!(name, "class" | "title" | "aria-label")
                                            || names.contains(&name)
                                        {
                                            return Err(Error(
                                                "unsupported or duplicate dynamic attribute",
                                            ));
                                        }
                                        names.push(name);
                                        self.slot(path, value.0, Kind::Attribute(name))?;
                                    }
                                    Attribute::Click(value) => {
                                        if names.contains(&"on:click") {
                                            return Err(Error("duplicate click target"));
                                        }
                                        names.push("on:click");
                                        self.slot(path, value.0, Kind::Click)?;
                                    }
                                    Attribute::Children(value) => {
                                        if !matches!(*tag, "div" | "tbody" | "td")
                                            || names.contains(&"children")
                                        {
                                            return Err(Error(
                                                "children outlet requires a div, tbody or td",
                                            ));
                                        }
                                        names.push("children");
                                        self.slot(path, value.0, Kind::Children)?;
                                    }
                                    Attribute::Class(token, value) => {
                                        if !crate::template::class_token(token)
                                            || toggles.contains(&token)
                                        {
                                            return Err(Error("invalid or duplicate class token"));
                                        }
                                        toggles.push(token);
                                        self.bytes += token.len();
                                        let initial = attrs
                                            .iter()
                                            .find_map(|a| {
                                                if let Attribute::Static("class", value) = a {
                                                    Some(
                                                        value
                                                            .split_ascii_whitespace()
                                                            .any(|s| s == token),
                                                    )
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or(false);
                                        self.slot(path, value.0, Kind::Class(token, initial))?;
                                    }
                                }
                            }
                            self.roots.push_str("],");
                            self.nodes(children, path, Some(tag), in_button || *tag == "button")?;
                            self.roots.push(']');
                        }
                    }
                    path.pop();
                    if self.bytes > 64 * 1024 {
                        return Err(Error("template string budget"));
                    }
                }
                self.roots.push(']');
                Ok(())
            }
        }
        if self.roots.is_empty() {
            return Err(Error("empty template"));
        }
        let mut c = Compiler {
            roots: String::new(),
            targets: Vec::new(),
            nodes: 0,
            bytes: 0,
        };
        c.nodes(self.roots, &mut Vec::new(), None, false)?;
        let mut wire = format!("[{},[", c.roots);
        for (i, (path, slot)) in c.targets.iter().enumerate() {
            if i > 0 {
                wire.push(',');
            }
            wire.push_str("[[");
            for (j, index) in path.iter().enumerate() {
                if j > 0 {
                    wire.push(',');
                }
                write!(wire, "{index}").unwrap();
            }
            wire.push_str("],");
            match slot.kind {
                Kind::Text => wire.push_str("0,null"),
                Kind::Click => wire.push_str("2,null"),
                Kind::Children => wire.push_str("3,null"),
                Kind::Range => wire.push_str("4,null"),
                Kind::Class(name, _) => {
                    wire.push_str("5,");
                    string(&mut wire, name);
                }
                Kind::Attribute(name) => {
                    wire.push_str("1,");
                    string(&mut wire, name);
                }
            }
            wire.push(']');
        }
        wire.push_str("]]");
        Ok(Compiled {
            wire,
            node_count: c.nodes,
            paths: c.targets.iter().map(|(path, _)| path.clone()).collect(),
            slots: c.targets.into_iter().map(|(_, s)| s).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_duplicate_names_cross_kind_and_ambiguous_attributes_before_wire_creation() {
        static DUPLICATE: Template = Template::new(&[button(
            &[on_click(Click::new("same"))],
            &[field(Field::new("same"))],
        )]);
        static ATTR: Template = Template::new(&[div(
            &[class("a"), bind_attr("class", Field::new("class"))],
            &[],
        )]);
        static CHILDREN: Template = Template::new(&[button(&[children(Outlet::new("body"))], &[])]);
        static ROOT: Template = Template::new(&[outlet(Outlet::new("root"))]);
        for template in [&DUPLICATE, &ATTR, &CHILDREN] {
            assert!(template.compile().is_err());
        }
        assert_eq!(ROOT.compile().unwrap().node_count, 2);
    }
    #[test]
    fn named_children_append_after_static_content_in_the_shared_declaration() {
        static TEMPLATE: Template = Template::new(&[div(
            &[children(Outlet::new("body"))],
            &[text("static prefix")],
        )]);
        let compiled = TEMPLATE.cached().unwrap();
        assert_eq!(compiled.node_count, 2);
        assert_eq!(compiled.slots[0].kind, Kind::Children);
        assert!(std::sync::Arc::ptr_eq(
            &compiled,
            &TEMPLATE.cached().unwrap()
        ));
    }
    #[test]
    fn named_slot_metadata_is_shared_declaration_data_and_limits_count_physical_nodes() {
        static SMALL: Template = Template::new(&[div(&[], &[field(Field::new("text"))])]);
        static LARGE: Template = Template::new(&[div(&[], &[Node::Text("x"); 256])]);
        let compiled = SMALL.compile().unwrap();
        assert_eq!(compiled.node_count, 2);
        assert_eq!(compiled.slot("text"), Some((0, Kind::Text)));
        assert!(compiled.slot("missing").is_none());
        assert!(LARGE.compile().is_err());
    }
}
