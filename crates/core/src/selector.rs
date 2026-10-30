//! Finite single-key selection: only the old and new live key channels publish.
use super::*;
use std::cell::Cell;
use std::hash::{BuildHasher, Hash, RandomState};

struct Channel<K> {
    id: u64,
    owner: Key,
    key: K,
    value: Cage<bool>,
    selected: Cell<bool>,
}
struct Control<K: 'static> {
    source: Cage<Option<K>>,
    owner: OwnerHandle,
    hash: RandomState,
    next: Cell<u64>,
    channels: RefCell<HashMap<u64, Vec<Rc<Channel<K>>>>>,
    // Only live channels actually read ahead of the last notification appear
    // here. Numeric IDs give deterministic order without retaining user keys.
    read_ahead: RefCell<BTreeMap<u64, Weak<Channel<K>>>>,
    previous: RefCell<Rc<Option<K>>>,
}

/// An Owner-owned index for a single selected key (`Cage<Option<K>>`).
///
/// `selected(key)` creates one read-only selection handle per live key/Owner,
/// owned by the current construction Owner (or the selector Owner outside
/// construction). Reads subscribe to that channel, not the shared selection.
/// A flush normally notifies only the old and new key's live channels; batched
/// changes coalesce to the final key. Channels actually read ahead of a flush
/// are also notified once, including after a net-zero batch, to invalidate any
/// cached intermediate result. Direct `Selected::get` reads compare against the
/// current source, including inside a batch before notification. This is not a general predicate
/// selector. Bind through `computed(move || selected.get())`; this dedicated
/// handle does not implicitly convert to `Vary<bool>`.
/// A derived `Bond` caching a selection read is invalidated by the flush-time
/// notification, so it may retain its previous value until that flush. Use the
/// `Selected` handle directly when reading intermediate, unflushed writes.
///
/// Keys must have pure, stable Clone/Hash/Eq implementations. Hash collisions
/// compare complete keys outside graph/index borrows. Storage contains only
/// live registered channels; retirement removes empty buckets. Handles are
/// local, non-owning, invalid after their creating Owner retires. Register
/// channels during construction, not inside pure computations.
pub struct Selector<K: 'static> {
    control: Cage<Rc<Control<K>>>,
}
/// A local, non-owning selected-key read. It subscribes only to its live
/// notification channel, while reading the authoritative selection untracked.
/// Both the selector Owner and registering Owner must still be alive.
pub struct Selected<K: 'static> {
    control: Weak<Control<K>>,
    channel: Weak<Channel<K>>,
}
impl<K> Clone for Selected<K> {
    fn clone(&self) -> Self {
        Self {
            control: self.control.clone(),
            channel: self.channel.clone(),
        }
    }
}
impl<K: Eq + 'static> Selected<K> {
    pub fn get(&self) -> bool {
        self.try_get().expect("read a live selection channel")
    }
    pub fn try_get(&self) -> Result<bool, AccessError> {
        let control = self.control.upgrade().ok_or(AccessError::Disposed)?;
        let channel = self.channel.upgrade().ok_or(AccessError::Disposed)?;
        // Subscribe before inspecting the value, without subscribing each row
        // to the shared source. This read is constant-time in the live index.
        channel.value.try_with(|_| ())?;
        let selected = control.source.try_with_untracked(|selected| {
            fields::project(|| Ok(selected.as_ref() == Some(&channel.key)))
        })??;
        if !channel.value.is_alive() {
            return Err(AccessError::Disposed);
        }
        if selected != channel.selected.get() {
            control
                .read_ahead
                .borrow_mut()
                .entry(channel.id)
                .or_insert_with(|| Rc::downgrade(&channel));
        }
        Ok(selected)
    }
}
impl<K> Copy for Selector<K> {}
impl<K> Clone for Selector<K> {
    fn clone(&self) -> Self {
        *self
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectorStats {
    pub buckets: usize,
    pub channels: usize,
    pub read_ahead: usize,
}
impl Owner {
    pub fn selector<K: Clone + Eq + Hash + 'static>(
        &self,
        source: Cage<Option<K>>,
    ) -> Result<Selector<K>, AccessError> {
        self.check_write()?;
        if source.runtime() != self.runtime {
            return Err(AccessError::WrongRuntime);
        }
        let previous = Rc::new(source.try_with_untracked(Clone::clone)?);
        self.check_write()?;
        let control = Rc::new(Control {
            source,
            owner: self.handle(),
            hash: RandomState::new(),
            next: Cell::new(0),
            channels: RefCell::new(HashMap::new()),
            read_ahead: RefCell::new(BTreeMap::new()),
            previous: RefCell::new(previous),
        });
        let selector = Selector {
            control: self.cage(control.clone()),
        };
        let weak = Rc::downgrade(&control);
        self.on_cleanup(move || {
            if let Some(control) = weak.upgrade() {
                let channels = std::mem::take(&mut *control.channels.borrow_mut());
                control.read_ahead.borrow_mut().clear();
                let previous = control.previous.replace(Rc::new(None));
                for channel in channels.values().flatten() {
                    BindingId {
                        runtime: channel.value.handle.runtime,
                        key: channel.value.handle.key,
                        local: PhantomData,
                    }
                    .dispose();
                }
                drop((channels, previous));
            }
        })?;
        self.structural(move || {
            let Ok(next) = control.source.try_with(Clone::clone) else {
                return;
            };
            control.publish(next).expect("pure selector key");
        });
        Ok(selector)
    }
}
impl<K: Clone + Eq + Hash + 'static> Control<K> {
    fn publish(&self, next: Option<K>) -> Result<(), AccessError> {
        let previous = self.previous.borrow().clone();
        let same = fields::project(|| Ok(*previous == next))?;
        let next = if same {
            previous.clone()
        } else {
            Rc::new(next)
        };
        let mut candidates: Vec<(Rc<Channel<K>>, bool, bool)> = Vec::new();
        let mut positions = HashMap::new();
        if !same {
            self.previous.replace(next.clone());
            for (key, selected) in [
                (previous.as_ref().as_ref(), false),
                (next.as_ref().as_ref(), true),
            ] {
                let Some(key) = key else { continue };
                for channel in self.matches(key)? {
                    positions.insert(channel.id, candidates.len());
                    candidates.push((channel, selected, false));
                }
            }
        }
        // Even a net-zero source update must invalidate a cached Bond which
        // first observed an intermediate selection. Merge those actual reads
        // with old/new candidates, publishing each live channel exactly once.
        let pending = std::mem::take(&mut *self.read_ahead.borrow_mut());
        for (id, weak) in pending {
            let Some(channel) = weak.upgrade() else {
                continue;
            };
            if let Some(&position) = positions.get(&id) {
                candidates[position].2 = true;
            } else {
                let selected =
                    fields::project(|| Ok(next.as_ref().as_ref() == Some(&channel.key)))?;
                positions.insert(id, candidates.len());
                candidates.push((channel, selected, true));
            }
        }
        for (channel, selected, force) in candidates {
            if channel.value.is_alive()
                && (force || channel.selected.get() != selected)
                && channel.value.try_revise(|value| *value = selected).is_ok()
            {
                channel.selected.set(selected);
            }
        }
        Ok(())
    }
    fn hash(&self, key: &K) -> Result<u64, AccessError> {
        fields::project(|| Ok(self.hash.hash_one(key)))
    }
    fn matches(&self, key: &K) -> Result<Vec<Rc<Channel<K>>>, AccessError> {
        let hash = self.hash(key)?;
        let candidates = self
            .channels
            .borrow()
            .get(&hash)
            .cloned()
            .unwrap_or_default();
        candidates
            .into_iter()
            .filter_map(
                |channel| match fields::project(|| Ok(channel.key == *key)) {
                    Ok(true) => Some(Ok(channel)),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                },
            )
            .collect()
    }
    fn remove(&self, hash: u64, id: u64) {
        self.read_ahead.borrow_mut().remove(&id);
        let removed = {
            let mut channels = self.channels.borrow_mut();
            let Some(bucket) = channels.get_mut(&hash) else {
                return;
            };
            let Some(index) = bucket.iter().position(|channel| channel.id == id) else {
                return;
            };
            let removed = bucket.swap_remove(index);
            if bucket.is_empty() {
                channels.remove(&hash);
            }
            removed
        };
        drop(removed);
    }
}
impl<K: Clone + Eq + Hash + 'static> Selector<K> {
    pub fn selected(self, key: K) -> Selected<K> {
        self.try_selected(key)
            .expect("select a key under a live non-pure Owner")
    }
    pub fn try_selected(self, key: K) -> Result<Selected<K>, AccessError> {
        allow_creation()?;
        let control = self.control.try_with_untracked(Rc::clone)?;
        let owner = CONSTRUCTING.with_borrow(|stack| {
            stack
                .last()
                .copied()
                .map(|(runtime, key)| OwnerHandle { runtime, key })
                .unwrap_or(control.owner)
        });
        if owner.runtime != control.owner.runtime {
            return Err(AccessError::WrongRuntime);
        }
        let hash = control.hash(&key)?;
        let matches = control.matches(&key)?;
        // User hashing/equality may have retired an Owner. Validate again before
        // reusing or registering a channel; never attach to its successor slot.
        let state = lookup(owner.runtime)?;
        if !self.control.is_alive() || state.borrow().owners.get(owner.key).is_none() {
            return Err(AccessError::Disposed);
        }
        if let Some(channel) = matches
            .into_iter()
            .find(|channel| channel.owner == owner.key)
        {
            return Ok(Selected {
                control: Rc::downgrade(&control),
                channel: Rc::downgrade(&channel),
            });
        }
        control.source.try_with_untracked(|_| ())?;
        // Notification state describes the last published source, not an
        // intermediate write visible only to direct Selected::get reads. An
        // intermediate registration must not suppress a later real transition.
        let previous = control.previous.borrow().clone();
        let selected = fields::project(|| Ok(previous.as_ref().as_ref() == Some(&key)))?;
        if !self.control.is_alive() || state.borrow().owners.get(owner.key).is_none() {
            return Err(AccessError::Disposed);
        }
        let id = control.next.get();
        control
            .next
            .set(id.checked_add(1).expect("selector channel ids exhausted"));
        let value = make_cage(owner.runtime, owner.key, selected)?;
        let channel = Rc::new(Channel {
            id,
            owner: owner.key,
            key,
            value,
            selected: Cell::new(selected),
        });
        control
            .channels
            .borrow_mut()
            .entry(hash)
            .or_default()
            .push(channel.clone());
        let weak = Rc::downgrade(&control);
        state
            .borrow_mut()
            .owners
            .get_mut(owner.key)
            .ok_or(AccessError::Disposed)?
            .cleanup
            .push(Box::new(move || {
                if let Some(control) = weak.upgrade() {
                    control.remove(hash, id);
                }
            }));
        Ok(Selected {
            control: Rc::downgrade(&control),
            channel: Rc::downgrade(&channel),
        })
    }
    /// Explicit O(live channels) diagnostics; no source or visited-key scan.
    pub fn stats(self) -> Result<SelectorStats, AccessError> {
        self.control.try_with_untracked(|control| {
            let channels = control.channels.borrow();
            SelectorStats {
                buckets: channels.len(),
                channels: channels.values().map(Vec::len).sum(),
                read_ahead: control.read_ahead.borrow().len(),
            }
        })
    }
}
