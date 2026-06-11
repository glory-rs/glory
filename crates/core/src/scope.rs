use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::{IndexMap, IndexSet};

#[cfg(not(feature = "single-app"))]
use crate::HolderId;
use crate::node::Node;
use crate::view::{VIEW_ID_DELIMITER, View, ViewId, ViewPlacement};
use crate::{Truck, reflow};

/// Per-view runtime state.
///
/// Every [`Widget`][crate::Widget] receives a `&mut Scope` in
/// `build` / `patch` / `attach`. Scope owns:
///
/// - **`view_id`** — stable identifier of this view in the holder.
/// - **`child_views`** — `IndexMap<ViewId, View>` whose iteration
///   order mirrors sibling rendering order in the DOM. Never use
///   `.remove`; always `shift_remove` to preserve order.
/// - **`visible_views`** — visible children (`IndexSet<ViewId>`). A
///   stored-but-not-shown child does not appear in the DOM.
/// - **`parent_node` / `render_node`** — where this widget's nodes
///   live in the DOM tree. `parent_node` is the enclosing element,
///   `render_node` is the element under which this widget renders
///   (usually the same; differs for fragment-like widgets that don't
///   create their own DOM node).
/// - **`first_child_node` / `last_child_node`** — anchors used by
///   sibling positioning logic in [`attach_child`][Scope::attach_child].
///   Element widgets set both to their own node in `build`.
/// - **`placement`** — the [`ViewPlacement`] this view is about to be
///   placed at (`Tail`, `Head`, `Before(node)`, `After(node)`). Set by
///   the parent during `attach_child`, reset to `Unset` at the end
///   so subsequent re-attaches go through fresh neighbour search.
/// - **`truck`** — shared `Rc<RefCell<Truck>>` for app-wide context.
///
/// Mutating these fields from inside a widget is the framework's
/// extension point; widgets like `Each` and `Switch` poke at
/// `child_views` directly during `patch`. External code should
/// stick to [`attach_child`][Scope::attach_child] and
/// [`detach_child`][Scope::detach_child].
#[derive(Debug)]
pub struct Scope {
    #[cfg(not(feature = "single-app"))]
    holder_id: HolderId,
    pub view_id: ViewId,
    pub(crate) is_root: bool,
    pub(crate) is_built: bool,
    pub(crate) is_building: bool,
    pub(crate) is_attached: bool,
    pub(crate) child_views: IndexMap<ViewId, View>,
    pub(crate) visible_views: IndexSet<ViewId>,
    pub(crate) placement: ViewPlacement,

    pub(crate) parent_node: Option<Node>,
    pub(crate) render_node: Option<Node>,
    pub(crate) first_child_node: Option<Node>,
    pub(crate) last_child_node: Option<Node>,

    next_child_view_id: AtomicU64,

    pub truck: Rc<RefCell<Truck>>,
    owner: reflow::Owner,
}
impl Scope {
    pub fn new(view_id: ViewId, truck: Rc<RefCell<Truck>>) -> Self {
        Self {
            #[cfg(not(feature = "single-app"))]
            holder_id: view_id.holder_id(),
            view_id,
            is_root: false,
            is_built: false,
            is_building: false,
            is_attached: false,
            child_views: IndexMap::new(),
            visible_views: IndexSet::new(),
            placement: ViewPlacement::Tail,

            parent_node: None,
            render_node: None,
            first_child_node: None,
            last_child_node: None,

            next_child_view_id: AtomicU64::new(0),
            truck,
            owner: reflow::Owner::new(),
        }
    }
    pub fn new_root(view_id: ViewId, truck: Rc<RefCell<Truck>>) -> Self {
        Self {
            #[cfg(not(feature = "single-app"))]
            holder_id: view_id.holder_id(),
            view_id,
            is_root: true,
            is_built: false,
            is_building: false,
            is_attached: false,
            child_views: IndexMap::new(),
            visible_views: IndexSet::new(),
            placement: ViewPlacement::Tail,

            parent_node: None,
            render_node: None,
            first_child_node: None,
            last_child_node: None,

            next_child_view_id: AtomicU64::new(0),
            truck,
            owner: reflow::Owner::new(),
        }
    }
    pub fn is_root(&self) -> bool {
        self.is_root
    }
    pub fn is_attached(&self) -> bool {
        self.is_attached
    }
    pub fn is_built(&self) -> bool {
        self.is_built
    }
    pub(crate) fn is_building(&self) -> bool {
        self.is_building
    }

    #[cfg(not(feature = "single-app"))]
    pub fn holder_id(&self) -> HolderId {
        self.holder_id
    }
    pub fn parent_node(&self) -> Option<&Node> {
        self.parent_node.as_ref()
    }
    pub fn view_id(&self) -> &ViewId {
        &self.view_id
    }
    pub(crate) fn next_child_view_id(&self) -> ViewId {
        cfg_if! {
          if #[cfg(feature = "single-app")] {
            ViewId::new(format!("{}{VIEW_ID_DELIMITER}{}", self.view_id, self.next_child_view_id.fetch_add(1, Ordering::Relaxed)))
          } else {
            ViewId::new(self.holder_id, format!("{}{VIEW_ID_DELIMITER}{}", self.view_id, self.next_child_view_id.fetch_add(1, Ordering::Relaxed)))
          }
        }
    }
    pub fn beget(&self) -> Self {
        Scope::new(self.next_child_view_id(), self.truck.clone())
    }
    pub fn owner(&self) -> &reflow::Owner {
        &self.owner
    }

    pub fn cage<T>(&self, value: T) -> reflow::Cage<T>
    where
        T: std::fmt::Debug + 'static,
    {
        self.owner.cage(value)
    }

    pub fn render_node(&self) -> Option<&Node> {
        self.render_node.as_ref()
    }

    pub fn truck(&self) -> Ref<'_, Truck> {
        self.truck.borrow()
    }
    pub fn truck_mut(&self) -> RefMut<'_, Truck> {
        self.truck.borrow_mut()
    }

    pub fn child_views(&self) -> &IndexMap<ViewId, View> {
        &self.child_views
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub(crate) fn set_single_node_bounds(&mut self, node: Node) {
        self.render_node = Some(node.clone());
        self.first_child_node = Some(node.clone());
        self.last_child_node = Some(node);
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub(crate) fn insert_node_at_placement(&self, node: &Node) {
        use wasm_bindgen::UnwrapThrowExt;

        let parent = self.parent_node.as_ref().unwrap_throw();
        match &self.placement {
            ViewPlacement::Head => parent.prepend_with_node_1(node).unwrap_throw(),
            ViewPlacement::Before(next_node) => next_node.before_with_node_1(node).unwrap_throw(),
            ViewPlacement::After(prev_node) => prev_node.after_with_node_1(node).unwrap_throw(),
            ViewPlacement::Tail | ViewPlacement::Unset => parent.append_with_node_1(node).unwrap_throw(),
        }
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub(crate) fn remove_node_from_parent(&self, node: &Node) {
        if let Some(parent) = self.parent_node.as_ref() {
            let _ = parent.remove_child(node);
        }
    }

    /// Register a reactive side effect on this scope. Convenience wrapper
    /// around [`crate::reflow::effect_in`]; see that function for the
    /// semantics.
    pub fn effect<F>(&mut self, closure: F) -> ViewId
    where
        F: FnMut() + 'static,
    {
        crate::reflow::effect_in(self, closure)
    }

    /// Register an asynchronous derived signal on this scope. Convenience
    /// wrapper around [`crate::reflow::resource_in`]; see that function
    /// for the stale-write caveat and SSR-hydration alternative.
    pub fn resource<T, F, Fut>(&mut self, future_fn: F) -> reflow::Cage<Option<T>>
    where
        T: std::fmt::Debug + 'static,
        F: Fn() -> Fut + 'static,
        Fut: std::future::Future<Output = T> + 'static,
    {
        crate::reflow::resource_in(self, future_fn)
    }
    pub fn attach_child(&mut self, view_id: &ViewId) {
        let Some(view) = self.child_views.get(view_id) else {
            crate::warn!("attached child view not found in current scope: {}", view_id);
            return;
        };
        self.visible_views.insert(view_id.clone());
        if view.is_attached() {
            return;
        }

        let mut placement = ViewPlacement::Unset;
        if view.scope.placement == ViewPlacement::Unset {
            let index = self.child_views.get_index_of(view_id).unwrap();
            if index > 0 {
                for i in (0..index).rev() {
                    let (_, prev_view) = self.child_views.get_index(i).unwrap();
                    if prev_view.scope.is_attached()
                        && let Some(prev_node) = prev_view.last_child_node()
                    {
                        placement = ViewPlacement::After(prev_node.clone());
                        break;
                    }
                }
            }
            if placement == ViewPlacement::Unset {
                for i in (index + 1)..self.child_views.len() {
                    let (_, next_view) = self.child_views.get_index(i).unwrap();
                    if next_view.scope.is_attached()
                        && let Some(next_node) = next_view.last_child_node()
                    {
                        placement = ViewPlacement::Before(next_node.clone());
                        break;
                    }
                }
            }
        }
        if placement == ViewPlacement::Unset {
            if self.parent_node == view.scope.parent_node {
                placement = self.placement.clone();
            } else {
                placement = ViewPlacement::Tail;
            }
        }

        let view = self.child_views.get_mut(view_id).unwrap();
        if placement != ViewPlacement::Unset {
            view.scope.placement = placement;
        }

        view.scope.parent_node = self.render_node.clone();
        debug_assert!(view.scope.parent_node.is_some(), "view.scope.parent_node should not None");
        if view.scope.render_node.is_none() {
            view.scope.render_node = self.render_node.clone();
        }
        debug_assert!(view.scope.render_node.is_some(), "view.scope.render_node should not None");

        cfg_if! {
            if #[cfg(feature = "single-app")] {
                reflow::batch(|| {
                    view.attach();
                });
            } else {
                reflow::batch(view_id.holder_id(), || {
                    view.attach();
                });
            }
        }
        view.scope.placement = ViewPlacement::Unset;
    }

    pub fn detach_child(&mut self, view_id: &ViewId) -> Option<View> {
        self.visible_views.shift_remove(view_id);

        let view = self.child_views.get_mut(view_id)?;
        if !view.scope.is_attached() {
            return None;
        }
        // `shift_remove` preserves the order of remaining siblings, which
        // sibling-positioning in `attach_child` and reorder paths in
        // widgets like `Each` depend on. The deprecated `remove` aliases
        // to `swap_remove`, which would relocate the last view into the
        // gap and silently corrupt sibling order.
        let mut view = self.child_views.shift_remove(view_id)?;

        cfg_if! {
            if #[cfg(feature = "single-app")] {
                reflow::batch(|| {
                    view.detach();
                });
            } else {
                reflow::batch(self.holder_id(), || {
                    view.detach();
                });
            }
        }
        Some(view)
    }

    pub(crate) fn detach_children_bulk(&mut self, view_ids: &[ViewId]) -> Vec<View> {
        if view_ids.is_empty() {
            return Vec::new();
        }
        if view_ids.len() == 1 {
            return self.detach_child(&view_ids[0]).into_iter().collect();
        }

        let targets: IndexSet<ViewId> = view_ids.iter().cloned().collect();
        for view_id in view_ids {
            self.visible_views.shift_remove(view_id);
        }

        let mut old_child_views = Some(std::mem::take(&mut self.child_views));
        let mut kept = IndexMap::new();
        let mut detached = Vec::with_capacity(targets.len());

        let mut detach_all = || {
            let old_child_views = old_child_views.take().unwrap_or_default();
            for (view_id, mut view) in old_child_views {
                if targets.contains(&view_id) && view.scope.is_attached() {
                    view.detach();
                    detached.push(view);
                } else {
                    kept.insert(view_id, view);
                }
            }
        };

        cfg_if! {
            if #[cfg(feature = "single-app")] {
                reflow::batch(&mut detach_all);
            } else {
                reflow::batch(self.holder_id(), &mut detach_all);
            }
        }

        self.child_views = kept;
        detached
    }

    pub(crate) fn mark_descendants_dom_detached(&mut self) {
        for view in self.child_views.values_mut() {
            view.scope.parent_node = None;
            view.scope.mark_descendants_dom_detached();
        }
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub fn nodes(&self) -> Vec<web_sys::Node> {
        if let (Some(first_node), Some(last_node)) = (&self.first_child_node, &self.last_child_node) {
            return crate::web::nodes_between(first_node, last_node);
        }
        let mut nodes = vec![];
        for view in self.child_views.values() {
            if let (Some(first_node), Some(last_node)) = (&view.scope.first_child_node, &view.scope.last_child_node) {
                nodes.extend(crate::web::nodes_between(first_node, last_node));
            }
        }
        nodes
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::{Scope, Truck, ViewId};

    #[test]
    fn scope_drop_invalidates_owned_cage() {
        let cage = {
            let scope = Scope::new(
                ViewId::new(
                    #[cfg(not(feature = "single-app"))]
                    crate::HolderId::null(),
                    "scope".to_string(),
                ),
                Rc::new(RefCell::new(Truck::new())),
            );
            scope.cage(1_i32)
        };

        assert!(cage.try_get_untracked().is_err());
    }
}
