//! Finite rich-document wire projection; application values never carry markup.
use super::*;
use serde::Deserialize;

pub(super) fn document(doc: &RichDocument) -> Value {
    Value::Array(doc.blocks.iter().map(|b| {
        let tag=match b.kind {RichBlockKind::Paragraph=>"p".into(),RichBlockKind::Quote=>"blockquote".into(),RichBlockKind::Heading(level)=>format!("h{level}")};
        let runs=b.runs.iter().map(|r| json!({"text":r.text,"bold":r.marks.bold,"italic":r.marks.italic,"code":r.marks.code,"link":r.marks.link})).collect::<Vec<_>>();
        json!({"tag":tag,"runs":runs})
    }).collect())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    blocks: Vec<Block>,
    selection: Selection,
    revision: String,
    composing: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    tag: String,
    runs: Vec<Run>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    link: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    anchor: Point,
    focus: Point,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    block: usize,
    offset: usize,
}
impl From<Point> for RichPoint {
    fn from(p: Point) -> Self {
        Self {
            block: p.block,
            offset: p.offset,
        }
    }
}
impl Snapshot {
    pub(super) fn decode(self) -> Result<Box<RichSnapshot>, Error> {
        let blocks = self
            .blocks
            .into_iter()
            .map(|block| {
                let kind = match block.tag.as_str() {
                    "p" => RichBlockKind::Paragraph,
                    "blockquote" => RichBlockKind::Quote,
                    tag if matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
                        RichBlockKind::Heading(tag.as_bytes()[1] - b'0')
                    }
                    _ => return Err(Error::InvalidEvent("invalid Wry rich block")),
                };
                Ok(RichBlock {
                    kind,
                    runs: block
                        .runs
                        .into_iter()
                        .map(|r| RichRun {
                            text: r.text,
                            marks: RichMarks {
                                bold: r.bold,
                                italic: r.italic,
                                code: r.code,
                                link: r.link,
                            },
                        })
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let snapshot = Box::new(RichSnapshot {
            document: Rc::new(RichDocument::new(blocks)?),
            selection: RichSelection {
                anchor: self.selection.anchor.into(),
                focus: self.selection.focus.into(),
            },
            revision: protocol::integer(&self.revision)?,
            composing: self.composing,
        });
        snapshot.validate()?;
        Ok(snapshot)
    }
}
pub(super) fn selection(s: RichSelection) -> Value {
    let point = |p: RichPoint| json!({"block":p.block,"offset":p.offset});
    json!({"anchor":point(s.anchor),"focus":point(s.focus)})
}
