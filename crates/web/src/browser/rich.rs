//! Bounded native contenteditable projection. Listeners only record current
//! input; document/selection reads and model observations happen in the host turn.
use super::{
    backend,
    dom::{Dom, Entry},
};
use glory_render::{
    Error, Event, NodeId, RichBlock, RichBlockKind, RichDocument, RichMarks, RichPoint, RichRun,
    RichSelection, RichSnapshot,
};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{Element, Node};

#[derive(Default)]
pub(super) struct Editing {
    pub dirty: bool,
    pub selected: bool,
    pub ending: bool,
    pub pending: Option<Rc<RichDocument>>,
    pub last_selection: Option<RichSelection>,
    pub insertion: Option<String>,
    pub rejected: bool,
}
fn invalid(message: &'static str) -> Error {
    Error::InvalidProps(message)
}
fn bounded_text(value: wasm_bindgen::JsValue, limit: usize) -> Result<String, Error> {
    if !value.is_string() || js_sys::JsString::from(value.clone()).length() as usize > limit {
        return Err(invalid("rich host text budget"));
    }
    let text = value.as_string().ok_or(invalid("rich host text"))?;
    if text.len() > limit {
        return Err(invalid("rich host UTF-8 text budget"));
    }
    Ok(text)
}
fn kind(element: &Element) -> Option<RichBlockKind> {
    match element.tag_name().as_str() {
        "P" | "DIV" => Some(RichBlockKind::Paragraph),
        "BLOCKQUOTE" => Some(RichBlockKind::Quote),
        "H1" => Some(RichBlockKind::Heading(1)),
        "H2" => Some(RichBlockKind::Heading(2)),
        "H3" => Some(RichBlockKind::Heading(3)),
        "H4" => Some(RichBlockKind::Heading(4)),
        "H5" => Some(RichBlockKind::Heading(5)),
        "H6" => Some(RichBlockKind::Heading(6)),
        _ => None,
    }
}
struct Read {
    nodes: usize,
    text: usize,
    runs: usize,
    links: usize,
}
impl Read {
    fn inline(
        &mut self,
        node: &Node,
        marks: RichMarks,
        out: &mut Vec<RichRun>,
        depth: usize,
    ) -> Result<(), Error> {
        self.nodes += 1;
        if self.nodes > 16_384 || depth > 16 {
            return Err(invalid("rich DOM node/depth budget"));
        }
        if node.node_type() == Node::TEXT_NODE {
            let raw = bounded_text(
                js_sys::Reflect::get(node, &"nodeValue".into()).map_err(backend)?,
                glory_render::MAX_RICH_TEXT_BYTES - self.text,
            )?;
            self.text = self
                .text
                .checked_add(raw.len())
                .ok_or(invalid("rich DOM text budget"))?;
            if self.text > glory_render::MAX_RICH_TEXT_BYTES {
                return Err(invalid("rich DOM text budget"));
            }
            let text = glory_render::normalize_text(&raw, false).into_owned();
            if text.contains(['\n', '\r']) {
                return Err(invalid("rich DOM inline line break"));
            }
            if !text.is_empty() {
                self.runs += 1;
                if self.runs > glory_render::MAX_RICH_RUNS {
                    return Err(invalid("rich DOM run budget"));
                }
                out.push(RichRun { text, marks });
            }
            return Ok(());
        }
        let Some(element) = node.dyn_ref::<Element>() else {
            return Err(invalid("rich DOM supports only text/elements"));
        };
        let mut marks = marks;
        match element.tag_name().as_str() {
            "BR" => return Ok(()),
            "B" | "STRONG" => marks.bold = true,
            "I" | "EM" => marks.italic = true,
            "CODE" => marks.code = true,
            "SPAN" => {
                if element.get_attribute("style").is_some() {
                    return Err(invalid("rich DOM unsupported styled span"));
                }
            }
            "A" => {
                let method = js_sys::Reflect::get(element, &"getAttribute".into())
                    .map_err(backend)?
                    .dyn_into::<js_sys::Function>()
                    .map_err(backend)?;
                let link = bounded_text(
                    method.call1(element, &"href".into()).map_err(backend)?,
                    glory_render::MAX_RICH_LINK_BYTES,
                )?;
                self.links = self
                    .links
                    .checked_add(link.len())
                    .ok_or(invalid("rich DOM metadata budget"))?;
                if self.links > glory_render::MAX_RICH_METADATA_BYTES {
                    return Err(invalid("rich DOM metadata budget"));
                }
                marks.link = Some(link);
                marks.validate()?;
            }
            _ => return Err(invalid("rich DOM unsupported inline tag")),
        }
        let mut child = node.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            self.inline(&current, marks.clone(), out, depth + 1)?;
        }
        Ok(())
    }
}
/// Does not mutate the existing subtree. All counters are checked while walking,
/// before collecting additional host text or metadata.
pub(super) fn read(root: &Element) -> Result<(Rc<RichDocument>, Vec<Node>), Error> {
    let mut reader = Read {
        nodes: 0,
        text: 0,
        runs: 0,
        links: 0,
    };
    let mut blocks = Vec::new();
    let mut roots = Vec::new();
    let mut child = root.first_child();
    while let Some(node) = child {
        child = node.next_sibling();
        if blocks.len() >= glory_render::MAX_RICH_BLOCKS {
            return Err(invalid("rich DOM block budget"));
        }
        let mut runs = Vec::new();
        let mut block_kind = RichBlockKind::Paragraph;
        if let Some(element) = node.dyn_ref::<Element>() {
            if let Some(value) = kind(element) {
                block_kind = value;
                let mut inline = node.first_child();
                while let Some(current) = inline {
                    inline = current.next_sibling();
                    reader.inline(&current, RichMarks::default(), &mut runs, 0)?;
                }
            } else {
                reader.inline(&node, RichMarks::default(), &mut runs, 0)?;
            }
        } else {
            reader.inline(&node, RichMarks::default(), &mut runs, 0)?;
        }
        if runs.is_empty() {
            runs.push(RichRun::default());
        }
        blocks.push(RichBlock {
            kind: block_kind,
            runs,
        });
        roots.push(node);
    }
    if blocks.is_empty() {
        return Ok((Rc::new(RichDocument::default()), roots));
    }
    Ok((Rc::new(RichDocument::new(blocks)?), roots))
}
fn length(node: &Node) -> usize {
    if node.node_type() == Node::TEXT_NODE {
        node.node_value().unwrap_or_default().encode_utf16().count()
    } else {
        let mut total = 0;
        let mut child = node.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            total += length(&current);
        }
        total
    }
}
fn offset(root: &Node, target: &Node, index: u32) -> Option<usize> {
    if root == target {
        if root.node_type() == Node::TEXT_NODE {
            return Some(index as usize);
        }
        let mut child = root.first_child();
        let mut total = 0;
        for _ in 0..index {
            let current = child?;
            child = current.next_sibling();
            total += length(&current);
        }
        return Some(total);
    }
    let mut total = 0;
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if current == *target || current.contains(Some(target)) {
            return offset(&current, target, index).map(|value| value + total);
        }
        total += length(&current);
    }
    None
}
pub(super) fn selection(
    root: &Element,
    document: &RichDocument,
    blocks: &[Node],
) -> Result<RichSelection, Error> {
    let point = |node: Option<Node>, index: u32| -> Result<RichPoint, Error> {
        let node = node.ok_or(Error::Unsupported("rich editor has no document selection"))?;
        if node == Node::from(root.clone()) {
            let block = (index as usize).min(document.blocks.len() - 1);
            return Ok(RichPoint {
                block,
                offset: if index as usize >= blocks.len() {
                    document.blocks[block].utf16_len()
                } else {
                    0
                },
            });
        }
        for (block, root) in blocks.iter().enumerate() {
            if root == &node || root.contains(Some(&node)) {
                return Ok(RichPoint {
                    block,
                    offset: offset(root, &node, index).ok_or(invalid("rich selection boundary"))?,
                });
            }
        }
        Err(Error::Unsupported("selection belongs to another editor"))
    };
    let selection = root
        .owner_document()
        .ok_or(invalid("rich document"))?
        .get_selection()
        .map_err(backend)?
        .ok_or(Error::Unsupported("rich document selection"))?;
    let result = RichSelection {
        anchor: point(selection.anchor_node(), selection.anchor_offset())?,
        focus: point(selection.focus_node(), selection.focus_offset())?,
    };
    document.validate_selection(result)?;
    Ok(result)
}
fn text_point(node: &Node, offset: usize) -> Option<(Node, u32)> {
    if node.node_type() == Node::TEXT_NODE {
        return (offset <= length(node)).then_some((node.clone(), offset as u32));
    }
    let mut remaining = offset;
    let mut child = node.first_child();
    let mut boundary = None;
    while let Some(current) = child {
        child = current.next_sibling();
        let size = length(&current);
        if remaining < size
            && let Some(result) = text_point(&current, remaining)
        {
            return Some(result);
        }
        if remaining == size {
            boundary = text_point(&current, remaining).or(boundary);
        }
        remaining = remaining.saturating_sub(size);
    }
    boundary.or_else(|| (offset == 0).then_some((node.clone(), 0)))
}
pub(super) fn select(root: &Element, selection: RichSelection) -> Result<(), Error> {
    let (document, blocks) = read(root)?;
    document.validate_selection(selection)?;
    let locate = |point: RichPoint| -> Result<(Node, u32), Error> {
        if blocks.is_empty() {
            return Ok((root.clone().into(), 0));
        }
        text_point(&blocks[point.block], point.offset).ok_or(invalid("rich selection text point"))
    };
    let (anchor, a) = locate(selection.anchor)?;
    let (focus, f) = locate(selection.focus)?;
    let selected = root
        .owner_document()
        .ok_or(invalid("rich document"))?
        .get_selection()
        .map_err(backend)?
        .ok_or(Error::Unsupported("rich selection"))?;
    selected
        .set_base_and_extent(&anchor, a, &focus, f)
        .map_err(backend)
}
pub(super) fn write(root: &Element, document: &RichDocument) -> Result<(), Error> {
    document.validate()?;
    let owner = root.owner_document().ok_or(invalid("rich document"))?;
    let fragment = owner.create_document_fragment();
    for block in &document.blocks {
        let tag = match block.kind {
            RichBlockKind::Paragraph => "p",
            RichBlockKind::Quote => "blockquote",
            RichBlockKind::Heading(1) => "h1",
            RichBlockKind::Heading(2) => "h2",
            RichBlockKind::Heading(3) => "h3",
            RichBlockKind::Heading(4) => "h4",
            RichBlockKind::Heading(5) => "h5",
            RichBlockKind::Heading(6) => "h6",
            RichBlockKind::Heading(_) => unreachable!(),
        };
        let paragraph = owner.create_element(tag).map_err(backend)?;
        for run in &block.runs {
            let mut parent = paragraph.clone();
            for tag in [
                run.marks.link.as_ref().map(|_| "a"),
                run.marks.bold.then_some("strong"),
                run.marks.italic.then_some("em"),
                run.marks.code.then_some("code"),
            ]
            .into_iter()
            .flatten()
            {
                let element = owner.create_element(tag).map_err(backend)?;
                if tag == "a" {
                    element
                        .set_attribute("href", run.marks.link.as_ref().expect("link"))
                        .map_err(backend)?;
                }
                parent.append_child(&element).map_err(backend)?;
                parent = element;
            }
            if !run.text.is_empty() {
                parent
                    .append_child(&owner.create_text_node(&run.text))
                    .map_err(backend)?;
            }
        }
        if block.runs.iter().all(|run| run.text.is_empty()) {
            paragraph
                .append_child(&owner.create_element("br").map_err(backend)?.into())
                .map_err(backend)?;
        }
        fragment.append_child(&paragraph).map_err(backend)?;
    }
    root.set_text_content(None);
    root.append_child(&fragment).map_err(backend)?;
    Ok(())
}
pub(super) fn property(entry: &Entry, document: Rc<RichDocument>) -> Result<(), Error> {
    let root = entry.control()?;
    let current = read(&root).ok();
    if entry.editing.borrow().composing
        && current
            .as_ref()
            .is_none_or(|(actual, _)| actual != &document)
    {
        entry
            .rich
            .borrow_mut()
            .get_or_insert_with(Default::default)
            .pending = Some(document);
        return Ok(());
    }
    if current
        .as_ref()
        .is_none_or(|(actual, _)| actual != &document)
    {
        let saved = current
            .as_ref()
            .and_then(|(actual, blocks)| selection(&root, actual, blocks).ok());
        write(&root, &document)?;
        if let Some(saved) = saved {
            let clamp = |point: RichPoint| {
                let block = point.block.min(document.blocks.len() - 1);
                RichPoint {
                    block,
                    offset: point.offset.min(document.blocks[block].utf16_len()),
                }
            };
            let saved = RichSelection {
                anchor: clamp(saved.anchor),
                focus: clamp(saved.focus),
            };
            if document.validate_selection(saved).is_ok() {
                select(&root, saved)?;
            }
        }
    }
    {
        let mut editing = entry.editing.borrow_mut();
        editing.revision = editing.requested;
    }
    entry
        .rich
        .borrow_mut()
        .get_or_insert_with(Default::default)
        .pending = None;
    Ok(())
}
pub(super) fn mark(dom: &Dom, id: NodeId) {
    let mut pending = dom.rich_pending.borrow_mut();
    if !pending.contains(&id) {
        pending.push(id);
    }
    drop(pending);
    dom.frame.input();
}
/// Current declarations only. Retiring/blocking an editor ends the one draft
/// and pending desired value; a late native composition event cannot restore it.
pub(super) fn update(
    dom: &Dom,
    tree: &glory_render::memory::Memory,
    changes: &crate::semantic_changes::StateChanges,
) -> Result<(), Error> {
    // Only semantic eligibility, focus policy, document revision and topology
    // affect this reconciliation. Native editing is observed separately.
    if !(changes.inherited() || changes.focus || changes.rich_document) {
        return Ok(());
    }
    let live: Vec<_> = dom.rich_live.borrow().iter().copied().collect();
    for id in live {
        let model = tree.node(id)?;
        let descriptor = model.descriptor.view();
        let entry = dom.entry(id)?;
        let root = entry.control()?;
        let enabled = tree.enabled(id);
        let local_enabled = descriptor.props.enabled();
        let attribute = |name: &str, value: Option<&str>| -> Result<(), Error> {
            if root.get_attribute(name).as_deref() != value {
                if let Some(value) = value {
                    root.set_attribute(name, value).map_err(backend)?;
                } else {
                    root.remove_attribute(name).map_err(backend)?;
                }
            }
            Ok(())
        };
        attribute(
            "data-gly-rich-ancestor-disabled",
            (local_enabled && !enabled).then_some(""),
        )?;
        attribute(
            "contenteditable",
            Some(if enabled && !descriptor.readonly() {
                "true"
            } else {
                "false"
            }),
        )?;
        attribute(
            "aria-disabled",
            (!enabled || descriptor.semantics.as_ref().is_some_and(|s| s.disabled))
                .then_some("true"),
        )?;
        attribute(
            "tabindex",
            if !enabled {
                Some("-1")
            } else {
                match descriptor.focus {
                    glory_render::FocusMode::Auto => None,
                    glory_render::FocusMode::TabStop => Some("0"),
                    glory_render::FocusMode::Programmatic => Some("-1"),
                }
            },
        )?;
        if tree.active(id)
            && tree.enabled(id)
            && !tree.inert(id)
            && !model.descriptor.view().readonly()
        {
            continue;
        }
        let composing = entry.editing.borrow_mut().composing;
        entry.editing.borrow_mut().composing = false;
        let state = entry.rich.borrow_mut().take();
        if composing
            || state.as_ref().is_some_and(|state| {
                state.pending.is_some() || state.dirty || state.insertion.is_some()
            })
        {
            let glory_render::Props::RichEditor(editor) = model.descriptor.view().props else {
                unreachable!()
            };
            property(&entry, editor.document.clone())?;
        }
        if composing {
            dom.queue(id, Event::Composition { active: false });
        }
    }
    Ok(())
}
pub(super) fn receive(
    dom: &Dom,
    id: NodeId,
    entry: &Entry,
    name: &str,
    event: &web_sys::Event,
) -> Result<bool, Error> {
    match name {
        "input" => {
            if event
                .dyn_ref::<web_sys::InputEvent>()
                .is_some_and(|event| event.is_composing())
            {
                entry.editing.borrow_mut().composing = true;
            }
            entry
                .rich
                .borrow_mut()
                .get_or_insert_with(Default::default)
                .dirty = true;
            mark(dom, id);
            Ok(true)
        }
        "compositionstart" => {
            entry.editing.borrow_mut().composing = true;
            dom.queue(id, Event::Composition { active: true });
            Ok(true)
        }
        "compositionend" => {
            {
                let mut state = entry.rich.borrow_mut();
                let state = state.get_or_insert_with(Default::default);
                state.ending = true;
                state.dirty = true;
            }
            mark(dom, id);
            Ok(true)
        }
        "beforeinput" => {
            if let Some(input) = event.dyn_ref::<web_sys::InputEvent>() {
                let input_type = input.input_type();
                if matches!(input_type.as_str(), "historyUndo" | "historyRedo") {
                    event.prevent_default();
                    if !entry.editing.borrow().composing {
                        dom.queue(
                            id,
                            Event::RichCommand {
                                command: if input_type == "historyUndo" {
                                    glory_render::RichCommand::Undo
                                } else {
                                    glory_render::RichCommand::Redo
                                },
                                revision: entry.editing.borrow().revision,
                            },
                        );
                    }
                    return Ok(true);
                }
                if matches!(
                    input_type.as_str(),
                    "formatBold"
                        | "formatItalic"
                        | "formatUnderline"
                        | "formatStrikeThrough"
                        | "insertOrderedList"
                        | "insertUnorderedList"
                        | "insertFromDrop"
                ) {
                    event.prevent_default();
                }
                if input_type == "insertLineBreak" {
                    event.prevent_default();
                    entry
                        .rich
                        .borrow_mut()
                        .get_or_insert_with(Default::default)
                        .insertion = Some("\n".into());
                    mark(dom, id);
                }
            }
            Ok(true)
        }
        "paste" => {
            event.prevent_default();
            let text = (|| -> Result<String, Error> {
                let data = event
                    .dyn_ref::<web_sys::ClipboardEvent>()
                    .and_then(|event| event.clipboard_data())
                    .ok_or(invalid("rich plain paste data"))?;
                let method = js_sys::Reflect::get(&data, &"getData".into())
                    .map_err(backend)?
                    .dyn_into::<js_sys::Function>()
                    .map_err(backend)?;
                bounded_text(
                    method.call1(&data, &"text/plain".into()).map_err(backend)?,
                    glory_render::MAX_RICH_TEXT_BYTES,
                )
            })();
            let Ok(text) = text else {
                entry
                    .rich
                    .borrow_mut()
                    .get_or_insert_with(Default::default)
                    .rejected = true;
                mark(dom, id);
                return Ok(true);
            };
            entry
                .rich
                .borrow_mut()
                .get_or_insert_with(Default::default)
                .insertion = Some(text);
            mark(dom, id);
            Ok(true)
        }
        "drop" => {
            event.prevent_default();
            Ok(true)
        }
        "keydown" => {
            let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                return Ok(false);
            };
            if key.is_composing()
                || entry.editing.borrow().composing
                || key.repeat()
                || key.alt_key()
                || !(key.ctrl_key() || key.meta_key())
            {
                return Ok(false);
            }
            use glory_render::RichCommand;
            let command = match key.key().to_ascii_lowercase().as_str() {
                "z" if key.shift_key() => Some(RichCommand::Redo),
                "z" => Some(RichCommand::Undo),
                "y" => Some(RichCommand::Redo),
                "b" => Some(RichCommand::Bold),
                "i" => Some(RichCommand::Italic),
                "`" => Some(RichCommand::Code),
                _ => None,
            };
            if let Some(command) = command {
                event.prevent_default();
                dom.queue(
                    id,
                    Event::RichCommand {
                        command,
                        revision: entry.editing.borrow().revision,
                    },
                );
                return Ok(true);
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
pub(super) struct Write {
    entry: Rc<Entry>,
    document: Rc<RichDocument>,
    selection: Option<RichSelection>,
    acknowledge: bool,
}
impl Write {
    pub(super) fn apply(self) -> Result<(), Error> {
        write(&self.entry.control()?, &self.document)?;
        if let Some(selection) = self.selection {
            select(&self.entry.control()?, selection)?;
        }
        if self.acknowledge {
            let mut editing = self.entry.editing.borrow_mut();
            editing.revision = editing.requested;
        }
        Ok(())
    }
}
pub(super) fn observe(dom: &Dom, tree: &glory_render::memory::Memory) -> Result<Vec<Write>, Error> {
    // Current live editor membership, bounded by the ordinary semantic node budget;
    // no visited document, prior request or source history is retained.
    let pending = std::mem::take(&mut *dom.rich_pending.borrow_mut());
    let mut writes = Vec::new();
    for id in pending {
        let Ok(entry) = dom.entry(id) else {
            continue;
        };
        let (dirty, selected, ending, insertion, rejected) = {
            let mut state = entry.rich.borrow_mut();
            let state = state.get_or_insert_with(Default::default);
            (
                std::mem::take(&mut state.dirty),
                std::mem::take(&mut state.selected),
                std::mem::take(&mut state.ending),
                state.insertion.take(),
                std::mem::take(&mut state.rejected),
            )
        };
        if !dirty && !selected && !ending && insertion.is_none() && !rejected {
            continue;
        }
        let model = tree.node(id)?;
        let glory_render::Props::RichEditor(accepted) = model.descriptor.view().props else {
            continue;
        };
        let root = entry.control()?;
        if !tree.active(id) || !tree.enabled(id) || tree.inert(id) {
            if dirty || insertion.is_some() {
                writes.push(Write {
                    entry: entry.clone(),
                    document: accepted.document.clone(),
                    selection: None,
                    acknowledge: true,
                });
            }
            continue;
        }
        let readonly = model.descriptor.view().readonly();
        let observed = if rejected {
            Err(invalid("rich insertion rejected"))
        } else {
            read(&root)
        };
        let (mut document, blocks) = match observed {
            Ok(value) => value,
            Err(_) => {
                let was_composing = entry.editing.borrow().composing;
                entry.editing.borrow_mut().composing = false;
                let revision = entry.editing.borrow().revision;
                writes.push(Write {
                    entry: entry.clone(),
                    document: accepted.document.clone(),
                    selection: None,
                    acknowledge: true,
                });
                dom.queue(id, Event::RichRejected { revision });
                if ending || was_composing {
                    dom.queue(id, Event::Composition { active: false });
                }
                continue;
            }
        };
        if readonly && (dirty || insertion.is_some()) {
            writes.push(Write {
                entry: entry.clone(),
                document: accepted.document.clone(),
                selection: None,
                acknowledge: true,
            });
            continue;
        }
        let mut selection = selection(&root, &document, &blocks).unwrap_or_default();
        let inserted = insertion.is_some();
        if let Some(text) = insertion {
            let (next, caret) = match document.replace(selection, &text, RichMarks::default()) {
                Ok(value) => value,
                Err(_) => {
                    writes.push(Write {
                        entry: entry.clone(),
                        document: accepted.document.clone(),
                        selection: None,
                        acknowledge: true,
                    });
                    dom.queue(
                        id,
                        Event::RichRejected {
                            revision: entry.editing.borrow().revision,
                        },
                    );
                    continue;
                }
            };
            document = Rc::new(next);
            selection = caret;
            writes.push(Write {
                entry: entry.clone(),
                document: document.clone(),
                selection: Some(selection),
                acknowledge: false,
            });
        }
        let composing = entry.editing.borrow().composing && !ending;
        let revision = entry.editing.borrow().revision;
        if dirty || inserted || ending {
            dom.queue(
                id,
                Event::RichEdit(Box::new(RichSnapshot {
                    document: document.clone(),
                    selection,
                    revision,
                    composing,
                })),
            );
        }
        if selected {
            let changed = entry
                .rich
                .borrow()
                .as_ref()
                .is_none_or(|state| state.last_selection != Some(selection));
            if changed {
                dom.queue(
                    id,
                    Event::RichSelection(Box::new(RichSnapshot {
                        document: document.clone(),
                        selection,
                        revision,
                        composing,
                    })),
                );
            }
        }
        entry
            .rich
            .borrow_mut()
            .get_or_insert_with(Default::default)
            .last_selection = Some(selection);
        if ending {
            entry.editing.borrow_mut().composing = false;
            dom.queue(id, Event::Composition { active: false });
            let pending = entry
                .rich
                .borrow_mut()
                .get_or_insert_with(Default::default)
                .pending
                .take();
            if let Some(pending) = pending {
                if pending != document {
                    writes.push(Write {
                        entry: entry.clone(),
                        document: pending,
                        selection: None,
                        acknowledge: true,
                    });
                } else {
                    let mut editing = entry.editing.borrow_mut();
                    editing.revision = editing.requested;
                }
            }
        }
    }
    Ok(writes)
}
