//! Keyed, owner-scoped asynchronous state. Serialization and transport belong
//! to outer adapters; the runtime never serializes arbitrary reactive values.
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use crate::{AccessError, Bond, Cage, Owner, TaskToken, untracked};

/// Restored terminal data. The caller explicitly decodes the key and result;
/// only a matching initial key suppresses the first request. A refresh always
/// fetches again, including after restoring a business error.
pub struct ResourceSeed<K, T, E> {
    pub key: K,
    pub result: Result<T, E>,
}

#[derive(Debug)]
pub enum ResourceState<K, T, E> {
    Idle,
    Pending {
        key: Rc<K>,
    },
    Ready {
        key: Rc<K>,
        value: Rc<T>,
    },
    Failed {
        key: Rc<K>,
        error: Rc<E>,
    },
    /// Missing/rejected execution services are distinct from a loader error.
    Unavailable {
        key: Rc<K>,
        error: AccessError,
    },
}
impl<K, T, E> Clone for ResourceState<K, T, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::Pending { key } => Self::Pending { key: key.clone() },
            Self::Ready { key, value } => Self::Ready {
                key: key.clone(),
                value: value.clone(),
            },
            Self::Failed { key, error } => Self::Failed {
                key: key.clone(),
                error: error.clone(),
            },
            Self::Unavailable { key, error } => Self::Unavailable {
                key: key.clone(),
                error: error.clone(),
            },
        }
    }
}
impl<K, T, E> ResourceState<K, T, E> {
    pub fn key(&self) -> Option<&K> {
        match self {
            Self::Idle => None,
            Self::Pending { key }
            | Self::Ready { key, .. }
            | Self::Failed { key, .. }
            | Self::Unavailable { key, .. } => Some(key),
        }
    }
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }
}

enum Request {
    Load,
    Cancel,
}
struct Flight {
    // A fresh allocation is an unforgeable local request identity. No wrapping
    // integer generation, and no old result can regain publication rights.
    ticket: Rc<()>,
    task: Option<TaskToken>,
}
struct Control {
    request: Rc<Request>,
    flight: Rc<RefCell<Flight>>,
}

/// A local Copy handle, invalidated with its Owner. Results use Rc so T/E need
/// neither Clone nor PartialEq. Source reads track only the resource driver;
/// reads in the loader are not dependencies of that driver.
pub struct Resource<K: 'static, T: 'static, E: 'static> {
    state: Cage<ResourceState<K, T, E>>,
    control: Cage<Control>,
}
impl<K, T, E> Copy for Resource<K, T, E> {}
impl<K, T, E> Clone for Resource<K, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, T, E> Resource<K, T, E> {
    pub fn is_alive(self) -> bool {
        self.state.is_alive()
    }
    pub fn try_with<R>(
        self,
        read: impl FnOnce(&ResourceState<K, T, E>) -> R,
    ) -> Result<R, AccessError> {
        self.state.try_with(read)
    }
    pub fn with<R>(self, read: impl FnOnce(&ResourceState<K, T, E>) -> R) -> R {
        self.state.with(read)
    }
    pub fn try_get(self) -> Result<ResourceState<K, T, E>, AccessError> {
        self.state.try_get()
    }
    pub fn get(self) -> ResourceState<K, T, E> {
        self.state.get()
    }
    pub fn map<U: 'static>(
        self,
        mapper: impl Fn(&ResourceState<K, T, E>) -> U + 'static,
    ) -> Bond<U> {
        self.state.map(mapper)
    }
    /// Queue one fresh request. Repeated calls in a batch coalesce. The current
    /// future loses publication rights immediately, before the next flush.
    pub fn refresh(self) -> Result<(), AccessError> {
        let flight = self.request(Request::Load)?;
        revoke(&flight);
        Ok(())
    }
    /// Cancel immediately and become Idle. The same key stays idle until an
    /// explicit refresh; a subsequent source-key change starts a new request.
    pub fn cancel(self) -> Result<(), AccessError> {
        let flight = self.request(Request::Cancel)?;
        let ticket = revoke(&flight);
        if current(&flight, &ticket) {
            replace(self.state, ResourceState::Idle)?;
        }
        Ok(())
    }
    fn request(self, request: Request) -> Result<Rc<RefCell<Flight>>, AccessError> {
        let (previous, flight) = self.control.try_revise(|control| {
            (
                std::mem::replace(&mut control.request, Rc::new(request)),
                control.flight.clone(),
            )
        })?;
        drop(previous);
        Ok(flight)
    }
}

fn current(flight: &RefCell<Flight>, ticket: &Rc<()>) -> bool {
    Rc::ptr_eq(&flight.borrow().ticket, ticket)
}
fn revoke(flight: &RefCell<Flight>) -> Rc<()> {
    let ticket = Rc::new(());
    let previous = flight.replace(Flight {
        ticket: ticket.clone(),
        task: None,
    });
    // Host cancellation drops arbitrary user futures. Never hold a graph,
    // reactive-value or flight borrow while executing it.
    if let Some(task) = previous.task {
        task.cancel();
    }
    ticket
}
fn replace<K: 'static, T: 'static, E: 'static>(
    state: Cage<ResourceState<K, T, E>>,
    value: ResourceState<K, T, E>,
) -> Result<(), AccessError> {
    let previous = state.try_revise(|state| std::mem::replace(state, value))?;
    drop(previous);
    Ok(())
}

impl Owner {
    /// Start/restart after tracked source-key changes at the effect phase. A
    /// missing Host produces Unavailable rather than a blocking fallback.
    pub fn resource<K, T, E, F>(
        &self,
        source: impl Fn() -> K + 'static,
        load: impl Fn(K) -> F + 'static,
    ) -> Resource<K, T, E>
    where
        K: Clone + PartialEq + 'static,
        T: 'static,
        E: 'static,
        F: Future<Output = Result<T, E>> + 'static,
    {
        self.resource_seeded(source, load, None)
    }

    pub fn resource_seeded<K, T, E, F>(
        &self,
        source: impl Fn() -> K + 'static,
        load: impl Fn(K) -> F + 'static,
        seed: Option<ResourceSeed<K, T, E>>,
    ) -> Resource<K, T, E>
    where
        K: Clone + PartialEq + 'static,
        T: 'static,
        E: 'static,
        F: Future<Output = Result<T, E>> + 'static,
    {
        let initial = match seed {
            Some(ResourceSeed {
                key,
                result: Ok(value),
            }) => ResourceState::Ready {
                key: Rc::new(key),
                value: Rc::new(value),
            },
            Some(ResourceSeed {
                key,
                result: Err(error),
            }) => ResourceState::Failed {
                key: Rc::new(key),
                error: Rc::new(error),
            },
            None => ResourceState::Idle,
        };
        let mut seed_key = initial.key().cloned();
        let state = self.cage(initial);
        let flight = Rc::new(RefCell::new(Flight {
            ticket: Rc::new(()),
            task: None,
        }));
        let initial_request = Rc::new(Request::Load);
        let control = self.cage(Control {
            request: initial_request.clone(),
            flight: flight.clone(),
        });
        let owner = self.handle();
        let load = Rc::new(load);
        let mut last_key = None;
        let mut last_request = None;
        self.effect(move || {
            let key = source();
            let request = control.with(|control| control.request.clone());
            let new_request = last_request
                .as_ref()
                .is_none_or(|previous| !Rc::ptr_eq(previous, &request));
            let new_key = last_key.as_ref() != Some(&key);
            let first = last_request.is_none();
            let restored = first
                && seed_key.take().as_ref() == Some(&key)
                && Rc::ptr_eq(&request, &initial_request);
            last_request = Some(request.clone());
            last_key = Some(key.clone());
            if !new_request && !new_key {
                return;
            }
            untracked(|| {
                let ticket = revoke(&flight);
                if !current(&flight, &ticket) || !state.is_alive() {
                    return;
                }
                if new_request && matches!(*request, Request::Cancel) {
                    let _ = replace(state, ResourceState::Idle);
                    return;
                }
                if restored {
                    return;
                }
                let key = Rc::new(key);
                if replace(state, ResourceState::Pending { key: key.clone() }).is_err()
                    || !current(&flight, &ticket)
                    || !state.is_alive()
                {
                    return;
                }
                let load = load.clone();
                let running = flight.clone();
                let identity = ticket.clone();
                let requested = key.clone();
                let task = owner.spawn(async move {
                    let result = untracked(|| load((*requested).clone())).await;
                    if current(&running, &identity) && state.is_alive() {
                        running.borrow_mut().task = None;
                        let result = match result {
                            Ok(value) => ResourceState::Ready {
                                key: requested,
                                value: Rc::new(value),
                            },
                            Err(error) => ResourceState::Failed {
                                key: requested,
                                error: Rc::new(error),
                            },
                        };
                        let _ = replace(state, result);
                    }
                });
                match task {
                    Ok(task) if current(&flight, &ticket) && state.is_alive() => {
                        flight.borrow_mut().task = Some(task)
                    }
                    Ok(task) => task.cancel(),
                    Err(error) if current(&flight, &ticket) && state.is_alive() => {
                        let _ = replace(state, ResourceState::Unavailable { key, error });
                    }
                    Err(_) => {}
                }
            });
        });
        Resource { state, control }
    }
}
