use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use glory_core::{Cage, Lotus};
use glory_render::{Descriptor, Props};

use crate::app::{Error, Mounted, Result, Scope, WeakScope, Widget};
use crate::list::{Cache, RowHost};

type Factory = Box<dyn FnMut() -> Box<dyn Widget>>;
pub struct When {
    condition: Option<Lotus<bool>>,
    yes: Option<Factory>,
    no: Option<Factory>,
}
pub fn when<W: Widget>(
    condition: impl Into<Lotus<bool>>,
    mut yes: impl FnMut() -> W + 'static,
) -> When {
    When {
        condition: Some(condition.into()),
        yes: Some(Box::new(move || Box::new(yes()))),
        no: None,
    }
}
impl When {
    pub fn otherwise<W: Widget>(mut self, mut no: impl FnMut() -> W + 'static) -> Self {
        self.no = Some(Box::new(move || Box::new(no())));
        self
    }
}
struct Branch {
    selected: Option<bool>,
    mounted: Option<Mounted>,
    yes: Factory,
    no: Option<Factory>,
}
impl Widget for When {
    fn build(&mut self, scope: &Scope) -> Result {
        let anchor = scope.create(Descriptor::new(Props::Group))?;
        let content = scope.child_in(anchor)?;
        let parent = content.weak();
        let session = Rc::downgrade(&scope.session()?);
        let condition = self.condition.take().ok_or(Error::Gone)?;
        let state = Rc::new(RefCell::new(Branch {
            selected: None,
            mounted: None,
            yes: self.yes.take().ok_or(Error::Gone)?,
            no: self.no.take(),
        }));
        scope.retain(content)?;
        scope.retain(state.clone())?;
        scope.structural(move || {
            let Some(session) = session.upgrade() else {
                return;
            };
            let Some(parent) = parent.upgrade() else {
                return;
            };
            let show = match condition.try_get() {
                Ok(show) => show,
                Err(error) => {
                    session.report(error.into());
                    return;
                }
            };
            if state.borrow().selected == Some(show) {
                return;
            }
            let old = state.borrow_mut().mounted.take();
            drop(old);
            let mut state = state.borrow_mut();
            let mounted = if show {
                Some(parent.mount_with(|| (state.yes)()))
            } else {
                state.no.as_mut().map(|factory| parent.mount_with(factory))
            };
            match mounted.transpose() {
                Ok(mounted) => {
                    state.mounted = mounted;
                    state.selected = Some(show);
                }
                Err(error) => {
                    state.selected = None;
                    session.report(error);
                }
            }
        });
        Ok(())
    }
}

enum RowFactory<T: 'static> {
    Reactive(Box<dyn FnMut(Cage<T>, Cage<usize>) -> Box<dyn Widget>>),
    Seed(Box<dyn FnMut(T) -> Box<dyn Widget>>),
}
type KeyFn<T, K> = Box<dyn Fn(&T) -> K>;
pub struct Each<T: Clone + 'static, K: Clone + Eq + Hash + 'static> {
    items: Option<Lotus<Vec<T>>>,
    key: Option<KeyFn<T, K>>,
    render: Option<RowFactory<T>>,
}
/// Mount the complete keyed sequence. For large scrollable data use windowed
/// list controls; ordinary each intentionally scans/renders the whole sequence.
/// Row factories receive owned reactive item/index handles and run once per key.
pub fn each<T: Clone + 'static, K: Clone + Eq + Hash + 'static, W: Widget>(
    items: impl Into<Lotus<Vec<T>>>,
    key: impl Fn(&T) -> K + 'static,
    mut render: impl FnMut(Cage<T>, Cage<usize>) -> W + 'static,
) -> Each<T, K> {
    Each {
        items: Some(items.into()),
        key: Some(Box::new(key)),
        render: Some(RowFactory::Reactive(Box::new(move |item, index| {
            Box::new(render(item, index))
        }))),
    }
}
/// Mount a complete keyed sequence whose factory consumes only the initial
/// value of each key. Retained keys keep their widget and local state: later
/// item replacements and index changes are deliberately not delivered to it.
/// Change the key to recreate a row, or use `each` for reactive item/index data.
/// The factory runs under the row Owner, without item/index Cages or a separate
/// data Owner. Removed keys retire immediately; reinsertion gets a fresh seed.
pub fn each_seed<T: Clone + 'static, K: Clone + Eq + Hash + 'static, W: Widget>(
    items: impl Into<Lotus<Vec<T>>>,
    key: impl Fn(&T) -> K + 'static,
    mut render: impl FnMut(T) -> W + 'static,
) -> Each<T, K> {
    Each {
        items: Some(items.into()),
        key: Some(Box::new(key)),
        render: Some(RowFactory::Seed(Box::new(move |item| {
            Box::new(render(item))
        }))),
    }
}
struct BoundRow<T: 'static> {
    mounted: Mounted,
    data: Option<BoundData<T>>,
}
struct BoundData<T: 'static> {
    _data: Scope,
    item: Cage<T>,
    index: Cage<usize>,
}
struct Rows<T: Clone + 'static, K> {
    parent: WeakScope,
    items: Vec<T>,
    key: KeyFn<T, K>,
    render: RowFactory<T>,
    error: Option<Error>,
}
impl<T: Clone + 'static, K: Clone + Eq + Hash> RowHost for Rows<T, K> {
    type Key = K;
    type Row = Option<BoundRow<T>>;
    fn key(&mut self, index: usize) -> K {
        (self.key)(&self.items[index])
    }
    fn mount(&mut self, index: usize, _: &K) -> Self::Row {
        let row = (|| {
            let parent = self.parent.upgrade().ok_or(Error::Gone)?;
            let RowFactory::Reactive(render) = &mut self.render else {
                let RowFactory::Seed(render) = &mut self.render else {
                    unreachable!()
                };
                let seed = self.items[index].clone();
                return Ok(BoundRow {
                    mounted: parent.mount_with(|| render(seed))?,
                    data: None,
                });
            };
            let data = parent.child()?;
            let item = data.cage(self.items[index].clone());
            let position = data.cage(index);
            let mounted = data.owner().enter(|| render(item, position)).mount(&data)?;
            Ok(BoundRow {
                mounted,
                data: Some(BoundData {
                    _data: data,
                    item,
                    index: position,
                }),
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
        if let Some(row) = row.as_mut().and_then(|row| row.data.as_mut()) {
            row.item.revise(|value| *value = self.items[index].clone());
            if row.index.get_untracked() != index {
                row.index.revise(|value| *value = index);
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
    fn has_content(&self, row: &Self::Row) -> bool {
        row.as_ref()
            .is_some_and(|row| !row.mounted.roots().is_empty())
    }
}
struct EachState<T: Clone + 'static, K> {
    cache: Cache<K, Option<BoundRow<T>>>,
    host: Rows<T, K>,
    revision: u64,
}
impl<T: Clone + 'static, K: Clone + Eq + Hash + 'static> Widget for Each<T, K> {
    fn build(&mut self, scope: &Scope) -> Result {
        let anchor = scope.create(Descriptor::new(Props::Group))?;
        let content = scope.child_in(anchor)?;
        let items = self.items.take().ok_or(Error::Gone)?;
        let host = Rows {
            parent: content.weak(),
            items: Vec::new(),
            key: self.key.take().ok_or(Error::Gone)?,
            render: self.render.take().ok_or(Error::Gone)?,
            error: None,
        };
        let state = Rc::new(RefCell::new(EachState {
            cache: Cache::default(),
            host,
            revision: 0,
        }));
        let session = Rc::downgrade(&scope.session()?);
        scope.retain(content)?;
        scope.retain(state.clone())?;
        scope.structural(move || {
            let Some(session) = session.upgrade() else {
                return;
            };
            let items = match items.try_get() {
                Ok(items) => items,
                Err(error) => {
                    session.report(error.into());
                    return;
                }
            };
            let mut state = state.borrow_mut();
            let EachState {
                cache,
                host,
                revision,
            } = &mut *state;
            host.items = items;
            *revision = revision.checked_add(1).expect("list revision exhausted");
            if let Err(error) = cache.reconcile(0..host.items.len(), *revision, host) {
                session.report(Error::List(error));
            }
            if let Some(error) = host.error.take() {
                cache.clear(host);
                session.report(error);
            }
        });
        Ok(())
    }
}
