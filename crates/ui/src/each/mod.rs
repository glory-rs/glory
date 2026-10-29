//! One keyed Each API and controller for all sources and Widget hosts.
use crate::{Result, Widget};
use glory_core::Scope as CoreScope;
use std::hash::Hash;
pub mod source;
use source::{Initial, IntoSource, Seeds, Source, Unkeyed, Values};
mod mounting;
#[doc(hidden)]
pub use mounting::{Direct, Ownership, Reactive, UiMount, UiRow};

pub struct EachBuilder<S> {
    source: S,
}
pub fn each<S: IntoSource>(source: S) -> EachBuilder<S::Source> {
    EachBuilder {
        source: source.into_source(),
    }
}
impl<T: Clone + 'static> EachBuilder<Unkeyed<T>> {
    pub fn key<K, F>(self, key: F) -> EachBuilder<Values<T, K, F>>
    where
        K: Clone + Eq + Hash + 'static,
        F: Fn(&T) -> K + 'static,
    {
        EachBuilder {
            source: Values {
                source: self.source.0,
                key,
                items: Vec::new(),
                marker: std::marker::PhantomData,
            },
        }
    }
}
impl<S: Source> EachBuilder<S> {
    pub fn children<F, W>(self, factory: F) -> Each<S, F>
    where
        F: FnMut(S::Row) -> W + 'static,
        W: Widget,
    {
        Each {
            source: Some(self.source),
            factory: Some(factory),
        }
    }
}
impl<S: Initial> EachBuilder<S> {
    /// Use an initial value per membership; retained rows keep local state.
    /// No item/index cages or additional data Owner are allocated.
    pub fn seed<F, W>(self, factory: F) -> Each<Seeds<S>, F>
    where
        F: FnMut(S::Value) -> W + 'static,
        W: Widget,
    {
        Each {
            source: Some(Seeds(self.source)),
            factory: Some(factory),
        }
    }
}

pub struct Each<S, F> {
    source: Option<S>,
    factory: Option<F>,
}

/// Physical row operations. Implemented by semantic and sparse Widget hosts.
#[doc(hidden)]
pub trait Mount: Clone + 'static {
    type Row<O: Ownership>: 'static;
    fn mount<O: Ownership, W: Widget>(
        &self,
        factory: impl FnOnce(&CoreScope) -> W,
    ) -> Result<Self::Row<O>>;
    fn place<O: Ownership>(&self, row: &Self::Row<O>, before: Option<&Self::Row<O>>) -> Result;
    fn has_content<O: Ownership>(&self, row: &Self::Row<O>) -> bool;
    fn report(&self, error: crate::Error);
    fn clear_on_error(&self) -> bool {
        false
    }
}
struct Row<M, D> {
    mounted: M,
    data: D,
}
struct Host<S: Source, F, M: Mount> {
    source: S,
    factory: F,
    parent: M,
    fault: Option<crate::Error>,
}
impl<S, F, W, M> crate::list::RowHost for Host<S, F, M>
where
    S: Source,
    F: FnMut(S::Row) -> W + 'static,
    W: Widget,
    M: Mount,
{
    type Key = S::Key;
    type Row = Option<Row<M::Row<S::Ownership>, S::Data>>;
    fn key(&mut self, index: usize) -> S::Key {
        self.source.key(index)
    }
    fn mount(&mut self, index: usize, key: &S::Key) -> Self::Row {
        if self.fault.is_some() {
            return None;
        }
        let seed = match self.source.prepare(index, key) {
            Ok(seed) => seed,
            Err(error) => {
                self.fault = Some(error);
                return None;
            }
        };
        let mut data = None;
        let result = self.parent.mount::<S::Ownership, _>(|owner| {
            let (row, state) = self.source.row(seed, owner);
            data = Some(state);
            (self.factory)(row)
        });
        match result {
            Ok(mounted) => Some(Row {
                mounted,
                data: data.expect("row factory completed"),
            }),
            Err(error) => {
                self.fault = Some(error);
                None
            }
        }
    }
    fn update(&mut self, index: usize, _: &S::Key, row: &mut Self::Row) {
        if let Some(row) = row {
            self.source.update(index, &mut row.data);
        }
    }
    fn has_content(&self, row: &Self::Row) -> bool {
        row.as_ref()
            .is_some_and(|row| self.parent.has_content(&row.mounted))
    }
    fn place(&mut self, row: &Self::Row, before: Option<&Self::Row>) {
        if self.fault.is_none()
            && let Some(row) = row
            && let Err(error) = self.parent.place(
                &row.mounted,
                before.and_then(Option::as_ref).map(|row| &row.mounted),
            )
        {
            self.fault = Some(error);
        }
    }
    fn dispose(&mut self, row: Self::Row) {
        drop(row);
    }
}
impl<S, F, W> Each<S, F>
where
    S: Source,
    F: FnMut(S::Row) -> W + 'static,
    W: Widget,
{
    #[doc(hidden)]
    pub fn validate(&self, scope: &CoreScope) -> Result {
        self.source
            .as_ref()
            .ok_or(crate::Error::Gone)?
            .validate(scope.runtime())
    }
    #[doc(hidden)]
    pub fn install<M: Mount>(mut self, scope: &CoreScope, parent: M) -> Result {
        self.install_in(scope, parent)
    }
    fn install_in<M: Mount>(&mut self, scope: &CoreScope, parent: M) -> Result {
        self.validate(scope)?;
        let mut host = Host {
            source: self.source.take().ok_or(crate::Error::Gone)?,
            factory: self.factory.take().ok_or(crate::Error::Gone)?,
            parent,
            fault: None,
        };
        let mut cache = crate::list::Cache::default();
        let mut revision = 0u64;
        scope.structural(move || {
            let count = match host.source.refresh() {
                Ok(count) => count,
                Err(error) => {
                    host.parent.report(error);
                    return;
                }
            };
            revision = revision.checked_add(1).expect("list revision exhausted");
            if let Err(error) = cache.reconcile(0..count, revision, &mut host) {
                host.parent.report(crate::Error::List(error));
            }
            if let Some(error) = host.fault.take() {
                if host.parent.clear_on_error() {
                    cache.clear(&mut host);
                }
                host.parent.report(error);
            }
        });
        Ok(())
    }
}
impl<S, F, W> Widget for Each<S, F>
where
    S: Source,
    F: FnMut(S::Row) -> W + 'static,
    W: Widget,
{
    fn build(&mut self, scope: &crate::Scope) -> Result {
        self.validate(scope)?;
        let anchor = scope.create(glory_render::Descriptor::new(glory_render::Props::Group))?;
        let content = scope.child_in(anchor)?;
        self.install_in(&content, UiMount::new(&content))?;
        scope.retain(content)
    }
}
