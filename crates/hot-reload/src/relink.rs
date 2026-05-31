use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use anyhow::{Result, bail};
use parking_lot::RwLock;

type ErasedFn<In, Out> = dyn Fn(In) -> Out + Send + Sync + 'static;

#[derive(Clone)]
pub struct ReloadableFn<In, Out>
where
    In: Send + 'static,
    Out: Send + 'static,
{
    id: Arc<str>,
    owner_id: Option<Arc<str>>,
    inner: Arc<RwLock<Box<ErasedFn<In, Out>>>>,
}

impl<In, Out> ReloadableFn<In, Out>
where
    In: Send + 'static,
    Out: Send + 'static,
{
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn owner_id(&self) -> Option<&str> {
        self.owner_id.as_deref()
    }

    pub fn call(&self, input: In) -> Out {
        (self.inner.read())(input)
    }

    pub fn replace(&self, func: impl Fn(In) -> Out + Send + Sync + 'static) {
        *self.inner.write() = Box::new(func);
    }
}

impl<In, Out> fmt::Debug for ReloadableFn<In, Out>
where
    In: Send + 'static,
    Out: Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReloadableFn").field("id", &self.id).finish()
    }
}

#[derive(Default)]
pub struct FunctionRegistry {
    functions: RwLock<HashMap<Arc<str>, FunctionEntry>>,
}

struct FunctionEntry {
    owner_id: Option<Arc<str>>,
    inner: Box<dyn Any + Send + Sync>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<In, Out>(&self, id: impl Into<Arc<str>>, func: impl Fn(In) -> Out + Send + Sync + 'static) -> Result<ReloadableFn<In, Out>>
    where
        In: Send + 'static,
        Out: Send + 'static,
    {
        self.register_inner(None, id.into(), func)
    }

    pub fn register_with_owner<In, Out>(
        &self,
        owner_id: impl Into<Arc<str>>,
        id: impl Into<Arc<str>>,
        func: impl Fn(In) -> Out + Send + Sync + 'static,
    ) -> Result<ReloadableFn<In, Out>>
    where
        In: Send + 'static,
        Out: Send + 'static,
    {
        self.register_inner(Some(owner_id.into()), id.into(), func)
    }

    fn register_inner<In, Out>(
        &self,
        owner_id: Option<Arc<str>>,
        id: Arc<str>,
        func: impl Fn(In) -> Out + Send + Sync + 'static,
    ) -> Result<ReloadableFn<In, Out>>
    where
        In: Send + 'static,
        Out: Send + 'static,
    {
        let mut functions = self.functions.write();
        if let Some(existing) = functions.get(&id) {
            if existing.owner_id != owner_id {
                bail!("hot function `{id}` already exists with a different owner");
            }
            let handle = existing
                .inner
                .downcast_ref::<Arc<RwLock<Box<ErasedFn<In, Out>>>>>()
                .ok_or_else(|| anyhow::anyhow!("hot function `{id}` already exists with a different signature"))?;
            return Ok(ReloadableFn {
                id,
                owner_id,
                inner: handle.clone(),
            });
        }

        let inner: Arc<RwLock<Box<ErasedFn<In, Out>>>> = Arc::new(RwLock::new(Box::new(func)));
        functions.insert(
            id.clone(),
            FunctionEntry {
                owner_id: owner_id.clone(),
                inner: Box::new(inner.clone()),
            },
        );
        Ok(ReloadableFn { id, owner_id, inner })
    }

    pub fn replace<In, Out>(&self, id: &str, func: impl Fn(In) -> Out + Send + Sync + 'static) -> Result<()>
    where
        In: Send + 'static,
        Out: Send + 'static,
    {
        let functions = self.functions.read();
        let Some(existing) = functions.get(id) else {
            bail!("hot function `{id}` is not registered");
        };
        let handle = existing
            .inner
            .downcast_ref::<Arc<RwLock<Box<ErasedFn<In, Out>>>>>()
            .ok_or_else(|| anyhow::anyhow!("hot function `{id}` exists with a different signature"))?;
        *handle.write() = Box::new(func);
        Ok(())
    }

    pub fn replace_with_owner<In, Out>(&self, owner_id: &str, id: &str, func: impl Fn(In) -> Out + Send + Sync + 'static) -> Result<()>
    where
        In: Send + 'static,
        Out: Send + 'static,
    {
        let functions = self.functions.read();
        let Some(existing) = functions.get(id) else {
            bail!("hot function `{id}` is not registered");
        };
        if existing.owner_id.as_deref() != Some(owner_id) {
            bail!("hot function `{id}` exists with a different owner");
        }
        let handle = existing
            .inner
            .downcast_ref::<Arc<RwLock<Box<ErasedFn<In, Out>>>>>()
            .ok_or_else(|| anyhow::anyhow!("hot function `{id}` exists with a different signature"))?;
        *handle.write() = Box::new(func);
        Ok(())
    }
}

impl fmt::Debug for FunctionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionRegistry").field("len", &self.functions.read().len()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_handles_call_replaced_body() {
        let registry = FunctionRegistry::new();
        let func = registry.register("row", |value: i32| value + 1).unwrap();

        assert_eq!(func.call(1), 2);
        registry.replace("row", |value: i32| value + 10).unwrap();
        assert_eq!(func.call(1), 11);
    }

    #[test]
    fn registering_same_id_returns_same_handle() {
        let registry = FunctionRegistry::new();
        let first = registry.register("row", |value: i32| value + 1).unwrap();
        let second = registry.register("row", |value: i32| value + 100).unwrap();

        first.replace(|value| value * 2);
        assert_eq!(second.call(3), 6);
    }

    #[test]
    fn signature_mismatch_is_rejected() {
        let registry = FunctionRegistry::new();
        registry.register("row", |value: i32| value + 1).unwrap();

        let err = registry.register("row", |value: String| value).unwrap_err();
        assert!(err.to_string().contains("different signature"));
        assert!(registry.replace("row", |value: String| value).is_err());
    }

    #[test]
    fn owner_key_keeps_stateful_handle_affinity() {
        let registry = FunctionRegistry::new();
        let first = registry.register_with_owner("scope-1", "row", |value: i32| value + 1).unwrap();
        let second = registry.register_with_owner("scope-1", "row", |value: i32| value + 100).unwrap();

        assert_eq!(first.owner_id(), Some("scope-1"));
        registry.replace_with_owner("scope-1", "row", |value: i32| value + 2).unwrap();
        assert_eq!(second.call(2), 4);

        let err = registry.register_with_owner("scope-2", "row", |value: i32| value).unwrap_err();
        assert!(err.to_string().contains("different owner"));
        assert!(registry.replace_with_owner("scope-2", "row", |value: i32| value).is_err());
    }
}
