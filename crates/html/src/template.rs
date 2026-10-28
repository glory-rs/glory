//! Shared, immutable HTML display data. No per-element semantic identity or UI
//! runtime is constructed. This first profile supports div/span/button, text,
//! ordinary attributes and click targets; editors/resources are not advertised.
use std::fmt::Write;

#[derive(Clone, Copy, Debug)]
pub enum Node {
    Element {
        tag: &'static str,
        attrs: &'static [(&'static str, &'static str)],
        children: &'static [Node],
    },
    Text(&'static str),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Attribute(&'static str),
    Click,
    Children,
    Range,
    Class(&'static str, bool),
}
#[derive(Clone, Copy, Debug)]
pub struct Target {
    /// Root index followed by physical child indices (including text nodes).
    pub path: &'static [u16],
    pub kind: Kind,
}
#[derive(Debug)]
pub struct Template {
    pub roots: &'static [Node],
    pub targets: &'static [Target],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}

pub(crate) fn attribute(name: &str) -> bool {
    matches!(
        name,
        "id" | "class" | "role" | "title" | "disabled" | "type"
    ) || name.starts_with("aria-")
}
pub(crate) fn class_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 128
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
pub(crate) fn context(parent: Option<&str>, child: &str, in_button: bool) -> bool {
    matches!(
        child,
        "div" | "span" | "button" | "a" | "h1" | "table" | "tbody" | "tr" | "td"
    ) && !(in_button && matches!(child, "button" | "a"))
        && match parent {
            Some("table") => child == "tbody",
            Some("tbody") => child == "tr",
            Some("tr") => child == "td",
            Some("span" | "button" | "a" | "h1") => child == "span",
            _ => !matches!(child, "tbody" | "tr" | "td") || parent.is_none(),
        }
}
impl Template {
    pub fn node(&self, path: &[u16]) -> Result<&Node, Error> {
        let (first, rest) = path.split_first().ok_or(Error("empty template path"))?;
        let mut node = self
            .roots
            .get(*first as usize)
            .ok_or(Error("template root"))?;
        for index in rest {
            let Node::Element { children, .. } = node else {
                return Err(Error("text has no children"));
            };
            node = children
                .get(*index as usize)
                .ok_or(Error("template child"))?;
        }
        Ok(node)
    }
    pub fn validate(&self) -> Result<usize, Error> {
        if self.roots.is_empty() || self.roots.len() > 256 || self.targets.len() > 256 {
            return Err(Error("template root/target budget"));
        }
        fn visit(
            nodes: &[Node],
            inline: bool,
            button: bool,
            depth: usize,
            count: &mut usize,
            bytes: &mut usize,
        ) -> Result<(), Error> {
            if depth > 64 {
                return Err(Error("template depth budget"));
            }
            for node in nodes {
                *count += 1;
                if *count > 256 {
                    return Err(Error("template node budget"));
                }
                match node {
                    Node::Text(value) => *bytes += value.len(),
                    Node::Element {
                        tag,
                        attrs,
                        children,
                    } => {
                        if !matches!(*tag, "div" | "span" | "button")
                            || (inline && *tag != "span")
                            || (button && *tag == "button")
                        {
                            return Err(Error("unsupported HTML tag or content context"));
                        }
                        for (index, (name, value)) in attrs.iter().enumerate() {
                            if !attribute(name)
                                || attrs[..index].iter().any(|(old, _)| old == name)
                                || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                            {
                                return Err(Error("unsupported/duplicate HTML attribute"));
                            }
                            if *name == "type" && (*tag != "button" || *value != "button") {
                                return Err(Error("only non-submit buttons supported"));
                            }
                            if *name == "disabled" && *tag != "button" {
                                return Err(Error("disabled target"));
                            }
                            *bytes += name.len() + value.len();
                        }
                        visit(
                            children,
                            inline || *tag != "div",
                            button || *tag == "button",
                            depth + 1,
                            count,
                            bytes,
                        )?;
                    }
                }
                if *bytes > 64 * 1024 {
                    return Err(Error("template string budget"));
                }
            }
            Ok(())
        }
        let (mut count, mut bytes) = (0, 0);
        visit(self.roots, false, false, 0, &mut count, &mut bytes)?;
        for (index, target) in self.targets.iter().enumerate() {
            let node = self.node(target.path)?;
            let valid = match (target.kind, node) {
                (Kind::Text, Node::Text(_)) => true,
                (Kind::Click, Node::Element { .. }) => true,
                (Kind::Children, Node::Element { tag: "div", .. }) => true,
                (Kind::Attribute(name), Node::Element { .. }) => {
                    matches!(name, "class" | "title" | "aria-label")
                }
                (Kind::Class(name, initial), Node::Element { attrs, .. }) => {
                    class_token(name)
                        && attrs
                            .iter()
                            .find(|(k, _)| *k == "class")
                            .is_some_and(|(_, v)| v.split_ascii_whitespace().any(|s| s == name))
                            == initial
                }
                _ => false,
            };
            if !valid
                || self.targets[..index]
                    .iter()
                    .any(|old| old.path == target.path && old.kind == target.kind)
            {
                return Err(Error("invalid/duplicate template target"));
            }
        }
        Ok(count)
    }
    /// Finite data for the host prototype builder, never executable markup.
    pub fn wire(&self) -> Result<String, Error> {
        self.validate()?;
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
        fn nodes(out: &mut String, values: &[Node]) {
            out.push('[');
            for (i, node) in values.iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                match node {
                    Node::Text(text) => {
                        out.push_str("[0,");
                        string(out, text);
                        out.push(']');
                    }
                    Node::Element {
                        tag,
                        attrs,
                        children,
                    } => {
                        out.push_str("[1,");
                        string(out, tag);
                        out.push_str(",[");
                        for (i, (name, value)) in attrs.iter().enumerate() {
                            if i != 0 {
                                out.push(',');
                            }
                            out.push('[');
                            string(out, name);
                            out.push(',');
                            string(out, value);
                            out.push(']');
                        }
                        out.push_str("],");
                        nodes(out, children);
                        out.push(']');
                    }
                }
            }
            out.push(']');
        }
        let mut out = String::new();
        out.push('[');
        nodes(&mut out, self.roots);
        out.push_str(",[");
        for (i, target) in self.targets.iter().enumerate() {
            if i != 0 {
                out.push(',');
            }
            out.push_str("[[");
            for (j, index) in target.path.iter().enumerate() {
                if j != 0 {
                    out.push(',');
                }
                write!(&mut out, "{index}").unwrap();
            }
            out.push_str("],");
            match target.kind {
                Kind::Text => out.push_str("0,null"),
                Kind::Attribute(name) => {
                    out.push_str("1,");
                    string(&mut out, name);
                }
                Kind::Click => out.push_str("2,null"),
                Kind::Children => out.push_str("3,null"),
                Kind::Range => out.push_str("4,null"),
                Kind::Class(name, _) => {
                    out.push_str("5,");
                    string(&mut out, name);
                }
            }
            out.push(']');
        }
        out.push_str("]]");
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_growth_does_not_add_dynamic_targets_and_wire_escapes_data() {
        static T: Template = Template {
            roots: &[Node::Element {
                tag: "div",
                attrs: &[("class", "a\"\\\n🙂")],
                children: &[
                    Node::Text(""),
                    Node::Element {
                        tag: "span",
                        attrs: &[],
                        children: &[Node::Text("static")],
                    },
                ],
            }],
            targets: &[Target {
                path: &[0, 0],
                kind: Kind::Text,
            }],
        };
        assert_eq!(T.validate(), Ok(4));
        assert_eq!(T.targets.len(), 1);
        assert!(T.wire().unwrap().contains("a\\\"\\\\\\u000a🙂"));
    }
    #[test]
    fn rejects_bad_paths_interactive_nesting_and_side_effecting_attributes() {
        let bad = Template {
            roots: &[Node::Element {
                tag: "button",
                attrs: &[],
                children: &[Node::Element {
                    tag: "button",
                    attrs: &[],
                    children: &[],
                }],
            }],
            targets: &[],
        };
        assert!(bad.validate().is_err());
        let bad = Template {
            roots: &[Node::Element {
                tag: "div",
                attrs: &[("onclick", "bad")],
                children: &[],
            }],
            targets: &[],
        };
        assert!(bad.validate().is_err());
        let bad = Template {
            roots: &[Node::Text("")],
            targets: &[Target {
                path: &[0],
                kind: Kind::Children,
            }],
        };
        assert!(bad.validate().is_err());
    }
}
