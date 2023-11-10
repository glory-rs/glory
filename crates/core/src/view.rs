use std::fmt::{self, Display};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use indexmap::IndexMap;

use crate::{Node, Scope, Widget};

#[cfg(not(feature = "__single_holder"))]
use crate::HolderId;

pub const VIEW_ID_DELIMITER: char = '-';

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId {
    pub(crate) raw_value: String,
    #[cfg(not(feature = "__single_holder"))]
    holder_id: HolderId,
}
impl ViewId {
    #[cfg(feature = "__single_holder")]
    pub fn new(raw_value: String) -> Self {
        Self { raw_value }
    }
    #[cfg(not(feature = "__single_holder"))]
    pub fn new(holder_id: HolderId, raw_value: String) -> Self {
        Self { raw_value, holder_id }
    }
    #[cfg(not(feature = "__single_holder"))]
    pub fn holder_id(&self) -> HolderId {
        self.holder_id
    }
    pub fn into_inner(self) -> String {
        self.raw_value
    }

    pub fn rise_to_root(&self) -> Vec<ViewId> {
        let mut segs = self.raw_value.split('-').rev().collect::<Vec<_>>();
        let mut cval = segs.pop().unwrap().to_owned();
        let mut list = Vec::with_capacity(segs.len());
        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                list.push(ViewId::new(cval.clone()));
            } else {
                list.push(ViewId::new(self.holder_id, cval.clone()));
            }
        }
        while !segs.is_empty() {
            cval.push(VIEW_ID_DELIMITER);
            cval.push_str(segs.pop().unwrap());
            cfg_if! {
                if #[cfg(feature = "__single_holder")] {
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
        self.raw_value.fmt(f)
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
        &self.raw_value
    }
}

#[derive(Default, PartialEq, Clone, Debug)]
pub enum ViewPosition {
    #[default]
    Unset,
    Head,
    Prev(Node),
    Next(Node),
    Tail,
}

#[derive(Default, Debug)]
pub struct ViewMap(pub IndexMap<ViewId, View>);
impl ViewMap {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }
    pub fn detach(&mut self, view_id: &ViewId) {
        if let Some(view) = self.0.get_mut(view_id) {
            view.detach();
        } else if let Some(view) = self.get_mut(view_id) {
            view.detach();
        }
    }
    pub fn get(&self, view_id: &ViewId) -> Option<&View> {
        let mut ids = view_id.rise_to_root();
        let mut view = self.0.get(&ids.pop()?)?;
        while !ids.is_empty() {
            view = view.child(&ids.pop()?)?;
        }
        Some(view)
    }
    pub fn get_mut(&mut self, view_id: &ViewId) -> Option<&mut View> {
        let mut ids = view_id.rise_to_root();
        let mut view = self.0.get_mut(&ids.pop()?)?;
        while !ids.is_empty() {
            view = view.child_mut(&ids.pop()?)?;
        }
        Some(view)
    }
}

impl Deref for ViewMap {
    type Target = IndexMap<ViewId, View>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ViewMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
    #[cfg(not(feature = "__single_holder"))]
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
        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
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

        #[cfg(not(feature = "__single_holder"))]
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
            if #[cfg(feature = "__single_holder")] {
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
