//! Finite document HTML. All strings are escaped, never interpreted as markup.
use crate::escape;
use glory_render::rich_text::{Block, Document, Inline, Kind};

#[cfg(feature = "wire")]
pub mod wire;

pub fn markup(document: &Document) -> String {
    let mut out = String::new();
    for child in &document.root.children {
        block(child, &mut out, &[]);
    }
    out
}
fn block(value: &Block, out: &mut String, columns: &[glory_render::rich_text::Alignment]) {
    let tag = match &value.kind {
        Kind::Root => "div",
        Kind::Paragraph => "p",
        Kind::Heading(level) => match level {
            1 => "h1",
            2 => "h2",
            3 => "h3",
            4 => "h4",
            5 => "h5",
            _ => "h6",
        },
        Kind::Quote => "blockquote",
        Kind::List(Some(_)) => "ol",
        Kind::List(None) => "ul",
        Kind::Item(_) => "li",
        Kind::Code(_) => "pre",
        Kind::Rule => "hr",
        Kind::Table(_) => "table",
        Kind::TableHead | Kind::Row => "tr",
        Kind::Cell => "td",
    };
    out.push_str(&format!("<{tag} data-gly-rich-block=\"{}\"", value.id));
    match &value.kind {
        Kind::List(Some(start)) => out.push_str(&format!(" start=\"{start}\"")),
        Kind::Item(Some(checked)) => out.push_str(&format!(" data-gly-task=\"{checked}\"")),
        Kind::Code(language) => {
            out.push_str(&format!(" data-gly-language=\"{}\"", escape(language)))
        }
        Kind::TableHead => out.push_str(" data-gly-table-head=\"\""),
        Kind::Table(alignments) => out.push_str(&format!(
            " data-gly-align=\"{}\"",
            alignments
                .iter()
                .map(|a| match a {
                    glory_render::rich_text::Alignment::None => "n",
                    glory_render::rich_text::Alignment::Left => "l",
                    glory_render::rich_text::Alignment::Center => "c",
                    glory_render::rich_text::Alignment::Right => "r",
                })
                .collect::<String>()
        )),
        _ => {}
    }
    if matches!(value.kind, Kind::Cell)
        && let Some(align) = columns.first()
    {
        let align = match align {
            glory_render::rich_text::Alignment::Left => "left",
            glory_render::rich_text::Alignment::Center => "center",
            glory_render::rich_text::Alignment::Right => "right",
            _ => "start",
        };
        out.push_str(&format!(" style=\"text-align:{align}\""));
    }
    out.push('>');
    if matches!(value.kind, Kind::Rule) {
        return;
    }
    if let Kind::Item(Some(checked)) = value.kind {
        out.push_str(if checked {"<input type=\"checkbox\" checked data-gly-task-control=\"\" aria-label=\"Task completed\" contenteditable=\"false\">"}else{"<input type=\"checkbox\" data-gly-task-control=\"\" aria-label=\"Task completed\" contenteditable=\"false\">"});
    }
    for span in &value.spans {
        match span {
            Inline::Text { text, marks } => {
                if let Some(link) = &marks.link {
                    out.push_str(&format!(
                        "<a href=\"{}\" rel=\"noreferrer noopener\">",
                        escape(link)
                    ));
                }
                if marks.bold {
                    out.push_str("<strong>");
                }
                if marks.italic {
                    out.push_str("<em>");
                }
                if marks.strike {
                    out.push_str("<s>");
                }
                if marks.code {
                    out.push_str("<code>");
                }
                out.push_str(&escape(text));
                if marks.code {
                    out.push_str("</code>");
                }
                if marks.strike {
                    out.push_str("</s>");
                }
                if marks.italic {
                    out.push_str("</em>");
                }
                if marks.bold {
                    out.push_str("</strong>");
                }
                if marks.link.is_some() {
                    out.push_str("</a>");
                }
            }
            Inline::Image {
                source,
                alternative,
                title,
            } => out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\" title=\"{}\" contenteditable=\"false\">",
                escape(source),
                escape(alternative),
                escape(title)
            )),
            Inline::HardBreak => out.push_str("<br>"),
        }
    }
    if value.editable() && value.spans.is_empty() {
        out.push_str("<br data-gly-rich-empty=\"\">");
    }
    for (index, child) in value.children.iter().enumerate() {
        let columns = match &value.kind {
            Kind::Table(align) => align.as_slice(),
            Kind::Row | Kind::TableHead => columns.get(index..index + 1).unwrap_or(&[]),
            _ => &[],
        };
        block(child, out, columns);
    }
    out.push_str(&format!("</{tag}>"));
}
