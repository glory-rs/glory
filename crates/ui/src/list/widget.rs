use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;

use glory_core::{Cage, Lotus, untracked};
use glory_render::profile::Phase;
use glory_render::{
    Action, Descriptor, Event, EventKind, FocusMode, ItemPosition, Navigation, NodeId, Props, Rect,
    Role, VirtualPart,
};
use glory_style::{Edges, Length, Overflow, Position, Style};

use super::measure::Measurements;
use super::navigation;
use super::{Anchor, Cache, DEFAULT_ROW_LIMIT, Error, RowHost, ScrollSpace, check_rows};
use crate::app::{Error as UiError, Mounted, Result, Scope, WeakScope, Widget};
use crate::{events, view};

type KeyAt<K> = Rc<dyn Fn(usize) -> K>;
type Locate<K> = Rc<dyn Fn(&K) -> Option<usize>>;
type RenderRow<K> = Box<dyn FnMut(Cage<usize>, K) -> Box<dyn Widget>>;

#[derive(Clone, Debug, PartialEq)]
pub struct ListStats {
    pub range: Range<usize>,
    pub rows: usize,
    /// Extra mounted rows outside range, currently at most one focused row.
    pub pinned: usize,
    pub measured: usize,
    pub height_slots: usize,
    pub height_capacity: usize,
    pub logical: f64,
    pub total: f64,
    pub viewport: Rect,
}

/// Lazily indexed rows with fixed height or measured height estimates. Factories
/// receive a reactive index and stable key and run once per mounted key. Only
/// the render window and one focused key are read. The focused row may stay
/// mounted outside that window; blur releases it. Ordinary each intentionally
/// renders everything.
pub struct List<K: Clone + Eq + Hash + 'static> {
    count: Option<Lotus<usize>>,
    name: Option<Lotus<String>>,
    key: Option<KeyAt<K>>,
    locate: Option<Locate<K>>,
    revision: Option<Lotus<u64>>,
    render: Option<RenderRow<K>>,
    height: f64,
    row_height: f64,
    adaptive: bool,
    measure_limit: usize,
    overscan: usize,
    row_limit: usize,
    limit: f64,
    position: Option<Lotus<f64>>,
    on_scroll: Option<Box<dyn FnMut(f64)>>,
    inspect: Option<Box<dyn FnMut(ListStats)>>,
}
pub fn list<K: Clone + Eq + Hash + 'static, W: Widget>(
    count: impl Into<Lotus<usize>>,
    key: impl Fn(usize) -> K + 'static,
    mut render: impl FnMut(Cage<usize>, K) -> W + 'static,
) -> List<K> {
    List {
        count: Some(count.into()),
        name: None,
        key: Some(Rc::new(key)),
        locate: None,
        revision: None,
        render: Some(Box::new(move |index, key| Box::new(render(index, key)))),
        height: 600.0,
        row_height: 24.0,
        adaptive: false,
        measure_limit: 1024,
        overscan: 4,
        row_limit: DEFAULT_ROW_LIMIT,
        limit: 1_000_000.0,
        position: None,
        on_scroll: None,
        inspect: None,
    }
}
impl<K: Clone + Eq + Hash> List<K> {
    pub fn name(mut self, value: impl Into<Lotus<String>>) -> Self {
        self.name = Some(value.into());
        self
    }
    pub fn height(mut self, value: f64) -> Self {
        self.height = value;
        self
    }
    pub fn row_height(mut self, value: f64) -> Self {
        self.row_height = value;
        self.adaptive = false;
        self
    }
    /// Enable real row Resize measurements, using this estimate until known.
    /// Row wrappers have automatic height and a 1px minimum. The sparse index
    /// allocates no full-data array; unmeasured/evicted rows use the estimate.
    pub fn measure(mut self, estimate: f64) -> Self {
        self.row_height = estimate;
        self.adaptive = true;
        self
    }
    /// Retained height metadata, independent of UI instances. At least the live
    /// render window is protected, even if it contains more rows than this limit.
    pub fn measure_limit(mut self, value: usize) -> Self {
        self.measure_limit = value;
        self
    }
    /// O(1)/O(log n) stable-key lookup for anchoring across large data edits.
    /// No source scan is supplied by the framework. Results are validated.
    pub fn locate(mut self, locate: impl Fn(&K) -> Option<usize> + 'static) -> Self {
        self.locate = Some(Rc::new(locate));
        self
    }
    /// Explicit data edit version for sources whose key closure is not reactive.
    /// Ordinary scrolls never relocate cached height metadata.
    pub fn revision(mut self, value: impl Into<Lotus<u64>>) -> Self {
        self.revision = Some(value.into());
        self
    }
    pub fn overscan(mut self, value: usize) -> Self {
        self.overscan = value;
        self
    }
    /// Reject an oversized render window before reading keys or mounting rows.
    /// Includes overscan; at most one additional focused row is allowed. This
    /// does not truncate visible data or silently change layout geometry.
    pub fn row_limit(mut self, value: usize) -> Self {
        self.row_limit = value;
        self
    }
    pub fn scroll_limit(mut self, value: f64) -> Self {
        self.limit = value;
        self
    }
    /// Programmatic logical offset. on_scroll may echo its reported value back
    /// into this source without issuing a new seek or cancelling navigation.
    pub fn position(mut self, value: impl Into<Lotus<f64>>) -> Self {
        self.position = Some(value.into());
        self
    }
    pub fn on_scroll(mut self, callback: impl FnMut(f64) + 'static) -> Self {
        self.on_scroll = Some(Box::new(callback));
        self
    }
    pub fn inspect(mut self, callback: impl FnMut(ListStats) + 'static) -> Self {
        self.inspect = Some(Box::new(callback));
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Viewport {
    physical: f64,
    height: f64,
    width: f64,
    sequence: u64,
    ack: Option<glory_render::ScrollAck>,
    anchor: Option<(NodeId, f64, u64)>,
}
#[derive(Clone, Copy)]
struct Focused {
    node: Option<NodeId>,
    sequence: u64,
}
struct ScrollState {
    space: ScrollSpace,
    total: f64,
    height: f64,
    width: f64,
    logical: f64,
    native_sequence: u64,
    desired: Option<f64>,
    reported: Option<f64>,
    expected: Option<ExpectedScroll>,
    bias: f64,
    physical: f64,
    ack: Option<glory_render::ScrollAck>,
    revision: Option<u64>,
    restore_sequence: u64,
}
struct ExpectedScroll {
    requested: f64,
    projected: f64,
}
struct Row {
    mounted: Mounted,
    data: Scope,
    index: Cage<usize>,
    item: ItemPosition,
}
struct Rows<K> {
    parent: WeakScope,
    keys: Vec<K>,
    start: usize,
    count: usize,
    render: RenderRow<K>,
    origin: Cage<f64>,
    height: f64,
    adaptive: bool,
    measurements: Rc<RefCell<Measurements<K>>>,
    geometry: Cage<u64>,
    measured: Cage<u64>,
    error: Option<UiError>,
}
impl<K: Clone + Eq + Hash + 'static> RowHost for Rows<K> {
    type Key = K;
    type Row = Option<Row>;
    fn key(&mut self, index: usize) -> K {
        self.keys[index - self.start].clone()
    }
    fn mount(&mut self, index: usize, key: &K) -> Self::Row {
        let row = (|| {
            let data = self.parent.upgrade().ok_or(UiError::Gone)?.child()?;
            let index = data.cage(index);
            let origin = self.origin;
            let height = self.height;
            let adaptive = self.adaptive;
            let measurements = self.measurements.clone();
            let geometry = self.geometry;
            let style = data
                .bond(move || {
                    let top = if adaptive {
                        geometry.get();
                        measurements
                            .borrow()
                            .layout
                            .heights()
                            .prefix(index.get())
                            .expect("live row index")
                    } else {
                        index.get() as f64 * height
                    };
                    let mut style = Style::new()
                        .position(Position::Absolute)
                        .top(Length::Px(top - origin.get()))
                        .left(Length::Px(0.0))
                        .width(Length::Percent(100.0));
                    if adaptive {
                        style.min_height = Some(Length::Px(1.0));
                    } else {
                        style.height = Some(Length::Px(height));
                    }
                    style
                })
                .with_partial_eq();
            let body = data.owner().enter(|| (self.render)(index, key.clone()));
            let item = ItemPosition {
                index: index.get_untracked(),
                count: Some(self.count),
            };
            let mut wrapper = view()
                .role(Role::ListItem)
                .focus(FocusMode::Programmatic)
                .item(Some(item))
                .backend_options(VirtualPart::Row)
                .style(style)
                .fill(body);
            if adaptive {
                let measurements = self.measurements.clone();
                let measured = self.measured;
                let key = key.clone();
                wrapper = wrapper.on(events::resize, move |event| {
                    if let Event::Resize { size, .. } = event
                        && size.height.is_finite()
                        && size.height >= 1.0
                    {
                        measurements.borrow_mut().queue(
                            key.clone(),
                            index.get_untracked(),
                            size.height,
                        );
                        measured.revise(|version| {
                            *version = version
                                .checked_add(1)
                                .expect("measurement revision exhausted")
                        });
                    }
                });
            }
            let mounted = wrapper.mount(&data)?;
            Ok(Row {
                mounted,
                data,
                index,
                item,
            })
        })();
        match row {
            Ok(row) => Some(row),
            Err(error) => {
                self.error.get_or_insert(error);
                None
            }
        }
    }
    fn update(&mut self, index: usize, _: &K, row: &mut Self::Row) {
        if let Some(row) = row {
            if row.index.get_untracked() != index {
                row.index.revise(|value| *value = index);
            }
            let item = ItemPosition {
                index,
                count: Some(self.count),
            };
            if row.item != item {
                if let Err(error) = row.data.patch(
                    row.mounted.roots()[0],
                    glory_render::Patch::Item(glory_render::Change::Set(Some(item))),
                ) {
                    self.error.get_or_insert(error);
                } else {
                    row.item = item;
                }
            }
        }
    }
    fn place(&mut self, row: &Self::Row, before: Option<&Self::Row>) {
        if let Some(row) = row
            && let Err(error) = row
                .mounted
                .place_before(before.and_then(Option::as_ref).map(|row| &row.mounted))
        {
            self.error.get_or_insert(error);
        }
    }
    fn dispose(&mut self, row: Self::Row) {
        drop(row);
    }
}

impl<K: Clone + Eq + Hash + 'static> Widget for List<K> {
    fn build(&mut self, scope: &Scope) -> Result {
        let count = self.count.take().ok_or(UiError::Gone)?;
        let initial = untracked(|| count.try_get())?;
        let height = self.height;
        let row_height = self.row_height;
        let overscan = self.overscan;
        let row_limit = self.row_limit;
        if row_limit == 0 {
            return Err(UiError::List(Error::InvalidSize));
        }
        let desired = self.position.take();
        let initial_position = desired
            .as_ref()
            .map(|value| untracked(|| value.try_get()))
            .transpose()?
            .unwrap_or(0.0);
        let adaptive = self.adaptive;
        let measurements = Rc::new(RefCell::new(
            Measurements::new(initial, row_height, adaptive, self.measure_limit, overscan)
                .map_err(UiError::List)?,
        ));
        let total = measurements.borrow().layout.heights().total();
        check_rows(
            measurements
                .borrow()
                .layout
                .window(initial_position, height)
                .map_err(UiError::List)?
                .render
                .len(),
            row_limit,
        )
        .map_err(UiError::List)?;
        let space = ScrollSpace::new(total, height, self.limit).map_err(UiError::List)?;
        let extent = scope.cage(space.extent());
        let origin = scope.cage(0.0);
        let range = scope.cage(0..0);
        let geometry = scope.cage(0_u64);
        let measured = scope.cage(0_u64);
        let viewport = scope.cage(Viewport {
            physical: 0.0,
            height,
            width: 0.0,
            sequence: 0,
            ack: None,
            anchor: None,
        });
        let mut descriptor = Descriptor::new(Props::View);
        descriptor.style = Style {
            padding: Some(Edges::all(0.0)),
            ..Style::new()
                .height(Length::Px(height))
                .overflow_y(Overflow::Auto)
                .scroll_anchor(false)
        };
        descriptor.role = Some(Role::List);
        descriptor.focus = FocusMode::TabStop;
        descriptor.options.insert(VirtualPart::Root);
        let root = scope.create(descriptor)?;
        if let Some(name) = self.name.take() {
            let session = Rc::downgrade(&scope.session()?);
            let mut previous = None;
            scope.binding(move || {
                if let Some(session) = session.upgrade() {
                    let value = name.get();
                    if previous.as_ref() != Some(&value) {
                        match session.patch(
                            root,
                            glory_render::Patch::Name(glory_render::Change::Set(Some(
                                value.clone(),
                            ))),
                        ) {
                            Ok(()) => previous = Some(value),
                            Err(error) => session.report(error),
                        }
                    }
                }
            });
        }
        let content = scope.child_in(root)?;
        let mut descriptor = Descriptor::new(Props::View);
        // Overscan or a very tall row must not enlarge the physical scroll
        // surface. Clip does not create a second focus/scroll owner.
        descriptor.style = Style::new()
            .height(Length::Px(space.extent()))
            .position(Position::Relative)
            .overflow_x(Overflow::Clip)
            .overflow_y(Overflow::Clip);
        descriptor.options.insert(VirtualPart::Surface);
        let body = content.create(descriptor)?;
        let focus = scope.cage(Focused {
            node: None,
            sequence: 0,
        });
        let navigation = Rc::new(RefCell::new(Vec::<navigation::Request>::new()));
        let requests = navigation.clone();
        let navigation_revision = scope.cage(0_u64);
        scope.on(root, events::navigate, move |event| {
            if let Event::Navigate { from, command } = event {
                requests
                    .borrow_mut()
                    .push(navigation::Request { from, command });
                navigation_revision.revise(|revision| {
                    *revision = revision
                        .checked_add(1)
                        .expect("navigation revision exhausted")
                });
            }
        })?;
        let row_parent = content.child_in(body)?;
        let key_at = self.key.take().ok_or(UiError::Gone)?;
        let key_count = count.clone();
        let key = key_at.clone();
        let data_revision = self.revision.take();
        let key_revision = data_revision.clone();
        let keys = scope
            .bond(move || {
                if let Some(revision) = &key_revision {
                    revision.get();
                }
                let len = key_count.get();
                range.with(|range| {
                    (range.start.min(len)..range.end.min(len))
                        .map(|index| key(index))
                        .collect::<Vec<_>>()
                })
            })
            .with_partial_eq();
        let mut rows = Rows {
            parent: row_parent.weak(),
            keys: Vec::new(),
            start: 0,
            count: initial,
            render: self.render.take().ok_or(UiError::Gone)?,
            origin,
            height: row_height,
            adaptive,
            measurements: measurements.clone(),
            geometry,
            measured,
            error: None,
        };
        let mut cache: Cache<K, Option<Row>> = Cache::default();
        let mut cache_revision = 0_u64;
        let mut state = ScrollState {
            space,
            total,
            height,
            width: 0.0,
            logical: 0.0,
            native_sequence: 0,
            desired: None,
            reported: None,
            expected: None,
            bias: 0.,
            physical: 0.,
            ack: None,
            revision: None,
            restore_sequence: 0,
        };
        let mut navigation_target: Option<navigation::Target<K>> = None;
        let session = Rc::downgrade(&scope.session()?);
        let weak_scope = scope.weak();
        let locate = self.locate.take();
        let mut on_scroll = self.on_scroll.take();
        let mut inspect = self.inspect.take();
        scope.retain(row_parent)?;
        scope.retain(content)?;
        // One controller computes geometry/anchors before reconciling rows.
        // Independent observers could first destroy the old positional window,
        // losing keyed row state during prepend even though it stays visible.
        scope.structural(move || {
            let Some(scope) = weak_scope.upgrade() else {
                return;
            };
            let Some(session) = session.upgrade() else {
                return;
            };
            let profile = session.runtime.profile();
            let _window = profile.as_ref().map(|profile| profile.span(Phase::Window));
            let result = (|| -> Result {
                let logical_before = state.logical;
                let len = count.try_get()?;
                let input = viewport.get();
                navigation_revision.get();
                let requests = std::mem::take(&mut *navigation.borrow_mut());
                if !requests.is_empty() {
                    let capabilities = scope.capabilities()?;
                    if !capabilities.scroll
                        || (!capabilities.focus
                            && requests
                                .iter()
                                .any(|request| request.command != Navigation::Reveal))
                    {
                        return Err(glory_render::Error::Unsupported(
                            "list navigation requires focus and scroll",
                        )
                        .into());
                    }
                }
                let focused_state = focus.get();
                let focused = focused_state.node;
                let focused_row = focused
                    .map(|node| scope.child_containing(body, node))
                    .transpose()?
                    .flatten();
                let mut pin = focused_row.and_then(|node| {
                    cache.order().iter().find_map(|key| {
                        cache
                            .row(key)
                            .and_then(Option::as_ref)
                            .filter(|row| row.mounted.roots().contains(&node))
                            .map(|row| (row.index.get_untracked(), key.clone()))
                    })
                });
                // Track the one off-window source key too. A data edit can
                // leave every visible key equal while deleting/moving focus.
                let pin_changed = pin
                    .as_ref()
                    .is_some_and(|(index, key)| *index >= len || key_at(*index) != *key);
                let had_focus = pin.is_some();
                measured.get();
                let revision = data_revision.as_ref().map(Lotus::try_get).transpose()?;
                let position = desired.as_ref().map(Lotus::try_get).transpose()?;
                let native_changed = state.native_sequence != input.sequence;
                let mut user_scrolled = false;
                if native_changed {
                    state.native_sequence = input.sequence;
                    let previous_physical = state.physical;
                    state.physical = input.physical;
                    let mut applied = None;
                    let mut external = false;
                    if let Some(ack) = input.ack {
                        if let Some(expected) = state
                            .expected
                            .as_ref()
                            .filter(|e| e.requested == ack.requested.y)
                        {
                            state.bias = expected.projected - ack.y;
                            applied = Some(ack.y);
                            state.expected = None;
                        } else if state.ack != Some(ack) {
                            external = true;
                            state.bias = 0.;
                            state.expected = None;
                        }
                        state.ack = Some(ack);
                    } else if let Some(expected) = state
                        .expected
                        .as_ref()
                        .filter(|e| (input.physical - e.requested).abs() < 0.25)
                    {
                        state.bias = expected.projected - input.physical;
                        applied = Some(input.physical);
                        state.expected = None;
                    }
                    // Accepted coordinates distinguish quantization from real
                    // movement, even when both reach the same coalesced frame.
                    if external || input.physical != applied.unwrap_or(previous_physical) {
                        user_scrolled = true;
                        state.expected = None;
                        state.logical = state
                            .space
                            .scrolled(input.physical + state.bias)
                            .map_err(UiError::List)?
                            .logical;
                    }
                }
                let previous_range = range.get_untracked();
                let keyed_change = pin_changed
                    || if adaptive || locate.is_some() {
                        keys.with(|snapshot| *snapshot != rows.keys)
                    } else {
                        false
                    };
                let mut metrics = measurements.borrow_mut();
                let data_changed = len != metrics.layout.heights().len()
                    || revision != state.revision
                    || keyed_change;
                let width_changed = adaptive && state.width > 0.0 && state.width != input.width;
                let mut anchor = metrics
                    .layout
                    .anchor(state.logical)
                    .map_err(UiError::List)?;
                let restored = input
                    .anchor
                    .filter(|(_, _, sequence)| *sequence != state.restore_sequence);
                if let Some((row, within, sequence)) = restored {
                    let index = rows
                        .keys
                        .iter()
                        .enumerate()
                        .find_map(|(index, key)| {
                            cache
                                .row(key)
                                .and_then(Option::as_ref)
                                .filter(|row_state| row_state.mounted.roots().contains(&row))
                                .map(|_| rows.start + index)
                        })
                        .ok_or(UiError::List(Error::InvalidAnchor))?;
                    anchor = Some(Anchor { index, within });
                    state.restore_sequence = sequence;
                }
                let old_key = anchor
                    .and_then(|anchor| anchor.index.checked_sub(rows.start))
                    .and_then(|index| rows.keys.get(index))
                    .cloned();
                if data_changed || width_changed {
                    let positions = if data_changed && locate.is_none() {
                        keys.with(|snapshot| {
                            snapshot
                                .iter()
                                .cloned()
                                .enumerate()
                                .map(|(index, key)| (key, previous_range.start + index))
                                .collect::<HashMap<_, _>>()
                        })
                    } else {
                        HashMap::new()
                    };
                    let resolve = |key: &K| -> std::result::Result<Option<usize>, Error> {
                        let index = if let Some(locate) = &locate {
                            untracked(|| locate(key))
                        } else {
                            positions.get(key).copied()
                        };
                        if let Some(index) = index
                            && (index >= len || untracked(|| key_at(index)) != *key)
                        {
                            return Err(Error::InvalidLookup);
                        }
                        Ok(index)
                    };
                    if data_changed && let Some(old) = anchor {
                        let located = old_key
                            .as_ref()
                            .map(&resolve)
                            .transpose()
                            .map_err(UiError::List)?
                            .flatten();
                        let next = if located.is_some() {
                            located
                        } else {
                            let mut survivor = None;
                            for key in rows
                                .keys
                                .iter()
                                .skip(old.index.saturating_sub(rows.start).saturating_add(1))
                            {
                                if let Some(index) = resolve(key).map_err(UiError::List)? {
                                    survivor = Some(index);
                                    break;
                                }
                            }
                            survivor
                        };
                        anchor = (len > 0).then_some(Anchor {
                            index: next.unwrap_or(old.index.min(len.saturating_sub(1))),
                            within: if located.is_some() { old.within } else { 0.0 },
                        });
                    }
                    metrics
                        .rebuild(len, overscan, |key| {
                            if width_changed {
                                Ok(None)
                            } else {
                                resolve(key)
                            }
                        })
                        .map_err(UiError::List)?;
                }
                state.revision = revision;
                state.width = input.width;
                let mut changed = data_changed || width_changed;
                if adaptive {
                    changed |= metrics
                        .apply(|index, key| untracked(|| key_at(index)) == *key)
                        .map_err(UiError::List)?;
                }
                if (changed || restored.is_some())
                    && let Some(anchor) = anchor
                {
                    state.logical = metrics.layout.restore(anchor).map_err(UiError::List)?;
                }
                // Eviction restores estimates outside the live window. Correct
                // its prefix delta around the same anchor, never scale wheel motion.
                loop {
                    let next = metrics
                        .layout
                        .window(state.logical, input.height)
                        .map_err(UiError::List)?;
                    check_rows(next.render.len(), row_limit).map_err(UiError::List)?;
                    if !adaptive || !metrics.prune(&next.render).map_err(UiError::List)? {
                        break;
                    }
                    changed = true;
                    if let Some(anchor) = anchor {
                        state.logical = metrics.layout.restore(anchor).map_err(UiError::List)?;
                    }
                }
                let total = metrics.layout.heights().total();
                if changed
                    || restored.is_some()
                    || total != state.total
                    || input.height != state.height
                {
                    state.logical = state
                        .space
                        .resize(total, input.height, state.logical)
                        .map_err(UiError::List)?
                        .logical;
                    state.total = total;
                    state.height = input.height;
                }
                if position != state.desired {
                    state.desired = position;
                    // Controlled on_scroll echoes acknowledge the reported
                    // value. Re-seeking would undo a measurement correction or
                    // retire the still-focused navigation intent.
                    if position != state.reported {
                        navigation_target = None;
                        state.bias = 0.;
                        state.expected = None;
                    }
                    if let Some(position) = position.filter(|value| Some(*value) != state.reported)
                    {
                        state.logical = state.space.seek(position).map_err(UiError::List)?.logical;
                    }
                }
                if user_scrolled || data_changed {
                    navigation_target = None;
                }
                if let Some(target) = &mut navigation_target
                    && target.focus_sequence != focused_state.sequence
                {
                    if pin.as_ref().is_some_and(|(_, key)| *key == target.key) {
                        target.focus_sequence = focused_state.sequence;
                    } else {
                        // A late row measurement must not scroll for an old
                        // navigation request after the user moves focus away.
                        navigation_target = None;
                    }
                }
                if !requests.is_empty() {
                    // A new logical destination establishes a fresh alignment;
                    // carrying a previous segment's quantization bias would
                    // accumulate error across repeated Home/End commands.
                    let mut cursor = None;
                    for request in requests {
                        let current = if let Some((_, index)) = cursor.filter(|(from, _)| {
                            *from == request.from && request.command != Navigation::Reveal
                        }) {
                            Some(index)
                        } else if let Some(wrapper) = scope.child_containing(body, request.from)? {
                            let entry = cache.order().iter().find_map(|key| {
                                cache
                                    .row(key)
                                    .and_then(Option::as_ref)
                                    .filter(|row| row.mounted.roots().contains(&wrapper))
                                    .map(|row| (key, row.index.get_untracked()))
                            });
                            if let Some((key, index)) = entry {
                                let index = if data_changed {
                                    if let Some(locate) = &locate {
                                        untracked(|| locate(key))
                                    } else if index < len && untracked(|| key_at(index)) == *key {
                                        Some(index)
                                    } else {
                                        None
                                    }
                                } else {
                                    Some(index)
                                };
                                if let Some(index) = index
                                    && (index >= len || untracked(|| key_at(index)) != *key)
                                {
                                    return Err(UiError::List(Error::InvalidLookup));
                                }
                                index
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some(index) = navigation::target(
                            &metrics.layout,
                            current,
                            state.logical,
                            input.height,
                            request.command,
                        )
                        .map_err(UiError::List)?
                        {
                            state.bias = 0.;
                            state.expected = None;
                            cursor = Some((request.from, index));
                            navigation_target = Some(navigation::Target {
                                index,
                                key: untracked(|| key_at(index)),
                                focus: request.command != Navigation::Reveal,
                                focus_sequence: focused_state.sequence,
                            });
                        }
                    }
                }
                let mut focus_target = None;
                let navigating = navigation_target
                    .as_ref()
                    .is_some_and(|target| target.focus);
                if let Some(target) = &mut navigation_target {
                    state.logical = state
                        .space
                        .seek(
                            navigation::reveal(
                                &metrics.layout,
                                target.index,
                                state.logical,
                                input.height,
                            )
                            .map_err(UiError::List)?,
                        )
                        .map_err(UiError::List)?
                        .logical;
                    if target.focus {
                        focus_target = Some(target.index);
                        target.focus = false;
                    }
                    // A measured placeholder is not a final size: images,
                    // fonts and wrapping can resize the same focused row later.
                    // Keep one adaptive reveal intent until user movement,
                    // external position/data changes or focus elsewhere.
                    if !adaptive {
                        navigation_target = None;
                    }
                }
                let next = metrics
                    .layout
                    .window(state.logical, input.height)
                    .map_err(UiError::List)?;
                check_rows(next.render.len(), row_limit).map_err(UiError::List)?;
                let measured_count = metrics.measured();
                let height_slots = metrics.layout.heights().storage_len();
                let height_capacity = metrics.layout.heights().storage_capacity();
                drop(metrics);
                if changed {
                    geometry.revise(|version| {
                        *version = version.checked_add(1).expect("geometry revision exhausted")
                    });
                }
                if previous_range != next.render {
                    range.revise(|range| *range = next.render.clone());
                }
                if extent.get_untracked() != state.space.extent() {
                    extent.revise(|extent| *extent = state.space.extent());
                }
                let projected = state.space.project(state.logical);
                let physical = (projected - state.bias)
                    .clamp(0., (state.space.extent() - input.height).max(0.));
                if physical != projected - state.bias {
                    state.bias = projected - physical;
                }
                // Ignore only arithmetic roundoff from logical/segment
                // coordinate conversion, never a subpixel height change.
                let roundoff =
                    4.0 * f64::EPSILON * projected.abs().max(input.physical.abs()).max(1.0);
                if scope.capabilities()?.scroll
                    && scope.is_active(root)?
                    && ((input.physical - physical).abs() > roundoff
                        || state
                            .expected
                            .as_ref()
                            .is_some_and(|expected| expected.projected != projected))
                    && !state
                        .expected
                        .as_ref()
                        .is_some_and(|expected| expected.projected == projected)
                {
                    scope.action(Action::ScrollTo {
                        node: root,
                        x: 0.0,
                        // Request the new projection itself. Subtracting the
                        // previous quantization error would accumulate bias
                        // across successive fractional measurements.
                        y: projected,
                    })?;
                    state.expected = Some(ExpectedScroll {
                        requested: projected,
                        projected,
                    });
                }
                // The same mapping must drive logical scrolling AND painting:
                // row_top = prefix - segment_origin - bias - host_scroll.
                // Host readback can differ from the requested projection while
                // the logical anchor and its exact within-row offset survive.
                let display_origin = state.space.origin() + state.bias;
                if origin.get_untracked() != display_origin {
                    origin.revise(|origin| *origin = display_origin);
                }
                let keys_changed = keys.with(|snapshot| {
                    if rows.start == next.render.start && rows.keys == *snapshot {
                        false
                    } else {
                        rows.start = next.render.start;
                        rows.keys.clone_from(snapshot);
                        true
                    }
                });
                rows.count = len;
                if keys_changed || data_changed {
                    cache_revision = cache_revision
                        .checked_add(1)
                        .expect("list revision exhausted");
                }
                if data_changed && let Some((index, key)) = &pin {
                    let located = if let Some(locate) = &locate {
                        untracked(|| locate(key))
                    } else if *index < len && untracked(|| key_at(*index)) == *key {
                        Some(*index)
                    } else {
                        // Without an index lookup only this bounded window is
                        // searchable. Never scan a large source for old focus.
                        rows.keys
                            .iter()
                            .position(|candidate| candidate == key)
                            .map(|at| rows.start + at)
                    };
                    if let Some(index) = located
                        && (index >= len || key_at(index) != *key)
                    {
                        cache.clear(&mut rows);
                        return Err(UiError::List(Error::InvalidLookup));
                    }
                    pin = located.map(|index| (index, key.clone()));
                }
                let restore_focus = had_focus && pin.is_none();
                let pinned = usize::from(
                    pin.as_ref()
                        .is_some_and(|(index, _)| !next.render.contains(index)),
                );
                let reconciled = {
                    let mut span = profile
                        .as_ref()
                        .map(|profile| profile.span(Phase::Reconcile).work(0));
                    let result =
                        cache.reconcile_pinned(next.render.clone(), cache_revision, pin, &mut rows);
                    if let (Some(span), Ok(stats)) = (&mut span, &result) {
                        span.set_work(stats.keys_read as u64);
                    }
                    result
                };
                if let Err(error) = reconciled {
                    // Geometry may already have changed. Do not leave old rows
                    // with now-out-of-range indices for the following prop phase.
                    cache.clear(&mut rows);
                    return Err(UiError::List(error));
                }
                if let Some(error) = rows.error.take() {
                    cache.clear(&mut rows);
                    return Err(error);
                }
                if let Some(index) = focus_target {
                    let key = rows
                        .keys
                        .get(
                            index
                                .checked_sub(rows.start)
                                .ok_or(UiError::List(Error::InvalidAnchor))?,
                        )
                        .ok_or(UiError::List(Error::InvalidAnchor))?;
                    let row = cache
                        .row(key)
                        .and_then(Option::as_ref)
                        .ok_or(UiError::List(Error::InvalidAnchor))?;
                    scope.action(Action::Focus(row.mounted.roots()[0]))?;
                } else if restore_focus && scope.capabilities()?.focus && scope.is_active(root)? {
                    scope.action(Action::Focus(root))?;
                }
                if (native_changed
                    || changed
                    || restored.is_some()
                    || navigating
                    || state.logical != logical_before)
                    && state.reported != Some(state.logical)
                    && let Some(callback) = &mut on_scroll
                {
                    state.reported = Some(state.logical);
                    untracked(|| callback(state.logical));
                }
                if let Some(inspect) = &mut inspect {
                    untracked(|| {
                        inspect(ListStats {
                            range: next.render,
                            rows: cache.len(),
                            pinned,
                            measured: measured_count,
                            height_slots,
                            height_capacity,
                            logical: state.logical,
                            total,
                            viewport: Rect {
                                width: input.width,
                                height: input.height,
                                ..Rect::default()
                            },
                        })
                    });
                }
                Ok(())
            })();
            if let Err(error) = result {
                if matches!(
                    error,
                    UiError::List(Error::RowLimit { .. } | Error::Allocation)
                ) {
                    // A failed geometry/data edit may invalidate old row
                    // indices. Release them after the measurements borrow ends,
                    // without ever publishing the oversized source range.
                    let focused = focus.get_untracked().node;
                    let restore = focused.is_some_and(|node| {
                        scope.child_containing(body, node).ok().flatten().is_some()
                    });
                    cache.clear(&mut rows);
                    rows.keys.clear();
                    rows.start = 0;
                    navigation_target = None;
                    if !range.get_untracked().is_empty() {
                        range.revise(|range| *range = 0..0);
                    }
                    if restore
                        && scope.capabilities().is_ok_and(|caps| caps.focus)
                        && scope.is_active(root).unwrap_or(false)
                        && let Err(focus_error) = scope.action(Action::Focus(root))
                    {
                        session.report(focus_error);
                    }
                }
                session.report(error);
            }
        });
        let session = Rc::downgrade(&scope.session()?);
        scope.binding(move || {
            if let Some(session) = session.upgrade() {
                let style = Style::new()
                    .height(Length::Px(extent.get()))
                    .position(Position::Relative)
                    .overflow_x(Overflow::Clip)
                    .overflow_y(Overflow::Clip);
                if let Err(error) = session.patch(
                    body,
                    glory_render::Patch::Style(glory_render::Change::Set(Box::new(style))),
                ) {
                    session.report(error);
                }
            }
        });
        scope.on(root, events::focus_within, move |event| {
            if let Event::FocusWithin { node, focused } = event {
                let previous = focus.get_untracked();
                if focused && previous.node != Some(node) {
                    focus.revise(|focus| {
                        focus.node = Some(node);
                        focus.sequence = focus
                            .sequence
                            .checked_add(1)
                            .expect("focus sequence exhausted");
                    });
                } else if !focused && previous.node == Some(node) {
                    focus.revise(|focus| {
                        focus.node = None;
                        focus.sequence = focus
                            .sequence
                            .checked_add(1)
                            .expect("focus sequence exhausted");
                    });
                }
            }
        })?;
        scope.on(root, events::scroll, move |event| {
            if let Event::Scroll {
                y,
                viewport: bounds,
                ack,
                ..
            } = event
                && y.is_finite()
                && bounds.height.is_finite()
                && bounds.height >= 0.0
                && bounds.width.is_finite()
                && bounds.width >= 0.0
            {
                let previous = viewport.get_untracked();
                let ack = ack.filter(|ack| {
                    ack.y.is_finite()
                        && ack.y >= 0.
                        && ack.requested.y.is_finite()
                        && ack.requested.y >= 0.
                });
                if previous.physical != y
                    || previous.height != bounds.height
                    || previous.width != bounds.width
                    || previous.ack != ack
                    || ack.is_some()
                {
                    viewport.revise(|value| {
                        value.physical = y.max(0.0);
                        value.height = bounds.height;
                        value.width = bounds.width;
                        value.ack = ack;
                        value.sequence = value
                            .sequence
                            .checked_add(1)
                            .expect("scroll sequence exhausted");
                    });
                }
            }
        })?;
        scope.on(root, EventKind::Anchor, move |event| {
            if let Event::Anchor {
                row,
                within,
                viewport: bounds,
            } = event
                && within.is_finite()
                && within >= 0.0
                && bounds.height.is_finite()
                && bounds.height >= 0.0
                && bounds.width.is_finite()
                && bounds.width >= 0.0
            {
                viewport.revise(|value| {
                    let sequence = value
                        .anchor
                        .map_or(0, |(_, _, sequence)| sequence)
                        .checked_add(1)
                        .expect("anchor sequence exhausted");
                    value.anchor = Some((row, within, sequence));
                    value.height = bounds.height;
                    value.width = bounds.width;
                });
            }
        })?;
        scope.on(root, EventKind::Resize, move |event| {
            if let Event::Resize {
                viewport: bounds, ..
            } = event
                && bounds.height.is_finite()
                && bounds.height >= 0.0
                && bounds.width.is_finite()
                && bounds.width >= 0.0
            {
                let previous = viewport.get_untracked();
                if previous.height != bounds.height || previous.width != bounds.width {
                    viewport.revise(|value| {
                        value.height = bounds.height;
                        value.width = bounds.width;
                    });
                }
            }
        })?;
        Ok(())
    }
}
