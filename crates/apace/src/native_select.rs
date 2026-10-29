//! Intrinsic Select owns only its bounded immutable declaration and current
//! physical state. Business sources and semantic children never participate.
use super::*;
use glory_render::{SelectEntry, SelectOption};

pub(super) const CSS: &str = r#"
[data-gly-select]{display:inline-flex;flex-direction:column;box-sizing:border-box;min-width:120px;min-height:30px;position:relative;border:1px solid #888;background:#fff;color:#202020;font:inherit;user-select:none}
[data-gly-select]:disabled{color:#777}
[data-gly-select-caption]{display:block;padding:4px 24px 4px 8px;white-space:pre-wrap;overflow:hidden}
[data-gly-select-arrow]{position:absolute;right:8px;top:12px;width:0;height:0;border-left:4px solid transparent;border-right:4px solid transparent;border-top:5px solid currentColor}
[data-gly-select-panel]{display:block;box-sizing:border-box;overflow:auto;overscroll-behavior:contain;background:inherit;color:inherit;min-width:0;flex-shrink:0}
[data-gly-select-panel=closed]{display:none}
[data-gly-select-panel=popup]{position:fixed;border:1px solid #888;background:#fff;color:#202020}
[data-gly-select-row]{display:block;box-sizing:border-box;min-height:28px;padding:4px 8px;white-space:pre-wrap}
[data-gly-select-row][data-selected]{background:#2358a5;color:white}
[data-gly-select-row][data-active]{outline:1px solid #2358a5;outline-offset:-1px}
[data-gly-select-row][data-disabled]{color:#888}
[data-gly-select-group]{display:block;padding:4px 6px;font-weight:bold;color:#666}
"#;

pub(super) struct Row {
    pub physical: EngineId,
    entry: usize,
    child: Option<usize>,
    pub selected: bool,
}
pub(super) struct Control {
    pub options: glory_html::SelectOptions,
    pub rows: Vec<Row>,
    pub panel: EngineId,
    caption: Option<EngineId>,
    #[cfg(feature = "shell")]
    pub inline: bool,
    #[cfg(feature = "shell")]
    pub multiple: bool,
    pub cursor: Option<usize>,
    #[cfg(feature = "shell")]
    anchor: Option<usize>,
    #[cfg(feature = "shell")]
    search: String,
    #[cfg(feature = "shell")]
    searched_at: Option<std::time::Instant>,
}
impl Control {
    pub fn option(&self, index: usize) -> (&SelectOption, bool) {
        let row = &self.rows[index];
        match &self.options.entries[row.entry] {
            SelectEntry::Option(option) => (option, option.disabled),
            SelectEntry::Group(group) => {
                let option = &group.options[row.child.expect("group row")];
                (option, group.disabled || option.disabled)
            }
        }
    }
    pub fn create(
        doc: &mut BaseDocument,
        main: EngineId,
        options: glory_html::SelectOptions,
    ) -> Self {
        let element = doc.get_node(main).expect("select main");
        let multiple = element.attr(attr("multiple").local).is_some();
        let size = element
            .attr(attr("size").local)
            .and_then(|s| s.parse::<u16>().ok());
        let inline = multiple || size.is_some_and(|n| n > 1);
        let mut m = doc.mutate();
        m.set_attribute(main, attr("data-gly-select"), "");
        let caption = (!inline).then(|| {
            let label = m.create_element(tag("span"), Vec::new());
            m.set_attribute(label, attr("data-gly-select-caption"), "");
            let text = m.create_text_node("");
            m.append_children(label, &[text]);
            m.append_children(main, &[label]);
            let arrow = m.create_element(tag("span"), Vec::new());
            m.set_attribute(arrow, attr("data-gly-select-arrow"), "");
            m.append_children(main, &[arrow]);
            text
        });
        let panel = m.create_element(tag("div"), Vec::new());
        m.set_attribute(
            panel,
            attr("data-gly-select-panel"),
            if inline { "inline" } else { "closed" },
        );
        if inline {
            m.set_attribute(
                panel,
                attr("style"),
                &format!("height:{}px", u32::from(size.unwrap_or(4)) * 28),
            );
        }
        m.append_children(main, &[panel]);
        let mut rows = Vec::new();
        for (entry, value) in options.entries.iter().enumerate() {
            let (items, group_disabled) = match value {
                SelectEntry::Option(option) => (std::slice::from_ref(option), false),
                SelectEntry::Group(group) => {
                    let label = m.create_element(tag("div"), Vec::new());
                    m.set_attribute(label, attr("data-gly-select-group"), "");
                    let text = m.create_text_node(&group.label);
                    m.append_children(label, &[text]);
                    m.append_children(panel, &[label]);
                    (group.options.as_slice(), group.disabled)
                }
            };
            for (index, option) in items.iter().enumerate() {
                let physical = m.create_element(tag("div"), Vec::new());
                m.set_attribute(
                    physical,
                    attr("data-gly-select-row"),
                    &rows.len().to_string(),
                );
                if group_disabled || option.disabled {
                    m.set_attribute(physical, attr("data-disabled"), "");
                }
                let text = m.create_text_node(&option.label);
                m.append_children(physical, &[text]);
                m.append_children(panel, &[physical]);
                rows.push(Row {
                    physical,
                    entry,
                    child: matches!(value, SelectEntry::Group(_)).then_some(index),
                    selected: false,
                });
            }
        }
        Self {
            options,
            rows,
            panel,
            caption,
            #[cfg(feature = "shell")]
            inline,
            #[cfg(feature = "shell")]
            multiple,
            cursor: None,
            #[cfg(feature = "shell")]
            anchor: None,
            #[cfg(feature = "shell")]
            search: String::new(),
            #[cfg(feature = "shell")]
            searched_at: None,
        }
    }
    fn paint(&self, doc: &mut BaseDocument) {
        let mut m = doc.mutate();
        for (index, row) in self.rows.iter().enumerate() {
            if row.selected {
                m.set_attribute(row.physical, attr("data-selected"), "");
            } else {
                m.clear_attribute(row.physical, attr("data-selected"));
            }
            if self.cursor == Some(index) {
                m.set_attribute(row.physical, attr("data-active"), "");
            } else {
                m.clear_attribute(row.physical, attr("data-active"));
            }
        }
        if let Some(caption) = self.caption {
            let text = self
                .rows
                .iter()
                .position(|row| row.selected)
                .map(|i| self.option(i).0.label.as_str())
                .unwrap_or_else(|| self.options.placeholder.as_deref().unwrap_or(""));
            m.set_node_text(caption, text);
        }
    }
    #[cfg(feature = "shell")]
    pub fn selected_keys(&self) -> Vec<glory_render::SemanticId> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.selected)
            .map(|(i, _)| self.option(i).0.key.clone())
            .collect()
    }
    #[cfg(feature = "shell")]
    fn first(&self, last: bool) -> Option<usize> {
        if last {
            (0..self.rows.len()).rev().find(|i| !self.option(*i).1)
        } else {
            (0..self.rows.len()).find(|i| !self.option(*i).1)
        }
    }
}

pub(super) fn reflect(dom: &mut Dom, id: NodeId, keys: &[String]) -> Result<(), Error> {
    let range = dom.ranges.get_mut(&id).ok_or(Error::NodeGone(id))?;
    let control = range
        .select
        .as_mut()
        .ok_or_else(|| invariant("selectedKeys on non-select"))?;
    let mut requested = keys.iter().peekable();
    for index in 0..control.rows.len() {
        let selected = requested
            .peek()
            .is_some_and(|key| key.as_str() == control.option(index).0.key.as_str());
        if selected {
            requested.next();
        }
        control.rows[index].selected = selected;
    }
    if requested.next().is_some() {
        return Err(invariant("invalid selected key order"));
    }
    range.editing.revision = range.editing.requested;
    control.paint(&mut dom.doc);
    dom.property_writes += 1;
    dom.invalidate();
    Ok(())
}

pub(super) fn close(dom: &mut Dom) {
    let Some(id) = dom.open_select.take() else {
        return;
    };
    let Some(control) = dom.ranges.get(&id).and_then(|r| r.select.as_ref()) else {
        return;
    };
    let panel = control.panel;
    // The physical subtree may already have been removed by a successful prefix.
    if dom.doc.get_node(panel).is_some() {
        let _ = dom.doc.set_top_layer(panel, None);
        dom.doc
            .mutate()
            .set_attribute(panel, attr("data-gly-select-panel"), "closed");
        dom.invalidate();
    }
}

pub(super) fn available(state: &State, id: NodeId) -> bool {
    let tree = state.mapper.tree();
    tree.active(id)
        && tree.enabled(id)
        && !tree.inert(id)
        && layers::visible(tree, id)
        && state.layers.allowed(tree, id)
        && focus_scope::allowed(state, id)
        && tree.node(id).is_ok_and(|n| !n.descriptor.view().readonly())
        && state
            .dom
            .ranges
            .get(&id)
            .is_some_and(|r| r.select.is_some())
}
pub(super) fn sync(state: &mut State) {
    if let Some(id) = state.dom.open_select
        && (!available(state, id) || state.interactive && state.focused != Some(id))
    {
        close(&mut state.dom);
    }
}

// Refresh only the one current popup. This is also called after viewport/layout
// changes, without walking unrelated controls or replacing their declarations.
pub(super) fn position(dom: &mut Dom) -> bool {
    let Some((id, control)) = dom.open_select.and_then(|id| {
        dom.ranges
            .get(&id)
            .and_then(|r| r.select.as_ref())
            .map(|s| (id, s))
    }) else {
        return false;
    };
    let Some(main_id) = dom.main(id).ok() else {
        return false;
    };
    let Some(mut main) = dom.doc.get_client_bounding_rect(main_id) else {
        return false;
    };
    if let Some(host) = dom.doc.get_node(main_id) {
        main.x += host.scroll_offset().x;
        main.y += host.scroll_offset().y;
    }
    let (vw, vh) = dom.doc.viewport().logical_size();
    let width = main.width.max(120.).min(f64::from(vw).max(0.));
    let x = main.x.max(0.).min((f64::from(vw) - width).max(0.));
    let below = (f64::from(vh) - main.y - main.height).clamp(0., f64::from(vh).max(0.));
    let above = main.y.max(0.).min(f64::from(vh));
    let down = below >= 160. || below >= above;
    let height = if down { below } else { above }.min(288.);
    let y = if down {
        main.y + main.height
    } else {
        main.y - height
    }
    .clamp(0., (f64::from(vh) - height).max(0.));
    let css = format!("left:{x}px;top:{y}px;width:{width}px;max-height:{height}px");
    let panel = control.panel;
    if dom
        .doc
        .get_node(panel)
        .and_then(|n| n.attr(attr("style").local))
        == Some(css.as_str())
    {
        return false;
    }
    dom.doc.mutate().set_attribute(panel, attr("style"), &css);
    true
}

#[cfg(any(feature = "shell", feature = "accesskit"))]
pub(super) fn open(state: &mut State, id: NodeId) -> Result<(), Error> {
    if !available(state, id) {
        return Ok(());
    }
    let control = state.dom.range(id)?.select.as_ref().expect("select");
    if control.inline || state.dom.open_select == Some(id) {
        return Ok(());
    }
    let panel = control.panel;
    // Admission precedes closing the previous lease, so a rejected open is inert.
    state
        .dom
        .doc
        .set_top_layer(
            panel,
            Some(apace_dom::TopLayerConfig {
                modal: false,
                contain_scroll: true,
                backdrop: None,
            }),
        )
        .map_err(|_| invariant("select popup admission"))?;
    close(&mut state.dom);
    state
        .dom
        .doc
        .mutate()
        .set_attribute(panel, attr("data-gly-select-panel"), "popup");
    state.dom.open_select = Some(id);
    let range = state.dom.ranges.get_mut(&id).expect("live select");
    let control = range.select.as_mut().expect("select");
    control.cursor = control
        .rows
        .iter()
        .enumerate()
        .find(|(i, row)| row.selected && !control.option(*i).1)
        .map(|(i, _)| i)
        .or_else(|| control.first(false));
    control.paint(&mut state.dom.doc);
    state.dom.invalidate();
    state.dom.resolve();
    reveal(state, id);
    Ok(())
}

#[cfg(any(feature = "shell", feature = "accesskit"))]
pub(super) fn choose(
    state: &mut State,
    id: NodeId,
    index: usize,
    toggle: bool,
    extend: bool,
) -> Result<bool, Error> {
    if !available(state, id) {
        return Ok(false);
    }
    let range = state.dom.ranges.get_mut(&id).ok_or(Error::NodeGone(id))?;
    let control = range.select.as_mut().expect("select");
    if index >= control.rows.len() || control.option(index).1 {
        return Ok(false);
    }
    let before = control.selected_keys();
    let anchor = control.anchor.unwrap_or(index);
    let was_selected = control.rows[index].selected;
    for i in 0..control.rows.len() {
        if control.option(i).1 {
            continue;
        }
        control.rows[i].selected = if control.multiple && extend {
            (anchor.min(index)..=anchor.max(index)).contains(&i)
                || toggle && control.rows[i].selected
        } else if control.multiple && toggle {
            if i == index {
                !was_selected
            } else {
                control.rows[i].selected
            }
        } else {
            i == index
        };
    }
    // Single controls may programmatically select a disabled option; user
    // selection replaces it, while multiple operations preserve disabled values.
    if !control.multiple {
        for (i, row) in control.rows.iter_mut().enumerate() {
            row.selected = i == index;
        }
    }
    control.cursor = Some(index);
    if !extend {
        control.anchor = Some(index);
    }
    let keys = control.selected_keys();
    control.paint(&mut state.dom.doc);
    if keys != before {
        state.inputs.push(Input::Event {
            node: id,
            event: Event::SelectChanged {
                keys,
                revision: range.editing.revision,
            },
        });
    }
    state.dom.invalidate();
    reveal(state, id);
    Ok(true)
}

#[cfg(any(feature = "shell", feature = "accesskit"))]
pub(super) fn reveal(state: &mut State, id: NodeId) {
    let Some(index) = state
        .dom
        .ranges
        .get(&id)
        .and_then(|r| r.select.as_ref())
        .and_then(|c| c.cursor)
    else {
        return;
    };
    reveal_row(state, id, index);
}

#[cfg(any(feature = "shell", feature = "accesskit"))]
pub(super) fn reveal_row(state: &mut State, id: NodeId, index: usize) {
    state.dom.resolve();
    let Some(control) = state.dom.ranges.get(&id).and_then(|r| r.select.as_ref()) else {
        return;
    };
    if index >= control.rows.len() {
        return;
    }
    if !control.inline && state.dom.open_select != Some(id) {
        return;
    }
    let panel = control.panel;
    let row = control.rows[index].physical;
    let (Some(row), Some(outer)) = (
        state.dom.doc.get_client_bounding_rect(row),
        state.dom.doc.get_client_bounding_rect(panel),
    ) else {
        return;
    };
    let host = state.dom.doc.get_node(panel).expect("select panel");
    // Apace includes the queried node's own scroll in its client rect. Undo
    // only that term; the row must keep the panel/ancestor scroll transform.
    let layout = host.final_layout();
    let top = outer.y + host.scroll_offset().y + f64::from(layout.border.top);
    let height = f64::from(
        (layout.size.height
            - layout.border.top
            - layout.border.bottom
            - layout.scrollbar_size.height)
            .max(0.),
    );
    let dy = scroll_into_view::nearest(row.y - top, row.height, height);
    if dy != 0. {
        state
            .dom
            .doc
            .scroll_by(panel, 0., dy, ScrollBehavior::Instant);
        state.dom.invalidate();
    }
}

#[cfg(feature = "shell")]
pub(super) fn popup_target(state: &State, mut target: EngineId) -> bool {
    let Some(panel) = state
        .dom
        .open_select
        .and_then(|id| state.dom.ranges.get(&id))
        .and_then(|r| r.select.as_ref())
        .map(|s| s.panel)
    else {
        return false;
    };
    loop {
        if target == panel {
            return true;
        }
        let Some(parent) = state.dom.doc.get_node(target).and_then(|n| n.parent) else {
            return false;
        };
        target = parent;
    }
}
#[cfg(feature = "shell")]
fn row_at(state: &State, id: NodeId, mut target: EngineId) -> Option<usize> {
    let control = state.dom.range(id).ok()?.select.as_ref()?;
    loop {
        let n = state.dom.doc.get_node(target)?;
        if let Some(index) = n
            .attr(attr("data-gly-select-row").local)
            .and_then(|s| s.parse::<usize>().ok())
        {
            return control
                .rows
                .get(index)
                .filter(|r| r.physical == target)
                .map(|_| index);
        }
        if target == control.panel {
            return None;
        }
        target = n.parent?;
    }
}
#[cfg(feature = "shell")]
pub(super) fn receive(
    state: &mut State,
    target: EngineId,
    event: &apace_traits::events::DomEventData,
) -> bool {
    use apace_traits::events::DomEventData as D;
    let id = state
        .dom
        .owner(target)
        .filter(|id| state.dom.ranges.get(id).is_some_and(|r| r.select.is_some()));
    if matches!(event, D::PointerDown(_)) {
        if state.dom.open_select.is_some() && id != state.dom.open_select {
            close(&mut state.dom);
        }
        if let Some(id) = id.filter(|id| available(state, *id))
            && let Err(error) = state.focus(id)
        {
            state.input_error = Some(error);
        }
        // Engine down/move/up owns touch pan and suppresses Click after a pan.
        return false;
    }
    if matches!(event, D::PointerCancel(_)) {
        close(&mut state.dom);
        return false;
    }
    let Some(id) = id else { return false };
    if let D::Click(pointer) = event {
        if available(state, id) {
            let result = if let Some(index) = row_at(state, id, target) {
                let result = choose(state, id, index, true, pointer.mods.shift());
                if result == Ok(true)
                    && !state
                        .dom
                        .range(id)
                        .expect("select")
                        .select
                        .as_ref()
                        .expect("select")
                        .inline
                {
                    close(&mut state.dom);
                }
                result.map(|_| ())
            } else if popup_target(state, target) {
                // Group labels and panel whitespace are not option activations.
                Ok(())
            } else if state.dom.open_select == Some(id) {
                close(&mut state.dom);
                Ok(())
            } else {
                open(state, id)
            };
            if let Err(error) = result {
                state.input_error = Some(error);
            }
        }
        return true;
    }
    if let D::KeyDown(key) | D::KeyUp(key) = event {
        return key_event(state, id, key);
    }
    false
}

#[cfg(feature = "shell")]
fn key_event(state: &mut State, id: NodeId, key: &apace_traits::events::ApaceKeyEvent) -> bool {
    use keyboard_types::Modifiers as M;
    if state.focused != Some(id) {
        return false;
    }
    let name = key.key.to_string();
    if name == "Tab" {
        close(&mut state.dom);
        return false;
    }
    if name == "Escape" && state.dom.open_select != Some(id) {
        return false;
    }
    let recognized = matches!(
        name.as_str(),
        "ArrowDown" | "ArrowUp" | "Home" | "End" | "PageDown" | "PageUp" | "Enter" | " " | "Escape"
    );
    if !key.state.is_pressed() {
        return recognized;
    }
    if !available(state, id) || key.is_composing {
        return recognized;
    }
    let chord = key.modifiers.intersects(M::CONTROL | M::META);
    let shift = key.modifiers.contains(M::SHIFT);
    let alt = key.modifiers.contains(M::ALT);
    if name == "Escape" {
        close(&mut state.dom);
        return true;
    }
    let control = state
        .dom
        .range(id)
        .expect("select")
        .select
        .as_ref()
        .expect("select");
    let inline = control.inline;
    let open_now = state.dom.open_select == Some(id);
    if matches!(name.as_str(), "Enter" | " ") {
        if !inline && !open_now {
            if let Err(error) = open(state, id) {
                state.input_error = Some(error);
            }
        } else if let Some(index) = control.cursor.or_else(|| control.first(false))
            && let Err(error) = choose(state, id, index, true, shift)
        {
            state.input_error = Some(error);
        }
        if !inline && open_now {
            close(&mut state.dom);
        }
        return true;
    }
    if alt && name == "ArrowUp" {
        close(&mut state.dom);
        return true;
    }
    if alt && name == "ArrowDown" {
        if let Err(error) = open(state, id) {
            state.input_error = Some(error);
        }
        return true;
    }
    if chord && !alt && !shift && control.multiple && name.eq_ignore_ascii_case("a") {
        let range = state.dom.ranges.get_mut(&id).expect("select");
        let control = range.select.as_mut().expect("select");
        let before = control.selected_keys();
        for i in 0..control.rows.len() {
            if !control.option(i).1 {
                control.rows[i].selected = true;
            }
        }
        let keys = control.selected_keys();
        control.paint(&mut state.dom.doc);
        if keys != before {
            state.inputs.push(Input::Event {
                node: id,
                event: Event::SelectChanged {
                    keys,
                    revision: range.editing.revision,
                },
            });
        }
        state.dom.invalidate();
        return true;
    }
    if alt || chord && !recognized {
        return false;
    }
    let current = control
        .cursor
        .or_else(|| control.rows.iter().position(|r| r.selected));
    let destination = match name.as_str() {
        "Home" => control.first(false),
        "End" => control.first(true),
        "ArrowDown" | "PageDown" => {
            let steps = if name == "PageDown" { 8 } else { 1 };
            let start = current.map_or(0, |i| i.saturating_add(steps));
            (start..control.rows.len())
                .find(|i| !control.option(*i).1)
                .or_else(|| control.first(true))
        }
        "ArrowUp" | "PageUp" => {
            let steps = if name == "PageUp" { 8 } else { 1 };
            current
                .and_then(|i| i.checked_sub(steps))
                .and_then(|i| (0..=i).rev().find(|i| !control.option(*i).1))
                .or_else(|| control.first(false))
        }
        _ => {
            if name.chars().count() != 1 || name.chars().any(char::is_control) {
                return false;
            }
            let now = std::time::Instant::now();
            let control = state
                .dom
                .ranges
                .get_mut(&id)
                .expect("select")
                .select
                .as_mut()
                .expect("select");
            if control
                .searched_at
                .is_none_or(|at| now.duration_since(at).as_millis() > 700)
            {
                control.search.clear();
            }
            let fragment = name.to_lowercase();
            if control.search.len() + fragment.len() > 256 {
                control.search.clear();
            }
            control.search.push_str(&fragment);
            control.searched_at = Some(now);
            let same = control
                .search
                .chars()
                .all(|c| control.search.starts_with(c));
            let query = if same {
                name.to_lowercase()
            } else {
                control.search.clone()
            };
            let start = current.map_or(0, |i| if same { i + 1 } else { i });
            (0..control.rows.len())
                .map(|offset| (start + offset) % control.rows.len())
                .find(|i| {
                    !control.option(*i).1
                        && control
                            .option(*i)
                            .0
                            .label
                            .to_lowercase()
                            .starts_with(&query)
                })
        }
    };
    if let Some(index) = destination {
        if open_now || inline && chord && !shift {
            let control = state
                .dom
                .ranges
                .get_mut(&id)
                .expect("select")
                .select
                .as_mut()
                .expect("select");
            control.cursor = Some(index);
            control.paint(&mut state.dom.doc);
            state.dom.invalidate();
            reveal(state, id);
        } else if let Err(error) = choose(state, id, index, chord, shift) {
            state.input_error = Some(error);
        }
    }
    true
}
