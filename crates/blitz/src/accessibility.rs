//! The AccessKit tree follows live semantic identities, not physical wrapper IDs.
//! IDs are never recycled within a document; only the live snapshot is retained.
use super::*;
use ::accesskit::{
    Action as AxAction, ActionData, ActionRequest, Affine, Live, Node, NodeId as AxId,
    Role as AxRole, Toggled, TreeId, TreeInfo, TreeUpdate,
};
use glory_render::{FocusMode, Props, Purpose, Role, TextKind};
use std::collections::{BTreeMap, HashMap, HashSet};

const ROOT: AxId = AxId(0);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key {
    Main(NodeId),
    Label(NodeId),
    TreeChildren(NodeId),
}

struct Relations {
    owner: AxId,
    controls: Option<AxId>,
    active_descendant: Option<AxId>,
    label: Option<AxId>,
    description: Option<AxId>,
    error: Option<AxId>,
    label_text: Option<String>,
    description_text: Option<String>,
    popup: Option<glory_render::PopupKind>,
}

#[derive(Default)]
pub(super) struct Tree {
    next: u64,
    ids: HashMap<Key, AxId>,
    reverse: HashMap<AxId, Key>,
    previous: BTreeMap<AxId, Node>,
    pending_reveals: Vec<NodeId>,
}
impl Tree {
    pub(super) fn clear(&mut self) {
        self.ids.clear();
        self.reverse.clear();
        self.previous.clear();
        self.pending_reveals.clear();
        // A late platform action must never address a new node after reactivation.
    }
    fn id(&mut self, key: Key) -> Result<AxId, Error> {
        if let Some(id) = self.ids.get(&key) {
            return Ok(*id);
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or(Error::InvalidProps("AccessKit identity space exhausted"))?;
        let id = AxId(self.next);
        self.ids.insert(key, id);
        self.reverse.insert(id, key);
        Ok(id)
    }
    pub(super) fn build(
        &mut self,
        state: &mut State,
        full: bool,
        inset: (f64, f64),
        title: &str,
    ) -> Result<TreeUpdate, Error> {
        state.dom.resolve();
        // The UI controller has now committed the logical destination. Only
        // then may native ancestors/viewport use physical row geometry.
        for id in std::mem::take(&mut self.pending_reveals) {
            if state.mapper.tree().active(id) && revealable(state, id) {
                reveal_native(state, id)?;
            }
        }
        state.sync_focus();
        let mut nodes = BTreeMap::new();
        let mut root = Node::new(AxRole::Window);
        root.set_label(title);
        root.set_transform(Affine::new([
            state.dom.doc.viewport().scale_f64(),
            0.0,
            0.0,
            state.dom.doc.viewport().scale_f64(),
            inset.0,
            inset.1,
        ]));
        nodes.insert(ROOT, root);
        let mut alive = HashSet::new();
        let mut totals: HashMap<AxId, (Option<usize>, bool)> = HashMap::new();
        let mut tree_items = Vec::new();
        let mut pending: Vec<_> = state
            .mapper
            .tree()
            .roots()
            .iter()
            .rev()
            .map(|id| (*id, ROOT, None))
            .collect();
        while let Some((id, parent, list)) = pending.pop() {
            let semantic = state.mapper.tree().node(id)?;
            if semantic.suspended
                || !state.layers.ax_branch(state.mapper.tree(), id)
                || semantic
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.inert)
            {
                continue;
            }
            let d = semantic.descriptor.view();
            if !state.layers.allowed(state.mapper.tree(), id) {
                pending.extend(
                    semantic
                        .children
                        .iter()
                        .rev()
                        .map(|child| (*child, parent, list)),
                );
                continue;
            }
            if matches!(d.props, Props::Group) {
                pending.extend(
                    semantic
                        .children
                        .iter()
                        .rev()
                        .map(|child| (*child, parent, list)),
                );
                continue;
            }
            let physical = state.dom.main(id)?;
            let host = state
                .dom
                .doc
                .get_node(physical)
                .ok_or(Error::NodeGone(id))?;
            if !host.flags.is_in_document()
                || host
                    .primary_styles()
                    .is_some_and(|s| s.clone_display().is_none())
            {
                continue;
            }
            if host
                .primary_styles()
                .is_some_and(|s| s.clone_visibility() != Visibility::Visible)
            {
                // Visibility can be overridden by a descendant, unlike display:none.
                pending.extend(
                    semantic
                        .children
                        .iter()
                        .rev()
                        .map(|child| (*child, parent, list)),
                );
                continue;
            }
            if matches!(&d.props, Props::Image(p) if p.alternative.is_none()) {
                continue;
            }
            if matches!(d.props, Props::Graphic(p) if p.alternative.is_none()) {
                continue;
            }
            let key = Key::Main(id);
            let ax_id = self.id(key)?;
            alive.insert(key);
            let mut node = Node::new(role(d));
            if d.role == Some(Role::Separator) {
                node.set_orientation(
                    match d
                        .semantics
                        .as_ref()
                        .and_then(|s| s.separator_axis)
                        .unwrap_or(glory_render::Orientation::Horizontal)
                    {
                        glory_render::Orientation::Horizontal => accesskit::Orientation::Horizontal,
                        glory_render::Orientation::Vertical => accesskit::Orientation::Vertical,
                    },
                );
            }
            if let Some(name) = &d.name {
                node.set_label(normalize_text(name, true).into_owned());
            }
            if !d.activation_enabled() || !state.mapper.tree().enabled(id) {
                node.set_disabled();
            }
            if let Some(s) = &d.semantics {
                if s.layer.as_ref().is_some_and(|layer| layer.modal) {
                    node.set_modal();
                }
                if let Some(sort) = s.sort {
                    use glory_render::SortDirection;
                    match sort {
                        SortDirection::None => {}
                        SortDirection::Ascending => {
                            node.set_sort_direction(accesskit::SortDirection::Ascending)
                        }
                        SortDirection::Descending => {
                            node.set_sort_direction(accesskit::SortDirection::Descending)
                        }
                    }
                }
                if let Some(table) = &s.table {
                    use glory_render::TableMetadata;
                    match table {
                        TableMetadata::Table { rows, columns } => {
                            if let Some(rows) = rows.filter(|rows| *rows <= i32::MAX as u32) {
                                node.set_row_count(rows as usize);
                            }
                            if let Some(columns) = columns {
                                node.set_column_count(usize::from(*columns));
                            }
                        }
                        TableMetadata::Row { index } => {
                            if *index < i32::MAX as u32 {
                                node.set_row_index(*index as usize);
                            }
                        }
                        TableMetadata::Cell {
                            row,
                            column,
                            row_span,
                            column_span,
                            ..
                        } => {
                            if *row < i32::MAX as u32 {
                                node.set_row_index(*row as usize);
                            }
                            node.set_column_index(usize::from(*column));
                            node.set_row_span(usize::from(*row_span));
                            node.set_column_span(usize::from(*column_span));
                        }
                    }
                }
                if let Some(spin) = &s.spinbutton {
                    if let Some(min) = spin.min {
                        node.set_min_numeric_value(min);
                    }
                    if let Some(max) = spin.max {
                        node.set_max_numeric_value(max);
                    }
                    if let Some(value) = spin.value {
                        node.set_numeric_value(value);
                    }
                    node.set_numeric_value_step(spin.step);
                    if let Some(text) = &spin.value_text {
                        node.set_value(normalize_text(text, true).into_owned());
                    }
                }
                if let Some(slider) = &s.slider {
                    node.set_min_numeric_value(slider.range.min);
                    node.set_max_numeric_value(slider.range.max);
                    node.set_numeric_value(slider.range.value);
                    node.set_numeric_value_step(slider.step);
                    node.set_orientation(match slider.axis {
                        glory_render::Orientation::Horizontal => accesskit::Orientation::Horizontal,
                        glory_render::Orientation::Vertical => accesskit::Orientation::Vertical,
                    });
                    if let Some(text) = &slider.value_text {
                        node.set_value(normalize_text(text, true).into_owned());
                    }
                }
                if let Some(range) = s.splitter {
                    node.set_min_numeric_value(range.min);
                    node.set_max_numeric_value(range.max);
                    node.set_numeric_value(range.value);
                }
                if let Some(progress) = &s.progress {
                    node.set_min_numeric_value(progress.min);
                    node.set_max_numeric_value(progress.max);
                    if let Some(current) = progress.current {
                        node.set_numeric_value(current);
                    }
                    if let Some(text) = &progress.value_text {
                        node.set_value(normalize_text(text, true).into_owned());
                    }
                    node.set_read_only();
                }
                if s.busy == Some(true) {
                    node.set_busy();
                }
                if let Some(current) = s.current {
                    node.set_aria_current(match current {
                        glory_render::Current::Page => accesskit::AriaCurrent::Page,
                        glory_render::Current::Step => accesskit::AriaCurrent::Step,
                        glory_render::Current::Location => accesskit::AriaCurrent::Location,
                        glory_render::Current::Date => accesskit::AriaCurrent::Date,
                    });
                }
                if s.grid_multiple {
                    node.set_multiselectable();
                }
                if let Some(tree) = s.tree_navigation
                    && tree.multiple
                {
                    node.set_multiselectable();
                }
                if let Some(item) = s.tree_item {
                    node.set_level(usize::from(item.level - 1));
                    node.set_position_in_set(usize::from(item.position));
                    let mut ancestor = semantic.parent;
                    while let Some(current) = ancestor {
                        let parent_node = state.mapper.tree().node(current)?;
                        if parent_node.descriptor.view().role == Some(Role::Tree) {
                            if let Some(tree) = self.ids.get(&Key::Main(current)).copied() {
                                tree_items.push((id, ax_id, parent, tree, item));
                            }
                            break;
                        }
                        ancestor = parent_node.parent;
                    }
                }
                if s.listbox_multiple {
                    node.set_multiselectable();
                }
                if s.required {
                    node.set_required();
                }
                if s.readonly {
                    node.set_read_only();
                }
                if s.invalid {
                    node.set_invalid(accesskit::Invalid::True);
                }
                if let Some(expanded) = s.expanded {
                    node.set_expanded(expanded);
                }
                if let Some(selected) = s.selected {
                    node.set_selected(selected);
                }
                if let Some(checked) = s.tree_checked {
                    node.set_toggled(match checked {
                        glory_render::TreeCheckState::Unchecked => Toggled::False,
                        glory_render::TreeCheckState::Checked => Toggled::True,
                        glory_render::TreeCheckState::Mixed => Toggled::Mixed,
                    });
                }
                if let Some(pressed) = s.pressed {
                    node.set_toggled(if pressed {
                        Toggled::True
                    } else {
                        Toggled::False
                    });
                }
                if matches!(
                    d.role,
                    Some(Role::TabList | Role::Toolbar | Role::RadioGroup)
                ) && let Some(axis) = s.navigation
                {
                    node.set_orientation(match axis {
                        glory_render::Orientation::Horizontal => {
                            ::accesskit::Orientation::Horizontal
                        }
                        glory_render::Orientation::Vertical => ::accesskit::Orientation::Vertical,
                    });
                }
            }
            if let Some(Role::Heading(level)) = d.role {
                node.set_level(usize::from(level - 1));
            }
            if let Some(rect) = state.dom.doc.get_client_bounding_rect(physical) {
                let own = *host.scroll_offset();
                node.set_bounds(::accesskit::Rect::new(
                    rect.x + own.x,
                    rect.y + own.y,
                    rect.x + own.x + rect.width,
                    rect.y + own.y + rect.height,
                ));
            }
            if let Some(item) = d.item {
                // UIA has a one-based i32 position. The pinned Windows adapter
                // adds one after conversion, so i32::MAX itself would overflow.
                // Omit unrepresentable metadata rather than clamp a logical row.
                #[cfg(target_os = "windows")]
                let position = (item.index < i32::MAX as usize).then_some(item.index);
                #[cfg(not(target_os = "windows"))]
                let position = Some(item.index);
                if let Some(position) = position {
                    node.set_position_in_set(position);
                }
                if let Some(list) = list {
                    let entry = totals.entry(list).or_insert((item.count, true));
                    entry.1 &= entry.0 == item.count;
                }
            }
            let list = if node.role() == AxRole::List {
                Some(ax_id)
            } else {
                list
            };
            match &d.props {
                Props::Text(p) => {
                    if matches!(p.kind, TextKind::Inline | TextKind::Kbd) {
                        node.set_value(normalize_text(&p.content, true).into_owned());
                    }
                    if let TextKind::Heading(level) = p.kind {
                        node.set_level(usize::from(level - 1));
                        if d.name.is_none() {
                            node.set_label(normalize_text(&p.content, true).into_owned());
                        }
                    }
                }
                Props::Input(p) => {
                    if let Some(hint) = d.semantics.as_ref().and_then(|s| s.placeholder.as_ref()) {
                        node.set_placeholder(hint.clone());
                    }
                    // Do not even copy raw password bytes into the accessibility cache.
                    if p.purpose != Purpose::Password
                        && !d.semantics.as_ref().is_some_and(|s| {
                            s.spinbutton
                                .as_ref()
                                .is_some_and(|s| s.value_text.is_some())
                        })
                    {
                        node.set_value(state.dom.text_value(id)?);
                    }
                }
                Props::Check(p) => {
                    if d.name.is_none() {
                        node.set_label(normalize_text(&p.label, true).into_owned());
                    }
                    let element = state
                        .dom
                        .control(id)
                        .ok()
                        .and_then(|control| state.dom.doc.get_node(control))
                        .and_then(|node| node.element_data());
                    let checked = element
                        .and_then(|element| element.checkbox_input_checked())
                        .unwrap_or(p.checked);
                    let mixed = element
                        .and_then(|element| element.checkbox_input_indeterminate())
                        .unwrap_or(false);
                    node.set_toggled(if mixed {
                        Toggled::Mixed
                    } else if checked {
                        Toggled::True
                    } else {
                        Toggled::False
                    });
                }
                Props::Image(p) => {
                    if d.name.is_none() {
                        node.set_label(
                            normalize_text(p.alternative.as_deref().unwrap_or_default(), true)
                                .into_owned(),
                        );
                    }
                }
                Props::Graphic(p) => {
                    if d.name.is_none() {
                        node.set_label(
                            normalize_text(p.alternative.as_deref().unwrap_or_default(), true)
                                .into_owned(),
                        );
                    }
                }
                Props::Link(p) => node.set_url(normalize_text(&p.target, true).into_owned()),
                _ => {}
            }
            if matches!(d.effective_role(), Some(Role::Status | Role::Alert)) {
                node.set_live(if d.effective_role() == Some(Role::Alert) {
                    Live::Assertive
                } else {
                    Live::Polite
                });
                node.set_live_atomic();
            }
            let control = state.dom.control(id).ok();
            if let Some(environment) = state.mapper.tree().inherited_environment(id)? {
                node.set_language(environment.language.as_str().to_owned());
            }
            if state.mapper.tree().enabled(id)
                && control.is_some_and(|control| state.dom.focus_visible(control))
            {
                if focus_scope::allowed(state, id)
                    && (d.focus != FocusMode::Auto
                        || control.is_some_and(|control| {
                            state
                                .dom
                                .doc
                                .get_node(control)
                                .is_some_and(|n| n.is_focussable())
                        }))
                {
                    node.add_action(AxAction::Focus);
                }
                if d.activation_enabled() && clickable(state, id) && !d.readonly() {
                    node.add_action(AxAction::Click);
                }
                if d.semantics
                    .as_ref()
                    .is_some_and(|s| s.splitter.is_some() || s.slider.is_some() && !d.readonly())
                {
                    node.add_action(AxAction::Increment);
                    node.add_action(AxAction::Decrement);
                    node.add_action(AxAction::SetValue);
                }
                if let Some(spin) = d.semantics.as_ref().and_then(|s| s.spinbutton.as_ref())
                    && !d.readonly()
                {
                    if spin.can_increase {
                        node.add_action(AxAction::Increment);
                    }
                    if spin.can_decrease {
                        node.add_action(AxAction::Decrement);
                    }
                }
                if matches!(d.props, Props::Input(_)) && !d.readonly() {
                    node.add_action(AxAction::SetValue);
                }
                if revealable(state, id) {
                    node.add_action(AxAction::ScrollIntoView);
                }
            }
            // Keep label text as a real child so names composed from label +
            // logical children use the platform's ordinary accessible-name rules.
            let label = match &d.props {
                Props::Button(p) => Some(p.label.as_str()),
                Props::Link(p) => Some(p.label.as_str()),
                Props::Text(p) if !matches!(p.kind, TextKind::Inline | TextKind::Kbd) => {
                    Some(p.content.as_str())
                }
                _ => None,
            };
            if let Some(label) = label.filter(|label| !label.is_empty()) {
                let key = Key::Label(id);
                let label_id = self.id(key)?;
                alive.insert(key);
                let mut text = Node::new(AxRole::Label);
                text.set_value(normalize_text(label, true).into_owned());
                node.push_child(label_id);
                nodes.insert(label_id, text);
            }
            nodes
                .get_mut(&parent)
                .expect("live accessibility parent")
                .push_child(ax_id);
            nodes.insert(ax_id, node);
            // Slider descendants are presentational, matching the HTML/ARIA
            // range control. Passive track/thumb boxes are never AX children.
            if d.role != Some(Role::Slider) {
                pending.extend(
                    semantic
                        .children
                        .iter()
                        .rev()
                        .map(|child| (*child, ax_id, list)),
                );
            }
        }
        // Flat ARIA trees carry explicit levels. AccessKit puts known sibling
        // size on the container, so lower the same live preorder to actual
        // TreeItem/Group ancestry. Scratch stacks contain live identities only;
        // synthetic groups retire as soon as their last live child disappears.
        let mut stacks: HashMap<AxId, Vec<(u8, NodeId, AxId)>> = HashMap::new();
        let mut tree_removals: HashMap<AxId, HashSet<AxId>> = HashMap::new();
        for (id, item_id, old_parent, tree, item) in tree_items {
            let stack = stacks.entry(tree).or_default();
            while stack
                .last()
                .is_some_and(|(level, _, _)| *level >= item.level)
            {
                stack.pop();
            }
            let container = if item.level == 1 {
                Some(tree)
            } else if let Some((_, parent, parent_id)) = stack
                .last()
                .copied()
                .filter(|(level, _, _)| *level + 1 == item.level)
            {
                let key = Key::TreeChildren(parent);
                let group = self.id(key)?;
                alive.insert(key);
                if let std::collections::btree_map::Entry::Vacant(entry) = nodes.entry(group) {
                    entry.insert(Node::new(AxRole::Group));
                    nodes
                        .get_mut(&parent_id)
                        .expect("live tree parent")
                        .push_child(group);
                }
                Some(group)
            } else {
                None
            };
            if let Some(container) = container {
                let count = Some(usize::from(item.count));
                let entry = totals.entry(container).or_insert((count, true));
                entry.1 &= entry.0 == count;
                if old_parent != container {
                    tree_removals.entry(old_parent).or_default().insert(item_id);
                    nodes
                        .get_mut(&container)
                        .expect("live tree container")
                        .push_child(item_id);
                }
            }
            stack.push((item.level, id, item_id));
        }
        for (parent, removed) in tree_removals {
            if let Some(parent) = nodes.get_mut(&parent) {
                let children = parent
                    .children()
                    .iter()
                    .copied()
                    .filter(|id| !removed.contains(id))
                    .collect::<Vec<_>>();
                parent.set_children(children);
            }
        }
        for (list, (count, consistent)) in totals {
            if consistent && let Some(count) = count {
                nodes
                    .get_mut(&list)
                    .expect("list exists")
                    .set_size_of_set(count);
            }
        }
        // All accessible target IDs now exist. Resolve only live, exported
        // nodes; forward references and hidden targets allocate no AX history.
        let mut relations = Vec::new();
        for (key, ax_id) in &self.ids {
            let Key::Main(id) = key else { continue };
            let Some(d) = state
                .mapper
                .tree()
                .node(*id)
                .ok()
                .map(|n| n.descriptor.view())
            else {
                continue;
            };
            let Some(s) = &d.semantics else { continue };
            let resolve = |name: &glory_render::SemanticId| {
                state
                    .mapper
                    .tree()
                    .named(name)
                    .and_then(|target| self.ids.get(&Key::Main(target)).copied())
                    .filter(|target| nodes.contains_key(target))
            };
            let controls = s.controls.as_ref().and_then(resolve);
            let active_descendant = state
                .mapper
                .tree()
                .resolve_active_descendant(*id)?
                .and_then(|option| self.ids.get(&Key::Main(option)).copied())
                .filter(|target| nodes.contains_key(target));
            let label = s.labelled_by.as_ref().and_then(resolve).or_else(|| {
                if d.name.is_some() || d.document_part() != Some(glory_render::DocumentPart::Table)
                {
                    return None;
                }
                // HTML tables receive their intrinsic name from a live caption.
                // Cross only transparent Groups; row contents are not captions.
                let mut pending = state
                    .mapper
                    .tree()
                    .node(*id)
                    .ok()?
                    .children
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                while let Some(child) = pending.pop() {
                    let child_node = state.mapper.tree().node(child).ok()?;
                    let descriptor = child_node.descriptor.view();
                    if matches!(descriptor.props, Props::Group) {
                        pending.extend(child_node.children.iter().rev().copied());
                    } else if descriptor.document_part()
                        == Some(glory_render::DocumentPart::TableCaption)
                    {
                        return self
                            .ids
                            .get(&Key::Main(child))
                            .copied()
                            .filter(|target| nodes.contains_key(target));
                    }
                }
                None
            });
            let description = s.described_by.as_ref().and_then(resolve);
            let error = s
                .error_message
                .as_ref()
                .filter(|_| s.invalid)
                .and_then(resolve);
            // Consumer naming does not recurse through a structural Label or
            // a generic description wrapper. Derive text from exported live
            // content only; never read editor values (especially passwords).
            let label_text = label
                .and_then(|target| relation_text(&nodes, target))
                .or_else(|| {
                    if matches!(
                        d.role,
                        Some(Role::Cell | Role::ColumnHeader | Role::GridCell)
                    ) || matches!(
                        d.document_part(),
                        Some(
                            glory_render::DocumentPart::ColumnHeader
                                | glory_render::DocumentPart::RowHeader
                                | glory_render::DocumentPart::TableCell
                        )
                    ) {
                        relation_text(&nodes, *ax_id)
                    } else {
                        None
                    }
                });
            let mut description_text = description.and_then(|target| relation_text(&nodes, target));
            if let Some(error) = error.and_then(|target| relation_text(&nodes, target)) {
                if let Some(description) = &mut description_text {
                    description.push(' ');
                    description.push_str(&error);
                } else {
                    description_text = Some(error);
                }
            }
            relations.push(Relations {
                owner: *ax_id,
                controls,
                active_descendant,
                label,
                description,
                error,
                label_text: label_text.filter(|_| d.name.is_none()),
                description_text,
                popup: s.popup,
            });
        }
        // Read text before installing any derived names, so relation cycles
        // and HashMap iteration order cannot change the resulting names.
        for relation in relations {
            if let Some(node) = nodes.get_mut(&relation.owner) {
                if let Some(target) = relation.controls {
                    node.set_controls(vec![target]);
                }
                if let Some(target) = relation.active_descendant {
                    node.set_active_descendant(target);
                }
                if let Some(target) = relation.label {
                    node.set_labelled_by(vec![target]);
                }
                if let Some(text) = relation.label_text {
                    node.set_label(text);
                }
                if let Some(target) = relation.description {
                    node.set_described_by(vec![target]);
                }
                if let Some(target) = relation.error {
                    node.set_error_message(target);
                    node.push_described_by(target);
                }
                if let Some(text) = relation.description_text {
                    node.set_description(text);
                }
                if let Some(popup) = relation.popup {
                    node.set_has_popup(match popup {
                        glory_render::PopupKind::Dialog => accesskit::HasPopup::Dialog,
                        glory_render::PopupKind::Menu => accesskit::HasPopup::Menu,
                        glory_render::PopupKind::Listbox => accesskit::HasPopup::Listbox,
                        glory_render::PopupKind::Tree => accesskit::HasPopup::Tree,
                    });
                }
            }
        }
        self.ids.retain(|key, _| alive.contains(key));
        self.reverse.retain(|_, key| alive.contains(key));
        let focus = state
            .focused
            .and_then(|id| self.ids.get(&Key::Main(id)).copied())
            .unwrap_or(ROOT);
        let initial = full || self.previous.is_empty();
        let changed = nodes
            .iter()
            .filter(|(id, node)| initial || self.previous.get(id) != Some(*node))
            .map(|(id, node)| (*id, node.clone()))
            .collect();
        self.previous = nodes;
        Ok(TreeUpdate {
            tree_id: TreeId::ROOT,
            nodes: changed,
            tree: initial.then(|| TreeInfo::new(ROOT)),
            focus,
        })
    }
    pub(super) fn action(
        &mut self,
        state: &mut State,
        request: &ActionRequest,
    ) -> Result<bool, Error> {
        if request.target_tree != TreeId::ROOT {
            return Ok(false);
        }
        let Some(Key::Main(id)) = self.reverse.get(&request.target_node).copied() else {
            return Ok(false);
        };
        if !state.mapper.tree().active(id) {
            return Ok(false);
        }
        let d = state.mapper.tree().node(id)?.descriptor.view();
        if !state.mapper.tree().enabled(id)
            || !state.layers.allowed(state.mapper.tree(), id)
            || d.readonly() && matches!(request.action, AxAction::SetValue | AxAction::Click)
        {
            return Ok(false);
        }
        state.dom.resolve();
        let control = state.dom.control(id)?;
        if !state.dom.focus_visible(control) {
            return Ok(false);
        }
        match (&request.action, &request.data) {
            (AxAction::Increment, None)
            | (AxAction::Decrement, None)
            | (AxAction::SetValue, Some(ActionData::NumericValue(_)))
                if d.semantics.as_ref().is_some_and(|s| s.spinbutton.is_some()) =>
            {
                if d.readonly() {
                    return Ok(false);
                }
                let command = match (&request.action, &request.data) {
                    (AxAction::Increment, _) => glory_render::RangeCommand::Increase,
                    (AxAction::Decrement, _) => glory_render::RangeCommand::Decrease,
                    (_, Some(ActionData::NumericValue(value))) => {
                        glory_render::RangeCommand::Set(*value)
                    }
                    _ => unreachable!(),
                };
                let spin = d
                    .semantics
                    .as_ref()
                    .and_then(|s| s.spinbutton.as_ref())
                    .expect("spin action branch");
                if matches!(command, glory_render::RangeCommand::Increase) && !spin.can_increase
                    || matches!(command, glory_render::RangeCommand::Decrease) && !spin.can_decrease
                {
                    return Ok(false);
                }
                let event = Event::Spin { from: id, command };
                event.validate_payload(id, d)?;
                state.inputs.push(Input::Event { node: id, event });
            }
            (AxAction::Increment, None)
            | (AxAction::Decrement, None)
            | (AxAction::SetValue, Some(ActionData::NumericValue(_)))
                if d.semantics.as_ref().is_some_and(|s| {
                    s.splitter.is_some() || s.slider.is_some() && !d.readonly()
                }) =>
            {
                let command = match (&request.action, &request.data) {
                    (AxAction::Increment, _) => glory_render::RangeCommand::Increase,
                    (AxAction::Decrement, _) => glory_render::RangeCommand::Decrease,
                    (_, Some(ActionData::NumericValue(value))) => {
                        glory_render::RangeCommand::Set(*value)
                    }
                    _ => unreachable!(),
                };
                let event = Event::Range { from: id, command };
                event.validate_payload(id, d)?;
                state.inputs.push(Input::Event { node: id, event });
            }
            (AxAction::ScrollIntoView, None | Some(ActionData::ScrollHint(_)))
                if revealable(state, id) =>
            {
                // The list owns logical coordinates, including a distant focus
                // pin. Native geometry cannot reconstruct its logical offset.
                state.inputs.push(Input::Event {
                    node: id,
                    event: Event::Navigate {
                        from: id,
                        command: glory_render::Navigation::Reveal,
                    },
                });
                if !self.pending_reveals.contains(&id) {
                    self.pending_reveals.push(id);
                }
                state.dom.invalidate();
            }
            (AxAction::Focus, None) => state.focus(id)?,
            (AxAction::Click, None) if clickable(state, id) => activate_control(state, id),
            (AxAction::SetValue, Some(ActionData::Value(value)))
                if matches!(d.props, Props::Input(_)) =>
            {
                let multiline = matches!(&d.props, Props::Input(p) if p.multiline);
                let value = normalize_text(value, multiline).into_owned();
                state.end_editing(id)?;
                state.dom.doc.with_text_input(control, |mut driver| {
                    driver.select_all();
                    driver.insert_or_replace_selection(&value);
                });
                let event = editing::text_event(state, id, &value)?;
                state.inputs.push(Input::Event { node: id, event });
                state.dom.invalidate();
            }
            _ => return Err(Error::Unsupported("AccessKit action/data on this target")),
        }
        Ok(true)
    }
}

fn reveal_native(state: &mut State, id: NodeId) -> Result<(), Error> {
    super::super::scroll_into_view::reveal(state, id)
}

fn revealable(state: &State, id: NodeId) -> bool {
    use glory_render::VirtualPart;
    let tree = state.mapper.tree();
    let Ok(node) = tree.node(id) else {
        return false;
    };
    if node.descriptor.view().options.get::<VirtualPart>() != Some(&VirtualPart::Row) {
        return false;
    }
    let mut cursor = node.parent;
    while let Some(id) = cursor {
        let Ok(node) = tree.node(id) else {
            return false;
        };
        if node.descriptor.view().options.get::<VirtualPart>() == Some(&VirtualPart::Root) {
            return state.subscriptions.contains(&(id, EventKind::Navigate));
        }
        cursor = node.parent;
    }
    false
}

#[cfg(test)]
#[path = "accessibility_tests.rs"]
mod tests;

fn clickable(state: &State, id: NodeId) -> bool {
    state.mapper.tree().node(id).is_ok_and(|node| {
        matches!(
            node.descriptor.view().props,
            Props::Button(_) | Props::Check(_) | Props::Link(_)
        )
    }) || state.subscriptions.contains(&(id, EventKind::Activate))
}
fn relation_text(nodes: &BTreeMap<AxId, Node>, target: AxId) -> Option<String> {
    let mut pending = vec![target];
    let mut text = String::new();
    while let Some(id) = pending.pop() {
        let Some(node) = nodes.get(&id) else { continue };
        let content = node.label().or_else(|| {
            (node.role() == AxRole::Label)
                .then(|| node.value())
                .flatten()
        });
        if let Some(content) = content.filter(|value| !value.is_empty()) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(content);
        } else {
            pending.extend(node.children().iter().rev().copied());
        }
    }
    (!text.is_empty()).then_some(text)
}

fn role(d: glory_render::DescriptorView<'_>) -> AxRole {
    if let Some(part) = d.document_part() {
        return match part {
            glory_render::DocumentPart::Blockquote => AxRole::Blockquote,
            glory_render::DocumentPart::OrderedList | glory_render::DocumentPart::UnorderedList => {
                AxRole::List
            }
            glory_render::DocumentPart::ListItem => AxRole::ListItem,
            glory_render::DocumentPart::DescriptionList => AxRole::DescriptionList,
            glory_render::DocumentPart::Term => AxRole::Term,
            glory_render::DocumentPart::Definition => AxRole::Definition,
            glory_render::DocumentPart::Table => AxRole::Table,
            glory_render::DocumentPart::TableCaption => AxRole::Caption,
            glory_render::DocumentPart::TableHead
            | glory_render::DocumentPart::TableBody
            | glory_render::DocumentPart::TableFoot => AxRole::RowGroup,
            glory_render::DocumentPart::TableRow => AxRole::Row,
            glory_render::DocumentPart::ColumnHeader => AxRole::ColumnHeader,
            glory_render::DocumentPart::RowHeader => AxRole::RowHeader,
            glory_render::DocumentPart::TableCell => AxRole::Cell,
        };
    }
    if let Some(part) = d.field_part() {
        return match part {
            glory_render::FieldPart::Form => AxRole::Form,
            glory_render::FieldPart::Fieldset => AxRole::Group,
            _ => AxRole::Label,
        };
    }
    if d.semantics.as_ref().is_some_and(|s| s.tab) {
        return AxRole::Tab;
    }
    // AccessKit's protected-text role must survive an input refinement.
    if matches!(d.props, Props::Input(p) if p.purpose == Purpose::Password) {
        return AxRole::PasswordInput;
    }
    if let Some(kind) = d.semantics.as_ref().and_then(|s| s.combobox) {
        return if kind == glory_render::ComboboxKind::Select {
            AxRole::ComboBox
        } else {
            AxRole::EditableComboBox
        };
    }
    if d.semantics.as_ref().is_some_and(|s| s.option.is_some()) {
        return AxRole::ListBoxOption;
    }
    if let Some(item) = d.semantics.as_ref().and_then(|s| s.menu_item.as_ref()) {
        return match item.kind {
            glory_render::MenuItemKind::Action => AxRole::MenuItem,
            glory_render::MenuItemKind::Checkbox => AxRole::MenuItemCheckBox,
            glory_render::MenuItemKind::Radio => AxRole::MenuItemRadio,
        };
    }
    match &d.props {
        Props::View => match d.effective_role() {
            Some(Role::Group) => AxRole::Group,
            Some(Role::ProgressBar) => AxRole::ProgressIndicator,
            Some(Role::Slider) => AxRole::Slider,
            Some(Role::SpinButton) => AxRole::SpinButton,
            Some(Role::Separator) => AxRole::Splitter,
            Some(Role::Main) => AxRole::Main,
            Some(Role::Navigation) => AxRole::Navigation,
            Some(Role::Section) => AxRole::Region,
            Some(Role::List) => AxRole::List,
            Some(Role::ListItem) => AxRole::ListItem,
            Some(Role::Status) => AxRole::Status,
            Some(Role::Alert) => AxRole::Alert,
            Some(Role::Table) => AxRole::Table,
            Some(Role::Grid) => AxRole::Grid,
            Some(Role::GridCell) => AxRole::GridCell,
            Some(Role::Tree) => AxRole::Tree,
            Some(Role::TreeItem) => AxRole::TreeItem,
            Some(Role::Row) => AxRole::Row,
            Some(Role::ColumnHeader) => AxRole::ColumnHeader,
            Some(Role::Cell) => AxRole::Cell,
            Some(Role::Dialog) => AxRole::Dialog,
            Some(Role::AlertDialog) => AxRole::AlertDialog,
            Some(Role::Tooltip) => AxRole::Tooltip,
            Some(Role::TabList) => AxRole::TabList,
            Some(Role::TabPanel) => AxRole::TabPanel,
            Some(Role::RadioGroup) => AxRole::RadioGroup,
            Some(Role::Toolbar) => AxRole::Toolbar,
            Some(Role::Banner) => AxRole::Banner,
            Some(Role::ContentInfo) => AxRole::ContentInfo,
            Some(Role::Menu) => AxRole::Menu,
            Some(Role::Menubar) => AxRole::MenuBar,
            Some(Role::Listbox) => AxRole::ListBox,
            Some(Role::Heading(_)) => AxRole::Heading,
            None => AxRole::GenericContainer,
        },
        Props::Text(p) => match p.kind {
            TextKind::Heading(_) => AxRole::Heading,
            TextKind::Paragraph => AxRole::Paragraph,
            TextKind::Inline => AxRole::Label,
            TextKind::Code => AxRole::Code,
            TextKind::Kbd => AxRole::Label,
            TextKind::Mark => AxRole::Mark,
        },
        Props::Button(_) | Props::File(_) => AxRole::Button,
        Props::Select(_) => AxRole::ComboBox,
        Props::Input(_) if d.semantics.as_ref().is_some_and(|s| s.spinbutton.is_some()) => {
            AxRole::SpinButton
        }
        Props::Input(p) if p.multiline => AxRole::MultilineTextInput,
        Props::Input(p) => match p.purpose {
            Purpose::Password => AxRole::PasswordInput,
            Purpose::Email => AxRole::EmailInput,
            Purpose::Url => AxRole::UrlInput,
            Purpose::Search => AxRole::SearchInput,
            Purpose::Tel => AxRole::PhoneNumberInput,
            Purpose::Text | Purpose::Number => AxRole::TextInput,
        },
        Props::Check(p) if p.mode == glory_render::CheckMode::Switch => AxRole::Switch,
        Props::Check(p) if p.mode == glory_render::CheckMode::Radio => AxRole::RadioButton,
        Props::Check(_) => AxRole::CheckBox,
        Props::Image(_) => AxRole::Image,
        Props::Graphic(_) => AxRole::Image,
        Props::Link(_) => AxRole::Link,
        Props::RichEditor(_) => unreachable!("native rich editing is rejected during preflight"),
        Props::Group => unreachable!("transparent group"),
    }
}
