use std::collections::HashSet;
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
    /// Optional hook fired right after a brand-new row has been
    /// attached (initial render OR patch). Receives the new row's
    /// `ViewId`. Use it to trigger enter animations or kick off
    /// per-row async work.
    #[educe(Debug(ignore))]
    on_enter: Option<Box<dyn Fn(&ViewId)>>,
    /// Optional hook fired right before a row whose key has
    /// disappeared is detached. Receives the leaving row's `ViewId`.
    /// Use it to trigger exit animations or release per-row resources.
    /// The hook runs synchronously before `detach_child`, so users
    /// wanting delayed unmount (CSS transition completion) should
    /// arrange the staging themselves.
    #[educe(Debug(ignore))]
    on_exit: Option<Box<dyn Fn(&ViewId)>>,
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
    /// Build a keyed `Each` over an arbitrary slice-like container.
    ///
    /// `key_fn` extracts a stable identity for each item (keys should be
    /// unique within one snapshot — duplicates collapse to a single
    /// view). `tmpl_fn` is invoked **once per newly-inserted key** to
    /// construct the child widget; on subsequent `patch` cycles the
    /// existing widget is **reused** as long as the key stays present.
    ///
    /// # Reactivity contract for item value changes
    ///
    /// `tmpl_fn` is NOT re-invoked when the underlying item value
    /// changes while the key stays the same. If you mutate an item
    /// in place (e.g. `items.revise(|mut v| v[0].text = "new")`) and
    /// expect the rendered child to update, the child widget must
    /// subscribe to that value itself. The idiomatic pattern is to
    /// model each entry as a struct holding its own `Cage<T>`:
    ///
    /// ```ignore
    /// #[derive(Clone, Debug)]
    /// struct TodoItem { id: u64, text: Cage<String> }
    ///
    /// // tmpl_fn captures a clone of the item, which clones the Cage
    /// // handle — mutations via item.text.revise(...) re-render only
    /// // this row.
    /// Each::new(items, |it| it.id, |it| li().html(it.text.clone()))
    /// ```
    ///
    /// If you pass a `Vec<PlainStruct>` with no inner `Cage`s and
    /// rely on full-list replacement, reorder works correctly but
    /// per-row content changes will be silently dropped.
    pub fn new(items: impl Into<Lotus<ITter>>, key_fn: KeyFn, tmpl_fn: TmplFn) -> Self {
        Self {
            items: items.into(),
            key_fn,
            tmpl_fn,
            key_view_ids: IndexMap::new(),
            on_enter: None,
            on_exit: None,
            _pd: PhantomData,
        }
    }

    /// Register an "on enter" hook fired right after a brand-new row
    /// is attached (both on initial render and on later patches that
    /// introduce a fresh key).
    ///
    /// ```ignore
    /// Each::new(items, |it| it.id, |it| row(it))
    ///     .on_enter(|view_id| start_fade_in(view_id))
    /// ```
    pub fn on_enter(mut self, hook: impl Fn(&ViewId) + 'static) -> Self {
        self.on_enter = Some(Box::new(hook));
        self
    }

    /// Register an "on exit" hook fired right before a row whose key
    /// has disappeared is detached. The hook is synchronous; for CSS
    /// transitions schedule the visual decay yourself before the
    /// detach actually removes the node.
    pub fn on_exit(mut self, hook: impl Fn(&ViewId) + 'static) -> Self {
        self.on_exit = Some(Box::new(hook));
        self
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
            on_enter: None,
            on_exit: None,
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
        // Collect new view ids first so we can fire `on_enter` after the
        // `items` borrow has been released (the hook is user code; we
        // don't want to be holding the items Ref when it runs).
        let mut entered: Vec<ViewId> = Vec::new();
        for item in self.items.get().as_ref() {
            let key = (self.key_fn)(item);
            let view_id = (self.tmpl_fn)(item).show_in(ctx);
            self.key_view_ids.insert(key, view_id.clone());
            if self.on_enter.is_some() {
                entered.push(view_id);
            }
        }
        if let Some(hook) = &self.on_enter {
            for view_id in &entered {
                hook(view_id);
            }
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        let prev_keys = std::mem::take(&mut self.key_view_ids);

        // Snapshot the new ordering. Use a separate scope so the items
        // borrow is released before we start mutating child_views deeply.
        let (mut new_key_view_ids, old_indices, newly_created, ordered_view_ids) = {
            let items_ref = self.items.get();
            let items: &[Value] = items_ref.as_ref();
            let new_len = items.len();

            let mut new_key_view_ids: IndexMap<Key, ViewId> = IndexMap::with_capacity(new_len);
            let mut old_indices: Vec<Option<usize>> = Vec::with_capacity(new_len);
            let mut newly_created: Vec<bool> = Vec::with_capacity(new_len);
            let mut ordered_view_ids: Vec<ViewId> = Vec::with_capacity(new_len);

            for item in items {
                let key = (self.key_fn)(item);
                if let Some((old_idx, _, view_id)) = prev_keys.get_full(&key) {
                    old_indices.push(Some(old_idx));
                    newly_created.push(false);
                    ordered_view_ids.push(view_id.clone());
                    new_key_view_ids.insert(key, view_id.clone());
                } else {
                    let view_id = (self.tmpl_fn)(item).store_in(ctx);
                    old_indices.push(None);
                    newly_created.push(true);
                    ordered_view_ids.push(view_id.clone());
                    new_key_view_ids.insert(key, view_id);
                }
            }

            (new_key_view_ids, old_indices, newly_created, ordered_view_ids)
        };

        // Detach any view whose key disappeared.
        let kept: HashSet<&ViewId> = new_key_view_ids.values().collect();
        let to_detach: Vec<ViewId> = prev_keys.values().filter(|vid| !kept.contains(*vid)).cloned().collect();
        drop(kept);

        // Fire on_exit hooks BEFORE the detach actually removes the
        // node, so user code can observe the leaving view's state
        // (e.g. read DOM bounds for a FLIP animation).
        if let Some(hook) = &self.on_exit {
            for view_id in &to_detach {
                hook(view_id);
            }
        }

        if !to_detach.is_empty() {
            #[cfg(not(feature = "single-app"))]
            let holder_id = ctx.holder_id();
            let detach_all = || {
                for view_id in &to_detach {
                    ctx.detach_child(view_id);
                }
            };
            cfg_if! {
                if #[cfg(feature = "single-app")] {
                    crate::reflow::batch(detach_all);
                } else {
                    crate::reflow::batch(holder_id, detach_all);
                }
            }
        }

        std::mem::swap(&mut self.key_view_ids, &mut new_key_view_ids);

        if ordered_view_ids.is_empty() {
            return;
        }

        // Reorder ctx.child_views to match the new ordering. Items that
        // belong to other widgets (rare for Each, but possible if mixed
        // children exist) sort to the end.
        let target_index: IndexMap<ViewId, usize> = ordered_view_ids.iter().enumerate().map(|(i, vid)| (vid.clone(), i)).collect();
        ctx.child_views.sort_by(|a, _, b, _| {
            let ai = target_index.get(a).copied().unwrap_or(usize::MAX);
            let bi = target_index.get(b).copied().unwrap_or(usize::MAX);
            ai.cmp(&bi)
        });

        // Compute the longest increasing subsequence over the previous
        // indices of reused items. Positions participating in the LIS are
        // already in correct relative order and do not need DOM moves.
        let stable: HashSet<usize> = lis_positions(&old_indices).into_iter().collect();

        // Pass 1: snap every "to-attach" view into a clean state UP FRONT.
        //
        // Two kinds of views need re-attachment: reused-but-moving views
        // (already attached at the wrong index) and freshly created views
        // (never attached). Both must hit `attach_child`'s neighbour
        // search path with `position == Unset`, otherwise `attach_child`
        // falls through to the `Tail` fallback and we lose ordering.
        //
        // The pre-mark also clears `is_attached` on moving reused views
        // BEFORE the attach loop. If we did it inside the loop, the
        // per-item neighbour search could anchor against a view that is
        // logically also moving but still flagged attached.
        for (i, view_id) in ordered_view_ids.iter().enumerate() {
            let is_stable_reuse = !newly_created[i] && stable.contains(&i);
            if is_stable_reuse {
                continue;
            }
            if let Some(view) = ctx.child_views.get_mut(view_id) {
                view.scope.is_attached = false;
                view.scope.position = crate::view::ViewPosition::Unset;
            }
        }

        // Pass 2: re-attach moved or new views. Stable reused views keep
        // their existing attachment and DOM position.
        for (i, view_id) in ordered_view_ids.iter().enumerate() {
            if !newly_created[i] && stable.contains(&i) {
                continue;
            }
            ctx.attach_child(view_id);
        }

        // Fire on_enter for freshly-created rows AFTER attach so the
        // DOM node exists when the hook runs (CSS transitions need the
        // element on the page to animate from).
        if let Some(hook) = &self.on_enter {
            for (i, view_id) in ordered_view_ids.iter().enumerate() {
                if newly_created[i] {
                    hook(view_id);
                }
            }
        }
    }
}

/// Indices into `seq` that form a longest increasing subsequence of the
/// `Some` values, preserving original order. `None` entries are skipped
/// (they represent freshly inserted positions with no prior index).
///
/// Runs in O(n log n) via the patience-sort variant.
fn lis_positions(seq: &[Option<usize>]) -> Vec<usize> {
    let pairs: Vec<(usize, usize)> = seq.iter().enumerate().filter_map(|(i, v)| v.map(|val| (val, i))).collect();
    if pairs.is_empty() {
        return Vec::new();
    }

    let n = pairs.len();
    let mut tail_value: Vec<usize> = Vec::with_capacity(n);
    let mut tail_pair_idx: Vec<usize> = Vec::with_capacity(n);
    let mut prev_pair_idx: Vec<Option<usize>> = vec![None; n];

    for i in 0..n {
        let val = pairs[i].0;
        let pos = tail_value.partition_point(|&v| v < val);
        if pos == tail_value.len() {
            tail_value.push(val);
            tail_pair_idx.push(i);
        } else {
            tail_value[pos] = val;
            tail_pair_idx[pos] = i;
        }
        if pos > 0 {
            prev_pair_idx[i] = Some(tail_pair_idx[pos - 1]);
        }
    }

    let mut chain = Vec::with_capacity(tail_pair_idx.len());
    let mut cur = tail_pair_idx.last().copied();
    while let Some(pair_i) = cur {
        chain.push(pair_i);
        cur = prev_pair_idx[pair_i];
    }
    chain.reverse();

    chain.into_iter().map(|pair_i| pairs[pair_i].1).collect()
}

#[cfg(test)]
mod tests {
    use super::lis_positions;

    #[test]
    fn lis_empty() {
        assert!(lis_positions(&[]).is_empty());
    }

    #[test]
    fn lis_all_none() {
        assert!(lis_positions(&[None, None, None]).is_empty());
    }

    #[test]
    fn lis_single() {
        assert_eq!(lis_positions(&[Some(5)]), vec![0]);
    }

    #[test]
    fn lis_monotone_increasing() {
        assert_eq!(lis_positions(&[Some(0), Some(1), Some(2)]), vec![0, 1, 2]);
    }

    #[test]
    fn lis_monotone_decreasing() {
        // Any single element is a valid LIS of length 1.
        let r = lis_positions(&[Some(2), Some(1), Some(0)]);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn lis_skip_singleton_insert() {
        // [reuse old=1, insert new, reuse old=0] -> LIS is just one of the reuses.
        let r = lis_positions(&[Some(1), None, Some(0)]);
        assert_eq!(r.len(), 1);
        assert!(r[0] == 0 || r[0] == 2);
    }

    #[test]
    fn lis_picks_increasing_around_inserts() {
        // pairs = [(1,0), (0,2), (2,3)]; LIS values = [0, 2] -> positions [2, 3]
        let r = lis_positions(&[Some(1), None, Some(0), Some(2)]);
        assert_eq!(r, vec![2, 3]);
    }

    #[test]
    fn lis_typical_swap_two_adjacent() {
        // Previously [A,B], now [B,A]; old_indices = [Some(1), Some(0)].
        // Any single element is a valid LIS; choose the latest as the algorithm does.
        let r = lis_positions(&[Some(1), Some(0)]);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn lis_append_tail() {
        // Previously [A,B,C], now [A,B,C,D]; old_indices = [Some(0),Some(1),Some(2),None]
        let r = lis_positions(&[Some(0), Some(1), Some(2), None]);
        assert_eq!(r, vec![0, 1, 2]);
    }

    #[test]
    fn lis_prepend_head() {
        // Previously [A,B,C], now [X,A,B,C]
        let r = lis_positions(&[None, Some(0), Some(1), Some(2)]);
        assert_eq!(r, vec![1, 2, 3]);
    }
}
