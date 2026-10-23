//! Optional host wire codec. The core semantic model has no serde dependency.
use glory_render::{Error, rich_text::*};
use serde_json::{Value, json};

pub fn encode(document: &Document) -> Value {
    fn block(value: &Block) -> Value {
        let kind = match &value.kind {
            Kind::Root => json!(["root"]),
            Kind::Paragraph => json!(["paragraph"]),
            Kind::Heading(level) => json!(["heading", level]),
            Kind::Quote => json!(["quote"]),
            Kind::List(start) => json!(["list", start.map(|v| v.to_string())]),
            Kind::Item(checked) => json!(["item", checked]),
            Kind::Code(language) => json!(["code", language]),
            Kind::Rule => json!(["rule"]),
            Kind::Table(alignments) => json!([
                "table",
                alignments
                    .iter()
                    .map(|a| match a {
                        Alignment::None => "none",
                        Alignment::Left => "left",
                        Alignment::Center => "center",
                        Alignment::Right => "right",
                    })
                    .collect::<Vec<_>>()
            ]),
            Kind::TableHead => json!(["head"]),
            Kind::Row => json!(["row"]),
            Kind::Cell => json!(["cell"]),
        };
        let spans = value.spans.iter().map(|span|match span {
            Inline::Text {text,marks} => json!({"text":text,"marks":{"bold":marks.bold,"italic":marks.italic,"strike":marks.strike,"code":marks.code,"link":marks.link}}),
            Inline::Image {source,alternative,title} => json!({"image":{"source":source,"alternative":alternative,"title":title}}),
            Inline::HardBreak => json!({"break":true}),
        }).collect::<Vec<_>>();
        json!({"id":value.id.to_string(),"kind":kind,"spans":spans,"children":value.children.iter().map(block).collect::<Vec<_>>()})
    }
    json!({"root":block(&document.root)})
}
fn invalid() -> Error {
    Error::InvalidProps("invalid finite rich-document wire value")
}
fn string(value: &Value) -> Result<String, Error> {
    value.as_str().map(str::to_owned).ok_or_else(invalid)
}
fn boolean(value: &Value) -> Result<bool, Error> {
    value.as_bool().ok_or_else(invalid)
}
pub fn decode(value: &Value) -> Result<Document, Error> {
    fn block(
        value: &Value,
        depth: usize,
        count: &mut usize,
        span_count: &mut usize,
        bytes: &mut usize,
    ) -> Result<Block, Error> {
        *count += 1;
        if depth > MAX_DEPTH || *count > MAX_BLOCKS {
            return Err(invalid());
        }
        let id = value["id"]
            .as_str()
            .ok_or_else(invalid)?
            .parse()
            .map_err(|_| invalid())?;
        let tag = value["kind"].as_array().ok_or_else(invalid)?;
        let arg = tag.get(1).unwrap_or(&Value::Null);
        let kind = match tag.first().and_then(Value::as_str).ok_or_else(invalid)? {
            "root" => Kind::Root,
            "paragraph" => Kind::Paragraph,
            "heading" => Kind::Heading(
                arg.as_u64()
                    .and_then(|v| u8::try_from(v).ok())
                    .ok_or_else(invalid)?,
            ),
            "quote" => Kind::Quote,
            "list" => Kind::List(if arg.is_null() {
                None
            } else {
                Some(
                    arg.as_str()
                        .ok_or_else(invalid)?
                        .parse()
                        .map_err(|_| invalid())?,
                )
            }),
            "item" => Kind::Item(if arg.is_null() {
                None
            } else {
                Some(boolean(arg)?)
            }),
            "code" => Kind::Code(string(arg)?),
            "rule" => Kind::Rule,
            "table" => Kind::Table(
                arg.as_array()
                    .ok_or_else(invalid)?
                    .iter()
                    .map(|v| match v.as_str() {
                        Some("none") => Ok(Alignment::None),
                        Some("left") => Ok(Alignment::Left),
                        Some("center") => Ok(Alignment::Center),
                        Some("right") => Ok(Alignment::Right),
                        _ => Err(invalid()),
                    })
                    .collect::<Result<_, _>>()?,
            ),
            "head" => Kind::TableHead,
            "row" => Kind::Row,
            "cell" => Kind::Cell,
            _ => return Err(invalid()),
        };
        let raw_spans = value["spans"].as_array().ok_or_else(invalid)?;
        let raw_children = value["children"].as_array().ok_or_else(invalid)?;
        if count.saturating_add(raw_children.len()) > MAX_BLOCKS
            || span_count.saturating_add(raw_spans.len()) > MAX_SPANS
        {
            return Err(invalid());
        }
        *span_count += raw_spans.len();
        let mut spans = Vec::with_capacity(raw_spans.len());
        for span in raw_spans {
            let span = if let Some(text) = span.get("text") {
                let text = string(text)?;
                let m = &span["marks"];
                let link = if m["link"].is_null() {
                    None
                } else {
                    Some(string(&m["link"])?)
                };
                *bytes = bytes
                    .saturating_add(text.len())
                    .saturating_add(link.as_ref().map_or(0, String::len));
                Inline::Text {
                    text,
                    marks: Marks {
                        bold: boolean(&m["bold"])?,
                        italic: boolean(&m["italic"])?,
                        strike: boolean(&m["strike"])?,
                        code: boolean(&m["code"])?,
                        link,
                    },
                }
            } else if let Some(image) = span.get("image") {
                let source = string(&image["source"])?;
                let alternative = string(&image["alternative"])?;
                let title = string(&image["title"])?;
                *bytes = bytes
                    .saturating_add(source.len())
                    .saturating_add(alternative.len())
                    .saturating_add(title.len());
                Inline::Image {
                    source,
                    alternative,
                    title,
                }
            } else if span.get("break") == Some(&Value::Bool(true)) {
                Inline::HardBreak
            } else {
                return Err(invalid());
            };
            if *bytes > MAX_BYTES {
                return Err(invalid());
            }
            spans.push(span);
        }
        let children = raw_children
            .iter()
            .map(|v| block(v, depth + 1, count, span_count, bytes))
            .collect::<Result<_, _>>()?;
        Ok(Block {
            id,
            kind,
            spans,
            children,
        })
    }
    let document = Document {
        root: block(&value["root"], 0, &mut 0, &mut 0, &mut 0)?,
    };
    document.validate()?;
    Ok(document)
}
pub fn encode_selection(value: Selection) -> Value {
    json!({"anchor":{"block":value.anchor.block.to_string(),"offset":value.anchor.offset},"focus":{"block":value.focus.block.to_string(),"offset":value.focus.offset}})
}
pub fn decode_selection(value: &Value) -> Result<Selection, Error> {
    fn point(value: &Value) -> Result<Point, Error> {
        Ok(Point {
            block: value["block"]
                .as_str()
                .ok_or_else(invalid)?
                .parse()
                .map_err(|_| invalid())?,
            offset: value["offset"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(invalid)?,
        })
    }
    Ok(Selection {
        anchor: point(&value["anchor"])?,
        focus: point(&value["focus"])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_and_selection_round_trip_full_width_identities() {
        let mut doc = Document::default();
        doc.root.children[0].id = u64::MAX;
        doc.root.children[0].push(Inline::Text {
            text: "中😀".into(),
            marks: Marks {
                bold: true,
                ..Default::default()
            },
        });
        assert_eq!(decode(&encode(&doc)).unwrap(), doc);
        let p = Point {
            block: u64::MAX,
            offset: 3,
        };
        let s = Selection {
            anchor: p,
            focus: p,
        };
        assert_eq!(decode_selection(&encode_selection(s)).unwrap(), s);
    }
    #[test]
    fn invalid_host_document_does_not_enter_the_semantic_model() {
        let mut v = encode(&Document::default());
        v["root"]["children"][0]["spans"] =
            json!([{"image":{"source":"javascript:bad","alternative":"","title":""}}]);
        assert!(decode(&v).is_err());
    }
}
