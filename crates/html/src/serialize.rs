use crate::{Kind, Part, Property, Shape, shape};
use glory_render::memory::Memory;
use glory_render::{Error, NodeId};

pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\0' => out.push('\u{fffd}'),
            _ => out.push(ch),
        }
    }
    out
}
pub fn serialize(tree: &Memory, key: impl Fn(NodeId) -> Option<u64>) -> Result<String, Error> {
    fn node(
        tree: &Memory,
        id: NodeId,
        key: &impl Fn(NodeId) -> Option<u64>,
        out: &mut String,
    ) -> Result<(), Error> {
        let model = tree.node(id)?;
        if model.suspended {
            return Ok(());
        }
        let key_value = key(id).ok_or(Error::NodeGone(id))?;
        let shape = shape(&model.descriptor)?;
        if shape.pieces.is_empty() {
            out.push_str(&format!("<!--gly:s:{key_value}-->"));
            for child in &model.children {
                node(tree, *child, key, out)?;
            }
            out.push_str(&format!("<!--gly:e:{key_value}-->"));
        } else {
            piece(&shape, Part::Main, key_value, out, &mut |out| {
                for child in &model.children {
                    node(tree, *child, key, out)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }
    fn piece(
        shape: &Shape,
        part: Part,
        key: u64,
        out: &mut String,
        children: &mut dyn FnMut(&mut String) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let item = shape
            .pieces
            .iter()
            .find(|piece| piece.part == part)
            .expect("HTML part");
        match &item.kind {
            Kind::Text(value) => {
                out.push_str(&format!("<!--gly:t:{key}-->"));
                out.push_str(&escape(value));
            }
            Kind::Element {
                tag,
                attributes,
                properties,
            } => {
                out.push('<');
                out.push_str(tag);
                out.push_str(&format!(
                    " data-gly-key=\"{key}\" data-gly-part=\"{}\"",
                    part.name()
                ));
                for (name, value) in attributes {
                    out.push(' ');
                    out.push_str(name);
                    out.push_str("=\"");
                    out.push_str(&escape(value));
                    out.push('"');
                }
                out.push('>');
                if matches!(*tag, "input" | "img") {
                    return Ok(());
                }
                if *tag == "textarea"
                    && let Some(Property::Text(value)) = properties.get("value")
                {
                    // HTML parsers drop one leading LF immediately after textarea.
                    if value.starts_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&escape(value));
                }
                if let Some(Property::RichDocument(document)) = properties.get("richDocument") {
                    out.push_str(&crate::rich_text::markup(document));
                }
                for child in shape
                    .pieces
                    .iter()
                    .filter(|piece| piece.parent == Some(part))
                {
                    piece(shape, child.part, key, out, &mut |_| Ok(()))?;
                }
                if shape.children == Some(part) {
                    children(out)?;
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }
        Ok(())
    }
    let mut out = String::new();
    for root in tree.roots() {
        node(tree, *root, &key, &mut out)?;
    }
    Ok(out)
}
