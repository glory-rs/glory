use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::Future;
use indexmap::{IndexMap, IndexSet};

use crate::node::Node;
use crate::view::{View, ViewId, ViewPosition, VIEW_ID_DELIMITER};
#[cfg(not(feature = "__single_holder"))]
use crate::HolderId;
use crate::{reflow, Truck};

#[derive(Debug)]
pub struct Scope {
    #[cfg(not(feature = "__single_holder"))]
    holder_id: HolderId,
    pub view_id: ViewId,
    pub(crate) is_root: bool,
    pub(crate) is_built: bool,
    pub(crate) is_attached: bool,
    pub(crate) child_views: IndexMap<ViewId, View>,
    pub(crate) show_list: IndexSet<ViewId>,
    pub(crate) position: ViewPosition,

    pub(crate) task_ids: IndexSet<TaskId>,

    pub(crate) parent_node: Option<Node>,
    pub(crate) graff_node: Option<Node>,
    pub(crate) first_child_node: Option<Node>,
    pub(crate) last_child_node: Option<Node>,

    next_child_view_id: AtomicU64,

    pub truck: Rc<RefCell<Truck>>,
}
impl Scope {
    pub fn new(view_id: ViewId, truck: Rc<RefCell<Truck>>) -> Self {
        Self {
            #[cfg(not(feature = "__single_holder"))]
            holder_id: view_id.holder_id(),
            view_id,
            is_root: false,
            is_built: false,
            is_attached: false,
            child_views: IndexMap::new(),
            show_list: IndexSet::new(),
            position: ViewPosition::Tail,

            task_ids: IndexSet::new(),

            parent_node: None,
            graff_node: None,
            first_child_node: None,
            last_child_node: None,

            next_child_view_id: AtomicU64::new(0),
            truck,
        }
    }
    pub fn new_root(view_id: ViewId, truck: Rc<RefCell<Truck>>) -> Self {
        Self {
            #[cfg(not(feature = "__single_holder"))]
            holder_id: view_id.holder_id(),
            view_id,
            is_root: true,
            is_built: false,
            is_attached: false,
            child_views: IndexMap::new(),
            show_list: IndexSet::new(),
            position: ViewPosition::Tail,

            task_ids: IndexSet::new(),

            parent_node: None,
            graff_node: None,
            first_child_node: None,
            last_child_node: None,

            next_child_view_id: AtomicU64::new(0),
            truck,
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

    #[cfg(not(feature = "__single_holder"))]
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
          if #[cfg(feature = "__single_holder")] {
            ViewId::new(format!("{}{VIEW_ID_DELIMITER}{}", self.view_id, self.next_child_view_id.fetch_add(1, Ordering::Relaxed)))
          } else {
            ViewId::new(self.holder_id, format!("{}{VIEW_ID_DELIMITER}{}", self.view_id, self.next_child_view_id.fetch_add(1, Ordering::Relaxed)))
          }
        }
    }
    pub fn beget(&self) -> Self {
        Scope::new(self.next_child_view_id(), self.truck.clone())
    }
    pub fn graff(&self) -> Option<&Node> {
        self.graff_node.as_ref()
    }

    pub fn truck(&self) -> Ref<'_, Truck> {
        self.truck.borrow()
    }
    pub fn truck_mut(&self) -> RefMut<'_, Truck> {
        self.truck.borrow_mut()
    }

    pub fn spawn_task(&mut self, task: impl FnOnce() -> Future<Output=()> + 'static) -> TaskId {
        let id = TaskId::next(self.view_id.clone());
        crate::reflow::spawn_task(id.clone(), task);
        id
    }

    pub fn child_views(&self) -> &IndexMap<ViewId, View> {
        &self.child_views
    }
    // pub fn child_views_mut(&mut self) -> &mut IndexMap<ViewId, View> {
    //     &mut self.child_views
    // }

    pub fn attach_child(&mut self, view_id: &ViewId) {
        let Some(view) = self.child_views.get(view_id) else {
            crate::warn!("attched child view not found in current scope: {}", view_id);
            return;
        };
        self.show_list.insert(view_id.clone());
        if view.is_attached() {
            return;
        }

        let mut position = ViewPosition::Unset;
        if view.scope.position == ViewPosition::Unset {
            let index = self.child_views.get_index_of(view_id).unwrap();
            if index > 0 {
                for i in (index - 1)..=0 {
                    let (_, prev_view) = self.child_views.get_index(i).unwrap();
                    if prev_view.scope.is_attached() {
                        if let Some(prev_node) = prev_view.last_child_node() {
                            position = ViewPosition::Prev(prev_node.clone());
                            break;
                        }
                    }
                }
            }
            if position == ViewPosition::Unset {
                for i in (index + 1)..self.child_views.len() {
                    let (_, next_view) = self.child_views.get_index(i).unwrap();
                    if next_view.scope.is_attached() {
                        if let Some(next_node) = next_view.last_child_node() {
                            position = ViewPosition::Next(next_node.clone());
                            break;
                        }
                    }
                }
            }
        }
        if position == ViewPosition::Unset {
            if self.parent_node == view.scope.parent_node {
                position = self.position.clone();
            } else {
                position = ViewPosition::Tail;
            }
        }

        let view = self.child_views.get_mut(view_id).unwrap();
        if position != ViewPosition::Unset {
            view.scope.position = position;
        }

        view.scope.parent_node = self.graff_node.clone();
        debug_assert!(view.scope.parent_node.is_some(), "view.scope.parent_node should not None");
        if view.scope.graff_node.is_none() {
            view.scope.graff_node = self.graff_node.clone();
        }
        debug_assert!(view.scope.graff_node.is_some(), "view.scope.parent_node should not None");

        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                reflow::batch(|| {
                    view.attach();
                });
            } else {
                reflow::batch(view_id.holder_id(), || {
                    view.attach();
                });
            }
        }
        view.scope.position = ViewPosition::Unset;
    }

    pub fn detach_child(&mut self, view_id: &ViewId) -> Option<View> {
        self.show_list.remove(view_id);

        let Some(view) = self.child_views.get_mut(view_id) else {
            return None;
        };
        if !view.scope.is_attached() {
            return None;
        }
        let Some(mut view) = self.child_views.remove(view_id) else {
            return None;
        };

        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
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
    // #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    // pub fn nodes(&self) -> Vec<Node> {
    //     if let (Some(first_node), Some(last_node)) = (&self.first_node, &self.last_node) {
    //         return crate::web::nodes_between(first_node, last_node);
    //     }
    //     let mut nodes = vec![];
    //     for view in self.child_views.values() {
    //         if let (Some(first_node), Some(last_node)) = (&view.scope.first_node, &view.scope.last_node) {
    //             nodes.extend(crate::web::nodes_between(first_node, last_node));
    //         }
    //     }
    //     nodes
    // }

    // pub fn wreck_child(&mut self, view_id: &ViewId) -> Option<View> {
    //     let mut view_ids = vec![];
    //     let mut views: Vec<&mut View> = self.scope.child_views.values_mut().collect();
    //     for view in views.iter_mut() {
    //         view.detach();
    //     }
    //     let mut views: Vec<&View> = self.scope.child_views.values().collect();
    //     while !views.is_empty() {
    //         let view = views.pop().unwrap();
    //         view_ids.push(view.id);
    //         for child_view in view.scope.child_views.values() {
    //             views.push(child_view);
    //         }
    //     }

    //     if let Some(mut child) = self.child_views.remove(view_id) {
    //         child.wreck();
    //         Some(child)
    //     } else {
    //         None
    //     }
    // }
}
