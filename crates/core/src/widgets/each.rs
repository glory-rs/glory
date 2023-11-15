use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

use educe::Educe;
use indexmap::IndexMap;

use crate::reflow::{Lotus, Revisable};
use crate::{Scope, ViewId, Widget};

#[derive(Educe)]
#[educe(Debug)]
pub struct Each<Value, ITter, KeyFn, Key, TmplFn, Tmpl>
where
    Value: fmt::Debug + 'static,
    ITter: AsRef<[Value]> + fmt::Debug + 'static,
    KeyFn: Fn(&Value) -> Key + 'static,
    Key: Eq + Hash + Clone + fmt::Debug + 'static,
    TmplFn: Fn(&Value) -> Tmpl + 'static,
    Tmpl: Widget + 'static,
{
    items: Lotus<ITter>,
    #[educe(Debug(ignore))]
    key_fn: KeyFn,
    #[educe(Debug(ignore))]
    tmpl_fn: TmplFn,
    key_view_ids: IndexMap<Key, ViewId>,
    #[educe(Debug(ignore))]
    _pd: PhantomData<(Value, ITter)>,
}

impl<Value, ITter, KeyFn, Key, TmplFn, Tmpl> Each<Value, ITter, KeyFn, Key, TmplFn, Tmpl>
where
    Value: fmt::Debug + 'static,
    ITter: AsRef<[Value]> + fmt::Debug + 'static,
    KeyFn: Fn(&Value) -> Key + 'static,
    Key: Eq + Hash + Clone + fmt::Debug + 'static,
    TmplFn: Fn(&Value) -> Tmpl + 'static,
    Tmpl: Widget + 'static,
{
    pub fn new(items: impl Into<Lotus<ITter>>, key_fn: KeyFn, tmpl_fn: TmplFn) -> Self {
        Self {
            items: items.into(),
            key_fn,
            tmpl_fn,
            key_view_ids: IndexMap::new(),
            _pd: PhantomData,
        }
    }
}
impl<Value, KeyFn, Key, TmplFn, Tmpl> Each<Value, Vec<Value>, KeyFn, Key, TmplFn, Tmpl>
where
    Value: fmt::Debug + 'static,
    KeyFn: Fn(&Value) -> Key + 'static,
    Key: Eq + Hash + Clone + fmt::Debug + 'static,
    TmplFn: Fn(&Value) -> Tmpl + 'static,
    Tmpl: Widget + 'static,
{
    pub fn from_vec(items: impl Into<Lotus<Vec<Value>>>, key_fn: KeyFn, tmpl_fn: TmplFn) -> Self {
        Self {
            items: items.into(),
            key_fn,
            tmpl_fn,
            key_view_ids: IndexMap::new(),
            _pd: PhantomData,
        }
    }
}

impl<Value, ITter, KeyFn, Key, TmplFn, Tmpl> Widget for Each<Value, ITter, KeyFn, Key, TmplFn, Tmpl>
where
    Value: fmt::Debug + 'static,
    ITter: AsRef<[Value]> + fmt::Debug + 'static,
    KeyFn: Fn(&Value) -> Key + 'static,
    Key: Eq + Hash + Clone + fmt::Debug + 'static,
    TmplFn: Fn(&Value) -> Tmpl + 'static,
    Tmpl: Widget + 'static,
{
    fn build(&mut self, ctx: &mut Scope) {
        self.items.bind_view(ctx.view_id());
        for item in self.items.get().as_ref() {
            let key = (self.key_fn)(item);
            let view_id = (self.tmpl_fn)(item).show_in(ctx);
            self.key_view_ids.insert(key, view_id);
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        let mut key_view_ids = std::mem::take(&mut self.key_view_ids);

        let mut operations = Vec::with_capacity(key_view_ids.len());
        for (index, item) in self.items.get().as_ref().iter().enumerate() {
            let key = (self.key_fn)(item);
            if let Some(view_id) = key_view_ids.remove(&key) {
                self.key_view_ids.insert(key, view_id.clone());
                operations.push(ViewOperation::Reuse(index, view_id));
            } else {
                let view_id = (self.tmpl_fn)(item).store_in(ctx);
                self.key_view_ids.insert(key, view_id.clone());
                operations.push(ViewOperation::Insert(index, view_id.clone()));
            }
        }
        crate::warn!("key_view_ids: {:?}", key_view_ids);
        if !key_view_ids.is_empty() {
            cfg_if! {
                if #[cfg(feature = "__single_holder")] {
                    crate::reflow::batch(|| {
                        for view_id in key_view_ids.values() {
                            ctx.detach_child(view_id);
                        }
                    });
                } else {
                    crate::reflow::batch(ctx.holder_id(), || {
                        for view_id in key_view_ids.values() {
                            ctx.detach_child(view_id);
                        }
                    });
                }
            }
        }
        operations.reverse();

        // crate::warn!("===========operations: {:#?}", operations);

        while let Some(operation) = operations.pop() {
            match operation {
                ViewOperation::Reuse(index, view_id) => {
                    let cur_index = ctx.child_views.get_index_of(&view_id).unwrap();
                    if cur_index != index {
                        ctx.child_views.move_index(cur_index, index);
                        let view = ctx.child_views.get_mut(&view_id).unwrap();
                        view.scope.is_attached = false;
                        ctx.attach_child(&view_id);
                    }
                }
                ViewOperation::Insert(index, view_id) => {
                    ctx.child_views.move_index(ctx.child_views.len() - 1, index);
                    ctx.attach_child(&view_id);
                }
            }
        }
    }
}

#[derive(Debug)]
enum ViewOperation {
    Reuse(usize, ViewId),
    Insert(usize, ViewId),
}
