//! Source adapters. Keyed members keep their direct aliases; vectors keep row cages.
use super::*;
use glory_core::{Bond, Cage, KeyedEntry, KeyedList, RowToken, RuntimeId, Vary};
use std::borrow::Cow;

mod sealed {
    pub trait Source {}
    pub trait Input {}
}

/// A stable keyed source for the shared Each controller.
#[doc(hidden)]
pub trait Source: sealed::Source + 'static {
    type Key: Clone + Eq + Hash + 'static;
    type Row;
    type Seed;
    type Data: 'static;
    type Ownership: super::Ownership;
    fn validate(&self, runtime: RuntimeId) -> Result;
    fn refresh(&mut self) -> Result<usize>;
    fn key(&self, index: usize) -> Self::Key;
    fn prepare(&self, index: usize, key: &Self::Key) -> Result<Self::Seed>;
    fn row(&self, seed: Self::Seed, owner: &CoreScope) -> (Self::Row, Self::Data);
    fn update(&self, index: usize, data: &mut Self::Data);
}

/// Input conversion performs no reactive reads or mounting.
pub trait IntoSource: sealed::Input {
    type Source;
    #[doc(hidden)]
    fn into_source(self) -> Self::Source;
}

pub struct Members<K: 'static, T: 'static> {
    source: KeyedList<K, T>,
    tokens: Vec<RowToken<K>>,
}
impl<K, T> sealed::Input for KeyedList<K, T> {}
impl<K, T> IntoSource for KeyedList<K, T> {
    type Source = Members<K, T>;
    fn into_source(self) -> Self::Source {
        Members {
            source: self,
            tokens: Vec::new(),
        }
    }
}
impl<K, T> sealed::Source for Members<K, T> {}
impl<K: Clone + Eq + Hash + 'static, T: 'static> Source for Members<K, T> {
    type Key = u64;
    type Row = KeyedEntry<K, T>;
    type Seed = KeyedEntry<K, T>;
    type Data = ();
    type Ownership = super::Direct;
    fn validate(&self, runtime: RuntimeId) -> Result {
        if self.source.runtime() != runtime || !self.source.is_alive() {
            return Err(glory_render::Error::InvalidProps("foreign or disposed list").into());
        }
        Ok(())
    }
    fn refresh(&mut self) -> Result<usize> {
        self.tokens = self.source.try_with_order(<[_]>::to_vec)?;
        Ok(self.tokens.len())
    }
    fn key(&self, index: usize) -> u64 {
        self.tokens[index].id()
    }
    fn prepare(&self, index: usize, _: &u64) -> Result<Self::Seed> {
        Ok(self.source.entry(&self.tokens[index])?)
    }
    fn row(&self, seed: Self::Seed, _: &CoreScope) -> (Self::Row, ()) {
        (seed, ())
    }
    fn update(&self, _: usize, _: &mut ()) {}
}

/// A vector needs an explicit stable key before it can render rows.
pub struct Unkeyed<T: 'static>(pub(super) Vary<Vec<T>>);
macro_rules! input {
    ($ty:ty) => {
        impl<T: Clone + 'static> sealed::Input for $ty {}
        impl<T: Clone + 'static> IntoSource for $ty {
            type Source = Unkeyed<T>;
            fn into_source(self) -> Self::Source {
                Unkeyed(self.into())
            }
        }
    };
}
input!(Vec<T>);
input!(Vary<Vec<T>>);
input!(Cage<Vec<T>>);
input!(Bond<Vec<T>>);

pub struct Values<T: 'static, K, F> {
    pub(super) source: Vary<Vec<T>>,
    pub(super) key: F,
    pub(super) items: Vec<T>,
    pub(super) marker: std::marker::PhantomData<K>,
}

/// Reactive vector row. Both handles belong to the row's data Owner.
pub struct ValueRow<K, T: 'static> {
    key: K,
    pub value: Cage<T>,
    pub index: Cage<usize>,
}
impl<K: Clone, T> Clone for ValueRow<K, T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value,
            index: self.index,
        }
    }
}
impl<K, T> ValueRow<K, T> {
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn try_with_untracked<R>(
        &self,
        read: impl FnOnce(&T) -> R,
    ) -> std::result::Result<R, glory_core::AccessError> {
        self.value.try_with_untracked(read)
    }
    pub fn update<R>(
        &self,
        edit: impl FnOnce(&mut T) -> R,
    ) -> std::result::Result<R, glory_core::AccessError> {
        self.value.try_revise(edit)
    }
    pub fn field<U: 'static>(
        &self,
        name: impl Into<Cow<'static, str>>,
        get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
        get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
    ) -> Cage<U> {
        self.value.field(name, get, get_mut)
    }
}
impl<T, K, F> sealed::Source for Values<T, K, F> {}
fn validate<T>(source: &Vary<Vec<T>>, runtime: RuntimeId) -> Result {
    let valid = match source {
        Vary::Value(_) => true,
        Vary::Cage(value) => value.runtime() == runtime && value.is_alive(),
        Vary::Bond(value) => value.runtime() == runtime && value.is_alive(),
    };
    if valid {
        Ok(())
    } else {
        Err(glory_render::Error::InvalidProps("foreign or disposed list").into())
    }
}
impl<T, K, F> Source for Values<T, K, F>
where
    T: Clone + 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn(&T) -> K + 'static,
{
    type Key = K;
    type Row = ValueRow<K, T>;
    type Seed = (usize, K);
    type Data = (Cage<T>, Cage<usize>);
    type Ownership = super::Reactive;
    fn validate(&self, runtime: RuntimeId) -> Result {
        validate(&self.source, runtime)
    }
    fn refresh(&mut self) -> Result<usize> {
        self.items = self.source.try_get()?;
        Ok(self.items.len())
    }
    fn key(&self, index: usize) -> K {
        (self.key)(&self.items[index])
    }
    fn prepare(&self, index: usize, key: &K) -> Result<Self::Seed> {
        Ok((index, key.clone()))
    }
    fn row(&self, (index, key): Self::Seed, owner: &CoreScope) -> (Self::Row, Self::Data) {
        let value = owner.cage(self.items[index].clone());
        let position = owner.cage(index);
        (
            ValueRow {
                key,
                value,
                index: position,
            },
            (value, position),
        )
    }
    fn update(&self, index: usize, (value, position): &mut Self::Data) {
        value.revise(|value| *value = self.items[index].clone());
        if position.get_untracked() != index {
            position.revise(|value| *value = index);
        }
    }
}

#[doc(hidden)]
pub trait Initial: Source {
    type Value;
    fn initial(&self, index: usize) -> Result<Self::Value>;
}
impl<T, K, F> Initial for Values<T, K, F>
where
    T: Clone + 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn(&T) -> K + 'static,
{
    type Value = T;
    fn initial(&self, index: usize) -> Result<T> {
        Ok(self.items[index].clone())
    }
}
impl<K: Clone + Eq + Hash + 'static, T: Clone + 'static> Initial for Members<K, T> {
    type Value = T;
    fn initial(&self, index: usize) -> Result<T> {
        Ok(self
            .source
            .entry(&self.tokens[index])?
            .try_with_untracked(Clone::clone)?)
    }
}
pub struct Seeds<S>(pub(super) S);
impl<S> sealed::Source for Seeds<S> {}
impl<S: Initial> Source for Seeds<S> {
    type Key = S::Key;
    type Row = S::Value;
    type Seed = S::Value;
    type Data = ();
    type Ownership = super::Direct;
    fn validate(&self, runtime: RuntimeId) -> Result {
        self.0.validate(runtime)
    }
    fn refresh(&mut self) -> Result<usize> {
        self.0.refresh()
    }
    fn key(&self, index: usize) -> Self::Key {
        self.0.key(index)
    }
    fn prepare(&self, index: usize, _: &Self::Key) -> Result<Self::Seed> {
        self.0.initial(index)
    }
    fn row(&self, seed: Self::Seed, _: &CoreScope) -> (Self::Row, ()) {
        (seed, ())
    }
    fn update(&self, _: usize, _: &mut ()) {}
}
