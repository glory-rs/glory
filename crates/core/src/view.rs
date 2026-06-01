use std::fmt::{self, Display};
use std::hash::Hash;
use std::ops::Deref;

use indexmap::IndexMap;

use crate::{Node, Scope, Widget};

#[cfg(not(feature = "single-app"))]
use crate::HolderId;

pub const VIEW_ID_DELIMITER: char = '-';

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId {
    pub(crate) path: String,
    #[cfg(not(feature = "single-app"))]
    holder_id: HolderId,
}
impl ViewId {
    #[cfg(feature = "single-app")]
    pub fn new(path: String) -> Self {
        Self { path }
    }
    #[cfg(not(feature = "single-app"))]
    pub fn new(holder_id: HolderId, path: String) -> Self {
        Self { path, holder_id }
    }
    #[cfg(not(feature = "single-app"))]
    pub fn holder_id(&self) -> HolderId {
        self.holder_id
    }
    pub fn into_inner(self) -> String {
        self.path
    }

    fn path_from_root(&self) -> Vec<ViewId> {
        let mut segs = self.path.split(VIEW_ID_DELIMITER).rev().collect::<Vec<_>>();
        let mut cval = segs.pop().unwrap().to_owned();
        let mut list = Vec::with_capacity(segs.len());
        cfg_if! {
            if #[cfg(feature = "single-app")] {
                list.push(ViewId::new(cval.clone()));
            } else {
                list.push(ViewId::new(self.holder_id, cval.clone()));
            }
        }
        while !segs.is_empty() {
            cval.push(VIEW_ID_DELIMITER);
            cval.push_str(segs.pop().unwrap());
            cfg_if! {
                if #[cfg(feature = "single-app")] {
                    list.push(ViewId::new(cval.clone()));
                } else {
                    list.push(ViewId::new(self.holder_id, cval.clone()));
                }
            }
        }
        list.reverse();
        list
    }
}
impl Display for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}
impl AsRef<ViewId> for ViewId {
    fn as_ref(&self) -> &ViewId {
        self
    }
}
impl Deref for ViewId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

#[derive(Default, PartialEq, Clone, Debug)]
pub enum ViewPlacement {
    #[default]
    Unset,
    Head,
    Before(Node),
    After(Node),
    Tail,
}

#[derive(Default, Debug)]
pub(crate) struct ViewTree {
    roots: IndexMap<ViewId, View>,
}
impl ViewTree {
    pub fn get_mut(&mut self, view_id: &ViewId) -> Option<&mut View> {
        let mut ids = view_id.path_from_root();
        let mut view = self.roots.get_mut(&ids.pop()?)?;
        while !ids.is_empty() {
            view = view.child_mut(&ids.pop()?)?;
        }
        Some(view)
    }

    pub fn insert(&mut self, view_id: ViewId, view: View) -> Option<View> {
        self.roots.insert(view_id, view)
    }
}
#[derive(Debug)]
pub struct View {
    pub(crate) widget: Box<dyn Widget + 'static>,
    pub(crate) scope: Scope,
    pub(crate) id: ViewId,
}

impl View {
    pub fn new(scope: Scope, widget: impl Widget) -> Self {
        Self {
            id: scope.view_id.clone(),
            widget: Box::new(widget),
            scope,
        }
    }
    #[cfg(not(feature = "single-app"))]
    pub fn holder_id(&self) -> HolderId {
        self.id.holder_id
    }
    pub fn is_built(&self) -> bool {
        self.scope.is_built
    }
    pub fn is_attached(&self) -> bool {
        self.scope.is_attached
    }
    pub fn child(&self, view_id: &ViewId) -> Option<&View> {
        self.scope.child_views.get(view_id)
    }
    pub fn child_mut(&mut self, view_id: &ViewId) -> Option<&mut View> {
        self.scope.child_views.get_mut(view_id)
    }
    fn build(&mut self) {
        if self.is_built() {
            return;
        }
        #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
        if crate::web::is_hydrating() {
            self.widget.hydrate(&mut self.scope);
        }
        cfg_if! {
            if #[cfg(feature = "single-app")] {
                crate::reflow::batch(|| {
                    self.widget.build(&mut self.scope);
                });
            } else {
                crate::reflow::batch(self.holder_id(), || {
                    self.widget.build(&mut self.scope);
                });
            }
        }
        self.scope.is_built = true;
    }

    pub fn attach(&mut self) {
        if self.is_attached() {
            return;
        }

        #[cfg(not(feature = "single-app"))]
        let holder_id = self.holder_id();
        let process = || {
            self.widget.attach(&mut self.scope);
            self.scope.is_attached = true;
            if !self.is_built() {
                self.build();
            }
            self.widget.flood(&mut self.scope);
        };
        cfg_if! {
            if #[cfg(feature = "single-app")] {
                crate::reflow::batch(process);
            } else {
                crate::reflow::batch(holder_id, process);
            }
        }
    }
    pub fn detach(&mut self) {
        self.widget.detach(&mut self.scope);
        self.scope.is_attached = false;
    }

    pub fn prepend_view(&mut self, new_view: View) {
        let new_view_id = new_view.id.clone();
        let (last_index, _) = self.scope.child_views.insert_full(new_view.id.clone(), new_view);
        if last_index != 0 {
            self.scope.child_views.move_index(last_index, 0);
        }
        self.scope.attach_child(&new_view_id);
    }
    pub fn append_view(&mut self, new_view: View) {
        let new_view_id = new_view.id.clone();
        self.scope.child_views.insert(new_view_id.clone(), new_view);
        self.scope.attach_child(&new_view_id);
    }
    pub fn before_view(&mut self, view_id: &ViewId, new_view: View) -> bool {
        let Some(index) = self.scope.child_views.get_index_of(view_id) else {
            crate::warn!("view `{}` not found when insert antoher view before it. {:#?}", view_id, new_view);
            return false;
        };
        let new_view_id = new_view.id.clone();
        let (last_index, _) = self.scope.child_views.insert_full(new_view_id.clone(), new_view);
        self.scope.child_views.move_index(last_index, index);
        self.scope.attach_child(&new_view_id);
        true
    }
    pub fn after_view(&mut self, view_id: &ViewId, new_view: View) -> bool {
        let Some(index) = self.scope.child_views.get_index_of(view_id) else {
            crate::warn!("view `{}` not found when insert antoher view after it. {:#?}", view_id, new_view);
            return false;
        };
        let new_view_id = new_view.id.clone();
        let (last_index, _) = self.scope.child_views.insert_full(new_view_id.clone(), new_view);
        if last_index != index + 1 {
            self.scope.child_views.move_index(last_index, index + 1);
        }
        self.scope.attach_child(&new_view_id);
        true
    }

    pub fn first_child_node(&self) -> Option<&crate::node::Node> {
        if let Some(node) = self.scope.first_child_node.as_ref() {
            return Some(node);
        }
        for child_view in self.scope.child_views.values() {
            if let Some(node) = child_view.first_child_node() {
                return Some(node);
            }
        }
        None
    }
    pub fn last_child_node(&self) -> Option<&crate::node::Node> {
        if let Some(node) = self.scope.last_child_node.as_ref() {
            return Some(node);
        }
        for child_view in self.scope.child_views.values().rev() {
            if let Some(node) = child_view.last_child_node() {
                return Some(node);
            }
        }
        None
    }
}

pub trait ViewFactory {
    fn make_view(&self, parent: &mut Scope) -> ViewId;
}

impl<F, T> ViewFactory for F
where
    F: Fn() -> T,
    T: Widget + 'static,
{
    fn make_view(&self, parent: &mut Scope) -> ViewId {
        (self)().store_in(parent)
    }
}
