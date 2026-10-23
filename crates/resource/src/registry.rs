use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::{Rc, Weak};

use glory_core::{AccessError, Owner, Resource, ResourceSeed, ResourceState, RuntimeId, untracked};
use serde::{Serialize, de::DeserializeOwned};

use crate::snapshot::{Outcome, Record, encode};
use crate::{Error, Limits, Snapshot, names};

type Read = Rc<dyn Fn() -> Result<Option<Record>, Error>>;
struct Entry {
    ticket: Rc<()>,
    read: Option<Read>,
}
struct Store {
    runtime: Option<RuntimeId>,
    seeds: BTreeMap<String, Record>,
    live: BTreeMap<String, Entry>,
    limits: Limits,
}
/// A request/app context. Duplicate live names are errors; sharing one resource
/// between consumers means sharing its Copy Resource handle, not rebinding it.
#[derive(Clone)]
pub struct Resources(Rc<RefCell<Store>>);
impl Default for Resources {
    fn default() -> Self {
        Self::restore(Snapshot::default()).expect("empty snapshot")
    }
}
impl Resources {
    pub fn restore(snapshot: Snapshot) -> Result<Self, Error> {
        snapshot.to_json()?;
        Ok(Self(Rc::new(RefCell::new(Store {
            runtime: None,
            seeds: snapshot
                .records
                .into_iter()
                .map(|record| (record.name.clone(), record))
                .collect(),
            live: BTreeMap::new(),
            limits: snapshot.limits,
        }))))
    }
    pub fn len(&self) -> usize {
        self.0.borrow().live.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn remaining(&self) -> usize {
        self.0.borrow().seeds.len()
    }
    /// Call after a successful, complete hydration attempt if deferred branches
    /// will not need unused seeds. Failed attempts should restore a fresh copy
    /// of the original Snapshot; reading the browser payload is non-destructive.
    pub fn discard_unused(&self) -> Vec<String> {
        let seeds = std::mem::take(&mut self.0.borrow_mut().seeds);
        seeds.into_keys().collect()
    }
    pub fn bind<K, T, E, F>(
        &self,
        owner: &Owner,
        name: &str,
        schema: &str,
        source: impl Fn() -> K + 'static,
        load: impl Fn(K) -> F + 'static,
    ) -> Result<Resource<K, T, E>, Error>
    where
        K: Clone + PartialEq + Serialize + DeserializeOwned + 'static,
        T: Serialize + DeserializeOwned + 'static,
        E: Serialize + DeserializeOwned + 'static,
        F: Future<Output = Result<T, E>> + 'static,
    {
        names(name, schema)?;
        if !owner.is_alive() {
            return Err(AccessError::Disposed.into());
        }
        let record = self.0.borrow().seeds.get(name).cloned();
        // User deserializers run with no registry or reactive graph borrow.
        let seed = match record {
            None => None,
            Some(record) => {
                if record.schema != schema {
                    return Err(Error::Schema {
                        name: name.into(),
                        expected: schema.into(),
                        found: record.schema,
                    });
                }
                let key = serde_json::from_value(record.key)?;
                let result = match record.result {
                    Outcome::Ready(value) => Ok(serde_json::from_value(value)?),
                    Outcome::Failed(error) => Err(serde_json::from_value(error)?),
                };
                Some(ResourceSeed { key, result })
            }
        };
        if !owner.is_alive() {
            return Err(AccessError::Disposed.into());
        }
        let ticket = Rc::new(());
        {
            let mut store = self.0.borrow_mut();
            if store
                .runtime
                .is_some_and(|runtime| runtime != owner.runtime())
            {
                return Err(Error::WrongRuntime);
            }
            if store.live.contains_key(name) {
                return Err(Error::Duplicate(name.into()));
            }
            if store.live.len() >= store.limits.records {
                return Err(Error::Limit("live resource records"));
            }
            store.runtime = Some(owner.runtime());
            store.live.insert(
                name.into(),
                Entry {
                    ticket: ticket.clone(),
                    read: None,
                },
            );
        }
        let mut reservation = Registration {
            store: Rc::downgrade(&self.0),
            name: name.into(),
            ticket: ticket.clone(),
            committed: false,
        };
        let resource = owner.resource_seeded(source, load, seed);
        let registration = Registration {
            store: reservation.store.clone(),
            name: name.into(),
            ticket,
            committed: false,
        };
        owner.on_cleanup(move || drop(registration))?;
        let id = name.to_owned();
        let version = schema.to_owned();
        let read: Read = Rc::new(move || {
            // Clone only Rc handles, then invoke arbitrary Serialize outside
            // the reactive value borrow. Disposed entries from an export's
            // earlier callback are skipped; no stale handle panic.
            match resource.try_get() {
                Ok(ResourceState::Ready { key, value }) => {
                    encode::<K, T, E>(&id, &version, &key, Ok(&value)).map(Some)
                }
                Ok(ResourceState::Failed { key, error }) => {
                    encode::<K, T, E>(&id, &version, &key, Err(&error)).map(Some)
                }
                Ok(_) | Err(AccessError::Disposed | AccessError::RuntimeGone) => Ok(None),
                Err(error) => Err(error.into()),
            }
        });
        {
            let mut store = self.0.borrow_mut();
            let entry = store.live.get_mut(name).ok_or(AccessError::Disposed)?;
            if !Rc::ptr_eq(&entry.ticket, &reservation.ticket) {
                return Err(Error::Busy(name.into()));
            }
            entry.read = Some(read);
            store.seeds.remove(name);
        }
        reservation.committed = true;
        Ok(resource)
    }
    /// Export current terminal records in deterministic name order. Pending,
    /// Idle and host failures are not transferable business results. Call after
    /// flushing the app; serialization callbacks should not mutate UI state.
    pub fn snapshot(&self) -> Result<Snapshot, Error> {
        let (limits, readers) = {
            let store = self.0.borrow();
            let readers = store
                .live
                .iter()
                .map(|(name, entry)| entry.read.clone().ok_or_else(|| Error::Busy(name.clone())))
                .collect::<Result<Vec<_>, _>>()?;
            (store.limits, readers)
        };
        untracked(|| {
            let mut snapshot = Snapshot::new(limits);
            for read in readers {
                if let Some(record) = read()? {
                    snapshot.records.push(record);
                }
            }
            snapshot.to_json()?;
            Ok(snapshot)
        })
    }
}
struct Registration {
    store: Weak<RefCell<Store>>,
    name: String,
    ticket: Rc<()>,
    committed: bool,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        if let Some(store) = self.store.upgrade() {
            let removed = {
                let mut store = store.borrow_mut();
                if store
                    .live
                    .get(&self.name)
                    .is_some_and(|entry| Rc::ptr_eq(&entry.ticket, &self.ticket))
                {
                    store.live.remove(&self.name)
                } else {
                    None
                }
            };
            drop(removed);
        }
    }
}
