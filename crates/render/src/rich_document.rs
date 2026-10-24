//! Finite rich document model, independent of HTML and host editing APIs.
//! Shared value/selection model used by the independently driven host editor.
use crate::Error;

pub const MAX_RICH_BLOCKS: usize = 128;
pub const MAX_RICH_RUNS: usize = 2048;
pub const MAX_RICH_TEXT_BYTES: usize = 65_536;
pub const MAX_RICH_LINK_BYTES: usize = 2048;
pub const MAX_RICH_METADATA_BYTES: usize = 131_072;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RichBlockKind {
    #[default]
    Paragraph,
    Heading(u8),
    Quote,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichMarks {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichRun {
    pub text: String,
    pub marks: RichMarks,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichBlock {
    pub kind: RichBlockKind,
    pub runs: Vec<RichRun>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichDocument {
    pub blocks: Vec<RichBlock>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RichPoint {
    pub block: usize,
    pub offset: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RichSelection {
    pub anchor: RichPoint,
    pub focus: RichPoint,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichFormat {
    Bold(bool),
    Italic(bool),
    Code(bool),
    Link(Option<String>),
    Block(RichBlockKind),
}
impl RichFormat {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Link(Some(link)) => validate_link(link),
            Self::Block(value) => kind(*value),
            _ => Ok(()),
        }
    }
}
impl Default for RichDocument {
    fn default() -> Self {
        Self {
            blocks: vec![RichBlock {
                kind: RichBlockKind::Paragraph,
                runs: vec![RichRun::default()],
            }],
        }
    }
}
fn invalid(message: &'static str) -> Error {
    Error::InvalidProps(message)
}
fn validate_link(link: &str) -> Result<(), Error> {
    if link.is_empty()
        || link.len() > MAX_RICH_LINK_BYTES
        || link.chars().any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(invalid(
            "rich link requires a finite http/https/mailto target",
        ));
    }
    let valid = link.split_once(':').is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && ["http", "https", "mailto"]
                .iter()
                .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    });
    if !valid {
        return Err(invalid(
            "rich link requires a finite http/https/mailto target",
        ));
    }
    Ok(())
}
impl RichMarks {
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(link) = &self.link {
            validate_link(link)?;
        }
        Ok(())
    }
    fn apply(&mut self, format: &RichFormat) {
        match format {
            RichFormat::Bold(value) => self.bold = *value,
            RichFormat::Italic(value) => self.italic = *value,
            RichFormat::Code(value) => self.code = *value,
            RichFormat::Link(value) => self.link = value.clone(),
            RichFormat::Block(_) => {}
        }
    }
}
fn kind(kind: RichBlockKind) -> Result<(), Error> {
    if matches!(kind,RichBlockKind::Heading(level) if !(1..=6).contains(&level)) {
        return Err(invalid("rich heading level must be 1..=6"));
    }
    Ok(())
}
fn byte_offset(text: &str, utf16: usize) -> Result<usize, Error> {
    let mut offset = 0;
    for (byte, ch) in text.char_indices() {
        if offset == utf16 {
            return Ok(byte);
        }
        offset += ch.len_utf16();
        if offset > utf16 {
            return Err(invalid("rich selection splits a UTF-16 surrogate pair"));
        }
    }
    if offset == utf16 {
        Ok(text.len())
    } else {
        Err(invalid("rich selection exceeds its block"))
    }
}
impl RichBlock {
    pub fn utf16_len(&self) -> usize {
        self.runs
            .iter()
            .map(|run| run.text.encode_utf16().count())
            .sum()
    }
    fn point(&self, offset: usize) -> Result<(), Error> {
        let mut remaining = offset;
        for run in &self.runs {
            let length = run.text.encode_utf16().count();
            if remaining <= length {
                byte_offset(&run.text, remaining)?;
                return Ok(());
            }
            remaining -= length;
        }
        Err(invalid("rich selection exceeds its block"))
    }
    fn slice(&self, start: usize, end: usize) -> Result<Vec<RichRun>, Error> {
        let mut output = Vec::new();
        let mut cursor = 0;
        for run in &self.runs {
            let length = run.text.encode_utf16().count();
            let left = start.saturating_sub(cursor).min(length);
            let right = end.saturating_sub(cursor).min(length);
            if left < right {
                output.push(RichRun {
                    text: run.text[byte_offset(&run.text, left)?..byte_offset(&run.text, right)?]
                        .into(),
                    marks: run.marks.clone(),
                });
            }
            cursor += length;
        }
        Ok(output)
    }
}
impl RichSelection {
    pub fn ordered(self) -> (RichPoint, RichPoint) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
    pub fn collapsed(point: RichPoint) -> Self {
        Self {
            anchor: point,
            focus: point,
        }
    }
    pub fn is_collapsed(self) -> bool {
        self.anchor == self.focus
    }
}
impl RichDocument {
    /// Canonicalize only an already bounded owned declaration. No business
    /// iterator/source is collected or scanned by this constructor.
    pub fn new(blocks: Vec<RichBlock>) -> Result<Self, Error> {
        let mut doc = Self { blocks };
        doc.validate_limits()?;
        for block in &mut doc.blocks {
            let mut runs: Vec<RichRun> = Vec::with_capacity(block.runs.len());
            for run in std::mem::take(&mut block.runs) {
                if run.text.is_empty() {
                    continue;
                }
                if let Some(previous) = runs
                    .last_mut()
                    .filter(|previous| previous.marks == run.marks)
                {
                    previous.text.push_str(&run.text);
                } else {
                    runs.push(run);
                }
            }
            if runs.is_empty() {
                runs.push(RichRun::default());
            }
            block.runs = runs;
        }
        doc.validate()?;
        Ok(doc)
    }
    fn validate_limits(&self) -> Result<(), Error> {
        if self.blocks.is_empty() || self.blocks.len() > MAX_RICH_BLOCKS {
            return Err(invalid("rich document requires 1..=128 blocks"));
        }
        let mut runs = 0usize;
        let mut text = 0usize;
        let mut metadata = 0usize;
        for block in &self.blocks {
            kind(block.kind)?;
            runs = runs
                .checked_add(block.runs.len())
                .ok_or_else(|| invalid("rich run count overflow"))?;
            if block.runs.is_empty() || runs > MAX_RICH_RUNS {
                return Err(invalid("rich document requires bounded nonempty runs"));
            }
            for run in &block.runs {
                if run.text.contains(['\0', '\r', '\n']) {
                    return Err(invalid("rich runs use canonical single-paragraph text"));
                }
                run.marks.validate()?;
                text = text
                    .checked_add(run.text.len())
                    .ok_or_else(|| invalid("rich text budget overflow"))?;
                metadata = metadata
                    .checked_add(run.marks.link.as_ref().map_or(0, String::len))
                    .ok_or_else(|| invalid("rich metadata overflow"))?;
                if text > MAX_RICH_TEXT_BYTES || metadata > MAX_RICH_METADATA_BYTES {
                    return Err(invalid("rich document text/metadata budget"));
                }
            }
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.validate_limits()?;
        for block in &self.blocks {
            if block.runs.iter().any(|run| {
                run.text.is_empty() && (block.runs.len() != 1 || run.marks != RichMarks::default())
            }) || block
                .runs
                .windows(2)
                .any(|pair| pair[0].marks == pair[1].marks)
            {
                return Err(invalid("rich runs must be canonically merged"));
            }
        }
        Ok(())
    }
    pub fn validate_selection(&self, selection: RichSelection) -> Result<(), Error> {
        for point in [selection.anchor, selection.focus] {
            self.blocks
                .get(point.block)
                .ok_or_else(|| invalid("rich selection block is missing"))?
                .point(point.offset)?;
        }
        Ok(())
    }
    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for (index, block) in self.blocks.iter().enumerate() {
            if index != 0 {
                text.push('\n');
            }
            for run in &block.runs {
                text.push_str(&run.text);
            }
        }
        text
    }
    /// Charged document payload and finite run/block bookkeeping. This is not
    /// an RSS/allocator-capacity claim, nor a budget for business-owned sources.
    pub fn charged_bytes(&self) -> usize {
        self.blocks.len() * std::mem::size_of::<RichBlock>()
            + self
                .blocks
                .iter()
                .map(|block| {
                    block.runs.len() * std::mem::size_of::<RichRun>()
                        + block
                            .runs
                            .iter()
                            .map(|run| {
                                run.text.len() + run.marks.link.as_ref().map_or(0, String::len)
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
    pub fn replace(
        &self,
        selection: RichSelection,
        text: &str,
        marks: RichMarks,
    ) -> Result<(Self, RichSelection), Error> {
        self.validate()?;
        self.validate_selection(selection)?;
        marks.validate()?;
        if text.len() > MAX_RICH_TEXT_BYTES {
            return Err(invalid("rich insertion exceeds text budget"));
        }
        let text = crate::normalize_text(text, true);
        let lines = text.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let (start, end) = selection.ordered();
        let blocks = self.blocks.len() - (end.block - start.block) + lines - 1;
        if blocks > MAX_RICH_BLOCKS {
            return Err(invalid("rich insertion exceeds block budget"));
        }
        let mut output = Vec::with_capacity(blocks);
        output.extend(self.blocks[..start.block].iter().cloned());
        let prefix = self.blocks[start.block].slice(0, start.offset)?;
        let suffix =
            self.blocks[end.block].slice(end.offset, self.blocks[end.block].utf16_len())?;
        let mut inserted = text.split('\n').peekable();
        let mut index = 0;
        let mut caret = RichPoint {
            block: start.block,
            offset: start.offset,
        };
        while let Some(line) = inserted.next() {
            let mut runs = if index == 0 {
                prefix.clone()
            } else {
                Vec::new()
            };
            runs.push(RichRun {
                text: line.into(),
                marks: marks.clone(),
            });
            caret = RichPoint {
                block: start.block + index,
                offset: if index == 0 { start.offset } else { 0 } + line.encode_utf16().count(),
            };
            if inserted.peek().is_none() {
                runs.extend(suffix.iter().cloned());
            }
            output.push(RichBlock {
                kind: self.blocks[start.block].kind,
                runs,
            });
            index += 1;
        }
        output.extend(self.blocks[end.block + 1..].iter().cloned());
        Ok((Self::new(output)?, RichSelection::collapsed(caret)))
    }
    pub fn format(&self, selection: RichSelection, format: RichFormat) -> Result<Self, Error> {
        self.validate()?;
        self.validate_selection(selection)?;
        format.validate()?;
        if selection.is_collapsed() && !matches!(format, RichFormat::Block(_)) {
            return Ok(self.clone());
        }
        let (start, end) = selection.ordered();
        let mut blocks = self.blocks.clone();
        for (index, block) in blocks
            .iter_mut()
            .enumerate()
            .take(end.block + 1)
            .skip(start.block)
        {
            if let RichFormat::Block(kind) = format {
                block.kind = kind;
                continue;
            }
            let first = if index == start.block {
                start.offset
            } else {
                0
            };
            let last = if index == end.block {
                end.offset
            } else {
                block.utf16_len()
            };
            let mut runs = block.slice(0, first)?;
            let mut selected = block.slice(first, last)?;
            for run in &mut selected {
                run.marks.apply(&format);
            }
            runs.extend(selected);
            runs.extend(block.slice(last, block.utf16_len())?);
            block.runs = runs;
        }
        Self::new(blocks)
    }
}
