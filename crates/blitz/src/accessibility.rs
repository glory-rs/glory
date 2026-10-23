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
            if semantic.suspended {
                continue;
            }
            let d = &semantic.descriptor;
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
            let key = Key::Main(id);
            let ax_id = self.id(key)?;
            alive.insert(key);
            let mut node = Node::new(role(d));
            if let Some(name) = &d.name {
                node.set_label(normalize_text(name, true).into_owned());
            }
            if !d.props.enabled() {
                node.set_disabled();
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
                    if p.kind == TextKind::Inline {
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
                    // Do not even copy raw password bytes into the accessibility cache.
                    if p.purpose != Purpose::Password {
                        node.set_value(state.dom.text_value(id)?);
                    }
                }
                Props::Check(p) => {
                    if d.name.is_none() {
                        node.set_label(normalize_text(&p.label, true).into_owned());
                    }
                    let checked = state
                        .dom
                        .control(id)
                        .ok()
                        .and_then(|control| state.dom.doc.get_node(control))
                        .and_then(|node| node.element_data())
                        .and_then(|element| element.checkbox_input_checked())
                        .unwrap_or(p.checked);
                    node.set_toggled(if checked {
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
            if d.props.enabled() && control.is_some_and(|control| state.dom.focus_visible(control))
            {
                if d.focus != FocusMode::Auto
                    || control.is_some_and(|control| {
                        state
                            .dom
                            .doc
                            .get_node(control)
                            .is_some_and(|n| n.is_focussable())
                    })
                {
                    node.add_action(AxAction::Focus);
                }
                if clickable(state, id) {
                    node.add_action(AxAction::Click);
                }
                if matches!(d.props, Props::Input(_)) {
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
                Props::Text(p) if p.kind != TextKind::Inline => Some(p.content.as_str()),
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
            pending.extend(
                semantic
                    .children
                    .iter()
                    .rev()
                    .map(|child| (*child, ax_id, list)),
            );
        }
        for (list, (count, consistent)) in totals {
            if consistent && let Some(count) = count {
                nodes
                    .get_mut(&list)
                    .expect("list exists")
                    .set_size_of_set(count);
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
        let d = &state.mapper.tree().node(id)?.descriptor;
        if !d.props.enabled() {
            return Ok(false);
        }
        state.dom.resolve();
        let control = state.dom.control(id)?;
        if !state.dom.focus_visible(control) {
            return Ok(false);
        }
        match (&request.action, &request.data) {
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
    let target = state.dom.main(id)?;
    if !state.dom.focus_visible(target) {
        return Ok(());
    }
    let viewport_before = state.dom.doc.viewport_scroll();
    let mut changed = false;
    let mut cursor = state.dom.doc.get_node(target).and_then(|n| n.parent);
    while let Some(parent) = cursor {
        let host = state.dom.doc.get_node(parent).ok_or(Error::NodeGone(id))?;
        cursor = host.parent;
        if !host.has_boxes() {
            continue;
        }
        let (Some(mut rect), Some(mut outer)) = (
            state.dom.doc.get_client_bounding_rect(target),
            state.dom.doc.get_client_bounding_rect(parent),
        ) else {
            continue;
        };
        let own = *state
            .dom
            .doc
            .get_node(target)
            .ok_or(Error::NodeGone(id))?
            .scroll_offset();
        rect.x += own.x;
        rect.y += own.y;
        let before = *host.scroll_offset();
        let layout = host.final_layout();
        outer.x += before.x + f64::from(layout.border.left);
        outer.y += before.y + f64::from(layout.border.top);
        let width = f64::from(
            (layout.size.width
                - layout.border.left
                - layout.border.right
                - layout.scrollbar_size.width)
                .max(0.0),
        );
        let height = f64::from(
            (layout.size.height
                - layout.border.top
                - layout.border.bottom
                - layout.scrollbar_size.height)
                .max(0.0),
        );
        state.dom.doc.scroll_by(
            parent,
            nearest(rect.x - outer.x, rect.width, width),
            nearest(rect.y - outer.y, rect.height, height),
            ScrollBehavior::Instant,
        );
        changed |= before
            != *state
                .dom
                .doc
                .get_node(parent)
                .ok_or(Error::NodeGone(id))?
                .scroll_offset();
    }
    state.dom.doc.scroll_into_view(
        target,
        ScrollBehavior::Instant,
        blitz_dom::ScrollLogicalPosition::Nearest,
        blitz_dom::ScrollLogicalPosition::Nearest,
    );
    if changed || viewport_before != state.dom.doc.viewport_scroll() {
        state.observation_pending = true;
        state.dom.invalidate();
    }
    Ok(())
}

fn nearest(start: f64, size: f64, viewport: f64) -> f64 {
    let end = start + size;
    if (start >= 0.0 && end <= viewport) || (start <= 0.0 && end >= viewport) {
        0.0
    } else if start.abs() < (end - viewport).abs() {
        start
    } else {
        end - viewport
    }
}

fn revealable(state: &State, id: NodeId) -> bool {
    use glory_render::VirtualPart;
    let tree = state.mapper.tree();
    let Ok(node) = tree.node(id) else {
        return false;
    };
    if node.descriptor.options.get::<VirtualPart>() != Some(&VirtualPart::Row) {
        return false;
    }
    let mut cursor = node.parent;
    while let Some(id) = cursor {
        let Ok(node) = tree.node(id) else {
            return false;
        };
        if node.descriptor.options.get::<VirtualPart>() == Some(&VirtualPart::Root) {
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
            node.descriptor.props,
            Props::Button(_) | Props::Check(_) | Props::Link(_)
        )
    }) || state.subscriptions.contains(&(id, EventKind::Activate))
}
fn role(d: &glory_render::Descriptor) -> AxRole {
    match &d.props {
        Props::View => match d.effective_role() {
            Some(Role::Group) => AxRole::Group,
            Some(Role::Main) => AxRole::Main,
            Some(Role::Navigation) => AxRole::Navigation,
            Some(Role::Section) => AxRole::Region,
            Some(Role::List) => AxRole::List,
            Some(Role::ListItem) => AxRole::ListItem,
            Some(Role::Status) => AxRole::Status,
            Some(Role::Alert) => AxRole::Alert,
            None => AxRole::GenericContainer,
        },
        Props::Text(p) => match p.kind {
            TextKind::Heading(_) => AxRole::Heading,
            TextKind::Paragraph => AxRole::Paragraph,
            TextKind::Inline => AxRole::Label,
        },
        Props::Button(_) => AxRole::Button,
        Props::Input(p) if p.multiline => AxRole::MultilineTextInput,
        Props::Input(p) => match p.purpose {
            Purpose::Password => AxRole::PasswordInput,
            Purpose::Email => AxRole::EmailInput,
            Purpose::Url => AxRole::UrlInput,
            Purpose::Search => AxRole::SearchInput,
            Purpose::Tel => AxRole::PhoneNumberInput,
            Purpose::Text | Purpose::Number => AxRole::TextInput,
        },
        Props::Check(_) => AxRole::CheckBox,
        Props::Image(_) => AxRole::Image,
        Props::Link(_) => AxRole::Link,
        Props::Group => unreachable!("transparent group"),
    }
}
