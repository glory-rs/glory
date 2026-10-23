//! Native finite rich document. Each editable leaf uses the actual Parley editor;
//! containers remain real engine list/table/quote boxes, never Markdown text.
use super::*;
use blitz_dom::node::TextBrush;
use blitz_dom::{
    EditorStyleSpan, FontStyle, FontWeight, InlineBox, InlineBoxKind, StyleProperty, VerticalAlign,
};
use glory_render::rich_text::{
    Block, Document, Inline, Kind as BlockKind, Marks, Point, Selection,
};

pub(super) struct Rich {
    pub document: Document,
    leaves: BTreeMap<u64, Leaf>,
    pub pending: Option<Document>,
    pub selection: Option<Selection>,
    pub drag: Option<Point>,
    pub tasks: BTreeMap<EngineId, u64>,
    pub typing: Option<(Selection, Marks)>,
    last_control: Option<EngineId>,
    pub composition_backup: Option<(Document, Selection)>,
}
struct Leaf {
    control: EngineId,
    wrapper: EngineId,
    styles: Vec<EditorStyleSpan<TextBrush>>,
    brushes: BTreeMap<EngineId, Marks>,
    objects: BTreeMap<u64, (EngineId, Inline)>,
    boxes: Vec<InlineBox>,
    text: String,
    initialized: bool,
    height: f32,
}
impl Rich {
    pub(super) fn retire_controls(&self, controls: &mut HashMap<EngineId, NodeId>) {
        for leaf in self.leaves.values() {
            controls.remove(&leaf.control);
        }
    }
    fn control(&self, focused: Option<EngineId>) -> Option<EngineId> {
        focused
            .filter(|id| self.leaves.values().any(|leaf| leaf.control == *id))
            .or(self.last_control)
            .or_else(|| self.leaves.values().next().map(|leaf| leaf.control))
    }
    fn block_for(&self, control: EngineId) -> Option<u64> {
        self.leaves
            .iter()
            .find_map(|(id, leaf)| (leaf.control == control).then_some(*id))
    }
}
fn text(block: &Block) -> String {
    block
        .spans
        .iter()
        .map(|span| match span {
            Inline::Text { text, .. } => text.as_str(),
            Inline::HardBreak => "\n",
            Inline::Image { .. } => "\u{fffc}",
        })
        .collect()
}
fn element(doc: &mut BaseDocument, parent: EngineId, name: &str, style: &str) -> EngineId {
    let mut m = doc.mutate();
    let node = m.create_element(tag(name), Vec::new());
    if !style.is_empty() {
        m.set_attribute(node, attr("style"), style);
    }
    m.append_children(parent, &[node]);
    node
}
fn build(
    doc: &mut BaseDocument,
    parent: EngineId,
    block: &Block,
    leaves: &mut BTreeMap<u64, Leaf>,
    tasks: &mut BTreeMap<EngineId, u64>,
) {
    let (tag_name, style) = match &block.kind {
        BlockKind::Root => ("div", ""),
        BlockKind::Paragraph => ("div", "margin:0 0 8px"),
        BlockKind::Heading(n) => (
            match n {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            },
            "margin:8px 0",
        ),
        BlockKind::Quote => (
            "blockquote",
            "border-left:3px solid #8892a0;margin:8px 0;padding-left:12px",
        ),
        BlockKind::List(Some(_)) => ("ol", "padding-left:28px;margin:4px 0"),
        BlockKind::List(None) => ("ul", "padding-left:28px;margin:4px 0"),
        BlockKind::Item(_) => ("li", "margin:4px 0"),
        BlockKind::Code(_) => (
            "div",
            "font-family:monospace;background:#eef0f4;padding:8px",
        ),
        BlockKind::Rule => ("hr", ""),
        BlockKind::Table(_) => (
            "table",
            "border-collapse:collapse;table-layout:fixed;width:100%",
        ),
        BlockKind::TableHead | BlockKind::Row => ("tr", ""),
        BlockKind::Cell => (
            "td",
            "border:1px solid #929baa;padding:6px;vertical-align:top",
        ),
    };
    let node = element(doc, parent, tag_name, style);
    if let BlockKind::List(Some(start)) = block.kind {
        doc.mutate()
            .set_attribute(node, attr("start"), &start.to_string());
    }
    if let BlockKind::Item(Some(checked)) = block.kind {
        doc.set_style_property(node, "position", "relative");
        doc.set_style_property(node, "padding-left", "24px");
        doc.set_style_property(node, "list-style", "none");
        let marker = element(
            doc,
            node,
            "input",
            "position:absolute;left:0;top:3px;margin:0",
        );
        let mut m = doc.mutate();
        m.set_attribute(marker, attr("type"), "checkbox");
        m.set_attribute(marker, attr("aria-label"), "Task completed");
        if checked {
            m.set_attribute(marker, attr("checked"), "true");
        }
        tasks.insert(marker, block.id);
    }
    if !block.editable() {
        for child in &block.children {
            build(doc, node, child, leaves, tasks);
        }
        if let BlockKind::Table(columns) = &block.kind {
            for row in &block.children {
                for (index, cell) in row.children.iter().enumerate() {
                    if let Some(leaf) = leaves.get(&cell.id) {
                        let value = match columns.get(index) {
                            Some(glory_render::rich_text::Alignment::Center) => "center",
                            Some(glory_render::rich_text::Alignment::Right) => "right",
                            _ => "left",
                        };
                        doc.set_style_property(leaf.control, "text-align", value);
                    }
                }
            }
        }
        return;
    }
    let wrapper = element(doc, node, "div", "position:relative;width:100%;min-width:0");
    let control = element(
        doc,
        wrapper,
        "textarea",
        "display:block;box-sizing:border-box;width:100%;min-width:0;height:24px;min-height:0;border:0;padding:0;margin:0;resize:none;overflow:hidden;background:transparent;color:inherit;font:inherit;white-space:pre-wrap",
    );
    let value = text(block);
    {
        let mut m = doc.mutate();
        m.set_attribute(control, attr("rows"), "1");
        let seed = m.create_text_node(&value);
        m.append_children(control, &[seed]);
    }
    let mut leaf = Leaf {
        control,
        wrapper,
        styles: vec![],
        brushes: BTreeMap::new(),
        objects: BTreeMap::new(),
        boxes: vec![],
        text: value,
        initialized: false,
        height: 24.0,
    };
    let mut offset = 0;
    for (index, span) in block.spans.iter().enumerate() {
        match span {
            Inline::Text { text, marks } => {
                let range = offset..offset + text.len();
                let mut css = String::from("display:none;");
                if marks.link.is_some() {
                    css.push_str("color:#2563eb;text-decoration:underline;");
                }
                if marks.strike {
                    css.push_str(if marks.link.is_some() {
                        "text-decoration:underline line-through;"
                    } else {
                        "text-decoration:line-through;"
                    });
                }
                if marks.code {
                    css.push_str("background:#e1e5ec;");
                }
                let brush = element(doc, wrapper, "span", &css);
                leaf.brushes.insert(brush, marks.clone());
                leaf.styles.push(EditorStyleSpan {
                    range: range.clone(),
                    property: StyleProperty::Brush(TextBrush { id: brush }),
                });
                if marks.bold {
                    leaf.styles.push(EditorStyleSpan {
                        range: range.clone(),
                        property: StyleProperty::FontWeight(FontWeight::BOLD),
                    });
                }
                if marks.italic {
                    leaf.styles.push(EditorStyleSpan {
                        range: range.clone(),
                        property: StyleProperty::FontStyle(FontStyle::Italic),
                    });
                }
                if marks.code {
                    leaf.styles.push(EditorStyleSpan {
                        range: range.clone(),
                        property: StyleProperty::FontFamily(
                            blitz_dom::GenericFamily::Monospace.into(),
                        ),
                    });
                }
                offset = range.end;
            }
            Inline::HardBreak => offset += 1,
            Inline::Image {
                source,
                alternative,
                title,
            } => {
                let image = element(
                    doc,
                    wrapper,
                    "img",
                    "position:absolute;left:0;top:0;width:160px;height:120px;object-fit:contain;pointer-events:none",
                );
                {
                    let mut m = doc.mutate();
                    m.set_attribute(image, attr("src"), source);
                    m.set_attribute(image, attr("alt"), alternative);
                    m.set_attribute(image, attr("title"), title);
                }
                let object = index as u64;
                leaf.objects.insert(object, (image, span.clone()));
                leaf.boxes.push(InlineBox {
                    id: object,
                    index: offset,
                    width: 160.0,
                    height: 120.0,
                    baseline: None,
                    kind: InlineBoxKind::InFlow,
                    vertical_align: VerticalAlign::BASELINE,
                });
                offset += 3;
            }
        }
    }
    leaves.insert(block.id, leaf);
}
impl Dom {
    pub(super) fn rich_all_selection(&self, id: NodeId) -> Result<Selection, Error> {
        let doc = &self.range(id)?.rich.as_ref().unwrap().document;
        let mut order = vec![];
        ordered(&doc.root, &mut order);
        let first = *order
            .first()
            .ok_or(Error::Unsupported("rich leaf absent"))?;
        let last = *order.last().unwrap();
        Ok(Selection {
            anchor: Point {
                block: first,
                offset: 0,
            },
            focus: Point {
                block: last,
                offset: doc.root.find(last).unwrap().units(),
            },
        })
    }
    pub(super) fn rich_control(&self, id: NodeId) -> Option<EngineId> {
        self.ranges
            .get(&id)?
            .rich
            .as_ref()?
            .control(self.doc.get_focussed_node_id())
    }
    pub(super) fn rich_write(&mut self, id: NodeId, document: &Document) -> Result<(), Error> {
        document.validate()?;
        if let Some(current) = self.ranges.get_mut(&id).and_then(|r| r.rich.as_mut()) {
            if current.document == *document {
                let r = self.ranges.get_mut(&id).unwrap();
                r.editing.revision = r.editing.requested;
                return Ok(());
            }
            if self.ranges[&id].editing.composing {
                self.ranges
                    .get_mut(&id)
                    .unwrap()
                    .rich
                    .as_mut()
                    .unwrap()
                    .pending = Some(document.clone());
                return Ok(());
            }
        }
        let main = self.main(id)?;
        if let Some(old) = self.ranges.get_mut(&id).unwrap().rich.take() {
            for leaf in old.leaves.values() {
                self.controls.remove(&leaf.control);
            }
        }
        let children = self
            .doc
            .get_node(main)
            .ok_or(Error::NodeGone(id))?
            .children
            .clone();
        for child in children {
            self.doc.mutate().remove_and_drop_node(child);
        }
        let mut leaves = BTreeMap::new();
        let mut tasks = BTreeMap::new();
        for child in &document.root.children {
            build(&mut self.doc, main, child, &mut leaves, &mut tasks);
        }
        for leaf in leaves.values() {
            self.controls.insert(leaf.control, id);
        }
        let range = self.ranges.get_mut(&id).unwrap();
        range.rich = Some(Rich {
            document: document.clone(),
            leaves,
            pending: None,
            selection: None,
            drag: None,
            tasks,
            typing: None,
            last_control: None,
            composition_backup: None,
        });
        range.editing.revision = range.editing.requested;
        if self.rich_drag.is_some_and(|(node, _)| node == id) {
            self.rich_drag = None;
        }
        self.rich_layout.insert(id);
        self.property_writes += 1;
        self.invalidate();
        Ok(())
    }
    pub(super) fn rich_layout(&mut self) {
        let pending = std::mem::take(&mut self.rich_layout);
        let mut resized = false;
        for id in pending {
            let Some(rich) = self.ranges.get_mut(&id).and_then(|r| r.rich.as_mut()) else {
                continue;
            };
            for leaf in rich.leaves.values_mut() {
                if !leaf.initialized {
                    if self
                        .doc
                        .set_attributed_text(leaf.control, &leaf.text, leaf.styles.clone())
                        .is_err()
                    {
                        continue;
                    }
                    if self
                        .doc
                        .set_attributed_objects(leaf.control, leaf.boxes.clone())
                        .is_err()
                    {
                        continue;
                    }
                    leaf.initialized = true;
                }
                let active: BTreeSet<_> = self
                    .doc
                    .get_node(leaf.control)
                    .and_then(|n| n.element_data())
                    .and_then(|e| e.text_input_data())
                    .map(|i| i.editor.inline_boxes().iter().map(|b| b.id).collect())
                    .unwrap_or_default();
                let removed: Vec<_> = leaf
                    .objects
                    .keys()
                    .filter(|id| !active.contains(id))
                    .copied()
                    .collect();
                for id in removed {
                    if let Some((node, _)) = leaf.objects.remove(&id) {
                        self.doc.mutate().remove_and_drop_node(node);
                    }
                }
                let width = self
                    .doc
                    .get_node(leaf.control)
                    .map(|n| n.final_layout().content_box_width())
                    .unwrap_or(0.0);
                if let Some((_, height)) = self.doc.measure_attributed_text(leaf.control, width) {
                    let height = height.max(24.0).ceil();
                    if (height - leaf.height).abs() > 0.5 {
                        self.doc
                            .set_style_property(leaf.control, "height", &format!("{height}px"));
                        leaf.height = height;
                        resized = true;
                    }
                }
                for (object, x, y, w, h) in self.doc.attributed_object_positions(leaf.control) {
                    if let Some((node, _)) = leaf.objects.get(&object) {
                        resized = true;
                        for (property, value) in
                            [("left", x), ("top", y), ("width", w), ("height", h)]
                        {
                            self.doc
                                .set_style_property(*node, property, &format!("{value}px"));
                        }
                    }
                }
            }
        }
        if resized {
            self.doc.resolve(self.time);
            self.layouts += 1;
        }
    }
    pub(super) fn rich_selection(&self, id: NodeId) -> Result<Selection, Error> {
        if let Some(selection) = self.range(id)?.rich.as_ref().and_then(|r| r.selection) {
            return Ok(selection);
        }
        let control = self
            .rich_control(id)
            .ok_or_else(|| invariant("rich document has no editable leaf"))?;
        let rich = self.range(id)?.rich.as_ref().unwrap();
        let block = rich.block_for(control).unwrap();
        let input = self
            .doc
            .get_node(control)
            .and_then(|n| n.element_data())
            .and_then(|e| e.text_input_data())
            .ok_or(Error::Unsupported("rich editor not initialized"))?;
        let selection = input.editor.raw_selection();
        let text = input.editor.raw_text();
        Ok(Selection {
            anchor: Point {
                block,
                offset: text[..selection.anchor().index()].encode_utf16().count(),
            },
            focus: Point {
                block,
                offset: text[..selection.focus().index()].encode_utf16().count(),
            },
        })
    }
    pub(super) fn rich_set_selection(
        &mut self,
        id: NodeId,
        selection: Selection,
    ) -> Result<(), Error> {
        self.resolve();
        if let Some(rich) = self.ranges.get_mut(&id).and_then(|r| r.rich.as_mut()) {
            if rich
                .typing
                .as_ref()
                .is_some_and(|(point, _)| *point != selection)
            {
                rich.typing = None;
            }
        }
        let rich = self
            .range(id)?
            .rich
            .as_ref()
            .ok_or(Error::Unsupported("not a rich editor"))?;
        rich.document.validate_selection(selection)?;
        let mut order = vec![];
        ordered(&rich.document.root, &mut order);
        let a = order
            .iter()
            .position(|b| *b == selection.anchor.block)
            .unwrap();
        let b = order
            .iter()
            .position(|b| *b == selection.focus.block)
            .unwrap();
        let (start, end, a, b) = if (a, selection.anchor.offset) <= (b, selection.focus.offset) {
            (selection.anchor, selection.focus, a, b)
        } else {
            (selection.focus, selection.anchor, b, a)
        };
        let controls: Vec<_> = order
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let leaf = &rich.leaves[block];
                let count = rich.document.root.find(*block).unwrap().units();
                let range = if index < a || index > b {
                    (0, 0)
                } else {
                    (
                        if index == a { start.offset } else { 0 },
                        if index == b { end.offset } else { count },
                    )
                };
                (*block, leaf.control, range)
            })
            .collect();
        let focus = rich.leaves[&selection.focus.block].control;
        for (block, control, (start, end)) in controls {
            let value = self
                .doc
                .get_node(control)
                .and_then(|n| n.element_data())
                .and_then(|e| e.text_input_data())
                .unwrap()
                .editor
                .raw_text();
            let anchor = byte(value, start);
            let finish = byte(value, end);
            let (anchor, finish) = if block == selection.focus.block
                && ((selection.anchor.block == selection.focus.block
                    && selection.anchor.offset > selection.focus.offset)
                    || (selection.focus.block != selection.anchor.block
                        && b == order
                            .iter()
                            .position(|v| *v == selection.anchor.block)
                            .unwrap()))
            {
                (finish, anchor)
            } else {
                (anchor, finish)
            };
            self.doc
                .with_text_input(control, |mut d| d.select_byte_range(anchor, finish));
            if let Some(input) = self
                .doc
                .get_node_mut(control)
                .and_then(|n| n.element_data_mut())
                .and_then(|e| e.text_input_data_mut())
            {
                input.document_selection = start != end && block != selection.focus.block;
            }
        }
        self.ranges
            .get_mut(&id)
            .unwrap()
            .rich
            .as_mut()
            .unwrap()
            .last_control = Some(focus);
        self.doc.set_focus_to(focus);
        self.ranges
            .get_mut(&id)
            .unwrap()
            .rich
            .as_mut()
            .unwrap()
            .selection = (selection.anchor.block != selection.focus.block).then_some(selection);
        self.invalidate();
        Ok(())
    }

    pub(super) fn rich_event(
        &mut self,
        id: NodeId,
        composing: bool,
        revision: u64,
    ) -> Result<Event, Error> {
        let control = self
            .rich_control(id)
            .ok_or_else(|| invariant("rich editor absent"))?;
        let input = self
            .doc
            .get_node(control)
            .and_then(|n| n.element_data())
            .and_then(|e| e.text_input_data())
            .ok_or_else(|| invariant("rich editor uninitialized"))?;
        let value = input.editor.raw_text();
        let rich = self.ranges.get_mut(&id).unwrap().rich.as_mut().unwrap();
        rich.last_control = Some(control);
        let block = rich.block_for(control).unwrap();
        let leaf = &rich.leaves[&block];
        let old = text(rich.document.root.find(block).unwrap());
        let mut prefix = old
            .chars()
            .zip(value.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        prefix = prefix.min(old.len()).min(value.len());
        let suffix = old[prefix..]
            .chars()
            .rev()
            .zip(value[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let typing_start = rich
            .typing
            .as_ref()
            .filter(|(selection, _)| selection.focus.block == block)
            .map(|(selection, _)| byte(value, selection.focus.offset));
        let inserted = typing_start.map_or(prefix, |start| prefix.min(start))..value.len() - suffix;
        let typing = rich
            .typing
            .as_ref()
            .filter(|(selection, _)| selection.focus.block == block)
            .map(|(_, marks)| marks.clone());
        let mut updated = rich.document.root.find(block).unwrap().clone();
        updated.spans.clear();
        let mut changes: BTreeMap<usize, Vec<(usize, Option<Marks>)>> = BTreeMap::new();
        let mut cuts = BTreeSet::from([0, value.len(), inserted.start, inserted.end]);
        for (index, span) in input.editor.style_spans().iter().enumerate() {
            if let StyleProperty::Brush(brush) = &span.property {
                if let Some(marks) = leaf.brushes.get(&brush.id) {
                    changes
                        .entry(span.range.start)
                        .or_default()
                        .push((index, Some(marks.clone())));
                    changes
                        .entry(span.range.end)
                        .or_default()
                        .push((index, None));
                    cuts.insert(span.range.start);
                    cuts.insert(span.range.end);
                }
            }
        }
        let objects: BTreeMap<_, _> = input
            .editor
            .inline_boxes()
            .iter()
            .filter_map(|b| leaf.objects.get(&b.id).map(|(_, span)| (b.index, span)))
            .collect();
        for index in objects.keys() {
            cuts.insert(*index);
            cuts.insert(*index + 3);
        }
        for (index, ch) in value.char_indices() {
            if ch == '\n' && !matches!(updated.kind, BlockKind::Code(_)) {
                cuts.insert(index);
                cuts.insert(index + 1);
            }
        }
        let cuts: Vec<_> = cuts.into_iter().collect();
        let mut active = BTreeMap::new();
        for pair in cuts.windows(2) {
            let offset = pair[0];
            let end = pair[1];
            if let Some(changes) = changes.get(&offset) {
                for (index, marks) in changes {
                    if let Some(marks) = marks {
                        active.insert(*index, marks.clone());
                    } else {
                        active.remove(index);
                    }
                }
            }
            if let Some(object) = objects.get(&offset) {
                updated.push((*object).clone());
                continue;
            }
            let text = &value[offset..end];
            if text == "\n" && !matches!(updated.kind, BlockKind::Code(_)) {
                updated.push(Inline::HardBreak);
            } else {
                updated.push(Inline::Text {
                    text: text.into(),
                    marks: if inserted.contains(&offset) {
                        typing.clone().unwrap_or_else(|| {
                            active
                                .last_key_value()
                                .map(|(_, m)| m.clone())
                                .unwrap_or_default()
                        })
                    } else {
                        active
                            .last_key_value()
                            .map(|(_, m)| m.clone())
                            .unwrap_or_default()
                    },
                });
            }
        }
        *rich.document.root.find_mut(block).unwrap() = updated;
        let document = rich.document.clone();
        document.validate()?;
        let selection = self.rich_selection(id)?;
        document.validate_selection(selection)?;
        if !composing {
            if let Some(typing) = &mut self
                .ranges
                .get_mut(&id)
                .unwrap()
                .rich
                .as_mut()
                .unwrap()
                .typing
            {
                typing.0 = selection;
            }
        }
        self.rich_apply_styles(id, block)?;
        self.rich_layout.insert(id);
        self.invalidate();
        Ok(Event::RichTextChanged {
            document,
            selection,
            composing,
            revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_ui::{App, rich_text};
    fn fixture(document: Document) -> (App, Renderer, NodeId) {
        let mut renderer = None;
        let app = App::new(|id| {
            let r = Renderer::new(id);
            renderer = Some(r.clone());
            r
        });
        let renderer = renderer.unwrap();
        renderer.set_viewport(600, 500, 1.0).unwrap();
        app.mount(rich_text().document(document)).unwrap();
        app.settle(8).unwrap();
        let id = app.tree().roots()[0];
        renderer.0.borrow_mut().dom.resolve();
        (app, renderer, id)
    }
    fn paragraph(id: u64, value: &str) -> Block {
        Block {
            id,
            kind: BlockKind::Paragraph,
            spans: vec![Inline::Text {
                text: value.into(),
                marks: Marks {
                    bold: true,
                    italic: true,
                    ..Marks::default()
                },
            }],
            children: vec![],
        }
    }
    #[test]
    fn actual_native_rich_leaf_preserves_marks_utf16_and_equal_controlled_caret() {
        let document = Document {
            root: Block {
                id: 0,
                kind: BlockKind::Root,
                spans: vec![],
                children: vec![paragraph(1, "A😀中文")],
            },
        };
        let (_app, renderer, id) = fixture(document.clone());
        let mut state = renderer.0.borrow_mut();
        let selection = Selection {
            anchor: Point {
                block: 1,
                offset: 1,
            },
            focus: Point {
                block: 1,
                offset: 3,
            },
        };
        state.dom.rich_set_selection(id, selection).unwrap();
        assert_eq!(state.dom.rich_selection(id).unwrap(), selection);
        let control = state.dom.control(id).unwrap();
        state.dom.rich_write(id, &document).unwrap();
        assert_eq!(state.dom.control(id).unwrap(), control);
        assert_eq!(state.dom.rich_selection(id).unwrap(), selection);
        state.dom.doc.with_text_input(control, |mut driver| {
            driver.insert_or_replace_selection("替换")
        });
        let Event::RichTextChanged {
            document,
            selection,
            ..
        } = state.dom.rich_event(id, false, 0).unwrap()
        else {
            panic!()
        };
        assert_eq!(text(&document.root.children[0]), "A替换中文");
        assert_eq!(selection.focus.offset, 3);
        assert!(
            document.root.children[0]
                .spans
                .iter()
                .all(|span| matches!(span,Inline::Text{marks,..}if marks.bold&&marks.italic))
        );
    }
    #[test]
    fn actual_native_table_list_and_inline_objects_have_boxes() {
        let mut p = paragraph(1, "before");
        p.spans.push(Inline::Image {
            source: "asset://app/test.png".into(),
            alternative: "test".into(),
            title: String::new(),
        });
        p.push(Inline::Text {
            text: "after".into(),
            marks: Marks::default(),
        });
        let cell = Block {
            id: 5,
            kind: BlockKind::Cell,
            spans: vec![Inline::Text {
                text: "cell".into(),
                marks: Marks::default(),
            }],
            children: vec![],
        };
        let table = Block {
            id: 3,
            kind: BlockKind::Table(vec![glory_render::rich_text::Alignment::Left]),
            spans: vec![],
            children: vec![Block {
                id: 4,
                kind: BlockKind::Row,
                spans: vec![],
                children: vec![cell],
            }],
        };
        let list = Block {
            id: 6,
            kind: BlockKind::List(Some(2)),
            spans: vec![],
            children: vec![Block {
                id: 7,
                kind: BlockKind::Item(Some(true)),
                spans: vec![],
                children: vec![paragraph(8, "task")],
            }],
        };
        let (_app, renderer, id) = fixture(Document {
            root: Block {
                id: 0,
                kind: BlockKind::Root,
                spans: vec![],
                children: vec![p, table, list],
            },
        });
        let state = renderer.0.borrow();
        let rich = state.dom.range(id).unwrap().rich.as_ref().unwrap();
        assert_eq!(rich.leaves.len(), 3);
        let leaf = &rich.leaves[&1];
        let objects = state.dom.doc.attributed_object_positions(leaf.control);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].3, 160.0);
        assert_eq!(objects[0].4, 120.0);
        assert!(leaf.height >= 120.0);
        for leaf in rich.leaves.values() {
            let rect = state
                .dom
                .doc
                .get_client_bounding_rect(leaf.control)
                .unwrap();
            assert!(rect.width > 10.0 && rect.height >= 24.0, "{rect:?}");
        }
    }
    #[test]
    fn cross_paragraph_selection_cut_and_split_keep_real_buffers_and_format() {
        let document = Document {
            root: Block {
                id: 0,
                kind: BlockKind::Root,
                spans: vec![],
                children: vec![paragraph(1, "one😀"), paragraph(2, "two中")],
            },
        };
        let (_app, renderer, id) = fixture(document);
        let mut state = renderer.0.borrow_mut();
        let selection = Selection {
            anchor: Point {
                block: 2,
                offset: 3,
            },
            focus: Point {
                block: 1,
                offset: 3,
            },
        };
        state.dom.rich_set_selection(id, selection).unwrap();
        assert_eq!(state.dom.rich_selection(id).unwrap(), selection);
        assert_eq!(state.dom.rich_selected_text(id).unwrap(), "😀\ntwo");
        state.dom.rich_replace(id, selection, "X").unwrap();
        let rich = state.dom.range(id).unwrap().rich.as_ref().unwrap();
        assert_eq!(rich.document.root.children.len(), 1);
        assert_eq!(text(&rich.document.root.children[0]), "oneX中");
        state.dom.rich_split(id).unwrap();
        let rich = state.dom.range(id).unwrap().rich.as_ref().unwrap();
        assert_eq!(rich.document.root.children.len(), 2);
        assert_eq!(text(&rich.document.root.children[0]), "oneX");
        assert_eq!(text(&rich.document.root.children[1]), "中");
        assert!(state.dom.rich_join_previous(id).unwrap());
        assert_eq!(
            text(
                &state
                    .dom
                    .range(id)
                    .unwrap()
                    .rich
                    .as_ref()
                    .unwrap()
                    .document
                    .root
                    .children[0]
            ),
            "oneX中"
        );
    }
    #[test]
    fn collapsed_typing_marks_reach_actual_preedit_and_next_input() {
        let (_app, renderer, id) = fixture(Document::default());
        let mut state = renderer.0.borrow_mut();
        let p = Point {
            block: 1,
            offset: 0,
        };
        state
            .dom
            .rich_set_selection(
                id,
                Selection {
                    anchor: p,
                    focus: p,
                },
            )
            .unwrap();
        state
            .dom
            .rich_set_marks(
                id,
                Marks {
                    bold: true,
                    strike: true,
                    ..Marks::default()
                },
            )
            .unwrap();
        let control = state.dom.control(id).unwrap();
        state
            .dom
            .doc
            .with_text_input(control, |mut d| d.set_compose("你", Some((3, 3))));
        let Event::RichTextChanged {
            document,
            composing,
            ..
        } = state.dom.rich_event(id, true, 0).unwrap()
        else {
            panic!()
        };
        assert!(composing);
        assert!(
            matches!(&document.root.children[0].spans[0],Inline::Text{marks,..}if marks.bold&&marks.strike)
        );
        let input = state
            .dom
            .doc
            .get_node(control)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap();
        assert!(input.editor.is_composing());
        assert!(input.editor.style_spans().iter().any(|span|matches!(span.property,StyleProperty::FontWeight(value)if value==FontWeight::BOLD)));
        state.dom.doc.with_text_input(control, |mut d| {
            d.clear_compose();
            d.insert_or_replace_selection("你好");
        });
        let Event::RichTextChanged { document, .. } = state.dom.rich_event(id, false, 0).unwrap()
        else {
            panic!()
        };
        assert_eq!(text(&document.root.children[0]), "你好");
        assert!(
            document.root.children[0]
                .spans
                .iter()
                .all(|s| matches!(s,Inline::Text{marks,..}if marks.bold&&marks.strike))
        );
    }
    #[test]
    fn ime_inherits_existing_marks_without_toolbar_override() {
        let mut doc = Document::default();
        doc.root.children[0].spans = vec![Inline::Text {
            text: "seed".into(),
            marks: Marks {
                bold: true,
                ..Marks::default()
            },
        }];
        let (_app, renderer, id) = fixture(doc);
        let mut state = renderer.0.borrow_mut();
        let p = Point {
            block: 1,
            offset: 0,
        };
        state
            .dom
            .rich_set_selection(
                id,
                Selection {
                    anchor: p,
                    focus: p,
                },
            )
            .unwrap();
        state.dom.rich_inherit_typing(id).unwrap();
        let control = state.dom.control(id).unwrap();
        state
            .dom
            .doc
            .with_text_input(control, |mut d| d.set_compose("你", Some((3, 3))));
        state.dom.rich_event(id, true, 0).unwrap();
        state.dom.doc.with_text_input(control, |mut d| {
            d.clear_compose();
            d.insert_or_replace_selection("你好");
        });
        state.dom.rich_event(id, false, 0).unwrap();
        let doc = &state.dom.range(id).unwrap().rich.as_ref().unwrap().document;
        assert_eq!(text(&doc.root.children[0]), "你好seed");
        assert!(
            doc.root.children[0]
                .spans
                .iter()
                .all(|s| matches!(s,Inline::Text{marks,..}if marks.bold))
        );
    }
    #[test]
    fn empty_list_enter_splits_real_list_and_preserves_following_items() {
        let mut doc = Document::default();
        doc.root.children = vec![Block {
            id: 2,
            kind: BlockKind::List(Some(1)),
            spans: vec![],
            children: vec![
                Block {
                    id: 3,
                    kind: BlockKind::Item(None),
                    spans: vec![Inline::Text {
                        text: "before".into(),
                        marks: Marks::default(),
                    }],
                    children: vec![],
                },
                Block {
                    id: 4,
                    kind: BlockKind::Item(None),
                    spans: vec![],
                    children: vec![],
                },
                Block {
                    id: 5,
                    kind: BlockKind::Item(None),
                    spans: vec![Inline::Text {
                        text: "after".into(),
                        marks: Marks::default(),
                    }],
                    children: vec![],
                },
            ],
        }];
        let (_app, renderer, id) = fixture(doc);
        let mut state = renderer.0.borrow_mut();
        let p = Point {
            block: 4,
            offset: 0,
        };
        state
            .dom
            .rich_set_selection(
                id,
                Selection {
                    anchor: p,
                    focus: p,
                },
            )
            .unwrap();
        state.dom.rich_split(id).unwrap();
        let doc = &state.dom.range(id).unwrap().rich.as_ref().unwrap().document;
        doc.validate().unwrap();
        assert_eq!(doc.root.children.len(), 3);
        assert!(matches!(doc.root.children[1].kind, BlockKind::Paragraph));
        assert_eq!(text(&doc.root.children[0].children[0]), "before");
        assert_eq!(text(&doc.root.children[2].children[0]), "after");
        assert_eq!(state.dom.rich_selection(id).unwrap().focus.block, 4);
    }
    #[test]
    fn cross_leaf_ime_cancel_restores_document_and_commit_replaces_whole_range() {
        let mut original = Document::default();
        original.root.children[0].spans = vec![Inline::Text {
            text: "before".into(),
            marks: Marks {
                bold: true,
                ..Marks::default()
            },
        }];
        original.root.children.push(Block {
            id: 2,
            kind: BlockKind::Paragraph,
            spans: vec![Inline::Text {
                text: "after".into(),
                marks: Marks::default(),
            }],
            children: vec![],
        });
        let (_app, renderer, id) = fixture(original.clone());
        let mut state = renderer.0.borrow_mut();
        let selected = Selection {
            anchor: Point {
                block: 1,
                offset: 2,
            },
            focus: Point {
                block: 2,
                offset: 3,
            },
        };
        for committed in [false, true] {
            state.dom.rich_set_selection(id, selected).unwrap();
            state.dom.rich_prepare_ime(id, true).unwrap();
            let control = state.dom.control(id).unwrap();
            state.dom.ranges.get_mut(&id).unwrap().editing.composing = true;
            state
                .dom
                .doc
                .with_text_input(control, |mut d| d.set_compose("你", Some((3, 3))));
            state.dom.rich_event(id, true, 0).unwrap();
            if committed {
                state.dom.doc.with_text_input(control, |mut d| {
                    d.clear_compose();
                    d.insert_or_replace_selection("你好");
                });
            }
            state.dom.finish_composition(id, committed).unwrap();
            state.dom.rich_event(id, false, 0).unwrap();
            let doc = &state.dom.range(id).unwrap().rich.as_ref().unwrap().document;
            if committed {
                assert_eq!(doc.root.children.len(), 1);
                assert_eq!(text(&doc.root.children[0]), "be你好er");
            } else {
                assert_eq!(doc, &original);
                assert_eq!(state.dom.rich_selection(id).unwrap(), selected);
            }
        }
    }
    #[test]
    fn table_selection_replaces_cell_content_without_removing_structural_cells() {
        let cell = |id, text: &str| Block {
            id,
            kind: BlockKind::Cell,
            spans: vec![Inline::Text {
                text: text.into(),
                marks: Marks::default(),
            }],
            children: vec![],
        };
        let document = Document {
            root: Block {
                id: 0,
                kind: BlockKind::Root,
                spans: vec![],
                children: vec![Block {
                    id: 1,
                    kind: BlockKind::Table(vec![glory_render::rich_text::Alignment::Left; 2]),
                    spans: vec![],
                    children: vec![Block {
                        id: 2,
                        kind: BlockKind::Row,
                        spans: vec![],
                        children: vec![cell(3, "left"), cell(4, "right")],
                    }],
                }],
            },
        };
        let (_app, renderer, id) = fixture(document.clone());
        let mut state = renderer.0.borrow_mut();
        let p = Point {
            block: 4,
            offset: 0,
        };
        state
            .dom
            .rich_set_selection(
                id,
                Selection {
                    anchor: p,
                    focus: p,
                },
            )
            .unwrap();
        assert!(!state.dom.rich_join_previous(id).unwrap());
        let p = Point {
            block: 3,
            offset: 4,
        };
        state
            .dom
            .rich_set_selection(
                id,
                Selection {
                    anchor: p,
                    focus: p,
                },
            )
            .unwrap();
        assert!(!state.dom.rich_join_next(id).unwrap());
        state
            .dom
            .rich_replace(
                id,
                Selection {
                    anchor: Point {
                        block: 3,
                        offset: 2,
                    },
                    focus: Point {
                        block: 4,
                        offset: 3,
                    },
                },
                "X",
            )
            .unwrap();
        let doc = &state.dom.range(id).unwrap().rich.as_ref().unwrap().document;
        doc.validate().unwrap();
        assert_eq!(doc.root.children[0].children[0].children.len(), 2);
        assert_eq!(text(doc.root.find(3).unwrap()), "leX");
        assert_eq!(text(doc.root.find(4).unwrap()), "ht");
    }
    #[test]
    fn delete_at_paragraph_end_joins_next_leaf_and_keeps_caret() {
        let mut document = Document::default();
        document.root.children[0].spans = vec![Inline::Text {
            text: "one".into(),
            marks: Marks::default(),
        }];
        document.root.children.push(Block {
            id: 2,
            kind: BlockKind::Paragraph,
            spans: vec![Inline::Text {
                text: "two".into(),
                marks: Marks {
                    bold: true,
                    ..Marks::default()
                },
            }],
            children: vec![],
        });
        let (_app, renderer, id) = fixture(document);
        let mut state = renderer.0.borrow_mut();
        let p = Point {
            block: 1,
            offset: 3,
        };
        let selection = Selection {
            anchor: p,
            focus: p,
        };
        state.dom.rich_set_selection(id, selection).unwrap();
        assert!(state.dom.rich_join_next(id).unwrap());
        assert_eq!(state.dom.rich_selection(id).unwrap(), selection);
        let doc = &state.dom.range(id).unwrap().rich.as_ref().unwrap().document;
        assert_eq!(doc.root.children.len(), 1);
        assert_eq!(text(&doc.root.children[0]), "onetwo");
        assert!(matches!(&doc.root.children[0].spans[1],Inline::Text{marks,..}if marks.bold));
    }
    #[test]
    fn removing_native_rich_editor_retires_internal_control_owners() {
        let (app, renderer, id) = fixture(Document::default());
        let control = renderer.0.borrow().dom.control(id).unwrap();
        drop(app);
        let state = renderer.0.borrow();
        assert!(!state.dom.controls.contains_key(&control));
        assert!(!state.dom.rich_layout.contains(&id));
    }
}
fn ordered(block: &Block, out: &mut Vec<u64>) {
    if block.editable() {
        out.push(block.id);
    }
    for child in &block.children {
        ordered(child, out);
    }
}
fn byte(text: &str, units: usize) -> usize {
    let mut n = 0;
    for (i, c) in text.char_indices() {
        if n == units {
            return i;
        }
        n += c.len_utf16();
    }
    text.len()
}
fn slice(block: &Block, start: usize, end: usize) -> Vec<Inline> {
    let mut out = Block {
        id: block.id,
        kind: block.kind.clone(),
        spans: vec![],
        children: vec![],
    };
    let mut offset = 0;
    for span in &block.spans {
        let next = offset + span.units();
        if next > start && offset < end {
            match span {
                Inline::Text { text, marks } => {
                    let a = byte(text, start.saturating_sub(offset));
                    let b = byte(text, (end - offset).min(span.units()));
                    out.push(Inline::Text {
                        text: text[a..b].into(),
                        marks: marks.clone(),
                    });
                }
                other => out.push(other.clone()),
            }
        }
        offset = next;
    }
    out.spans
}
fn parent(block: &mut Block, id: u64) -> Option<&mut Block> {
    if block.children.iter().any(|c| c.id == id) {
        return Some(block);
    }
    block.children.iter_mut().find_map(|c| parent(c, id))
}
fn fresh(document: &Document) -> Result<u64, Error> {
    let mut queue = vec![&document.root];
    let mut max = 0;
    while let Some(b) = queue.pop() {
        max = max.max(b.id);
        queue.extend(&b.children);
    }
    max.checked_add(1)
        .ok_or(Error::Unsupported("rich block identities exhausted"))
}
impl Dom {
    pub(super) fn rich_point(&self, id: NodeId, x: f32, y: f32) -> Option<Point> {
        let rich = self.ranges.get(&id)?.rich.as_ref()?;
        let mut closest = None;
        let mut distance = f32::INFINITY;
        for (block, leaf) in &rich.leaves {
            let rect = self.doc.get_client_bounding_rect(leaf.control)?;
            let dy = if y < rect.y as f32 {
                rect.y as f32 - y
            } else {
                (y - (rect.y + rect.height) as f32).max(0.0)
            };
            let dx = if x < rect.x as f32 {
                rect.x as f32 - x
            } else {
                (x - (rect.x + rect.width) as f32).max(0.0)
            };
            if dy * dy + dx * dx < distance {
                distance = dy * dy + dx * dx;
                closest = Some((*block, leaf, rect));
            }
        }
        let (block, leaf, rect) = closest?;
        let input = self
            .doc
            .get_node(leaf.control)?
            .element_data()?
            .text_input_data()?;
        let scale = self.doc.viewport().scale();
        let cursor = blitz_dom::Cursor::from_point(
            input.editor.try_layout()?,
            (x - rect.x as f32) * scale,
            (y - rect.y as f32 + input.scroll_offset) * scale,
        );
        Some(Point {
            block,
            offset: input.editor.raw_text()[..cursor.index()]
                .encode_utf16()
                .count(),
        })
    }
    pub(super) fn rich_replace(
        &mut self,
        id: NodeId,
        selection: Selection,
        value: &str,
    ) -> Result<Selection, Error> {
        let mut document = self.range(id)?.rich.as_ref().unwrap().document.clone();
        document.validate_selection(selection)?;
        let mut order = vec![];
        ordered(&document.root, &mut order);
        let a = order
            .iter()
            .position(|x| *x == selection.anchor.block)
            .unwrap();
        let b = order
            .iter()
            .position(|x| *x == selection.focus.block)
            .unwrap();
        let (start, end, a, b) = if (a, selection.anchor.offset) <= (b, selection.focus.offset) {
            (selection.anchor, selection.focus, a, b)
        } else {
            (selection.focus, selection.anchor, b, a)
        };
        let pending = self.range(id)?.rich.as_ref().unwrap().typing.clone();
        let first = document.root.find(start.block).unwrap();
        let inherited = first
            .spans
            .iter()
            .scan(0, |n, s| {
                *n += s.units();
                Some((*n, s))
            })
            .find_map(|(n, s)| {
                if n >= start.offset {
                    if let Inline::Text { marks, .. } = s {
                        Some(marks.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let marks = pending
            .as_ref()
            .filter(|(point, _)| *point == selection)
            .map(|(_, marks)| marks.clone())
            .unwrap_or(inherited);
        let mut spans = slice(first, 0, start.offset);
        if !value.is_empty() {
            spans.push(Inline::Text {
                text: value.into(),
                marks,
            });
        }
        let last = document.root.find(end.block).unwrap();
        let tail = slice(last, end.offset, last.units());
        let same_parent = parent(&mut document.root, start.block).is_some_and(|p| {
            !matches!(p.kind, BlockKind::Row | BlockKind::TableHead)
                && p.children.iter().any(|c| c.id == end.block)
        });
        if a == b {
            spans.extend(tail);
        } else if same_parent {
            spans.extend(tail);
            let container = parent(&mut document.root, start.block).unwrap();
            let first = container
                .children
                .iter()
                .position(|c| c.id == start.block)
                .unwrap();
            let last = container
                .children
                .iter()
                .position(|c| c.id == end.block)
                .unwrap();
            container.children.drain(first + 1..=last);
        } else {
            for leaf in &order[a + 1..b] {
                document.root.find_mut(*leaf).unwrap().spans.clear();
            }
            document.root.find_mut(end.block).unwrap().spans = tail;
        }
        document.root.find_mut(start.block).unwrap().spans = spans;
        document.validate()?;
        self.rich_write(id, &document)?;
        self.resolve();
        let point = Point {
            block: start.block,
            offset: start.offset + value.encode_utf16().count(),
        };
        let selection = Selection {
            anchor: point,
            focus: point,
        };
        self.rich_set_selection(id, selection)?;
        if let Some((_, marks)) = pending {
            self.ranges
                .get_mut(&id)
                .unwrap()
                .rich
                .as_mut()
                .unwrap()
                .typing = Some((selection, marks));
        }
        Ok(selection)
    }
    pub(super) fn rich_split(&mut self, id: NodeId) -> Result<(), Error> {
        let selection = self.rich_selection(id)?;
        let selection = if selection.anchor != selection.focus {
            self.rich_replace(id, selection, "")?
        } else {
            selection
        };
        let mut document = self.range(id)?.rich.as_ref().unwrap().document.clone();
        let point = selection.focus;
        let leaf = document.root.find(point.block).unwrap();
        if matches!(leaf.kind, BlockKind::Code(_) | BlockKind::Cell) {
            self.rich_replace(id, selection, "\n")?;
            return Ok(());
        }
        if leaf.units() == 0 {
            let item = if matches!(leaf.kind, BlockKind::Item(_)) {
                Some(leaf.id)
            } else {
                parent(&mut document.root, point.block)
                    .filter(|p| matches!(p.kind, BlockKind::Item(_)) && p.children.len() == 1)
                    .map(|p| p.id)
            };
            if let Some(item) = item {
                let next = fresh(&document)?;
                let list = parent(&mut document.root, item).unwrap();
                let list_id = list.id;
                let item_index = list.children.iter().position(|b| b.id == item).unwrap();
                let tail = list.children.split_off(item_index + 1);
                list.children.pop();
                let kind = list.kind.clone();
                let container = parent(&mut document.root, list_id).unwrap();
                let index = container
                    .children
                    .iter()
                    .position(|b| b.id == list_id)
                    .unwrap();
                let position = if container.children[index].children.is_empty() {
                    container.children.remove(index);
                    index
                } else {
                    index + 1
                };
                container.children.insert(
                    position,
                    Block {
                        id: item,
                        kind: BlockKind::Paragraph,
                        spans: vec![],
                        children: vec![],
                    },
                );
                if !tail.is_empty() {
                    container.children.insert(
                        position + 1,
                        Block {
                            id: next,
                            kind,
                            spans: vec![],
                            children: tail,
                        },
                    );
                }
                document.validate()?;
                self.rich_write(id, &document)?;
                self.resolve();
                let point = Point {
                    block: item,
                    offset: 0,
                };
                return self.rich_set_selection(
                    id,
                    Selection {
                        anchor: point,
                        focus: point,
                    },
                );
            }
        }
        let leaf = document.root.find(point.block).unwrap();
        let new_id = fresh(&document)?;
        let right = slice(leaf, point.offset, leaf.units());
        let left = slice(leaf, 0, point.offset);
        let container = parent(&mut document.root, point.block).unwrap();
        let pos = container
            .children
            .iter()
            .position(|b| b.id == point.block)
            .unwrap();
        container.children[pos].spans = left;
        if matches!(container.kind, BlockKind::Item(_)) {
            let item_id = container.id;
            let kind = match container.kind {
                BlockKind::Item(Some(_)) => BlockKind::Item(Some(false)),
                _ => BlockKind::Item(None),
            };
            let mut children = vec![Block {
                id: new_id,
                kind: BlockKind::Paragraph,
                spans: right,
                children: vec![],
            }];
            children.extend(container.children.drain(pos + 1..));
            let new_item = Block {
                id: new_id
                    .checked_add(1)
                    .ok_or(Error::Unsupported("rich identity exhausted"))?,
                kind,
                spans: vec![],
                children,
            };
            let list = parent(&mut document.root, item_id).unwrap();
            let pos = list.children.iter().position(|b| b.id == item_id).unwrap();
            list.children.insert(pos + 1, new_item);
        } else {
            let kind = match container.kind {
                BlockKind::List(_) => BlockKind::Item(None),
                _ => BlockKind::Paragraph,
            };
            container.children.insert(
                pos + 1,
                Block {
                    id: new_id,
                    kind,
                    spans: right,
                    children: vec![],
                },
            );
        }
        document.validate()?;
        self.rich_write(id, &document)?;
        self.resolve();
        let point = Point {
            block: new_id,
            offset: 0,
        };
        self.rich_set_selection(
            id,
            Selection {
                anchor: point,
                focus: point,
            },
        )
    }
    pub(super) fn rich_selected_text(&self, id: NodeId) -> Result<String, Error> {
        let selection = self.rich_selection(id)?;
        let doc = &self.range(id)?.rich.as_ref().unwrap().document;
        let mut order = vec![];
        ordered(&doc.root, &mut order);
        let a = order
            .iter()
            .position(|x| *x == selection.anchor.block)
            .unwrap();
        let b = order
            .iter()
            .position(|x| *x == selection.focus.block)
            .unwrap();
        let (start, end, a, b) = if (a, selection.anchor.offset) <= (b, selection.focus.offset) {
            (selection.anchor, selection.focus, a, b)
        } else {
            (selection.focus, selection.anchor, b, a)
        };
        let mut pieces = vec![];
        for (index, leaf) in order.iter().enumerate().take(b + 1).skip(a) {
            let leaf = doc.root.find(*leaf).unwrap();
            let spans = slice(
                leaf,
                if index == a { start.offset } else { 0 },
                if index == b { end.offset } else { leaf.units() },
            );
            pieces.push(
                spans
                    .iter()
                    .map(|s| match s {
                        Inline::Text { text, .. } => text.as_str(),
                        Inline::HardBreak => "\n",
                        Inline::Image { alternative, .. } => alternative.as_str(),
                    })
                    .collect::<String>(),
            );
        }
        Ok(pieces.join("\n"))
    }
}
impl Dom {
    pub(super) fn rich_join_previous(&mut self, id: NodeId) -> Result<bool, Error> {
        let selection = self.rich_selection(id)?;
        if selection.anchor != selection.focus || selection.focus.offset != 0 {
            return Ok(false);
        }
        let mut document = self.range(id)?.rich.as_ref().unwrap().document.clone();
        let block = selection.focus.block;
        let container = parent(&mut document.root, block).unwrap();
        let index = container
            .children
            .iter()
            .position(|b| b.id == block)
            .unwrap();
        if matches!(container.kind, BlockKind::Row | BlockKind::TableHead)
            || index == 0
            || !container.children[index - 1].editable()
        {
            return Ok(false);
        }
        let removed = container.children.remove(index);
        let previous = &mut container.children[index - 1];
        let point = Point {
            block: previous.id,
            offset: previous.units(),
        };
        for span in removed.spans {
            previous.push(span);
        }
        document.validate()?;
        self.rich_write(id, &document)?;
        self.resolve();
        self.rich_set_selection(
            id,
            Selection {
                anchor: point,
                focus: point,
            },
        )?;
        Ok(true)
    }
    pub(super) fn rich_join_next(&mut self, id: NodeId) -> Result<bool, Error> {
        let selection = self.rich_selection(id)?;
        if selection.anchor != selection.focus {
            return Ok(false);
        }
        let mut document = self.range(id)?.rich.as_ref().unwrap().document.clone();
        let block = selection.focus.block;
        if document.root.find(block).unwrap().units() != selection.focus.offset {
            return Ok(false);
        }
        let container = parent(&mut document.root, block).unwrap();
        let index = container
            .children
            .iter()
            .position(|b| b.id == block)
            .unwrap();
        if matches!(container.kind, BlockKind::Row | BlockKind::TableHead)
            || !container
                .children
                .get(index + 1)
                .is_some_and(Block::editable)
        {
            return Ok(false);
        }
        let next = container.children.remove(index + 1);
        for span in next.spans {
            container.children[index].push(span);
        }
        document.validate()?;
        self.rich_write(id, &document)?;
        self.resolve();
        self.rich_set_selection(id, selection)?;
        Ok(true)
    }
    pub(super) fn rich_adjacent(&self, id: NodeId, forward: bool) -> Result<Option<Point>, Error> {
        let selection = self.rich_selection(id)?;
        let doc = &self.range(id)?.rich.as_ref().unwrap().document;
        let mut order = vec![];
        ordered(&doc.root, &mut order);
        let index = order
            .iter()
            .position(|b| *b == selection.focus.block)
            .unwrap();
        let target = if forward {
            order.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|index| order.get(index))
        };
        Ok(target.map(|block| Point {
            block: *block,
            offset: if forward {
                0
            } else {
                doc.root.find(*block).unwrap().units()
            },
        }))
    }
}
impl Dom {
    pub(super) fn rich_prepare_ime(&mut self, id: NodeId, backup: bool) -> Result<(), Error> {
        self.rich_inherit_typing(id)?;
        let selection = self.rich_selection(id)?;
        if selection.anchor.block != selection.focus.block {
            let document = self.range(id)?.rich.as_ref().unwrap().document.clone();
            self.rich_replace(id, selection, "")?;
            if backup {
                self.ranges
                    .get_mut(&id)
                    .unwrap()
                    .rich
                    .as_mut()
                    .unwrap()
                    .composition_backup = Some((document, selection));
            }
        }
        Ok(())
    }
    pub(super) fn rich_inherit_typing(&mut self, id: NodeId) -> Result<(), Error> {
        if self.range(id)?.rich.as_ref().unwrap().typing.is_some() {
            return Ok(());
        }
        let selection = self.rich_selection(id)?;
        let doc = &self.range(id)?.rich.as_ref().unwrap().document;
        let mut order = vec![];
        ordered(&doc.root, &mut order);
        let a = order
            .iter()
            .position(|id| *id == selection.anchor.block)
            .unwrap();
        let b = order
            .iter()
            .position(|id| *id == selection.focus.block)
            .unwrap();
        let point = if (a, selection.anchor.offset) <= (b, selection.focus.offset) {
            selection.anchor
        } else {
            selection.focus
        };
        let leaf = self
            .range(id)?
            .rich
            .as_ref()
            .unwrap()
            .document
            .root
            .find(point.block)
            .unwrap();
        let marks = leaf
            .spans
            .iter()
            .scan(0, |n, s| {
                *n += s.units();
                Some((*n, s))
            })
            .find_map(|(n, s)| {
                if n >= point.offset {
                    if let Inline::Text { marks, .. } = s {
                        Some(marks.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();
        self.ranges
            .get_mut(&id)
            .unwrap()
            .rich
            .as_mut()
            .unwrap()
            .typing = Some((
            Selection {
                anchor: point,
                focus: point,
            },
            marks,
        ));
        Ok(())
    }
    pub(super) fn rich_set_marks(&mut self, id: NodeId, marks: Marks) -> Result<(), Error> {
        marks.validate()?;
        let selection = self.rich_selection(id)?;
        if selection.anchor != selection.focus {
            return Err(Error::Unsupported(
                "typing marks require a collapsed selection",
            ));
        }
        self.ranges
            .get_mut(&id)
            .unwrap()
            .rich
            .as_mut()
            .unwrap()
            .typing = Some((selection, marks));
        Ok(())
    }
    fn rich_apply_styles(&mut self, id: NodeId, block: u64) -> Result<(), Error> {
        let rich = self.ranges.get_mut(&id).unwrap().rich.as_mut().unwrap();
        let value = rich.document.root.find(block).unwrap();
        let leaf = rich.leaves.get_mut(&block).unwrap();
        let mut styles = vec![];
        let mut used = BTreeSet::new();
        let mut offset = 0;
        for span in &value.spans {
            match span {
                Inline::Text { text, marks } => {
                    let brush =
                        if let Some((id, _)) = leaf.brushes.iter().find(|(_, m)| *m == marks) {
                            *id
                        } else {
                            let mut css = String::from("display:none;");
                            if marks.link.is_some() {
                                css.push_str("color:#2563eb;text-decoration:underline;");
                            }
                            if marks.strike {
                                css.push_str(if marks.link.is_some() {
                                    "text-decoration:underline line-through;"
                                } else {
                                    "text-decoration:line-through;"
                                });
                            }
                            if marks.code {
                                css.push_str("background:#e1e5ec;");
                            }
                            let brush = element(&mut self.doc, leaf.wrapper, "span", &css);
                            leaf.brushes.insert(brush, marks.clone());
                            brush
                        };
                    used.insert(brush);
                    let range = offset..offset + text.len();
                    styles.push(EditorStyleSpan {
                        range: range.clone(),
                        property: StyleProperty::Brush(TextBrush { id: brush }),
                    });
                    if marks.bold {
                        styles.push(EditorStyleSpan {
                            range: range.clone(),
                            property: StyleProperty::FontWeight(FontWeight::BOLD),
                        });
                    }
                    if marks.italic {
                        styles.push(EditorStyleSpan {
                            range: range.clone(),
                            property: StyleProperty::FontStyle(FontStyle::Italic),
                        });
                    }
                    if marks.code {
                        styles.push(EditorStyleSpan {
                            range: range.clone(),
                            property: StyleProperty::FontFamily(
                                blitz_dom::GenericFamily::Monospace.into(),
                            ),
                        });
                    }
                    offset = range.end;
                }
                Inline::HardBreak => offset += 1,
                Inline::Image { .. } => offset += 3,
            }
        }
        let current = text(value);
        self.doc
            .set_attributed_text(leaf.control, &current, styles.clone())
            .map_err(Error::Unsupported)?;
        leaf.styles = styles;
        leaf.text = current;
        let retired: Vec<_> = leaf
            .brushes
            .keys()
            .filter(|id| !used.contains(id))
            .copied()
            .collect();
        for id in retired {
            leaf.brushes.remove(&id);
            self.doc.mutate().remove_and_drop_node(id);
        }
        Ok(())
    }
}
