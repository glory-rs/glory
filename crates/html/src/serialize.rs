use crate::{Kind, Part, Property, Shape};
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
        let shape = crate::shape_in(tree, id)?;
        if shape.pieces.is_empty() {
            out.push_str(&format!("<!--gly:s:{key_value}-->"));
            for child in &model.children {
                node(tree, *child, key, out)?;
            }
            out.push_str(&format!("<!--gly:e:{key_value}-->"));
        } else {
            piece(&shape, shape.root_part()?, key_value, out, &mut |out| {
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
                if let Some(Property::RichDocument(document)) = properties.get("richDocument") {
                    out.push_str(&rich_document(document));
                }
                if *tag == "select"
                    && let Some(options) = &shape.select_options
                {
                    let keys = match properties.get("selectedKeys") {
                        Some(Property::SelectedKeys(keys)) => keys.as_slice(),
                        _ => &[],
                    };
                    out.push_str(&select_options(options, keys));
                }
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

/// Serialize the finite intrinsic editor document; never accepts raw markup.
pub fn rich_document(document: &glory_render::RichDocument) -> String {
    use glory_render::RichBlockKind;
    let mut out = String::new();
    for block in &document.blocks {
        let tag = match block.kind {
            RichBlockKind::Paragraph => "p",
            RichBlockKind::Heading(1) => "h1",
            RichBlockKind::Heading(2) => "h2",
            RichBlockKind::Heading(3) => "h3",
            RichBlockKind::Heading(4) => "h4",
            RichBlockKind::Heading(5) => "h5",
            RichBlockKind::Heading(6) => "h6",
            RichBlockKind::Quote => "blockquote",
            RichBlockKind::Heading(_) => unreachable!("validated rich heading"),
        };
        out.push('<');
        out.push_str(tag);
        out.push('>');
        for run in &block.runs {
            if let Some(link) = &run.marks.link {
                out.push_str("<a href=\"");
                out.push_str(&escape(link));
                out.push_str("\">");
            }
            if run.marks.bold {
                out.push_str("<strong>");
            }
            if run.marks.italic {
                out.push_str("<em>");
            }
            if run.marks.code {
                out.push_str("<code>");
            }
            out.push_str(&escape(&run.text));
            if run.marks.code {
                out.push_str("</code>");
            }
            if run.marks.italic {
                out.push_str("</em>");
            }
            if run.marks.bold {
                out.push_str("</strong>");
            }
            if run.marks.link.is_some() {
                out.push_str("</a>");
            }
        }
        if block.runs.iter().all(|run| run.text.is_empty()) {
            out.push_str("<br>");
        }
        out.push_str("</");
        out.push_str(tag);
        out.push('>');
    }
    out
}

/// Serialize only the finite intrinsic children of a native select.
pub fn select_options(config: &crate::SelectOptions, keys: &[String]) -> String {
    fn option(value: &glory_render::SelectOption, keys: &[String], out: &mut String) {
        out.push_str("<option data-gly-option=\"");
        out.push_str(&escape(value.key.as_str()));
        out.push_str("\" value=\"");
        out.push_str(&escape(&value.value));
        out.push('"');
        if value.disabled {
            out.push_str(" disabled");
        }
        if keys.iter().any(|key| key == value.key.as_str()) {
            out.push_str(" selected");
        }
        out.push('>');
        out.push_str(&escape(&value.label));
        out.push_str("</option>");
    }
    let mut out = String::new();
    if let Some(placeholder) = &config.placeholder {
        out.push_str("<option data-gly-placeholder=\"\" value=\"\" disabled hidden");
        if keys.is_empty() {
            out.push_str(" selected");
        }
        out.push('>');
        out.push_str(&escape(placeholder));
        out.push_str("</option>");
    }
    for entry in config.entries.iter() {
        match entry {
            glory_render::SelectEntry::Option(value) => option(value, keys, &mut out),
            glory_render::SelectEntry::Group(group) => {
                out.push_str("<optgroup label=\"");
                out.push_str(&escape(&group.label));
                out.push('"');
                if group.disabled {
                    out.push_str(" disabled");
                }
                out.push('>');
                for value in &group.options {
                    option(value, keys, &mut out);
                }
                out.push_str("</optgroup>");
            }
        }
    }
    out
}
