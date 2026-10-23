//! Finite rich-document data. Host markup and editing engines stay outside this model.
use crate::Error;
use std::collections::BTreeSet;

pub const MAX_BYTES: usize = 1_048_576;
pub const MAX_BLOCKS: usize = 8_192;
pub const MAX_SPANS: usize = 32_768;
pub const MAX_DEPTH: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text {
        text: String,
        marks: Marks,
    },
    Image {
        source: String,
        alternative: String,
        title: String,
    },
    HardBreak,
}
impl Inline {
    pub fn units(&self) -> usize {
        match self {
            Self::Text { text, .. } => text.encode_utf16().count(),
            _ => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    None,
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Root,
    Paragraph,
    Heading(u8),
    Quote,
    List(Option<u64>),
    Item(Option<bool>),
    Code(String),
    Rule,
    Table(Vec<Alignment>),
    TableHead,
    Row,
    Cell,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: u64,
    pub kind: Kind,
    pub spans: Vec<Inline>,
    pub children: Vec<Block>,
}
impl Block {
    pub fn find(&self, id: u64) -> Option<&Self> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }
    pub fn find_mut(&mut self, id: u64) -> Option<&mut Self> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
    pub fn units(&self) -> usize {
        self.spans.iter().map(Inline::units).sum()
    }
    pub fn editable(&self) -> bool {
        matches!(
            self.kind,
            Kind::Paragraph | Kind::Heading(_) | Kind::Item(_) | Kind::Code(_) | Kind::Cell
        ) && self.children.is_empty()
    }
    pub fn push(&mut self, value: Inline) {
        if let (
            Some(Inline::Text {
                text: previous,
                marks: old,
            }),
            Inline::Text { text, marks },
        ) = (self.spans.last_mut(), &value)
        {
            if old == marks {
                previous.push_str(text);
                return;
            }
        }
        self.spans.push(value);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub block: u64,
    pub offset: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Point,
    pub focus: Point,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub root: Block,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            root: Block {
                id: 0,
                kind: Kind::Root,
                spans: vec![],
                children: vec![Block {
                    id: 1,
                    kind: Kind::Paragraph,
                    spans: vec![],
                    children: vec![],
                }],
            },
        }
    }
}
fn invalid(message: &'static str) -> Error {
    Error::InvalidProps(message)
}
fn target(value: &str, image: bool) -> bool {
    if value.is_empty()
        || value.len() > 8192
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || (!image && lower.starts_with("mailto:"))
        || (image && (lower.starts_with("blob:") || lower.starts_with("asset://")))
        || (value.starts_with('/') && !value.starts_with("//"))
}
fn canonical(value: &str) -> bool {
    !value.contains(['\r', '\0'])
}
fn child_allowed(parent: &Kind, child: &Kind) -> bool {
    match parent {
        Kind::Root | Kind::Quote | Kind::Item(_) => matches!(
            child,
            Kind::Paragraph
                | Kind::Heading(_)
                | Kind::Quote
                | Kind::List(_)
                | Kind::Code(_)
                | Kind::Rule
                | Kind::Table(_)
        ),
        Kind::List(_) => matches!(child, Kind::Item(_)),
        Kind::Table(_) => matches!(child, Kind::TableHead | Kind::Row),
        Kind::TableHead | Kind::Row => matches!(child, Kind::Cell),
        _ => false,
    }
}
impl Document {
    /// Bounded descriptor-local validation; never reads host nodes or calls business code.
    pub fn validate(&self) -> Result<(), Error> {
        if !matches!(self.root.kind, Kind::Root)
            || self.root.id != 0
            || self.root.children.is_empty()
        {
            return Err(invalid("rich document requires a nonempty root"));
        }
        let mut stack = vec![(&self.root, 0_usize)];
        let mut ids = BTreeSet::new();
        let mut bytes = 0_usize;
        let mut spans = 0_usize;
        let mut editable = false;
        while let Some((block, depth)) = stack.pop() {
            editable |= block.editable();
            if depth > MAX_DEPTH || !ids.insert(block.id) || ids.len() > MAX_BLOCKS {
                return Err(invalid(
                    "rich document structure exceeds its limits or repeats an identity",
                ));
            }
            if matches!(block.kind, Kind::Heading(level) if !(1..=6).contains(&level)) {
                return Err(invalid("invalid rich heading level"));
            }
            if matches!(&block.kind, Kind::Table(columns) if columns.is_empty() || columns.len()>64)
            {
                return Err(invalid("invalid rich table width"));
            }
            if let Kind::Table(columns) = &block.kind {
                if block.children.is_empty()
                    || block.children.iter().enumerate().any(|(i, row)| {
                        row.children.len() != columns.len()
                            || (matches!(row.kind, Kind::TableHead) && i != 0)
                    })
                {
                    return Err(invalid(
                        "rich table rows must match their columns with at most one initial header",
                    ));
                }
            }
            if let Kind::Code(language) = &block.kind {
                bytes = bytes.saturating_add(language.len());
                if !canonical(language) || language.chars().any(char::is_control) {
                    return Err(invalid("invalid code language"));
                }
            }
            if !block.spans.is_empty() && (!block.editable() || !block.children.is_empty()) {
                return Err(invalid("rich text spans require an editable leaf"));
            }
            if ids
                .len()
                .saturating_add(stack.len())
                .saturating_add(block.children.len())
                > MAX_BLOCKS
            {
                return Err(invalid(
                    "rich document pending structure exceeds its budget",
                ));
            }
            for child in block.children.iter().rev() {
                if !child_allowed(&block.kind, &child.kind) {
                    return Err(invalid("invalid rich document child"));
                }
                stack.push((child, depth + 1));
            }
            spans = spans.saturating_add(block.spans.len());
            for span in &block.spans {
                match span {
                    Inline::Text { text, marks } => {
                        if !canonical(text)
                            || marks.link.as_deref().is_some_and(|url| !target(url, false))
                        {
                            return Err(invalid("invalid rich text or link"));
                        }
                        if matches!(block.kind, Kind::Code(_)) && *marks != Marks::default() {
                            return Err(invalid("code blocks cannot carry inline marks"));
                        }
                        bytes = bytes
                            .saturating_add(text.len())
                            .saturating_add(marks.link.as_ref().map_or(0, String::len));
                    }
                    Inline::Image {
                        source,
                        alternative,
                        title,
                    } => {
                        if !target(source, true)
                            || !canonical(alternative)
                            || !canonical(title)
                            || matches!(block.kind, Kind::Code(_))
                        {
                            return Err(invalid("invalid rich image"));
                        }
                        bytes = bytes
                            .saturating_add(source.len())
                            .saturating_add(alternative.len())
                            .saturating_add(title.len());
                    }
                    Inline::HardBreak => {}
                }
            }
            if bytes > MAX_BYTES || spans > MAX_SPANS {
                return Err(invalid("rich document content exceeds its limits"));
            }
        }
        if !editable {
            return Err(invalid("rich document requires an editable leaf"));
        }
        Ok(())
    }
    pub fn validate_selection(&self, selection: Selection) -> Result<(), Error> {
        for point in [selection.anchor, selection.focus] {
            let block = self
                .root
                .find(point.block)
                .ok_or_else(|| invalid("rich selection block is absent"))?;
            if !block.editable() {
                return Err(invalid("rich selection requires an editable leaf"));
            }
            let mut remaining = point.offset;
            let mut valid = remaining == 0;
            for span in &block.spans {
                let units = span.units();
                if remaining <= units {
                    valid = match span {
                        Inline::Text { text, .. } => {
                            let mut count = 0;
                            remaining == 0
                                || text.chars().any(|c| {
                                    count += c.len_utf16();
                                    count == remaining
                                })
                        }
                        _ => true,
                    };
                    break;
                }
                remaining -= units;
            }
            if !valid {
                return Err(invalid(
                    "rich selection splits a surrogate or exceeds its leaf",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_document_rejects_duplicate_ids_bad_structure_targets_and_noncanonical_text() {
        Document::default().validate().unwrap();
        let mut document = Document::default();
        document
            .root
            .children
            .push(document.root.children[0].clone());
        assert!(document.validate().is_err());
        for value in ["bad\rtext", "bad\0text"] {
            let mut d = Document::default();
            d.root.children[0].push(Inline::Text {
                text: value.into(),
                marks: Marks::default(),
            });
            assert!(d.validate().is_err());
        }
        for url in [
            "javascript:alert(1)",
            "data:text/html,hello",
            "//evil.example",
            "https://good.example\n",
        ] {
            let mut d = Document::default();
            d.root.children[0].push(Inline::Text {
                text: "link".into(),
                marks: Marks {
                    link: Some(url.into()),
                    ..Default::default()
                },
            });
            assert!(d.validate().is_err());
        }
        let mut d = Document::default();
        d.root.children[0].kind = Kind::Cell;
        assert!(d.validate().is_err());
    }
    #[test]
    fn selections_use_utf16_in_leaf_identity_and_one_object_unit_for_images() {
        let mut d = Document::default();
        d.root.children[0].spans = vec![
            Inline::Text {
                text: "A😀".into(),
                marks: Marks::default(),
            },
            Inline::Image {
                source: "blob:https://example.com/current".into(),
                alternative: "image".into(),
                title: String::new(),
            },
        ];
        d.validate().unwrap();
        for offset in [0, 1, 3, 4] {
            let point = Point { block: 1, offset };
            d.validate_selection(Selection {
                anchor: point,
                focus: point,
            })
            .unwrap();
        }
        for (block, offset) in [(1, 2), (1, 5), (0, 0), (17, 0)] {
            let point = Point { block, offset };
            assert!(
                d.validate_selection(Selection {
                    anchor: point,
                    focus: point
                })
                .is_err()
            );
        }
    }
}

/// Finite rich-editing shortcuts, dispatched with the current complete document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Undo,
    Redo,
    Bold,
    Italic,
    Strike,
    Code,
}
impl Command {
    pub fn shortcut(key: &str, chord: bool, shift: bool, alt: bool) -> Option<Self> {
        if !chord || alt {
            return None;
        }
        match key.to_ascii_lowercase().as_str() {
            "z" => Some(if shift { Self::Redo } else { Self::Undo }),
            "y" if !shift => Some(Self::Redo),
            "b" if !shift => Some(Self::Bold),
            "i" if !shift => Some(Self::Italic),
            "x" if shift => Some(Self::Strike),
            "e" if !shift => Some(Self::Code),
            _ => None,
        }
    }
}

impl Marks {
    pub fn validate(&self) -> Result<(), Error> {
        if self.link.as_deref().is_some_and(|url| !target(url, false)) {
            Err(invalid("invalid typing link"))
        } else {
            Ok(())
        }
    }
}

impl Document {
    /// Text alternative for an accessible editor value, with image alternatives.
    pub fn plain_text(&self) -> String {
        fn write(block: &Block, out: &mut String, first: &mut bool) {
            if block.editable() {
                if !*first {
                    out.push('\n');
                }
                *first = false;
                for span in &block.spans {
                    match span {
                        Inline::Text { text, .. } => out.push_str(text),
                        Inline::HardBreak => out.push('\n'),
                        Inline::Image { alternative, .. } => out.push_str(alternative),
                    }
                }
            }
            for child in &block.children {
                write(child, out, first);
            }
        }
        let mut out = String::new();
        write(&self.root, &mut out, &mut true);
        out
    }
}

impl Selection {
    /// Document-independent bounds for asynchronous host query replies.
    pub fn validate_bounds(&self) -> Result<(), Error> {
        if [self.anchor, self.focus]
            .iter()
            .any(|point| point.block == 0 || point.offset > MAX_BYTES + MAX_SPANS)
        {
            return Err(invalid("rich selection identity or offset exceeds limits"));
        }
        Ok(())
    }
}
